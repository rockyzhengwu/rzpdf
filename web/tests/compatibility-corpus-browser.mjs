// All-page real-Wasm corpus probe. Inputs stay local; no PDF/text is uploaded.
// RZPDF_COMPAT_MANIFEST is a private hash-pinned manifest from inventory_pdf_corpus.py.
import {chromium} from 'playwright-core';
import {launchOwnedBrowser} from './owned-browser.mjs';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,appendFile,stat} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {createReadStream} from 'node:fs';
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
const repo=fileURLToPath(new URL('../../',import.meta.url)),web=resolve(process.env.RZPDF_COMPAT_WEB??resolve(repo,'web'));
if(!process.env.RZPDF_COMPAT_MANIFEST)throw Error('Set RZPDF_COMPAT_MANIFEST to a private corpus manifest');
const manifestPath=resolve(process.env.RZPDF_COMPAT_MANIFEST),manifestBytes=await readFile(manifestPath),manifest=JSON.parse(manifestBytes);
const shardCount=Number(process.env.RZPDF_COMPAT_SHARDS??1),shardIndex=Number(process.env.RZPDF_COMPAT_SHARD_INDEX??0);
if(!Number.isInteger(shardCount)||shardCount<1||shardCount>8||!Number.isInteger(shardIndex)||shardIndex<0||shardIndex>=shardCount)throw Error('Invalid corpus shard');
const cases=manifest.cases.filter((_,index)=>index%shardCount===shardIndex);
const out=resolve(process.env.RZPDF_COMPAT_OUTPUT??resolve(repo,'output/compatibility-browser'));await mkdir(out,{recursive:true});
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
let substitute;
if(process.env.RZPDF_COMPAT_SUBSTITUTE_FONT){
 const source=resolve(process.env.RZPDF_COMPAT_SUBSTITUTE_FONT),info=await stat(source);
 if(!info.isFile()||info.size<1||info.size>32*1024*1024)throw Error('Substitute font must be a regular file within 32 MiB');
 substitute=await readFile(source);if(substitute.length!==info.size)throw Error('Font changed during reading');
}
const fontConfiguration=substitute?{kind:'explicit-single-substitute',sha256:hash(substitute),bytes:substitute.length}:null;
const artifacts={};
for(const name of ['rzpdf_web.wasm','rzpdf_web.mjs','client.js','worker-client.js','pdf-worker.js','protocol.js','render-planner.js','errors.js','raster.js','fonts.js','interaction.js','text.js'])artifacts[name]=hash(await readFile(resolve(web,'dist',name)));
const report={browserHarnessSha256:hash(await readFile(resolve(repo,'web/tests/owned-browser.mjs'))),fontConfiguration,shard:{count:shardCount,index:shardIndex,documentIds:cases.map(e=>e.id)},referenceDriverSha256:hash(await readFile(resolve(repo,'scripts/compatibility_visual_worker.py'))),driverSha256:hash(await readFile(fileURLToPath(import.meta.url))),actualWasm:true,manifestSha256:hash(manifestBytes),artifacts,documents:[],status:'failed',visualComparison:'MuPDF and PDFium overview screening; not correctness proof'};
const server=createServer(async(req,res)=>{try{
 const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
 if(path==='/substitute.font'&&substitute){res.setHeader('Content-Type','application/octet-stream');res.end(substitute);return;}
 if(path==='/'){res.setHeader('Content-Type','text/html');res.end('<input id="file" type="file">');return;}
 const file=resolve(web,'.'+path);if(!file.startsWith(web+sep)){res.writeHead(403).end();return;}
 res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));
}catch{res.writeHead(404).end();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
const visual=spawn(process.env.RZPDF_COMPAT_PYTHON??resolve(repo,'output/benchmark-venv/bin/python'),[resolve(repo,'scripts/compatibility_visual_worker.py'),manifestPath,resolve(out,'visual')],{stdio:['pipe','pipe','inherit']});
let visualError;visual.on('error',error=>{visualError=error;});visual.stdin.on('error',error=>{visualError=error;});
const lines=createInterface({input:visual.stdout})[Symbol.asyncIterator]();
async function compare(request){
 let timer;
 try{
  if(visualError)throw visualError;
  if(visual.exitCode!==null)throw Error('Visual reference helper exited');
  await new Promise((done,reject)=>visual.stdin.write(JSON.stringify(request)+'\n',error=>error?reject(error):done()));
  const response=await Promise.race([lines.next(),new Promise((_,reject)=>{timer=setTimeout(()=>{visual.kill();reject(Error('Visual reference helper timed out'));},30000);})]);
  if(response.done)throw Error('Visual reference helper exited');
  return JSON.parse(response.value);
 }finally{clearTimeout(timer);}
}

let browser,ownedBrowser;
try{
 const hello=await lines.next();if(hello.done)throw Error('Visual reference helper did not initialize');
 report.referenceEngines=JSON.parse(hello.value);if(!report.referenceEngines.ready)throw Error('Invalid reference helper handshake');
 ownedBrowser=await launchOwnedBrowser(chromium,{executablePath:process.env.RZPDF_CHROMIUM,headless:true});browser=ownedBrowser.browser;report.browser=await browser.version();
 for(const entry of cases){
  const digest=createHash('sha256');for await(const block of createReadStream(entry.path))digest.update(block);
  if(digest.digest('hex')!==entry.sha256)throw Error('Input hash changed: '+entry.id);
  const context=await browser.newContext(),page=await context.newPage();
  const result={id:entry.id,sha256:entry.sha256,referencePages:entry.referencePages,operations:[],visual:[],status:'failed'};
  await page.exposeFunction('compareCompatibility',async frame=>{
   let metric;
   try{metric=await compare({id:entry.id,...frame});}
   catch(error){metric={needsReview:true,error:'Reference transport: '+error.message};}
   result.visual.push({page:frame.page,...metric});return metric;
  });
  await writeFile(resolve(out,entry.id+'.jsonl'),'');
  let lastProgress=Date.now();
  await page.exposeFunction('recordCompatibility',async event=>{
   if(Date.now()-lastProgress>10000){console.log(JSON.stringify({id:entry.id,page:event.page,operation:event.operation,shard:shardIndex}));lastProgress=Date.now();}
   result.operations.push(event);
   await appendFile(resolve(out,entry.id+'.jsonl'),JSON.stringify(event)+'\n');
  });
  try{
   await page.goto(`http://127.0.0.1:${server.address().port}`);
   await page.locator('#file').setInputFiles(entry.path);
   const finish=await page.evaluate(async({registerSubstitute})=>{
    const {createWorkerEngine,renderPreview}=await import('/dist/worker-client.js');
    const file=document.querySelector('#file').files[0];
    const engine=await createWorkerEngine({taskTimeoutMs:30000});
    const record=event=>globalThis.recordCompatibility(event);
    const error=e=>({status:'failed',code:e.code??'internal',message:e.message,resource:e.resource});
    let doc;
    try{
     if(registerSubstitute){
      try{await engine.setSubstituteFont(new Uint8Array(await(await fetch('/substitute.font')).arrayBuffer()));await record({operation:'fontSetup',result:{status:'passed'}});}
      catch(e){await record({operation:'fontSetup',result:error(e)});return {complete:false};}
     }
     try{doc=await engine.openBlob(file,{mode:'compatible'});await record({operation:'open',result:{status:'passed',pages:doc.pageCount}});}
     catch(e){await record({operation:'open',result:error(e)});return {complete:false};}
     const original=await doc.state();
     for(let page=0;page<doc.pageCount;page++){
      await record({operation:'page-start',page});
      try{
       const start=performance.now(),frame=await renderPreview(doc,page,{maxWidth:512,maxHeight:512,renderOptions:{annotations:false,includeWidgets:false}});
       const sha256=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',frame.pixels)),b=>b.toString(16).padStart(2,'0')).join('');
       let encoded='';for(let i=0;i<frame.pixels.length;i+=16384)encoded+=String.fromCharCode(...frame.pixels.subarray(i,i+16384));
       await globalThis.compareCompatibility({page,width:frame.width,height:frame.height,dpi:frame.dpi,rgba:btoa(encoded)});
       await record({operation:'render',page,result:{status:'passed',dpi:frame.dpi,width:frame.width,height:frame.height,sha256},milliseconds:performance.now()-start});
      }catch(e){await record({operation:'render',page,result:error(e)});if(['internal','invalid_handle','cancelled'].includes(e.code))return {complete:false};}
      try{const text=await doc.extractText(page);await record({operation:'text',page,result:{status:'passed',characters:text.characters.length,missingUnicode:text.characters.filter(c=>c.unicode===null).length}});}
      catch(e){await record({operation:'text',page,result:error(e)});if(['internal','invalid_handle','cancelled'].includes(e.code))return {complete:false};}
      if(page%32===31){await doc.clearRenderCaches();await doc.clearSourceStreamCache();}
     }
     const after=await doc.state();if(after.stamp.revision!==original.stamp.revision)throw Error('Read-only probe changed the document');
     try{
      const saved=await doc.save(),reopened=await engine.openBlob(new Blob([saved.bytes]),{mode:'strict'});
      if(reopened.pageCount!==doc.pageCount)throw Error('Saved page count changed');
      await reopened.close();await record({operation:'saveReopen',result:{status:'passed',bytes:saved.bytes.length}});
     }catch(e){await record({operation:'saveReopen',result:error(e)});}
     if(doc.pageCount){
      try{
       const before=await doc.pageInfo(0);
       await doc.apply({operation:'rotatePage',stamp:(await doc.state()).stamp,page:0,degrees:90});
       await doc.apply({operation:'rotatePage',stamp:(await doc.state()).stamp,page:0,degrees:-90});
       const after=await doc.pageInfo(0);
       if(before.width!==after.width||before.height!==after.height||before.rotation!==after.rotation)throw Error('Rotation roundtrip changed geometry');
       await record({operation:'editRoundtrip',result:{status:'passed'}});
      }catch(e){await record({operation:'editRoundtrip',result:error(e)});}
     }
     await doc.close();doc=undefined;
     return {complete:true,statistics:await engine.statistics()};
    }finally{engine.dispose();}
   },{registerSubstitute:Boolean(substitute)});
   Object.assign(result,finish);
   result.pageCountMismatch=result.operations.find(e=>e.operation==='open')?.result.pages!==entry.referencePages;
   result.status=finish.complete&&!result.pageCountMismatch&&!result.operations.some(e=>e.result?.status==='failed')&&!result.visual.some(v=>v.needsReview)?'passed':'failed';
  }catch(error){result.reason=error.message;}
  finally{await context.close();}
  await writeFile(resolve(out,entry.id+'.json'),JSON.stringify(result,null,2)+'\n');
  report.documents.push(result);
  await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify({id:entry.id,status:result.status,completed:report.documents.length,total:cases.length}));
 }
 const counts={documents:cases.length,pages:cases.reduce((sum,e)=>sum+e.referencePages,0)};
 for(const operation of ['open','render','text','saveReopen','editRoundtrip']){
  counts[operation+'Passed']=report.documents.flatMap(d=>d.operations).filter(e=>e.operation===operation&&e.result.status==='passed').length;
  counts[operation+'Failed']=report.documents.flatMap(d=>d.operations).filter(e=>e.operation===operation&&e.result.status==='failed').length;
  counts[operation+'Untested']=Math.max(0,(['render','text'].includes(operation)?counts.pages:counts.documents)-counts[operation+'Passed']-counts[operation+'Failed']);
 }
 counts.visualReviewPages=report.documents.reduce((sum,d)=>sum+d.visual.filter(v=>v.needsReview).length,0);
 report.counts=counts;report.status=report.documents.every(d=>d.status==='passed')?'passed':'failed';
 if(report.status!=='passed'&&process.env.RZPDF_COMPAT_REQUIRE_CLEAN==='1')process.exitCode=1;
}finally{visual.stdin.end();await ownedBrowser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n');}
