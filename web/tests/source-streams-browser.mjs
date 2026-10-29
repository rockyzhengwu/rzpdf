// Fixed editor fixture through optimized Wasm, SDK and Worker, at default limits.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';
import {largeImagePages} from '../../scripts/source-stream-fixture.mjs';
const root=fileURLToPath(new URL('../../',import.meta.url)),web=resolve(root,'web');
const out=resolve(process.env.RZPDF_SOURCE_STREAM_OUTPUT??resolve(root,'output/source-streams-20261029/browser'));
await mkdir(out,{recursive:true});
const fixture=largeImagePages(),digest=bytes=>createHash('sha256').update(bytes).digest('hex');
assert.equal(fixture.length,22023982);
assert.equal(digest(fixture),'03be863a438eb2b44709947a5a211a9b94d120e231c92bfaae26be521f1f2551');
const server=createServer(async(req,res)=>{
 try {
  const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
  if(path==='/fixture.pdf'){res.setHeader('Content-Type','application/pdf');res.end(fixture);return;}
  const file=resolve(web,path==='/'?'index.html':'.'+path);
  if(!file.startsWith(web+sep)){res.writeHead(403).end();return;}
  res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html'})[extname(file)]??'application/octet-stream');
  res.end(await readFile(file));
 } catch {res.writeHead(404).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser;
const report={status:'failed',fixture:{bytes:fixture.length,sha256:digest(fixture)},artifacts:{},checks:[],errors:[]};
try {
 for(const name of ['rzpdf_web.wasm','rzpdf_web.mjs','client.js','worker-client.js','pdf-worker.js']) report.artifacts[name]=digest(await readFile(resolve(web,'dist',name)));
 report.driverSha256=digest(await readFile(fileURLToPath(import.meta.url)));
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true});
 report.browser=await browser.version();
 const page=await browser.newPage();page.setDefaultTimeout(180000);
 page.on('pageerror',e=>report.errors.push(e.stack));
 await page.goto(`http://127.0.0.1:${server.address().port}`);
 report.checks=await page.evaluate(async()=>{
  const {createEngine}=await import('/dist/client.js'),{createWorkerEngine}=await import('/dist/worker-client.js');
  const bytes=new Uint8Array(await(await fetch('/fixture.pdf')).arrayBuffer());
  const check=(v,m)=>{if(!v)throw Error(m);};
  const hash=async bytes=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');
  const checks=[],direct=[];
  for(const worker of [false,true]) {
   const engine=await(worker?createWorkerEngine():createEngine());
   let stage='open';
   try {
    const doc=await engine.open(bytes,{mode:'strict'});
    check(doc.pageCount===7,'page count');
    const capabilities=await doc.capabilities();
    check(capabilities.maxCachedStreamBytes===16*1024*1024&&capabilities.maxDecodedBytes===16*1024*1024&&capabilities.maxOperationBytes===64*1024*1024,'default ceilings changed');
    const initial=await doc.cacheStatistics();
    check(initial.sourceInputBytes===22023982&&initial.rawStreamResidentBytes===0,'input range accounting');
    stage='cold search';
    const firstSearch=await doc.searchText('Image page',{maxResults:10000});
    check(firstSearch.matches.length===7&&firstSearch.pagesScanned===7&&!firstSearch.truncated,'cold search');
    const cold=await doc.cacheStatistics();
    check(cold.rawStreamResidentBytes===0&&cold.rawStreamMisses===0,'search materialized raw image storage');
    const baseline=[],rounds=[];
    for(let round=0;round<3;round++) {
     const frames=[];
     for(const index of [0,1,6,0,1,2,3,4,5,6]) {
      stage=`render round ${round} page ${index}`;
      const image=await doc.render(index,72);
      check(image.width===400&&image.height===400,'frame geometry');
      // Image rectangle interior has the fixture's blue channel and alpha.
      const p=(200*image.width+200)*4;
      check(Math.abs(image.pixels[p+2]-(80+index*20))<=1&&image.pixels[p+3]===255,'blank/wrong image page '+index);
      const sha256=await hash(image.pixels);
      if(round===0)baseline[index]=sha256;
      check(sha256===baseline[index],'navigation changed image');
      if(!worker&&round===0)direct[index]=sha256;
      if(worker)check(sha256===direct[index],'worker pixels differ');
      frames.push({page:index,sha256});
     }
     stage=`search round ${round}`;
     const search=await doc.searchText('Image page',{maxResults:10000});
     check(search.matches.length===7&&search.pagesScanned===7&&!search.truncated,'search after navigation');
     const stamp=(await doc.state()).stamp;
     const stats=await doc.clearSourceStreamCache();
     check(stats.rawStreamResidentBytes===0&&stats.rawStreamTransientBytes===0,'source cache clear');
     check(stats.pageEntries<=2&&stats.pageEstimatedBytes<=capabilities.maxPreparedPageEstimatedBytes,'unbounded prepared-page cache');
     const retained=await engine.statistics();
     check(retained.handles===1&&retained.retainedBytes===bytes.byteLength,'input retained more than once');
     await doc.clearRenderCaches();
     check((await doc.state()).stamp.revision===stamp.revision,'cache clear changed document revision');
     rounds.push({round,frames,searchMatches:search.matches.length,statistics:stats,retained});
    }
    const request={operation:'addAnnotation',stamp:(await doc.state()).stamp,page:0,annotation:{kind:'Square',rect:[10,10,25,25],color:[1,0,0]}};
    stage='annotation edit';
    await doc.apply(request);
    const editedState=await doc.state(),annotations=await doc.annotations(0);
    await doc.clearSourceStreamCache();await doc.clearRenderCaches();
    check((await doc.state()).commitRevision===editedState.commitRevision&&(await doc.annotations(0)).records.length===annotations.records.length,'cache clear lost edits');
    stage='save';
    const saved=await doc.save();
    check(saved.bytes.byteLength>22000000,'save truncated source streams');
    await doc.close();
    stage='reopen';
    const reopened=await engine.open(saved.bytes,{mode:'strict'});
    check((await reopened.searchText('Image page',{maxResults:10000})).matches.length===7,'saved search');
    check((await reopened.annotations(0)).records.length===annotations.records.length,'saved annotations');
    for(const index of [1,6])check(await hash((await reopened.render(index,72)).pixels)===baseline[index],'saved image changed');
    await reopened.close();
    const released=await engine.statistics();check(released.handles===0&&released.retainedBytes===0,'document handles retained');
    checks.push({transport:worker?'worker':'sync',capabilities,initial,cold,rounds,savedBytes:saved.bytes.byteLength,savedSha256:await hash(saved.bytes),editPreserved:true,released});
   } catch(error) {throw Error(JSON.stringify({transport:worker?'worker':'sync',stage,message:error.message,code:error.code,resource:error.resource,stack:error.stack}),{cause:error});}
   finally {engine.dispose();}
  }
  return checks;
 });
 assert.equal(report.checks.length,2);assert.equal(report.errors.length,0);report.status='passed';
} catch(error) {report.errors.push(error.stack);process.exitCode=1;}
finally {
 await browser?.close();await new Promise(done=>server.close(done));
 await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));
 console.log(JSON.stringify({status:report.status,transports:report.checks.length,errors:report.errors,report:resolve(out,'report.json')},null,2));
}
