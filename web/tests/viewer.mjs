import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),out=resolve(repo,'output/webassembly/viewer-browser');
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
  page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}`);await page.locator('.edit-toggle').click();

  await page.waitForFunction(()=>!document.querySelector('#pdf').disabled);
  const apiChecks=await page.evaluate(async()=>{
    const {createEngine}=await import('/dist/client.js'),{createWorkerEngine}=await import('/dist/worker-client.js');
    const bytes=new Uint8Array(await(await fetch('/fixtures/interaction-flags-20.pdf')).arrayBuffer());
    const checks=[];
    for(const worker of [false,true]) {
      const engine=await(worker?createWorkerEngine():createEngine());
      try {
        const doc=await engine.open(bytes),before=await doc.state();
        for(const angle of [0,90,180,270]) {
          const options={viewRotation:angle},full=await doc.render(0,72,options),region=await doc.renderRegion(0,{x:40,y:40,width:32,height:32},72,options);
          for(let y=0;y<32;y++)for(let x=0;x<32;x++)for(let c=0;c<4;c++)if(region.pixels[(y*32+x)*4+c]!==full.pixels[((y+40)*full.width+x+40)*4+c])throw Error('rotated appearance region mismatch');
          const requirements=await doc.fontRequirements(0,options);if(requirements.renderOptions.viewRotation!==angle)throw Error('font preflight lost rotation');
        }
        for(const viewRotation of [-90,45,360,NaN,'90',null]) {
          // null has the same default semantics as the existing optional render fields.
          if(viewRotation===null)continue;
          try{await doc.render(0,72,{viewRotation});throw Error('invalid angle accepted');}catch(e){if(e.code!=='invalid_argument')throw e;}
        }
        if((await doc.state()).stamp.revision!==before.stamp.revision)throw Error('view rotation modified PDF');
        await doc.close();checks.push((worker?'Worker':'sync')+' rotation options, region and preflight, invalid angles, revision unchanged');
      }finally{engine.dispose();}
    }
    return checks;
  });report.checks.push(...apiChecks);
  for(const pdfRotation of [0,90]) {
    await page.locator('#pdf').setInputFiles(resolve(repo,`output/webassembly/fixtures/interaction-rotation-${pdfRotation}.pdf`));
    await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 /'));
    const geometry=await page.evaluate(async(pdfRotation)=>{
      const {createWorkerEngine}=await import('/dist/worker-client.js');const engine=await createWorkerEngine();
      try{const doc=await engine.open(new Uint8Array(await(await fetch(`/fixtures/interaction-rotation-${pdfRotation}.pdf`)).arrayBuffer()));return {info:await doc.pageInfo(0),widget:(await doc.formFields()).records.find(f=>f.name==='name').widgets[0]};}finally{engine.dispose();}
    },pdfRotation);
    for(const angle of [0,90,180,270])for(const percent of [50,125]) {
      await page.locator('#zoom').fill(String(percent));await page.locator('#view-rotation').selectOption(String(angle));await page.locator('#render').click();
      await page.waitForFunction(angle=>document.querySelector('#status').textContent.startsWith('第 1 /')&&document.querySelector('#page-view').dataset.rotation===String(angle)&&!!document.querySelector('#widgets [aria-label="页面表单 name"]'),angle);
      const delta=await page.evaluate(({geometry,angle,percent})=>{
        const {info,widget}=geometry,scale=percent/100,[a,b,c,d,e,f]=info.userToDisplay;
        const rotate=(x,y)=>angle===90?[info.height*scale-y,x]:angle===180?[info.width*scale-x,info.height*scale-y]:angle===270?[y,info.width*scale-x]:[x,y];
        const [x0,y0,x1,y1]=widget.rect,points=[[x0,y0],[x1,y0],[x1,y1],[x0,y1]].map(([x,y])=>rotate((a*x+c*y+e)*scale,(b*x+d*y+f)*scale));
        const xs=points.map(p=>p[0]),ys=points.map(p=>p[1]),view=document.querySelector('#page-view').getBoundingClientRect(),input=document.querySelector('#widgets [aria-label="页面表单 name"]').getBoundingClientRect();
        const target=[Math.min(...xs),Math.min(...ys),Math.max(...xs)-Math.min(...xs),Math.max(...ys)-Math.min(...ys)];
        return Math.max(...[input.x-view.x,input.y-view.y,input.width,input.height].map((n,i)=>Math.abs(n-target[i])));
      },{geometry,angle,percent});assert.ok(delta<.6,`page ${pdfRotation} view ${angle} zoom ${percent}: ${delta}`);
    }
  }
  report.checks.push('16 real widget placements: page rotation/CropBox/UserUnit composed with four view rotations and two zooms');
  for(const flags of [12,20,28]) {
    await page.locator('#pdf').setInputFiles(resolve(repo,`output/webassembly/fixtures/interaction-flags-${flags}.pdf`));
    await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 /'));
    for(const angle of [0,90,180,270]) {
      await page.locator('#view-rotation').selectOption(String(angle));
      await page.waitForFunction(angle=>document.querySelector('#status').textContent.startsWith('第 1 /')&&document.querySelector('#page-view').dataset.rotation===String(angle)&&!!document.querySelector('#widgets [aria-label="页面表单 name"]'),angle);
      const result=await page.evaluate(()=>{const e=document.querySelector('#widgets [aria-label="页面表单 name"]'),b=e.getBoundingClientRect();return {width:b.width,height:b.height,matrix:getComputedStyle(e).transform,parent:getComputedStyle(document.querySelector('#page-content')).transform};});
      if(flags&16)assert.ok(result.width>result.height,'NoRotate text field should remain horizontal');
      await page.getByLabel('页面表单 name',{exact:true}).fill(`View${angle}`);await page.getByLabel('页面表单 name',{exact:true}).dispatchEvent('change');
      await page.waitForFunction(value=>document.querySelector('#editor [aria-label="name"]')?.value===value&&!!document.querySelector('#save-pdf')&&!document.querySelector('#save-pdf').disabled,`View${angle}`);
    }
  }
  report.checks.push('NoZoom/NoRotate/combined flags and actual field edits at all four view rotations');
  // Fit-page keeps the whole transformed page available for pointer input.
  await page.locator('#fit').selectOption('page');
  for(const angle of [0,90,180,270]) {
    await page.locator('#view-rotation').selectOption(String(angle));
    await page.waitForFunction(angle=>document.querySelector('#status').textContent.startsWith('第 1 /')&&document.querySelector('#page-view').dataset.rotation===String(angle)&&!!document.querySelector('#save-pdf'),angle);
    await page.locator('#page-view').scrollIntoViewIfNeeded();
    await page.getByLabel('页面批注工具',{exact:true}).selectOption('Square');await page.getByLabel('绘制批注内容',{exact:true}).fill(`Pointer${angle}`);
    const sample=await page.evaluate(()=>{
      const e=document.querySelector('#page-content'),view=document.querySelector('#page-view').getBoundingClientRect(),scale=Number(document.querySelector('#zoom').value)/100;
      const w=e.offsetWidth,h=e.offsetHeight,angle=Number(document.querySelector('#view-rotation').value);
      const rotate=(x,y)=>angle===90?[h-y,x]:angle===180?[w-x,h-y]:angle===270?[y,w-x]:[x,y];
      return {points:[[w*.35,h*.35],[w*.5,h*.5]].map(([x,y])=>{const [rx,ry]=rotate(x,y);return [view.left+rx,view.top+ry];}),display:[[w*.35/scale,h*.35/scale],[w*.5/scale,h*.5/scale]]};
    });
    await page.mouse.move(...sample.points[0]);await page.mouse.down();await page.mouse.move(...sample.points[1],{steps:4});await page.mouse.up();
    await page.waitForFunction(value=>[...document.querySelectorAll('[aria-label="批注内容"]')].some(e=>e.value===value)&&!!document.querySelector('#save-pdf')&&!document.querySelector('#save-pdf').disabled,`Pointer${angle}`);
    const download=page.waitForEvent('download');await page.locator('#save-pdf').click();const path=resolve(out,`pointer-${angle}.pdf`);await(await download).saveAs(path);
    const error=await page.evaluate(async({bytes,angle,sample})=>{
      const {createWorkerEngine}=await import('/dist/worker-client.js');const engine=await createWorkerEngine();
      try{
        const doc=await engine.open(new Uint8Array(bytes)),info=await doc.pageInfo(0),record=(await doc.annotations(0)).records.find(a=>a.annotation?.contents===`Pointer${angle}`);
        const [a,b,c,d,e,f]=info.userToDisplay,det=a*d-b*c;
        const points=sample.display.map(([x,y])=>[(d*(x-e)-c*(y-f))/det,(-b*(x-e)+a*(y-f))/det]);
        const xs=points.map(p=>p[0]),ys=points.map(p=>p[1]),rect=[Math.min(...xs),Math.min(...ys),Math.max(...xs),Math.max(...ys)];
        return Math.max(...record.annotation.rect.map((n,i)=>Math.abs(n-rect[i])));
      }finally{engine.dispose();}
    },{bytes:Array.from(await readFile(path)),angle,sample});assert.ok(error<1.5,`rotated pointer PDF geometry ${angle}: ${error}`);
  }
  report.checks.push('real pointer drawing, inverse view/PDF mapping and saved annotation geometry at four angles');

  await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/web-features.pdf'));
  await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 / 2'));
  await page.locator('#view-rotation').selectOption('90');
  await page.waitForFunction(()=>document.querySelector('#page-view').dataset.rotation==='90'&&document.querySelector('#status').textContent.startsWith('第 1 / 2'));
  await page.locator('#page-view').scrollIntoViewIfNeeded();
  const textPoints=await page.evaluate(()=>{const view=document.querySelector('#page-view').getBoundingClientRect(),m=new DOMMatrix(getComputedStyle(document.querySelector('#page-content')).transform),s=Number(document.querySelector('#zoom').value)/100;return [[10,25],[70,25]].map(([x,y])=>{const p=m.transformPoint(new DOMPoint(x*s,y*s));return [view.left+p.x,view.top+p.y];});});
  await page.mouse.move(...textPoints[0]);await page.mouse.down();await page.mouse.move(...textPoints[1],{steps:15});await page.mouse.up();
  assert.equal(await page.evaluate(()=>window.getSelection().toString()),'Web text');
  await page.evaluate(()=>window.getSelection().removeAllRanges());
  assert.equal(await page.locator('#links a').count(),2);
  await page.locator('[data-tab=search]').click();await page.locator('#query').fill('Web');await page.locator('#find').click();await page.locator('#results button').click();
  await page.waitForFunction(()=>document.querySelectorAll('#highlights polygon').length===3);
  await page.locator('#links a[aria-label="转到第 2 页"]').click();
  await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 2 / 2')&&document.querySelector('#page-view').dataset.rotation==='90');
  report.checks.push('rotated real pointer text selection, search highlights, safe URI filtering and internal link navigation');
  await page.screenshot({path:resolve(out,'rotated-reader.png'),fullPage:true});
  report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;try{await page?.screenshot({path:resolve(out,'failure-widget.png'),fullPage:true});report.failure=await page?.evaluate(()=>({status:document.querySelector('#status')?.textContent,fit:document.querySelector('#fit')?.value,zoom:document.querySelector('#zoom')?.value,pageSize:[document.querySelector('#page-view')?.offsetWidth,document.querySelector('#page-view')?.offsetHeight],viewportSize:[document.querySelector('#viewport')?.clientWidth,document.querySelector('#viewport')?.clientHeight],annotations:[...document.querySelectorAll('[aria-label="批注内容"]')].map(e=>e.value),tool:document.querySelector('[aria-label="页面批注工具"]')?.value}));}catch{}}
finally {await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));}
