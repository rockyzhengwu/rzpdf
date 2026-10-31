// Actual optimized Wasm: bounded source scans, typed failures and Blob lifetime.
import assert from 'node:assert/strict';
import {chromium} from 'playwright-core';
import {launchOwnedBrowser} from './owned-browser.mjs';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,open} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
const repo=fileURLToPath(new URL('../../',import.meta.url)),web=resolve(repo,'web');
const out=resolve(process.env.RZPDF_COMPAT_RESOURCE_OUTPUT??resolve(repo,'output/compatibility-resources'));await mkdir(out,{recursive:true});
// Sparse local file: a valid unreferenced 80 MiB stream before the xref table.
// The test must open/read its page without materializing that unused stream.
const large=resolve(out,'large-source.pdf'),handle=await open(large,'w');
let prefix='%PDF-1.7\n%\u00e2\u00e3\u00cf\u00d3\n',offsets=[];
for(const [i,body] of ['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R] /Count 1 >>','<< /Type /Page /Parent 2 0 R /MediaBox [0 0 20 20] /Contents 4 0 R >>','<< /Length 23 >>\nstream\n1 0 0 rg 0 0 20 20 re f\nendstream'].entries()){
 offsets.push(Buffer.byteLength(prefix));prefix+=`${i+1} 0 obj\n${body}\nendobj\n`;
}
offsets.push(Buffer.byteLength(prefix));prefix+='5 0 obj\n<< /Length 83886080 >>\nstream\n';
const tail='\nendstream\nendobj\n',xref=Buffer.byteLength(prefix)+83886080+Buffer.byteLength(tail);
await handle.write(prefix);await handle.write(tail,Buffer.byteLength(prefix)+83886080);
await handle.write(`xref\n0 6\n0000000000 65535 f \n${offsets.map(n=>String(n).padStart(10,'0')+' 00000 n \n').join('')}trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`,xref);await handle.close();
const server=createServer(async(req,res)=>{try{
 const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
 if(path==='/'){res.setHeader('Content-Type','text/html');res.end('<input id="source" type="file">');return;}
 const fixture=['scan-flate.pdf','scan-ccitt.pdf','scan-jbig2.pdf','hint-failed.pdf','hint-reference.pdf','encrypted-r4.pdf','encrypted-r6.pdf'].find(n=>path==='/fixtures/'+n);
 const file=fixture?(fixture==='encrypted-r4.pdf'?resolve(repo,'pdf/tests/fixtures/r4/password-user.pdf'):fixture==='encrypted-r6.pdf'?resolve(repo,'pdf/tests/fixtures/linearized/two-pages-r6.pdf'):resolve(repo,'pdf/tests/fixtures/compatibility',fixture)):resolve(web,'.'+path);
 if(!fixture&&!file.startsWith(web+sep)){res.writeHead(403).end();return;}
 res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm'})[extname(file)]??'application/pdf');res.end(await readFile(file));
}catch{res.writeHead(404).end();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
const fixtureManifest=JSON.parse(await readFile(resolve(repo,'pdf/tests/fixtures/compatibility/manifest.json')));
const report={browserHarnessSha256:createHash('sha256').update(await readFile(resolve(repo,'web/tests/owned-browser.mjs'))).digest('hex'),status:'failed',workerFixtureSha256:createHash('sha256').update(await readFile(resolve(repo,'web/tests/fixtures/blob-ids-worker.mjs'))).digest('hex'),driverSha256:createHash('sha256').update(await readFile(fileURLToPath(import.meta.url))).digest('hex'),fixtures:fixtureManifest,artifacts:{},checks:[],errors:[]};let browser,ownedBrowser;
try{
 report.encryptedFixtures={};for(const name of ['pdf/tests/fixtures/r4/password-user.pdf','pdf/tests/fixtures/linearized/two-pages-r6.pdf'])report.encryptedFixtures[name]=createHash('sha256').update(await readFile(resolve(repo,name))).digest('hex');
 for(const [name,entry] of Object.entries(fixtureManifest)){const data=await readFile(resolve(repo,'pdf/tests/fixtures/compatibility',name));assert.equal(data.length,entry.bytes);assert.equal(createHash('sha256').update(data).digest('hex'),entry.sha256);}
 for(const name of ['rzpdf_web.wasm','rzpdf_web.mjs','client.js','errors.js','worker-client.js','pdf-worker.js','protocol.js','render-planner.js','raster.js','fonts.js','interaction.js','text.js'])report.artifacts[name]=createHash('sha256').update(await readFile(resolve(web,'dist',name))).digest('hex');
 ownedBrowser=await launchOwnedBrowser(chromium,{executablePath:process.env.RZPDF_CHROMIUM,headless:true});browser=ownedBrowser.browser;const page=await browser.newPage();
 await page.goto(`http://127.0.0.1:${server.address().port}`);await page.locator('#source').setInputFiles(large);
 report.checks=await page.evaluate(async()=>{
  const {createEngine,renderPreview,renderTiles}=await import('/dist/client.js');
  const {createWorkerEngine}=await import('/dist/worker-client.js');
  const check=(condition,message)=>{if(!condition)throw Error(message);};
  const checks=[];
  for(const worker of [false,true]){
   const engine=await(worker?createWorkerEngine():createEngine());
   try{
    const caps=await engine.capabilities();check(caps.maxImageSourceBytes===134217728&&caps.maxDecodedBytes===16777216&&caps.imageDecode.progressiveJpegTargetResolution&&caps.imageDecode.ccittTargetResolution&&caps.imageDecode.jbig2TargetResolution,'decode capabilities');
    for(const name of ['scan-flate.pdf','scan-ccitt.pdf','scan-jbig2.pdf']){
     const bytes=new Uint8Array(await(await fetch('/fixtures/'+name)).arrayBuffer());
     const doc=await engine.open(bytes,{mode:'strict'});
     for(const dpi of [18,36,72]){
      const image=await doc.render(0,dpi,{imageDecode:'target'});
      const sample=(x,y)=>[...image.pixels.subarray((Math.floor(y*image.height)*image.width+Math.floor(x*image.width))*4,(Math.floor(y*image.height)*image.width+Math.floor(x*image.width))*4+4)];
      if(name==='scan-flate.pdf')check(sample(.25,.5).every((v,c)=>v===(c===3?255:121)),'Flate scan color');
      else if(name==='scan-ccitt.pdf'){check(sample(.25,.5).every((v,c)=>v===[255,0,0,255][c]),'CCITT stencil paint');check(sample(.75,.5).every((v,c)=>Math.abs(v-[179,204,230,255][c])<=1),'CCITT stencil transparency');}
      else {check(sample(.25,.5).every((v,c)=>v===[255,0,0,255][c]),'JBIG2 opaque stencil');for(const x of [.6,.65,.7,.75,.8,.85,.9])check(sample(x,.5).every((v,c)=>Math.abs(v-[198,153,173,255][c])<=8),'JBIG2 thin-stroke fractional coverage');}
     }
     const preview=await renderPreview(doc,0,{maxWidth:320,maxHeight:320});check(preview.width<=320&&preview.height<=320,'preview fitted dimensions');
     await doc.close();
    }
    for(const dpi of [18,36,72]){
     const images=[];
     for(const name of ['hint-reference.pdf','hint-failed.pdf']){
      const doc=await engine.open(new Uint8Array(await(await fetch('/fixtures/'+name)).arrayBuffer()),{mode:'strict'});
      const frame=await doc.render(0,dpi);images.push(frame);
      const offset=(Math.floor(17*dpi/72)*frame.width+Math.floor(13*dpi/72))*4;
      check(frame.pixels[offset]<180&&frame.pixels[offset+3]===255,'visible outline survives failed font hinting at '+dpi+'/'+name+': '+[...frame.pixels.subarray(offset,offset+4)]);
      await doc.close();
     }
     check(images[0].width===images[1].width&&images[0].height===images[1].height,'hint retry dimensions');
    }
    checks.push({transport:worker?'worker':'sync',scans:'passed',hintRecovery:'passed',statistics:await engine.statistics()});
   }finally{engine.dispose();}
  }
  const engine=await createWorkerEngine();
  try{
   const file=document.querySelector('#source').files[0];check(file.size>80*1024*1024,'large input file');
   for(let round=0;round<3;round++){
    const doc=await engine.openBlob(file,{mode:'strict'});check(doc.pageCount===1,'Blob page count');
    if(round===0){
     let pixels=0;
     for await(const tile of renderTiles(doc,0,{dpi:576,tileSide:64})){check(tile.pixels.every((v,i)=>v===[255,0,0,255][i%4]),'planned high-DPI tile pixels');pixels+=tile.width*tile.height;}
     check(pixels===160*160,'planned high-DPI coverage');
     const abort=new AbortController(),tiles=renderTiles(doc,0,{dpi:576,tileSide:64,signal:abort.signal});
     await tiles.next();abort.abort();let cancelled;try{await tiles.next();}catch(e){cancelled=e;}
     check(cancelled?.name==='AbortError','cooperative tile cancellation');await doc.state();
    }
    const frame=await doc.render(0,72);check(frame.pixels.every((v,i)=>v===[255,0,0,255][i%4]),'Blob exact red pixels');
    const cache=await doc.cacheStatistics();check(cache.sourceInputBytes===0&&cache.rawStreamResidentBytes<65536,'external source and unused stream stayed out of retained input storage');
    const active=await engine.statistics();check(active.externalSources===1&&active.externalSourceBytes===file.size,'source ownership');
    check(active.linearMemoryCapacityBytes<64*1024*1024,'Blob did not materialize in linear memory');
    await doc.close();const released=await engine.statistics();check(released.externalSources===0&&released.externalSourceBytes===0&&released.handles===0,'source release');
   }
   let invalid;try{await engine.openBlob(new Blob(['invalid pdf']));}catch(error){invalid=error;}check(invalid?.code==='format','invalid source returns error');
   check((await engine.statistics()).externalSources===0,'failed source release');
   const scan=new Blob([await(await fetch('/fixtures/scan-flate.pdf')).arrayBuffer()]);
   const a=await engine.openBlob(scan,{mode:'strict'}),b=await engine.openBlob(scan,{mode:'strict'});
   await a.close();await b.render(0,36);await b.close();check((await engine.statistics()).externalSources===0,'independent reader lifetimes');
   const maxHandles=Number((await engine.capabilities()).maxHandles),owners=[];
   for(let index=0;index<maxHandles-1;index++)owners.push(await engine.openBlob(file,{mode:'strict'}));
   let exhausted;try{await engine.openBlob(file,{mode:'strict'});}catch(error){exhausted=error;}
   check(exhausted?.code==='resource_limit','handle exhaustion returns a resource error');
   const full=await engine.statistics();check(full.handles===owners.length&&full.externalSources===owners.length,'failed publication releases only the rejected source');
   for(const owner of owners)await owner.close();
   check((await engine.statistics()).externalSources===0,'handle exhaustion leaves no source references');
   for(const name of ['encrypted-r4.pdf','encrypted-r6.pdf']){
    const bytes=new Uint8Array(await(await fetch('/fixtures/'+name)).arrayBuffer()),blob=new Blob([bytes]);
    for(const [options,code] of [[{},'password_required'],[{password:'wrong'},'password_invalid']]){
     let rejected;try{await engine.openBlob(blob,options);}catch(error){rejected=error;}
     check(rejected?.code===code,'Blob password error');check((await engine.statistics()).externalSources===0,'failed encrypted source release');
    }
    const options={mode:'strict',password:'synthetic-user',resourceProfile:'conservative'};
    const external=await engine.openBlob(blob,options),copied=await engine.open(bytes,options);
    const caps=await external.capabilities();check(caps.maxOperationBytes===33554432&&caps.maxInputBytes===2147483648&&caps.maxByteInputBytes===33554432,'Blob effective profile and input limits');
    const a=await external.render(0,36),b=await copied.render(0,36);
    check(a.width===b.width&&a.height===b.height&&a.pixels.every((value,index)=>value===b.pixels[index]),'encrypted Blob pixels equal byte-backed reader');
    await external.close();await copied.close();check((await engine.statistics()).externalSources===0,'encrypted reader source release');
   }
   checks.push({blob:'80 MiB source, repeated close/reopen, invalid input, handle exhaustion and independent owners passed',encryptedBlob:'R4/R6 passwords, conservative profile, exact pixel parity and failure cleanup passed',statistics:await engine.statistics()});
  }finally{engine.dispose();}
  const highIds=await new Promise((resolve,reject)=>{
   const worker=new Worker('/tests/fixtures/blob-ids-worker.mjs',{type:'module'});
   const timer=setTimeout(()=>{worker.terminate();reject(Error('Source-ID boundary probe timed out'));},30000);
   worker.onmessage=event=>{clearTimeout(timer);worker.terminate();event.data.error?reject(Error(event.data.error)):resolve(event.data);};
   worker.onerror=event=>{clearTimeout(timer);worker.terminate();reject(Error(event.message));};
   worker.postMessage(document.querySelector('#source').files[0]);
  });
  checks.push(highIds);
  return checks;
 });
 assert.equal(report.checks.length,4);report.status='passed';
}catch(error){report.errors.push(error.stack);process.exitCode=1;}
finally{await ownedBrowser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report,null,2));}
