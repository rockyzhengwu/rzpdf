// Fixed RZPDF-003 inputs through optimized Wasm, synchronous SDK and Worker.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';
const root=fileURLToPath(new URL('../../',import.meta.url)),web=resolve(root,'web');
const fixtures=resolve(root,'pdf/tests/fixtures/image-preview');
const out=resolve(process.env.RZPDF_IMAGE_PREVIEW_OUTPUT??resolve(root,'output/image-preview-20261029/browser'));
await mkdir(out,{recursive:true});
const names=['preview-flate.pdf','preview-jpeg.pdf','preview-progressive.pdf','preview-flate-corrupt.pdf'];
const server=createServer(async(req,res)=>{try{
 const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
 const fixture=names.find(n=>path==='/fixtures/'+n);
 const file=fixture?resolve(fixtures,fixture):resolve(web,'.'+(path==='/'?'/index.html':path));
 if(!fixture&&!file.startsWith(web+sep)){res.writeHead(403).end();return;}
 res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html'})[extname(file)]??'application/pdf');
 res.end(await readFile(file));
}catch{res.writeHead(404).end();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser;const report={status:'failed',artifacts:{},fixtures:{},checks:[],errors:[]};
try{
 const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
 const manifest=JSON.parse(await readFile(resolve(fixtures,'manifest.json')));
 for(const name of names){const bytes=await readFile(resolve(fixtures,name));assert.equal(digest(bytes),manifest[name].sha256);report.fixtures[name]=manifest[name];}
 for(const name of ['rzpdf_web.wasm','rzpdf_web.mjs','client.js','worker-client.js','pdf-worker.js'])report.artifacts[name]=digest(await readFile(resolve(web,'dist',name)));
 report.driverSha256=digest(await readFile(fileURLToPath(import.meta.url)));
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true});report.browser=await browser.version();
 const page=await browser.newPage();page.setDefaultTimeout(180000);page.on('pageerror',e=>report.errors.push(e.stack));
 await page.goto(`http://127.0.0.1:${server.address().port}`);
 report.checks=await page.evaluate(async()=>{
  const {createEngine}=await import('/dist/client.js'),{createWorkerEngine}=await import('/dist/worker-client.js');
  const bytes=async name=>new Uint8Array(await(await fetch('/fixtures/'+name)).arrayBuffer());
  const flate=await bytes('preview-flate.pdf'),jpeg=await bytes('preview-jpeg.pdf'),progressive=await bytes('preview-progressive.pdf'),corrupt=await bytes('preview-flate-corrupt.pdf');
  const hash=async value=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',value)),v=>v.toString(16).padStart(2,'0')).join('');
  const check=(v,m)=>{if(!v)throw Error(m);};
  const oracle=(frame,rotation=0)=>{
   const points=[[.5,.2,[230,180,60,255]],[.5,.375,[34,96,180,255]],[.02,.9,[34,96,180,255]],
    ...Array.from({length:8},(_,i)=>[.5,(1130+i*160)/2665,[240,245,250,255]])];
   for(const [u,v,color] of points){
    const x=Math.floor((rotation===90?1-v:u)*frame.width),y=Math.floor((rotation===90?u:v)*frame.height),offset=(y*frame.width+x)*4;
    check(color.every((value,c)=>Math.abs(frame.pixels[offset+c]-value)<=1),'incorrect/blank preview at '+[u,v]);
   }
  };
  const reject=async(fn,reason)=>{let error;try{await fn();}catch(e){error=e;}check(error?.code==='resource_limit'&&error.resource?.reason===reason,'unexpected rejection '+error?.message);return error;};
  const checks=[],direct=new Map();
  for(const worker of [false,true]){
   const engine=await(worker?createWorkerEngine():createEngine());let stage='open';
   try{
    const doc=await engine.open(flate,{mode:'compatible'}),cap=await doc.capabilities();
    check(cap.maxImagePixels===4000000&&cap.maxDecodedBytes===16777216&&cap.maxOperationBytes===67108864,'default budgets changed');
    check(cap.imageDecode.flateTargetResolution&&cap.imageDecode.flateStorage==='scanlines-and-target'&&cap.imageDecode.progressiveJpegTargetResolution,'capabilities');
    const originalState=await doc.state(),baseline=new Map(),rounds=[];
    for(let round=0;round<3;round++){
     for(const dpi of [72,36,18]){
      stage=`round ${round} dpi ${dpi}`;
      const start=performance.now();const frame=await doc.render(1,dpi,{imageDecode:'target'});oracle(frame);
      check(frame.width===Math.ceil(595*dpi/72)&&frame.height===Math.ceil(842*dpi/72),'geometry');
      const sha256=await hash(frame.pixels);if(round===0)baseline.set(dpi,sha256);
      check(sha256===baseline.get(dpi),'preview changed after cache clearing');
      if(!worker&&round===0)direct.set(dpi,sha256);if(worker)check(sha256===direct.get(dpi),'transport mismatch');
      rounds.push({round,dpi,sha256,width:frame.width,height:frame.height,milliseconds:performance.now()-start});
     }
     const fallback=await doc.render(1,72);oracle(fallback);check(await hash(fallback.pixels)===baseline.get(72),'budgeted fallback');
     const stats=await doc.cacheStatistics();check(stats.pageEntries<=2&&stats.pageEstimatedBytes<=cap.maxPreparedPageEstimatedBytes,'unbounded cache');
     check(stats.rawStreamResidentBytes===0&&stats.rawStreamTransientBytes===0,'source payload unexpectedly resident');
     await doc.clearRenderCaches();await doc.clearSourceStreamCache();
    }
    stage='view rotation context';const view=await doc.render(1,72,{imageDecode:'target',viewRotation:90});oracle(view);check(await hash(view.pixels)===baseline.get(72),'view rotation unexpectedly changed page pixels');
    stage='output rejection';
    const high=await reject(()=>doc.renderRegion(1,{x:0,y:0,width:32,height:32},288,{imageDecode:'target'}),'image_pixels');
    check(high.resource.image.encoding==='flate'&&high.resource.image.width===1916&&high.resource.image.height===2665&&high.resource.image.targetDecodeSupported,'image context');
    check((await doc.state()).commitRevision===originalState.commitRevision,'render changed document');oracle(await doc.render(1,72,{imageDecode:'target'}));
    stage='document rotation';await doc.apply({operation:'rotatePage',stamp:(await doc.state()).stamp,page:1,degrees:90});
    const rotated=await doc.render(1,72,{imageDecode:'target'});check(rotated.width===842&&rotated.height===595,'rotated geometry');oracle(rotated,90);
    await doc.apply({operation:'rotatePage',stamp:(await doc.state()).stamp,page:1,degrees:-90});
    check(await hash((await doc.render(1,72,{imageDecode:'target'})).pixels)===baseline.get(72),'rotation restore');
    stage='save';const saved=await doc.save();await doc.close();
    const reopened=await engine.open(saved.bytes,{mode:'strict'});oracle(await reopened.render(1,72,{imageDecode:'target'}));
    check(await hash((await reopened.render(1,72,{imageDecode:'target'})).pixels)===baseline.get(72),'saved preview differs');await reopened.close();
    stage='source/resident separation';const small=await engine.open(flate,{mode:'compatible',resourceProfile:'conservative'});
    oracle(await small.render(1,18,{imageDecode:'target'}));await small.render(0,72);await small.close();
    stage='JPEG regression';
    for(const input of [jpeg,progressive]){
     const jpegDoc=await engine.open(input,{mode:'compatible'});
     for(const dpi of [72,36,18]){
      const frame=await jpegDoc.render(1,dpi,{imageDecode:'target'});
      check(frame.width===Math.ceil(595*dpi/72)&&frame.height===Math.ceil(842*dpi/72),'JPEG preview geometry');
      check(frame.pixels.some((v,i)=>i%4!==3&&v<200),'nonblank JPEG preview');
     }
     await jpegDoc.close();
    }
    stage='corrupt Flate atomicity';const damaged=await engine.open(corrupt,{mode:'compatible'}),beforeFailure=await damaged.state();
    let invalidData;try{await damaged.render(1,72,{imageDecode:'target'});}catch(e){invalidData=e;}
    check(invalidData?.code==='format'&&invalidData.resource?.image?.encoding==='flate'&&invalidData.resource.image.targetDecodeSupported,'corrupt Flate diagnostic '+JSON.stringify({code:invalidData?.code,resource:invalidData?.resource}));
    check((await damaged.cacheStatistics()).pageEntries===0,'partial page published');
    check((await damaged.state()).commitRevision===beforeFailure.commitRevision,'failed decode changed document');
    await damaged.render(0,72);await damaged.close();
    let cancellation=false;
    if(worker){
     stage='active cancellation';const disposable=await engine.open(flate,{mode:'compatible'});const controller=new AbortController();
     const pending=disposable.render(1,72,{imageDecode:'target',signal:controller.signal}).catch(e=>e);controller.abort();
     check((await pending).code==='cancelled','cancellation');let invalid=false;try{await disposable.state();}catch(e){invalid=e.code==='invalid_handle';}check(invalid,'cancelled session still usable');
     const restored=await engine.open(flate,{mode:'compatible'});oracle(await restored.render(1,72,{imageDecode:'target'}));await restored.close();cancellation=true;
    }
    const released=await engine.statistics();check(released.handles===0&&released.retainedBytes===0,'resource release');
    checks.push({transport:worker?'worker':'sync',rounds,sourceResidentSeparation:true,progressivePositive:true,cancellation,strictSavedReopen:true,savedBytes:saved.bytes.length,savedSha256:await hash(saved.bytes),released});
   }catch(error){throw Error(JSON.stringify({transport:worker?'worker':'sync',stage,message:error.message,code:error.code,resource:error.resource,stack:error.stack}),{cause:error});}
   finally{engine.dispose();}
  }return checks;
 });
 assert.equal(report.checks.length,2);assert.equal(report.errors.length,0);report.status='passed';
}catch(error){report.errors.push(error.stack);process.exitCode=1;}
finally{await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,transports:report.checks.length,errors:report.errors,report:resolve(out,'report.json')},null,2));}
