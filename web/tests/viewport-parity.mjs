import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),out=resolve(repo,'output/webassembly/viewport-parity');
await mkdir(out,{recursive:true});
const server=createServer(async(req,res)=>{
  const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
  const file=path.startsWith('/fixtures/')?resolve(repo,'output/webassembly',path.slice(1)):resolve(repo,'web',path==='/'?'index.html':path.slice(1));
  if(!file.startsWith(repo)){res.writeHead(403).end();return;}
  try{const data=await readFile(file);res.setHeader('Content-Type',({'.css':'text/css','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.pdf':'application/pdf'})[extname(file)]??'application/octet-stream');res.end(data);}catch{res.writeHead(404).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser,page;
const report={status:'failed',checks:[],errors:[]};
try {
  browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});
  page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}`);


  report.browser=await browser.version();report.cases=[];
  await page.waitForFunction(()=>!document.querySelector('#pdf').disabled);
  for(const [fixture,percent,angle] of [
    ['glyph-phase-pattern',200,0],['glyph-phase-pattern',200,90],
    ['glyph-phase-pattern',400/3,0],['glyph-phase-pattern',400/3,270],
    ['curve-clip-alpha',400,0],['curve-clip-alpha',400,180],
    ['widget-base14',400,0],['widget-base14',400,90],['widget-base14',400,180],['widget-base14',400,270]
  ]) {
    await page.locator('#zoom').fill(String(percent));await page.locator('#view-rotation').selectOption(String(angle));
    await page.locator('#pdf').setInputFiles(resolve(repo,`output/webassembly/fixtures/region-${fixture}.pdf`));
    await page.waitForFunction(angle=>document.querySelector('#status').textContent.startsWith('第 1 /')&&document.querySelector('#canvas').getAttribute('aria-busy')==='false'&&document.querySelector('#canvas').width>1&&document.querySelector('#page-view').dataset.rotation===String(angle),angle);
    const result=await page.evaluate(async({fixture,percent,angle})=>{
      const {createWorkerEngine}=await import('/dist/worker-client.js'),engine=await createWorkerEngine();
      try{
        const doc=await engine.open(new Uint8Array(await(await fetch(`/fixtures/region-${fixture}.pdf`)).arrayBuffer()));
        const zoom=percent/100,dpi=Math.min(288,72*zoom*window.devicePixelRatio),full=await doc.render(0,dpi,{viewZoom:zoom,viewRotation:angle});
        const c=document.querySelector('#canvas'),area={x:Number(c.dataset.rasterX),y:Number(c.dataset.rasterY),width:c.width,height:c.height};
        const pixels=c.getContext('2d').getImageData(0,0,c.width,c.height).data;
        let maximum=0,differing=0,ink=0,alpha=0;
        for(let y=0;y<area.height;y++)for(let x=0;x<area.width;x++)for(let channel=0;channel<4;channel++){
          const i=(y*area.width+x)*4+channel,ref=full.pixels[((y+area.y)*full.width+x+area.x)*4+channel],delta=Math.abs(pixels[i]-ref);
          maximum=Math.max(maximum,delta);if(delta){differing++;if(channel===3)alpha++;}if(channel!==3&&pixels[i]<240)ink++;
        }
        return {fixture,percent,angle,dpi,dpr:window.devicePixelRatio,area,maximum,differing,ink,alpha,statistics:{...c.dataset}};
      }finally{engine.dispose();}
    },{fixture,percent,angle});
    assert.ok(result.ink>100,`blank comparison: ${fixture}`);assert.equal(result.alpha,0);
    const integerScale=Math.abs(result.dpi/72-Math.round(result.dpi/72))<1e-6;
    assert.ok(result.maximum<=(integerScale?0:1),JSON.stringify(result));
    assert.ok(Number(result.statistics.cacheBytes)<=16*1024*1024);assert.ok(Number(result.statistics.surfaceBytes)<=32*1024*1024);
    report.cases.push(result);
  }
  await page.screenshot({path:resolve(out,'widget-tiles.png'),fullPage:true});
  report.checks.push('10 actual reader mosaics compared to same-DPI whole-page Worker output: pattern glyphs, alpha clip and NoRotate Widget');
  report.checks.push('integer zero difference, fractional maximum 1, alpha exact, nonblank ink, bounded canvas/cache');
  report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;try{await page?.screenshot({path:resolve(out,'failure-widget.png'),fullPage:true});report.failure=await page?.evaluate(()=>({status:document.querySelector('#status')?.textContent,fit:document.querySelector('#fit')?.value,zoom:document.querySelector('#zoom')?.value,pageSize:[document.querySelector('#page-view')?.offsetWidth,document.querySelector('#page-view')?.offsetHeight],viewportSize:[document.querySelector('#viewport')?.clientWidth,document.querySelector('#viewport')?.clientHeight],annotations:[...document.querySelectorAll('[aria-label="批注内容"]')].map(e=>e.value),tool:document.querySelector('[aria-label="页面批注工具"]')?.value}));}catch{}}
finally {await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));}
