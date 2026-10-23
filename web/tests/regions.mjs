// WA-T12/13: bounded true region rasters, edge halos and real Worker ownership.
import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,basename,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),fixtures=resolve(repo,'output/webassembly/fixtures'),out=resolve(repo,'output/webassembly/region-browser');await mkdir(out,{recursive:true});
const hash=b=>createHash('sha256').update(b).digest('hex'),manifest=JSON.parse(await readFile(resolve(fixtures,'region-matrix.json'))),routes=new Map();
const report={status:'failed',checks:[],cases:[],errors:[],threshold:{integerScale:0,fractionalScale:1,flatCurveRegions:0},manifest_sha256:hash(await readFile(resolve(fixtures,'region-matrix.json')))};let browser,server;
try{
 report.source=JSON.parse(await readFile(resolve(repo,'output/webassembly/wa03-build.json'))).source;report.artifacts={};
 for(const name of ['rzpdf_web.mjs','rzpdf_web.wasm','client.js','worker-client.js','protocol.js','pdf-worker.js','raster.js'])report.artifacts[name]=hash(await readFile(resolve(repo,'web/dist',name)));
 const native=resolve(repo,'target/debug/examples/rgba_reference');report.native_reference_sha256=hash(await readFile(native));
 for(const record of manifest.records){assert.equal(basename(record.file),record.file);const input=resolve(fixtures,record.file);assert.equal(hash(await readFile(input)),record.sha256);routes.set('/fixtures/'+record.file,input);
  if(!record.large&&record.nativeFull!==false){const reference=resolve(out,'native',record.id);await mkdir(reference,{recursive:true});const env={...process.env};delete env.RZPDF_SUBSTITUTE_FONT;delete env.RZPDF_SUBSTITUTE_FONT_INDEX;execFileSync(native,[input,reference],{env});routes.set('/native/'+record.id,resolve(reference,'0.rgba'));}
 }
 server=createServer(async(req,res)=>{try{const path=new URL(req.url,'http://localhost').pathname;if(path==='/'){res.setHeader('Content-Type','text/html');res.end('<!doctype html><title>Region rendering probes</title>');return;}const file=routes.get(path)??(path.startsWith('/dist/')&&basename(path)===path.slice(6)?resolve(repo,'web/dist',basename(path)):undefined);if(!file){res.writeHead(404).end();return;}res.setHeader('Content-Type',({'.css':'text/css','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));}catch{res.writeHead(404).end();}});await new Promise(done=>server.listen(0,'127.0.0.1',done));
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});report.browser=browser.version();const page=await browser.newPage();await page.exposeFunction('__captureRegionNative',async item=>{
  const full=new Uint8Array(item.pixels),native=await readFile(resolve(out,'native',item.id,'0.rgba'));let differing=0,max=0,left=item.width,top=item.height,right=0,bottom=0;
  for(let i=0;i<full.length;i++)if(full[i]!==native[i]){differing++;max=Math.max(max,Math.abs(full[i]-native[i]));const x=Math.floor(i/4)%item.width,y=Math.floor(i/4/item.width);left=Math.min(left,x);top=Math.min(top,y);right=Math.max(right,x);bottom=Math.max(bottom,y);}
  const comparison={path:item.path,id:item.id,differingChannels:differing,maxChannelDifference:max,bounds:[left,top,right,bottom]};report.nativeComparisons??=[];report.nativeComparisons.push(comparison);
  await writeFile(resolve(out,item.path+'-'+item.id+'-native-comparison.rgba'),full);await writeFile(resolve(out,'native-comparisons.json'),JSON.stringify(report.nativeComparisons,null,2));return comparison;
 });await page.exposeFunction('__captureRegionStitch',async item=>{
  for(const key of ['full','tiles'])await writeFile(resolve(out,item.path+'-'+item.id+'-'+item.dpi+'-failed-'+key+'.rgba'),new Uint8Array(item[key]));
  await writeFile(resolve(out,'stitch-failure.json'),JSON.stringify({path:item.path,id:item.id,dpi:item.dpi,width:item.width,height:item.height,max:item.max,differing:item.differing},null,2));
 });page.on('console',m=>{if(m.type()==='log')console.log(m.text());});page.on('pageerror',e=>report.errors.push(e.stack));await page.goto(`http://127.0.0.1:${server.address().port}`);
 const result=await page.evaluate(async manifest=>{
  const {createEngine}=await import('/dist/client.js'),{createWorkerEngine}=await import('/dist/worker-client.js'),{rasterSize}=await import('/dist/raster.js');
  const read=async path=>{const r=await fetch(path);if(!r.ok)throw Error('fixture read '+path);return new Uint8Array(await r.arrayBuffer());};
  const check=(v,m)=>{if(!v)throw Error(m);},equal=(a,b)=>a.length===b.length&&a.every((v,i)=>v===b[i]);
  const reject=async(fn,code)=>{try{await fn();}catch(e){check(e.code===code,`Expected ${code}, got ${e.code}: ${e.message}`);return;}throw Error('Expected '+code);};
  const checks=[],cases=[],images=[],nativeErrors=[],stitchErrors=[];
  for(const worker of [false,true]){
   const engine=await(worker?createWorkerEngine():createEngine()),path=worker?'worker':'sync';
   for(const fixture of manifest.records){const doc=await engine.open(await read('/fixtures/'+fixture.file));
    check((await doc.capabilities()).renderRegion===true,'region capability missing');const state=await doc.state();
    if(fixture.large){
     const before=await engine.statistics();await reject(()=>doc.render(0),'resource_limit');
     for(const area of [{x:0,y:0,width:64,height:32,color:[255,0,0,255]},{x:5990,y:0,width:10,height:10,color:[0,255,0,255]},{x:5990,y:3990,width:10,height:10,color:[255,0,0,255]},{x:3000,y:2000,width:64,height:32,color:[255,0,0,255]}]){
      const {color,...region}=area,image=await doc.renderRegion(0,region);check(image.pixels.length===region.width*region.height*4&&image.pixels.every((v,i)=>v===color[i%4]),'large logical page region '+JSON.stringify(region));
     }
     const after=await engine.statistics();check(before.handles===after.handles&&before.retainedBytes===after.retainedBytes,'large page buffers retained');cases.push({path,id:fixture.id,logicalPixels:24000000,fullRejected:true,regions:4});
    }else{
     for(const dpi of [72,96,144]){
      console.log(`region ${path}: ${fixture.id} ${dpi}dpi`);const full=await doc.render(0,dpi,fixture.options??{}),original=full.pixels.slice(),info=await doc.pageInfo(0),size=rasterSize(info.width,info.height,dpi);check(size.width===full.width&&size.height===full.height,'allocation-free raster size differs from native');check(full.pixels.some((v,i)=>i%4!==3&&v<200),'blank full oracle '+fixture.id);
      for(const sample of fixture.pixelOracle??[]){const x=Math.floor(sample.x*dpi/72),y=Math.floor(sample.y*dpi/72),offset=(y*full.width+x)*4;check(sample.rgba.every((v,c)=>full.pixels[offset+c]===v),`${fixture.id} ${dpi}dpi: independent interior colour oracle`);}
      if(dpi===72&&fixture.nativeFull!==false){const native=await read('/native/'+fixture.id);if(!equal(full.pixels,native)){
       const comparison=await window.__captureRegionNative({path,id:fixture.id,width:full.width,height:full.height,pixels:Array.from(full.pixels)}),profile=fixture.nativeProfile,b=comparison.bounds;
       if(!(profile&&comparison.differingChannels<=profile.differingChannels&&comparison.maxChannelDifference<=profile.maxChannelDifference&&b[0]>=profile.bounds[0]&&b[1]>=profile.bounds[1]&&b[2]<=profile.bounds[2]&&b[3]<=profile.bounds[3]))nativeErrors.push(`${path}: native reference differs outside reviewed glyph profile ${fixture.id}: ${comparison.differingChannels} channels, max ${comparison.maxChannelDifference}`);
      }}
      const assembled=new Uint8ClampedArray(full.pixels.length);let tiles=0;
      for(let y=0;y<full.height;y+=fixture.tileSize??13)for(let x=0;x<full.width;x+=fixture.tileSize??17){
       const region={x,y,width:Math.min(fixture.tileSize??17,full.width-x),height:Math.min(fixture.tileSize??13,full.height-y)},image=await doc.renderRegion(0,region,dpi,fixture.options??{});
       check(image.x===x&&image.y===y&&image.width===region.width&&image.height===region.height&&image.pixels.length===region.width*region.height*4,'region metadata/copy bounds');
       for(let row=0;row<image.height;row++)assembled.set(image.pixels.subarray(row*image.width*4,(row+1)*image.width*4),((y+row)*full.width+x)*4);tiles++;
      }
      let max=0,differing=0;for(let i=0;i<assembled.length;i++){max=Math.max(max,Math.abs(assembled[i]-full.pixels[i]));if(assembled[i]!==full.pixels[i])differing++;}
      const wasmBytes=worker?null:engine.module.HEAPU8.byteLength;
      if(!worker&&fixture.stressMaxWasmBytes)check(wasmBytes<=fixture.stressMaxWasmBytes,`${fixture.id}: repeated region renders exceeded fixed heap regression budget`);
      const tolerance=dpi===96?1:0;
      if(fixture.curveRaster)for(let i=0;i<assembled.length;i+=4){
       check(assembled[i+3]===full.pixels[i+3],'curve alpha changed');if(assembled.subarray(i,i+3).every((v,c)=>v===full.pixels[i+c]))continue;
       const x=i/4%full.width,y=Math.floor(i/4/full.width);let edge=false;
       for(let yy=Math.max(0,y-1);yy<=Math.min(full.height-1,y+1);yy++)for(let xx=Math.max(0,x-1);xx<=Math.min(full.width-1,x+1);xx++){const p=(yy*full.width+xx)*4;for(let c=0;c<3;c++)edge ||= assembled[p+c]!==assembled[i+c]||full.pixels[p+c]!==full.pixels[i+c];}
       check(edge,'flat curve-render region changed');
      }
      if(max>tolerance){await window.__captureRegionStitch({path,id:fixture.id,dpi,width:full.width,height:full.height,max,differing,full:Array.from(full.pixels),tiles:Array.from(assembled)});stitchErrors.push(`${path}: ${fixture.id} ${dpi}dpi: ${differing} channels differ, max ${max} > ${tolerance}`);}check(equal(original,full.pixels),'previous RGBA copy changed after regions');
      cases.push({path,id:fixture.id,dpi,nativeCompared:dpi===72&&fixture.nativeFull!==false,nativeProfile:fixture.nativeProfile??null,width:full.width,height:full.height,tiles,differingChannels:differing,maxChannelDifference:max,tolerance,wasmBytes});
      if(dpi===96)images.push({name:path+'-'+fixture.id,width:full.width,height:full.height,full:Array.from(full.pixels),tiles:Array.from(assembled)});
     }
    }
    for(const area of [{x:-1,y:0,width:1,height:1},{x:0,y:0,width:0,height:1},{x:0,y:0,width:1025,height:1},{x:32768,y:0,width:1,height:1},{x:0,y:0,width:1.5,height:1}])await reject(()=>doc.renderRegion(0,area),'invalid_argument');
    const area={x:0,y:0,width:1,height:1},owned=doc.renderRegion(0,area);area.x=-99;check((await owned).x===0,'caller mutated queued region');
    if(worker){const first=doc.renderRegion(0,{x:0,y:0,width:1,height:1}),abort=new AbortController(),queued=doc.renderRegion(0,{x:0,y:0,width:1,height:1},72,{signal:abort.signal});abort.abort();await reject(()=>queued,'cancelled');await first;await doc.pageInfo(0);}
    check((await doc.state()).stamp.revision===state.stamp.revision,'region rendering mutated PDF');await doc.close();await reject(()=>doc.renderRegion(0,{x:0,y:0,width:1,height:1}),'invalid_handle');
   }
   const stats=await engine.statistics();check(stats.handles===0&&stats.retainedBytes===0,'region resources retained');engine.dispose();
   checks.push(`${path}: full/native and stitched region oracles, fractional edges, patterns/backdrops/masks/shading/knockout/geometry/images, large-page bounded regions, invalid parameters, immutable queued area, cancellation and release`);
  }
  return {checks,cases,images,nativeErrors,stitchErrors};
 },manifest);
 report.checks=result.checks;report.cases=result.cases;
 report.errors.push(...result.nativeErrors,...result.stitchErrors);
 for(const item of result.images){for(const kind of ['full','tiles'])await writeFile(resolve(out,item.name+'-'+kind+'.rgba'),new Uint8Array(item[kind]));}
 assert.equal(report.errors.length,0);report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;}
finally{await browser?.close();if(server)await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,checks:report.checks,cases:report.cases.length,errors:report.errors}));}
