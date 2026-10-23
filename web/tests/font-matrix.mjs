// WA-T06/07/08: real in-memory font formats, PDF mapping/metrics and indexed faces.
import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname,basename} from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),fixtures=resolve(repo,'output/webassembly/fixtures'),out=resolve(repo,'output/webassembly/font-matrix-browser');
await mkdir(out,{recursive:true});
const hash=b=>createHash('sha256').update(b).digest('hex');
const manifest=JSON.parse(await readFile(resolve(fixtures,'font-matrix.json'))),routes=new Map();
const report={status:'failed',checks:[],cases:[],errors:[],cjkMatrix:manifest.cjkMatrix,manifest_sha256:hash(await readFile(resolve(fixtures,'font-matrix.json')))};
let browser,server;
try{
 report.source=JSON.parse(await readFile(resolve(repo,'output/webassembly/wa03-build.json'))).source;
 report.artifacts={};for(const name of ['rzpdf_web.mjs','rzpdf_web.wasm','client.js','fonts.js','font-resources.js','worker-client.js','protocol.js','pdf-worker.js'])report.artifacts[name]=hash(await readFile(resolve(repo,'web/dist',name)));
 const native=resolve(repo,'target/debug/examples/rgba_reference');report.native_reference_sha256=hash(await readFile(native));
 const expose=async asset=>{assert.equal(basename(asset.file),asset.file);const p=resolve(fixtures,asset.file);assert.equal(hash(await readFile(p)),asset.sha256);routes.set('/fixtures/'+asset.file,p);};
 const reference=async(id,input,font,index=0)=>{
  await expose(input);if(font)await expose(font);const dir=resolve(out,'native',id);await mkdir(dir,{recursive:true});
  const env={...process.env};delete env.RZPDF_SUBSTITUTE_FONT;delete env.RZPDF_SUBSTITUTE_FONT_INDEX;
  if(font){env.RZPDF_SUBSTITUTE_FONT=resolve(fixtures,font.file);env.RZPDF_SUBSTITUTE_FONT_INDEX=String(index);}
  execFileSync(native,[resolve(fixtures,input.file),dir],{env});
  const pages=JSON.parse(await readFile(resolve(dir,'pages.json')));
  for(let page=0;page<pages.length;page++)routes.set(`/native/${id}/${page}.rgba`,resolve(dir,`${page}.rgba`));
  return pages;
 };
 for(const record of manifest.records)record.pages=await reference(record.id,record.input,record.font,record.index);
 manifest.embedded.pages=await reference('embedded',manifest.embedded.input);
 manifest.sparseCid.pages=await reference('sparse-cid',manifest.sparseCid.input);
 manifest.base14.pages=await reference('base14',manifest.base14.input);
 await expose(manifest.negative.cffClip);
 server=createServer(async(req,res)=>{try{
  const path=new URL(req.url,'http://localhost').pathname;
  const file=routes.get(path)??(path==='/'?resolve(repo,'web/index.html'):path.startsWith('/dist/')&&basename(path)===path.slice(6)?resolve(repo,'web/dist',basename(path)):undefined);
  if(!file){res.writeHead(404).end();return;}
  res.setHeader('Content-Type',({'.css':'text/css','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));
 }catch{res.writeHead(404).end();}});
 await new Promise(done=>server.listen(0,'127.0.0.1',done));
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});report.browser=browser.version();
 const page=await browser.newPage();page.on('pageerror',e=>report.errors.push(e.stack));await page.goto(`http://127.0.0.1:${server.address().port}`);
 const result=await page.evaluate(async manifest=>{
  const {createEngine}=await import('/dist/client.js'),{createWorkerEngine}=await import('/dist/worker-client.js'),{FontResourceService}=await import('/dist/font-resources.js');
  const read=async path=>{const r=await fetch(path);if(!r.ok)throw Error('Cannot read '+path);return new Uint8Array(await r.arrayBuffer());};
  const check=(v,m)=>{if(!v)throw Error(m);},checks=[],cases=[],images=[];
  const reject=async(fn,code)=>{try{await fn();}catch(e){check(e.code===code,`Expected ${code}, got ${e.code}: ${e.message}`);return;}throw Error('Expected '+code);};
  const equal=(a,b)=>a.length===b.length&&a.every((v,i)=>v===b[i]);
  const diff=(a,b)=>{check(a.length===b.length,'pixel length');let differing=0,max=0;for(let i=0;i<a.length;i++){if(a[i]!==b[i])differing++;max=Math.max(max,Math.abs(a[i]-b[i]));}return {differingChannels:differing,maxChannelDifference:max,channels:a.length};};
  for(const worker of [false,true]){
   const engine=await(worker?createWorkerEngine():createEngine()),path=worker?'worker':'sync';
   const embedded=await engine.open(await read('/fixtures/'+manifest.embedded.input.file));
   for(let i=0;i<manifest.embedded.cases.length;i++){
    const c=manifest.embedded.cases[i],text=await embedded.extractText(i);check(text.characters.length===c.codes.length,'embedded count '+c.kind);
    for(let j=0;j<c.codes.length;j++){const t=text.characters[j];check(t.unicode===c.unicode[j]&&t.charcode===parseInt(c.codes[j],16)&&t.codeLength===c.codes[j].length/2,'embedded code/Unicode '+c.kind);check(t.writingMode===(c.kind==='cid-v'?'Vertical':'Horizontal'),'writing mode '+c.kind);check(Math.abs(Math.hypot(...t.advance)-Math.abs(c.widths[j]*.02))<.001,'PDF advance '+c.kind);}
    const preflight=await embedded.fontRequirements(i);check(preflight.complete&&preflight.fonts.length===0,'embedded/base14 erroneously requested replacement '+c.kind);
    const pixels=await embedded.render(i),native=await read(`/native/embedded/${i}.rgba`);check(equal(pixels.pixels,native),'embedded native pixels '+c.kind);
    cases.push({path,kind:c.kind,page:i,native:diff(pixels.pixels,native),unicode:c.unicode,codes:c.codes});
   }
   const original=await embedded.render(0);const saved=await embedded.save();const reopened=await engine.open(saved.bytes);check(equal((await reopened.render(0)).pixels,original.pixels),'embedded save/reopen');await reopened.close();await embedded.close();
   const sparse=await engine.open(await read('/fixtures/'+manifest.sparseCid.input.file));check((await sparse.extractText(0)).characters[0].unicode==='B','sparse CID Unicode');const sparsePixels=await sparse.render(0);check(equal(sparsePixels.pixels,await read('/native/sparse-cid/0.rgba')),'sparse CID 42/GID 2 native pixels');await sparse.close();cases.push({path,kind:'sparse-cid-cff',cid:42,gid:2});
   const base14=await engine.open(await read('/fixtures/'+manifest.base14.input.file));
   for(let i=0;i<manifest.base14.names.length;i++){
    const text=await base14.extractText(i),pixels=await base14.render(i),native=await read(`/native/base14/${i}.rgba`);
    check(text.characters.map(c=>c.unicode??'').join('')==='AB','Base14 Unicode independence '+i);
    check((await base14.fontRequirements(i)).complete,'Base14 requested external face '+i);
    check(equal(pixels.pixels,native)&&pixels.pixels.some((v,i)=>i%4!==3&&v<180),'Base14 native/nonblank '+i);
    cases.push({path,kind:'base14-'+manifest.base14.names[i],native:diff(pixels.pixels,native)});
   }await base14.close();
   let collectionZero;
   for(const c of manifest.records){
    const bytes=await read('/fixtures/'+c.font.file),input=await read('/fixtures/'+c.input.file),doc=await engine.open(input);
    const before=await doc.state(),preflight=await doc.fontRequirements(0);check(!preflight.complete&&preflight.fonts.length===1,'missing face input '+c.id);await reject(()=>doc.render(0),'font');
    const font={family:'Matrix',aliases:['MissingMatrix'],weight:400,italic:false,serif:false,languages:c.language?[c.language]:[],index:c.index};
    const resource={id:c.id,version:'matrix-1',format:c.format,sha256:c.font.sha256,sizeBytes:c.font.sizeBytes,font,license:{spdx:c.license==='OFL-1.1'?'OFL-1.1':c.id==='type1'?'LicenseRef-URW-Embedding':'AGPL-3.0-only'}};
    let loads=0;const service=new FontResourceService({version:1,resources:[resource]},{persistent:false,load:async()=>{loads++;return bytes;}});
    const ready=await service.ensure(doc,0);check(ready.complete&&loads===1,'resource format registration '+c.id);if(c.language)check(ready.fonts[0].language===c.language,'Catalog locale missing '+c.id);const selected=ready.fonts[0].selected;check(selected.length===1&&selected[0].sha256===c.font.sha256&&selected[0].index===c.index,'selected face identity '+c.id);
    const registered=await doc.render(0),text=await doc.extractText(0);check(registered.pixels.some((v,i)=>i%4!==3&&v<180),'blank registered face '+c.id);
    await reject(()=>engine.registerFont(new Uint8Array([1,2,3,4]),font),'font');check(equal((await doc.render(0)).pixels,registered.pixels),'bad program damaged registered face '+c.id);check(text.characters.map(c=>c.unicode??'').join('')===c.unicode,'Unicode '+c.id);const axis=c.writingMode==='Vertical'?1:0;check(text.characters.every(t=>t.writingMode===(c.writingMode??'Horizontal')),'registered writing mode '+c.id);check(Math.abs(text.characters[1].position[axis]-text.characters[0].position[axis]-c.advance)<.001,'source PDF advance replaced '+c.id);check((await doc.state()).stamp.revision===before.stamp.revision,'display font mutated PDF');
    const next=await engine.open((await doc.save()).bytes);check(equal((await next.render(0)).pixels,registered.pixels),'font source save/reopen '+c.id);await next.close();
    await engine.clearFonts();await engine.setSubstituteFont(bytes,c.index);const explicit=await doc.render(0);check(equal(registered.pixels,explicit.pixels),'registered vs explicit index '+c.id);await engine.setSubstituteFont();
    const native=await read(`/native/${c.id}/0.rgba`),difference=diff(registered.pixels,native);check(difference.differingChannels===0,'registered native pixels '+c.id);
    if(c.id==='collection-0')collectionZero=registered.pixels.slice();if(c.id==='collection-1')check(!equal(collectionZero,registered.pixels),'TTC indices selected same face');
    // One TTC buffer must also retain two independent indexed registry identities.
    if(c.id==='collection-1'){const zero=await engine.registerFont(bytes,{...font,index:0}),one=await engine.registerFont(bytes,font);check(zero.sha256===one.sha256&&zero.index===0&&one.index===1,'indexed identity conflated');const identity=await doc.fontRequirements(0);check(identity.fonts[0].selected[0].index===0,'stable registration tie');await reject(()=>engine.registerFont(bytes,{...font,index:2}),'font');await engine.clearFonts();}
    cases.push({path,kind:c.id,sha256:c.font.sha256,index:c.index,native:difference});
    images.push({name:path+'-'+c.id,width:registered.width,height:registered.height,pixels:Array.from(registered.pixels)});
    service.dispose();await doc.close();await engine.clearFonts();
   }
   const clipping=await engine.open(await read('/fixtures/'+manifest.negative.cffClip.file));await reject(()=>clipping.render(0),'unsupported');await clipping.close();cases.push({path,kind:'cff-outline-clip',expected:'unsupported'});
   const stats=await engine.statistics();check(stats.handles===0&&stats.retainedBytes===0,'matrix retained tracked resources');engine.dispose();
   checks.push(`${path}: eight embedded mapping/format pages, all Base14 faces, sparse CID CFF, ${manifest.records.length} registered format/index/language cases, resource format validation, PDF advances, source/save integrity and release`);
  }
  return {checks,cases,images};
 },manifest);
 report.checks=result.checks;report.cases=result.cases;
 for(const item of result.images)await writeFile(resolve(out,item.name+'.rgba'),new Uint8Array(item.pixels));
 assert.equal(report.errors.length,0);report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;}
finally{await browser?.close();if(server)await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,checks:report.checks,cases:report.cases.length,errors:report.errors}));}
