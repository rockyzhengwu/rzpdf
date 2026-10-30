// RZPDF-004: optimized WASM, both transports, independent reference engines.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';
const root=fileURLToPath(new URL('../../',import.meta.url)),web=resolve(root,'web');
const fixtures=resolve(root,'pdf/tests/fixtures/masked-preview');
const out=resolve(process.env.RZPDF_MASKED_PREVIEW_OUTPUT??resolve(root,'output/masked-preview-20261030'));
const references=resolve(process.env.RZPDF_MASKED_REFERENCES??resolve(out,'references'));
await mkdir(out,{recursive:true});
const manifest=JSON.parse(await readFile(resolve(fixtures,'manifest.json'))),names=Object.keys(manifest);
const refManifest=JSON.parse(await readFile(resolve(references,'manifest.json')));
const server=createServer(async(req,res)=>{try{
 const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
 let file;
 if(path.startsWith('/fixtures/')){assert(names.includes(path.slice(10)));file=resolve(fixtures,path.slice(10));}
 else if(path.startsWith('/references/')){const name=path.slice(12);assert(refManifest.files.some(f=>f.name===name));file=resolve(references,name);}
 else {file=resolve(web,'.'+(path==='/'?'/index.html':path));assert(file.startsWith(web+sep));}
 res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.png':'image/png'})[extname(file)]??'application/pdf');
 res.end(await readFile(file));
}catch{res.writeHead(404).end();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
const digest=b=>createHash('sha256').update(b).digest('hex');
let browser;const report={status:'failed',fixtures:manifest,referenceEngines:{mupdf:refManifest.mupdf,pdfium:refManifest.pdfium},artifacts:{},checks:[],errors:[]};
try{
 for(const name of names)assert.equal(digest(await readFile(resolve(fixtures,name))),manifest[name].sha256);
 for(const ref of refManifest.files)assert.equal(digest(await readFile(resolve(references,ref.name))),ref.sha256);
 for(const name of ['rzpdf_web.wasm','rzpdf_web.mjs','client.js','worker-client.js','pdf-worker.js'])report.artifacts[name]=digest(await readFile(resolve(web,'dist',name)));
 report.driverSha256=digest(await readFile(fileURLToPath(import.meta.url)));
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true});report.browser=await browser.version();
 const page=await browser.newPage();page.on('pageerror',e=>report.errors.push(e.stack));page.setDefaultTimeout(180000);
 await page.goto(`http://127.0.0.1:${server.address().port}`);
 report.checks=await page.evaluate(async(names)=>{
  const {createEngine}=await import('/dist/client.js'),{createWorkerEngine}=await import('/dist/worker-client.js');
  const check=(v,m)=>{if(!v)throw Error(m);};
  const bytes=async name=>new Uint8Array(await(await fetch('/fixtures/'+name)).arrayBuffer());
  const hash=async value=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',value)),v=>v.toString(16).padStart(2,'0')).join('');
  const compare=async(frame,name,engine,dpi)=>{
   const bitmap=await createImageBitmap(await(await fetch(`/references/${name.slice(0,-4)}-${engine}-${dpi}.png`)).blob());
   check(frame.width===bitmap.width&&frame.height===bitmap.height,'reference geometry');
   const canvas=new OffscreenCanvas(bitmap.width,bitmap.height),ctx=canvas.getContext('2d');ctx.drawImage(bitmap,0,0);bitmap.close();
   const pixels=ctx.getImageData(0,0,frame.width,frame.height).data;
   let sum=0,bad=0,max=0;
   for(let i=0;i<pixels.length;i+=4){let error=0;for(let c=0;c<3;c++){const d=Math.abs(pixels[i+c]-frame.pixels[i+c]);sum+=d;error=Math.max(error,d);max=Math.max(max,d);}if(error>12)bad++;}
   const mae=sum/(frame.width*frame.height*3),badFraction=bad/(frame.width*frame.height);
   check(mae<2&&badFraction<.025,`reference mismatch ${name} ${engine} ${dpi}: ${mae}/${badFraction}`);
   return {engine,dpi,mae,badFraction,max};
  };
  const checks=[],direct=new Map();
  for(const worker of [false,true]){
   const engine=await(worker?createWorkerEngine():createEngine());let stage='open';
   try{
    const comparisons=[];
    for(const name of names.filter(n=>!n.includes('corrupt'))){
     stage=name;const doc=await engine.open(await bytes(name),{mode:'strict'}),before=await doc.state(),cap=await doc.capabilities();
     check(cap.maxImagePixels===4000000&&cap.maxDecodedBytes===16777216,'default limits');
     check(cap.imageDecode.maskedReduction&&cap.imageDecode.maskedReductionKinds.includes('soft')&&cap.imageDecode.softMaskMatte,'masked capabilities');
     const originals=new Map();
     for(const dpi of [72,36,18]){
      const frame=await doc.render(0,dpi,{imageDecode:'target'}),h=await hash(frame.pixels);originals.set(dpi,h);
      const key=name+':'+dpi;if(!worker)direct.set(key,h);else check(direct.get(key)===h,'transport pixels');
      for(const ref of ['mupdf','pdfium'])comparisons.push({name,...await compare(frame,name,ref,dpi)});
      check((await doc.cacheStatistics()).pageEntries<=2,'cache entries');await doc.clearRenderCaches();
      check(await hash((await doc.render(0,dpi,{imageDecode:'target'})).pixels)===h,'cache clear changed preview');
     }
     check(await hash((await doc.render(0,72)).pixels)===originals.get(72),'budgeted fallback');
     check((await doc.state()).commitRevision===before.commitRevision,'render mutated document');
     const stats=await doc.cacheStatistics();check(stats.rawStreamResidentBytes===0&&stats.rawStreamTransientBytes===0,'source residency');
     const saved=await doc.save();await doc.close();const reopened=await engine.open(saved.bytes,{mode:'strict'});
     check(await hash((await reopened.render(0,72,{imageDecode:'target'})).pixels)===originals.get(72),'save/reopen pixels');await reopened.close();
    }
    stage='typed target rejection';const doc=await engine.open(await bytes('soft-matte.pdf'),{mode:'strict'});
    let targetError;try{await doc.renderRegion(0,{x:0,y:0,width:32,height:32},288,{imageDecode:'target'});}catch(e){targetError=e;}
    const details=targetError?.resource?.image;
    check(targetError?.code==='resource_limit'&&targetError.resource.reason==='image_pixels'&&details?.maskKind==='soft'&&details.maskWidth===2665&&details.maskHeight===1916&&details.maskEncoding==='flate'&&details.maskBitsPerComponent===8&&details.mattePresent&&details.targetDecodeSupported,'target error details '+JSON.stringify(targetError?.resource));
    await doc.render(1,72);await doc.close();
    stage='mask checksum';const damaged=await engine.open(await bytes('soft-corrupt.pdf'),{mode:'strict'}),stamp=await damaged.state();
    let failure;try{await damaged.render(0,72,{imageDecode:'target'});}catch(e){failure=e;}
    check(failure?.code==='format'&&failure.resource.image.maskKind==='soft'&&failure.resource.image.targetDecodeSupported,'checksum diagnostic');
    check((await damaged.cacheStatistics()).pageEntries===0,'partial publication');check((await damaged.state()).commitRevision===stamp.commitRevision,'failed decode mutation');await damaged.render(1,72);await damaged.close();
    stage='source budget';const small=await engine.open(await bytes('soft-white.pdf'),{mode:'strict',resourceProfile:'conservative'});
    let limit;try{await small.render(0,18);}catch(e){limit=e;}
    check(limit?.code==='resource_limit'&&limit.resource.reason==='image_source_bytes'&&limit.resource.image.maskKind==='soft','source budget');await small.render(1,72);await small.close();
    let cancellation=false;
    if(worker){stage='cancel';const disposable=await engine.open(await bytes('soft-white.pdf'),{mode:'strict'}),controller=new AbortController();
     const pending=disposable.render(0,72,{imageDecode:'target',signal:controller.signal}).catch(e=>e);controller.abort();check((await pending).code==='cancelled','cancel');
     let invalid=false;try{await disposable.state();}catch(e){invalid=e.code==='invalid_handle';}check(invalid,'stale handle after cancellation');
     const recovered=await engine.open(await bytes('soft-white.pdf'),{mode:'strict'});await recovered.render(0,72);await recovered.close();cancellation=true;
    }
    const released=await engine.statistics();check(released.handles===0&&released.retainedBytes===0,'retained resources');
    checks.push({transport:worker?'worker':'sync',comparisons,sourceError:limit.resource,targetError:targetError.resource,corruptMask:true,saveReopen:true,cancellation,released});
   }catch(error){throw Error(JSON.stringify({transport:worker?'worker':'sync',stage,message:error.message,code:error.code,resource:error.resource,stack:error.stack}));}finally{engine.dispose();}
  }return checks;
 },names);
 assert.equal(report.checks.length,2);assert.equal(report.errors.length,0);report.status='passed';
}catch(error){report.errors.push(error.stack);process.exitCode=1;}
finally{await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'browser-report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,transports:report.checks.length,errors:report.errors,report:resolve(out,'browser-report.json')},null,2));}
