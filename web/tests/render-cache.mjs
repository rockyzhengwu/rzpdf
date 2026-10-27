// Compare the same region requests against an optional frozen pre-change module.
// Timing is evidence, not a machine-dependent CI pass threshold.
import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,stat} from 'node:fs/promises';
import {resolve,extname,basename,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),out=resolve(repo,'output/webassembly/render-cache');
await mkdir(out,{recursive:true});
const baseline=process.env.RZPDF_BASELINE_DIR?resolve(process.env.RZPDF_BASELINE_DIR):null;
const font=process.env.RZPDF_PERF_FONT?resolve(process.env.RZPDF_PERF_FONT):null;
const input=process.env.RZPDF_PERF_PDF?resolve(process.env.RZPDF_PERF_PDF):resolve(repo,'output/webassembly/fixtures/matrix-sparse-cid.pdf');
const server=createServer(async(req,res)=>{try{
 const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);let file;
 if(path==='/input.pdf')file=input;
 else if(path==='/substitute.font'&&font)file=font;
 else if(path.startsWith('/before/')){const name=basename(path);file=baseline&&['rzpdf_web.wasm','rzpdf_web.mjs'].includes(name)?resolve(baseline,name):resolve(repo,'web/dist',path.slice('/before/'.length));}
 else file=resolve(repo,'web',path==='/'?'index.html':'.'+path);
 if(file!==input&&file!==font&&!file.startsWith(resolve(repo,'web')+sep)&&!(baseline&&file.startsWith(baseline+sep))){res.writeHead(403).end();return;}
 res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.css':'text/css'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));
 }catch{res.writeHead(404).end();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser,page;const report={status:'failed',inputSha256:createHash('sha256').update(await readFile(input)).digest('hex'),runs:[],errors:[]};
try{
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});
 page=await browser.newPage();page.setDefaultTimeout(180000);page.on('pageerror',e=>report.errors.push(e.message));
 await page.goto(`http://127.0.0.1:${server.address().port}`);
 await page.waitForFunction(()=>!document.querySelector('#pdf').disabled);
 const frames=[];
 await page.exposeFunction('__cacheFrame',({label,index,pixels})=>{frames.push({label,index,pixels});});
 for(const label of baseline?['before','after']:['after']){
  const basePage=Number(process.env.RZPDF_PERF_PAGE??0),fontIndex=Number(process.env.RZPDF_PERF_FONT_INDEX??0);
  const result=await page.evaluate(async ({label,basePage,hasFont,fontIndex})=>{
   const prefix=label==='before'?'/before/':'/dist/';
   const {createWorkerEngine}=await import(prefix+'worker-client.js');
   const bytes=new Uint8Array(await(await fetch('/input.pdf')).arrayBuffer());
   const start=performance.now(),engine=await createWorkerEngine(),startupMs=performance.now()-start;
   try{
    if(hasFont)await engine.setSubstituteFont(new Uint8Array(await(await fetch('/substitute.font')).arrayBuffer()),fontIndex);
    const doc=await engine.open(bytes,{mode:'compatible'});basePage=Math.max(0,Math.min(doc.pageCount-1,basePage));const info=await doc.pageInfo(basePage);
    const requests=[{page:basePage,dpi:72,x:0,y:0},{page:basePage,dpi:72,x:16,y:16},{page:basePage,dpi:72,x:32,y:0},
      {page:basePage,dpi:96,x:0,y:0,zoom:1.25},{page:basePage,dpi:144,x:0,y:0,zoom:1.5},
      {page:Math.min(doc.pageCount-1,basePage+1),dpi:72,x:0,y:0},{page:basePage,dpi:72,x:0,y:0}];
    const timings=[];let ink=0;
    for(const [index,request] of requests.entries()){
     const geometry=request.page===basePage?info:await doc.pageInfo(request.page),width=Math.ceil(Math.fround(geometry.width)*Math.fround(request.dpi)/72),height=Math.ceil(Math.fround(geometry.height)*Math.fround(request.dpi)/72);
     const area={x:Math.min(request.x,Math.max(0,width-1)),y:Math.min(request.y,Math.max(0,height-1)),width:Math.min(512,width),height:Math.min(512,height)};
     area.width=Math.min(area.width,width-area.x);area.height=Math.min(area.height,height-area.y);
     const start=performance.now(),image=await doc.renderRegion(request.page,area,request.dpi,{viewZoom:request.zoom??1});
     timings.push({request,region:area,ms:performance.now()-start});
     ink+=image.pixels.filter((v,i)=>i%4!==3&&v<240).length;
     await window.__cacheFrame({label,index,pixels:Array.from(image.pixels)});
    }
    const beforeClose=await engine.statistics(),pageCount=doc.pageCount;await doc.close();if(hasFont)await engine.setSubstituteFont();const closed=await engine.statistics();
    if(closed.handles!==0||closed.retainedBytes!==0)throw Error('closed document retained resources');
    return {startupMs,pageCount,timings,ink,beforeClose,closed};
   }finally{engine.dispose();}
  },{label,basePage,hasFont:!!font,fontIndex});
  const artifact=label==='before'?resolve(baseline,'rzpdf_web.wasm'):resolve(repo,'web/dist/rzpdf_web.wasm');
  const data=await readFile(artifact);report.runs.push({label,wasmBytes:(await stat(artifact)).size,wasmSha256:createHash('sha256').update(data).digest('hex'),...result});
 }
 for(const run of report.runs)assert.ok(run.ink>0,'benchmark regions must include actual page content');
 if(font)report.substituteFont={sha256:createHash('sha256').update(await readFile(font)).digest('hex'),index:Number(process.env.RZPDF_PERF_FONT_INDEX??0)};
 if(baseline){for(let index=0;index<7;index++)assert.deepEqual(frames.find(f=>f.label==='after'&&f.index===index).pixels,frames.find(f=>f.label==='before'&&f.index===index).pixels,`region ${index} differs from frozen baseline`);report.pixelComparison='7 regions exactly equal to frozen pre-change Wasm';}
 assert.equal(report.errors.length,0);report.status='passed';
}catch(error){report.errors.push(error.stack);process.exitCode=1;}
finally{await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));}
