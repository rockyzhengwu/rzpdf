// Local transparency targets, byte budgets, ICC ownership and recovery.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';
const root=fileURLToPath(new URL('../../',import.meta.url)),web=resolve(root,'web');
const out=resolve(process.env.RZPDF_GROUP_OUTPUT??resolve(root,'output/group-resources-20261031'));
const fixtures=resolve(root,'pdf/tests/fixtures/group-resources'),refs=resolve(out,'references');
const manifest=JSON.parse(await readFile(resolve(fixtures,'manifest.json'))),refManifest=JSON.parse(await readFile(resolve(refs,'manifest.json')));
const corpus=process.env.RZPDF_GROUP_PDF;
await mkdir(out,{recursive:true});
const server=createServer(async(req,res)=>{try{
 const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);let file;
 if(path==='/corpus.pdf'&&corpus)file=corpus;
 else if(path.startsWith('/fixtures/')){assert(Object.hasOwn(manifest,path.slice(10)));file=resolve(fixtures,path.slice(10));}
 else if(path.startsWith('/references/')){assert(refManifest.files.some(f=>f.name===path.slice(12)));file=resolve(refs,path.slice(12));}
 else{file=resolve(web,'.'+(path==='/'?'/index.html':path));assert(file.startsWith(web+sep));}
 res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.png':'image/png'})[extname(file)]??'application/pdf');res.end(await readFile(file));
}catch{res.writeHead(404).end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser;
const digest=b=>createHash('sha256').update(b).digest('hex');
const report={status:'failed',artifacts:{},fixtures:manifest,referenceEngines:{mupdf:refManifest.mupdf,pdfium:refManifest.pdfium},checks:[],errors:[]};
try{
 for(const [n,v]of Object.entries(manifest))assert.equal(digest(await readFile(resolve(fixtures,n))),v.sha256);
 for(const v of refManifest.files)assert.equal(digest(await readFile(resolve(refs,v.name))),v.sha256);
 if(corpus)report.corpus={path:corpus,sha256:digest(await readFile(corpus))};
 for(const name of ['rzpdf_web.wasm','rzpdf_web.mjs','client.js','worker-client.js','pdf-worker.js'])report.artifacts[name]=digest(await readFile(resolve(web,'dist',name)));
 report.driverSha256=digest(await readFile(fileURLToPath(import.meta.url)));
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true});report.browser=await browser.version();
 const page=await browser.newPage();page.setDefaultTimeout(180000);page.on('pageerror',e=>report.errors.push(e.stack));await page.goto(`http://127.0.0.1:${server.address().port}`);
 report.checks=await page.evaluate(async({names,hasCorpus})=>{
  const {createEngine}=await import('/dist/client.js'),{createWorkerEngine}=await import('/dist/worker-client.js');
  const check=(v,m)=>{if(!v)throw Error(m);};
  const bytes=async path=>new Uint8Array(await(await fetch(path)).arrayBuffer());
  const hash=async v=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',v)),v=>v.toString(16).padStart(2,'0')).join('');
  const png=async frame=>{const c=new OffscreenCanvas(frame.width,frame.height);c.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(frame.pixels),frame.width,frame.height),0,0);const b=await c.convertToBlob();return new Promise(r=>{const f=new FileReader();f.onload=()=>r(f.result);f.readAsDataURL(b);});};
  const reference=async(frame,name,dpi,engine)=>{
   const bitmap=await createImageBitmap(await(await fetch(`/references/${name.slice(0,-4)}-${engine}-${dpi}.png`)).blob());
   check(bitmap.width===frame.width&&bitmap.height===frame.height,'reference dimensions');const c=new OffscreenCanvas(bitmap.width,bitmap.height),ctx=c.getContext('2d');ctx.drawImage(bitmap,0,0);bitmap.close();const expected=ctx.getImageData(0,0,c.width,c.height).data;
   let sum=0,bad=0;for(let i=0;i<expected.length;i+=4){let error=0;for(let k=0;k<3;k++){const d=Math.abs(frame.pixels[i+k]-expected[i+k]);sum+=d;error=Math.max(error,d);}if(error>12)bad++;}
   const mae=sum/(c.width*c.height*3),badFraction=bad/(c.width*c.height);check(mae<2&&badFraction<.025,`reference ${name}/${dpi}/${engine}: ${mae},${badFraction}`);return{engine,mae,badFraction};
  };
  const probe=(frame,name,dpi)=>{
   let x=90,y=780,expected=[188,188,188];
   if(name==='nested.pdf'){x=110;y=790;}
   if(name==='rotated.pdf'){x=150+.8660254*50-.5*20;y=700+.5*50+.8660254*20;}
   if(name==='soft-mask.pdf')expected=[183,196,209];
   if(name==='knockout.pdf'){x=115;y=780;expected=[179,204,230];}
   const offset=(Math.floor((840-y)*dpi/72)*frame.width+Math.floor(x*dpi/72))*4;
   check(expected.every((v,k)=>Math.abs(frame.pixels[offset+k]-v)<=3),`color/alpha oracle ${name} ${dpi}: ${Array.from(frame.pixels.slice(offset,offset+4))}`);
  };
  const results=[],direct=new Map();
  for(const worker of [false,true]){const engine=await(worker?createWorkerEngine():createEngine());let stage='open';try{
   const samples=[];
   for(const name of names.filter(n=>n!=='large-group.pdf')){
    stage=name;const doc=await engine.open(await bytes('/fixtures/'+name),{mode:'strict'}),state=await doc.state(),cap=await doc.capabilities();
    check(cap.maxRenderPixels===4000000&&cap.maxRenderBytes===16000000&&cap.maxWasmMemoryBytes===268435456,'limits changed');check(cap.renderResources.localTransparencyGroups,'capability');
    for(const dpi of [72,96,144]){
     const frame=await doc.render(0,dpi,{annotations:false,includeWidgets:false});probe(frame,name,dpi);const sha256=await hash(frame.pixels),key=name+':'+dpi;
     if(!worker)direct.set(key,sha256);else check(direct.get(key)===sha256,'transport mismatch');
     const comparisons=[];for(const ref of ['mupdf','pdfium'])comparisons.push(await reference(frame,name,dpi,ref));
     if(dpi===144){const rect={x:96,y:64,width:256,height:128},tile=await doc.renderRegion(0,rect,dpi,{annotations:false,includeWidgets:false});let max=0;for(let y=0;y<tile.height;y++)for(let x=0;x<tile.width;x++)for(let k=0;k<4;k++)max=Math.max(max,Math.abs(tile.pixels[(y*tile.width+x)*4+k]-frame.pixels[((y+rect.y)*frame.width+x+rect.x)*4+k]));check(max<=1,'group tile seam '+name+': '+max);}
     samples.push({name,dpi,sha256,comparisons,wasmLinearMemoryBytes:worker?null:engine.module.HEAPU8.buffer.byteLength});await doc.clearRenderCaches();check((await doc.cacheStatistics()).pageEntries===0,'clear caches');
    }
    check((await doc.state()).commitRevision===state.commitRevision,'render mutation');const saved=await doc.save();await doc.close();const reopened=await engine.open(saved.bytes,{mode:'strict'});probe(await reopened.render(0,144),name,144);await reopened.close();
   }
   stage='live budget failure';const large=await engine.open(await bytes('/fixtures/large-group.pdf'),{mode:'strict'}),before=await large.state(),owned=await engine.statistics();
   let failure;try{await large.render(0,144);}catch(e){failure=e;}
   check(failure?.code==='resource_limit'&&failure.resource.reason==='render_live_bytes'&&failure.resource.stage==='render_group'&&failure.resource.limit===16000000&&failure.resource.actual>failure.resource.limit,'typed group refusal '+JSON.stringify(failure?.resource));
   check((await engine.statistics()).retainedBytes===owned.retainedBytes,'partial output retained');check((await large.state()).commitRevision===before.commitRevision,'failed render mutation');await large.render(0,96);await large.clearRenderCaches();
   for(const dpi of [144,288,576]){const tile=await large.renderRegion(0,{x:32,y:32,width:128,height:96},dpi);check(tile.width===128&&tile.height===96,'tile geometry');check(Math.abs(tile.pixels[(48*128+64)*4]-128)<=3,'ICC tile color '+Array.from(tile.pixels.slice((48*128+64)*4,(48*128+64)*4+4)));}
   await large.close();
   let paper=[];
   if(hasCorpus){stage='original paper';const doc=await engine.open(await bytes('/corpus.pdf'),{mode:'compatible'});for(const dpi of [72,102,108,144]){const f=await doc.render(0,dpi,{annotations:false,includeWidgets:false});paper.push({dpi,width:f.width,height:f.height,sha256:await hash(f.pixels),...(!worker&&dpi===144?{png:await png(f)}:{})});await doc.clearRenderCaches();}
    let error;try{await doc.render(0,288);}catch(e){error=e;}check(error?.code==='resource_limit'&&error.resource.reason==='render_pixels','expected oversized whole page');await doc.renderRegion(0,{x:0,y:0,width:256,height:128},576);await doc.close();}
   let cancellation=false;if(worker){stage='cancel/recover';const doc=await engine.open(await bytes('/fixtures/local-icc.pdf'),{mode:'strict'}),controller=new AbortController();const pending=doc.render(0,144,{signal:controller.signal}).catch(e=>e);controller.abort();check((await pending).code==='cancelled','cancel');const fresh=await engine.open(await bytes('/fixtures/local-icc.pdf'),{mode:'strict'});probe(await fresh.render(0,144),'local-icc.pdf',144);await fresh.close();cancellation=true;}
   const released=await engine.statistics();check(released.handles===0&&released.retainedBytes===0,'release');results.push({transport:worker?'worker':'sync',samples,groupFailure:failure.resource,paper,cancellation,released});
  }catch(error){throw Error(JSON.stringify({transport:worker?'worker':'sync',stage,message:error.message,code:error.code,resource:error.resource,stack:error.stack}));}finally{engine.dispose();}}
  return results;
 },{names:Object.keys(manifest),hasCorpus:!!corpus});
 for(const check of report.checks)for(const p of check.paper)if(p.png){const name=`paper-${check.transport}-${p.dpi}.png`;await writeFile(resolve(out,name),Buffer.from(p.png.split(',')[1],'base64'));delete p.png;p.output=name;}
 assert.equal(report.checks.length,2);assert.equal(report.errors.length,0);report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;}
finally{await browser?.close();await new Promise(r=>server.close(r));await writeFile(resolve(out,'browser-report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,transports:report.checks.length,errors:report.errors,report:resolve(out,'browser-report.json')},null,2));}
