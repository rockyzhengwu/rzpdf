import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
const repo=fileURLToPath(new URL('../../',import.meta.url)),out=resolve(repo,'output/webassembly/viewport-browser');
await mkdir(out,{recursive:true});
const server=createServer(async(req,res)=>{
  const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
  const file=path.startsWith('/fixtures/')?resolve(repo,'output/webassembly',path.slice(1)):resolve(repo,'web',path==='/'?'index.html':path.slice(1));
  if(!file.startsWith(repo)){res.writeHead(403).end();return;}
  try{const data=await readFile(file);res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.pdf':'application/pdf'})[extname(file)]??'application/octet-stream');res.end(data);}catch{res.writeHead(404).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser,page;
const report={status:'failed',checks:[],errors:[]};
try {
  browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});
  page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}`);


  report.browser=await browser.version();report.generated_at=new Date().toISOString();report.measurements=[];report.artifacts={};
  for(const name of ['demo.js','viewport-raster.js','rzpdf_web.mjs','rzpdf_web.wasm'])report.artifacts[name]=createHash('sha256').update(await readFile(resolve(repo,'web/dist',name))).digest('hex');
  report.driver_sha256=createHash('sha256').update(await readFile(fileURLToPath(import.meta.url))).digest('hex');
  const measure=async(label)=>{const stats=await page.locator('#canvas').evaluate(c=>({cacheBytes:Number(c.dataset.cacheBytes),cacheEntries:Number(c.dataset.cacheEntries),surfaceBytes:Number(c.dataset.surfaceBytes),renderedTiles:Number(c.dataset.renderedTiles),cacheHits:Number(c.dataset.cacheHits),dpr:window.devicePixelRatio}));report.measurements.push({label,...stats});assert.ok(stats.cacheBytes<=16*1024*1024);assert.ok(stats.cacheEntries<=128);assert.ok(stats.surfaceBytes<=32*1024*1024);};
  await page.waitForFunction(()=>!document.querySelector('#pdf').disabled);
  const ready=()=>page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 ')&&document.querySelector('#canvas').getAttribute('aria-busy')==='false'&&document.querySelector('#canvas').width>1);
  await page.evaluate(()=>{const original=Worker.prototype.postMessage;window.__restoreRegionHook=()=>{Worker.prototype.postMessage=original;};let first=true;Worker.prototype.postMessage=function(message,...args){if(first&&message?.operation==='renderRegion'){first=false;queueMicrotask(()=>document.querySelector('#viewport').scrollTo({left:600,top:500}));}return original.call(this,message,...args);};});
  await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/region-large.pdf'));await ready();
  await page.evaluate(()=>window.__restoreRegionHook());
  report.checks.push('scroll during initial render still completes current page metadata and editor');
  await page.locator('#viewport').evaluate(v=>v.scrollTo({left:0,top:0}));

  const initial=await page.locator('#canvas').evaluate(c=>({width:c.width,height:c.height,...c.dataset}));
  assert.ok(initial.width<6000&&initial.height<4000,'reader still allocated a full-page canvas');
  await measure('initial');assert.ok(initial.renderedTiles>0);assert.ok(initial.surfaceBytes<=32*1024*1024);
  const sample=async(x,y,rgba)=>{
    await page.waitForFunction(({x,y})=>{const c=document.querySelector('#canvas'),rx=Number(c.dataset.rasterX),ry=Number(c.dataset.rasterY);return c.getAttribute('aria-busy')==='false'&&x>=rx&&y>=ry&&x<rx+c.width&&y<ry+c.height;},{x,y});
    const result=await page.locator('#canvas').evaluate((c,{x,y})=>Array.from(c.getContext('2d').getImageData(x-Number(c.dataset.rasterX),y-Number(c.dataset.rasterY),1,1).data),{x,y});assert.deepEqual(result,rgba);
  };
  await sample(5,5,[255,0,0,255]);
  await page.locator('#viewport').evaluate(v=>v.scrollTo({left:600,top:0}));await sample(1600,5,[255,0,0,255]);
  assert.ok(await page.locator('#canvas').evaluate(c=>Number(c.dataset.cacheHits)>0),'neighboring scroll did not reuse cached tiles');
  await page.locator('#viewport').evaluate(v=>v.scrollTo({left:10000,top:0}));await sample(5995,5,[0,255,0,255]);
  await page.locator('#viewport').evaluate(v=>v.scrollTo({left:10000,top:10000}));await sample(5995,3995,[255,0,0,255]);
  await page.locator('#viewport').evaluate(v=>v.scrollTo({left:0,top:0}));await sample(5,5,[255,0,0,255]);
  report.checks.push('24-million-pixel logical page opens without whole-page allocation; top/right/bottom pixels and cache reuse');
  for(const angle of [90,180,270,0]) {
    await page.locator('#view-rotation').selectOption(String(angle));await ready();
    await page.locator('#viewport').evaluate(v=>v.scrollTo({left:0,top:0}));
    const point=angle===90?[5,3995]:angle===180?[5995,3995]:angle===270?[5995,5]:[5,5];
    await sample(...point,angle===270?[0,255,0,255]:[255,0,0,255]);
  }
  report.checks.push('inverse viewport mapping and sampled corner pixels at all four view rotations');
  for(let y=0;y<=3600;y+=900)for(let x=0;x<=5400;x+=900) {
    await page.locator('#viewport').evaluate((v,{x,y})=>v.scrollTo({left:x,top:y}),{x,y});
    await page.waitForFunction(({x,y})=>{const c=document.querySelector('#canvas'),rx=Number(c.dataset.rasterX),ry=Number(c.dataset.rasterY);return c.getAttribute('aria-busy')==='false'&&x>=rx&&y>=ry&&x<rx+c.width&&y<ry+c.height;},{x:Math.min(5995,x+5),y:Math.min(3995,y+5)});
    await measure(`scroll ${x},${y}`);
  }
  assert.ok(await page.locator('#canvas').evaluate(c=>Number(c.dataset.renderedTiles)>16));
  report.checks.push('35 scroll windows exercise bounded LRU eviction and 32 MiB surface/16 MiB tile limits');
  await page.locator('#viewport').evaluate(v=>{for(let i=0;i<30;i++)v.scrollTo({left:(i*719)%5500,top:(i*397)%3500});v.scrollTo({left:10000,top:0});});await sample(5995,5,[0,255,0,255]);
  assert.doesNotMatch(await page.locator('#status').textContent(),/会话已结束/);
  report.checks.push('rapid scroll latest viewport wins and preserves Worker session');
  const cdp=await page.context().newCDPSession(page);
  await cdp.send('Emulation.setDeviceMetricsOverride',{width:1100,height:800,deviceScaleFactor:2,mobile:false});
  await page.waitForFunction(()=>window.devicePixelRatio===2);
  await ready();await page.locator('#viewport').evaluate(v=>v.scrollTo({left:10000,top:0}));await sample(11995,5,[0,255,0,255]);await measure('DPR 2');
  await page.locator('#zoom').fill('125');await page.locator('#render').click();await ready();
  await page.locator('#viewport').evaluate(v=>v.scrollTo({left:10000,top:0}));await sample(14995,5,[0,255,0,255]);await measure('DPR 2 zoom 125');
  report.checks.push('actual DPR transition and 125% zoom rebuild raster source without lowering requested DPI');
  await cdp.send('Emulation.setDeviceMetricsOverride',{width:1280,height:720,deviceScaleFactor:1,mobile:false});
  await page.waitForFunction(()=>window.devicePixelRatio===1);await ready();
  await page.locator('#zoom').fill('100');await page.locator('#render').click();await ready();
  await page.locator('#viewport').evaluate(v=>v.scrollTo({left:0,top:0}));await sample(5,5,[255,0,0,255]);
  await page.locator('#viewport').evaluate(v=>v.scrollTo({left:10000,top:10000}));
  await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/region-fractional-edges.pdf'));await ready();
  assert.equal(await page.locator('#canvas').evaluate(c=>c.width),30);await sample(5,5,[0,255,0,255]);
  assert.ok(await page.locator('#canvas').evaluate(c=>Number(c.dataset.cacheBytes)<=30*20*4),'old document cache retained after file replacement');
  report.checks.push('file replacement during scrolling clears old pixels/cache and ignores late results');
  await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/interaction.pdf'));await ready();
  const before=await page.locator('#canvas').evaluate(c=>Array.from(c.getContext('2d').getImageData(0,0,c.width,c.height).data));
  await page.getByLabel('页面表单 name',{exact:true}).fill('Cached pixels changed');await page.getByLabel('页面表单 name',{exact:true}).dispatchEvent('change');
  await page.waitForFunction(()=>document.querySelector('#editor [aria-label="name"]')?.value==='Cached pixels changed'&&document.querySelector('#canvas').getAttribute('aria-busy')==='false'&&!!document.querySelector('#save-pdf')&&!document.querySelector('#save-pdf').disabled);
  const after=await page.locator('#canvas').evaluate(c=>Array.from(c.getContext('2d').getImageData(0,0,c.width,c.height).data));assert.notDeepEqual(after,before);
  report.checks.push('form mutation invalidates cached display pixels');
  await page.evaluate(()=>window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:true})));
  const released=await page.locator('#canvas').evaluate(c=>({width:c.width,cache:Number(c.dataset.cacheBytes),pending:Number(c.dataset.pendingTiles)}));assert.deepEqual(released,{width:1,cache:0,pending:0});
  await measure('pagehide');report.checks.push('pagehide clears surface, tile cache and pending display demand');
  report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;try{await page?.screenshot({path:resolve(out,'failure-widget.png'),fullPage:true});report.failure=await page?.evaluate(()=>({status:document.querySelector('#status')?.textContent,fit:document.querySelector('#fit')?.value,zoom:document.querySelector('#zoom')?.value,pageSize:[document.querySelector('#page-view')?.offsetWidth,document.querySelector('#page-view')?.offsetHeight],viewportSize:[document.querySelector('#viewport')?.clientWidth,document.querySelector('#viewport')?.clientHeight],annotations:[...document.querySelectorAll('[aria-label="批注内容"]')].map(e=>e.value),tool:document.querySelector('[aria-label="页面批注工具"]')?.value}));}catch{}}
finally {await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));}
