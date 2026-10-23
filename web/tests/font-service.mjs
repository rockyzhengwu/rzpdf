import {chromium} from 'playwright-core';
import {createServer} from 'node:http';import {readFile,writeFile,mkdir} from 'node:fs/promises';import {resolve,extname} from 'node:path';import {fileURLToPath} from 'node:url';import {createHash} from 'node:crypto';import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),out=resolve(repo,'output/webassembly/font-service-browser');await mkdir(out,{recursive:true});
const server=createServer(async(req,res)=>{const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname),file=path.startsWith('/fixtures/')?resolve(repo,'output/webassembly',path.slice(1)):resolve(repo,'web',path==='/'?'index.html':path.slice(1));if(!file.startsWith(repo)){res.writeHead(403).end();return;}try{res.setHeader('Content-Type',({'.css':'text/css','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.pdf':'application/pdf'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));}catch{res.writeHead(404).end();}});await new Promise(done=>server.listen(0,'127.0.0.1',done));let browser,page;const report={status:'failed',checks:[],errors:[]};
try{
 const hash=b=>createHash('sha256').update(b).digest('hex');report.source=JSON.parse(await readFile(resolve(repo,'output/webassembly/wa03-build.json'))).source;report.artifacts={};for(const name of ['rzpdf_web.mjs','rzpdf_web.wasm','client.js','fonts.js','worker-client.js','protocol.js','interaction.js','pdf-worker.js','content-editor.js','editor.js','demo.js'])report.artifacts[name]=hash(await readFile(resolve(repo,'web/dist',name)));
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});page=await browser.newPage();page.on('pageerror',e=>report.errors.push(e.stack));await page.goto(`http://127.0.0.1:${server.address().port}`);

 const result=await page.evaluate(async()=>{
  const {createEngine}=await import('/dist/client.js'),{createWorkerEngine}=await import('/dist/worker-client.js');
  const bytes=async name=>new Uint8Array(await(await fetch('/fixtures/'+name)).arrayBuffer());
  const input=await bytes('font-service.pdf'),cjk=await bytes('font-write-source.ttf'),symbol=await bytes('font-service-symbol.ttf');
  const check=(v,m)=>{if(!v)throw Error(m);};const reject=async(fn,codes)=>{try{await fn();}catch(e){check(codes.includes(e.code),`Expected ${codes}, got ${e.code}: ${e.message}`);return;}throw Error('Expected rejection');};
  const hash=async b=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',b)),b=>b.toString(16).padStart(2,'0')).join('');
  const metadata={family:'SameName',aliases:['MissingFace'],weight:400,italic:false,serif:false,languages:[]};const checks=[];
  for(const worker of [false,true]){
   const engine=await(worker?createWorkerEngine():createEngine()),doc=await engine.open(input);const before=await doc.state();
   const requirements=await doc.fontRequirements(0);check(!requirements.complete&&requirements.fonts.length===1&&requirements.fonts[0].missingCodepoints.join(',')==='215,26032','wrong missing-font request');
   check(requirements.stamp.revision===before.stamp.revision,'preflight changed PDF revision');
   for(const page of [2,3,4,9]){const nested=await doc.fontRequirements(page);check(!nested.complete&&nested.fonts.some(f=>f.missingCount===2),'nested Form/pattern/mask/Type3 font not discovered, page '+page);}
   const type3=await doc.fontRequirements(5);check(type3.complete&&type3.fonts.length===0,'Type3 metadata or unused resource requested a font');await doc.render(5);await reject(()=>doc.fontRequirements(10),['unsupported']);await reject(()=>doc.render(10),['unsupported']);
   const hidden=await doc.fontRequirements(8);check(hidden.complete&&hidden.fonts.length===0,'hidden layer triggered font request');await doc.render(8);
   const withoutAppearance=await doc.fontRequirements(9,{annotations:false});check(withoutAppearance.complete&&withoutAppearance.fonts.length===0,'disabled appearance triggered font request');const blank=await doc.render(9,72,{annotations:false});check(blank.pixels.every(v=>v===255),'annotations=false still painted appearance');
   await reject(()=>doc.render(0),['font']);
   const a=await doc.registerFont(cjk,metadata,requirements.stamp);check(a.sha256===await hash(cjk)&&a.index===0,'CJK identity mismatch');
   await reject(()=>doc.render(0),['font']);const partiallyReady=await doc.fontRequirements(0);check(partiallyReady.fonts[0].missingCodepoints.join(',')==='215','preflight selection differs from rendering');
   const b=await doc.registerFont(symbol,metadata,partiallyReady.stamp);check(b.sha256===await hash(symbol)&&b.sha256!==a.sha256,'same name merged identities');
   for(const page of [0,1,2,3,4,5,9]){const ready=await doc.fontRequirements(page);check(ready.complete,'font requirements not fulfilled on page '+page);const image=await doc.render(page);check(image.pixels.some((v,i)=>i%4!==3&&v<180),'blank nested page '+page);}
   const unmapped=await doc.fontRequirements(6),shaping=await doc.fontRequirements(7);check(!unmapped.complete&&unmapped.fonts[0].missingUnicode===2&&unmapped.fonts[0].missingCount===0,'unmapped code guessed as Unicode');check(!shaping.complete&&shaping.fonts[0].needsShaping===2,'multi-character mapping did not report shaping');await reject(()=>doc.render(6),['font']);await reject(()=>doc.render(7),['unsupported']);
   const first=await doc.render(0);check(first.pixels.some((v,i)=>i%4!==3&&v<180),'blank mixed-face render');
   const other=await doc.render(1);check(other.pixels.some((v,i)=>v!==first.pixels[i]),'different scoped ToUnicode mappings merged by BaseFont');
   check((await doc.extractText(0)).characters.map(c=>c.unicode??'').join('')==='新×新'&&(await doc.extractText(1)).characters.map(c=>c.unicode??'').join('')==='×新×','scoped character mappings wrong');
   const size=(await engine.statistics()).retainedBytes;const same=await engine.registerFont(cjk,metadata);check(same.sha256===a.sha256&&(await engine.statistics()).retainedBytes===size,'not idempotent');
   await reject(()=>engine.registerFont(cjk,{...metadata,family:'Conflict'}),['font']);await reject(()=>engine.registerFont(new Uint8Array([1,2,3]),metadata),['font']);await reject(()=>engine.registerFont(symbol,{...metadata,index:999}),['font']);await reject(()=>engine.registerFont(symbol,{...metadata,index:0xffffffff}),['font']);
   const next=await doc.render(0);check(next.pixels.every((v,i)=>v===first.pixels[i]),'failed registration changed registry');
   const after=await doc.state();check(after.stamp.revision===before.stamp.revision,'display font registration changed PDF');
   const saved=await doc.save();check((await hash(saved.bytes))===await hash((await doc.save()).bytes),'unstable save');
   const beforeClear=(await doc.fontRequirements(0)).stamp;await engine.clearFonts();await reject(()=>doc.registerFont(cjk,metadata,beforeClear),['invalid_handle']);await reject(()=>doc.render(0),['font']);
   await engine.setSubstituteFont(symbol);await engine.registerFont(cjk,metadata);const legacy=await doc.render(0);check(legacy.pixels.every((v,i)=>v===first.pixels[i]),'legacy last fallback differs');
   check(cjk.byteLength>0&&symbol.byteLength>0,'caller input detached');
   const foreign=await engine.open(input);const foreignStamp=(await foreign.state()).stamp;await reject(()=>doc.registerFont(cjk,metadata,foreignStamp),['invalid_handle']);await foreign.close();
   const stale=(await doc.fontRequirements(0)).stamp;await doc.apply({operation:'rotatePage',stamp:stale,page:0,degrees:90});await reject(()=>doc.registerFont(cjk,metadata,stale),['invalid_handle']);
   await doc.close();await reject(()=>doc.registerFont(cjk,metadata,stale),['invalid_handle']);await engine.clearFonts();await engine.setSubstituteFont();
   const restricted=await engine.open(await bytes('font-service-no-copy.pdf'),{password:'user'});const redacted=await restricted.fontRequirements(0);check(redacted.codepointsRedacted&&!redacted.complete&&redacted.fonts[0].missingCount===2&&redacted.fonts.every(f=>f.missingCodepoints.length===0),'copy-denied request leaked Unicode');
   await reject(()=>restricted.extractText(0),['permission']);await restricted.registerFont(cjk,metadata,redacted.stamp);await restricted.registerFont(symbol,metadata,redacted.stamp);check((await restricted.fontRequirements(0)).complete,'copy-denied display fonts unresolved');await restricted.render(0);await restricted.close();await engine.clearFonts();await engine.setSubstituteFont();const final=await engine.statistics();check(final.handles===0&&final.retainedBytes===0,'font resources retained');engine.dispose();
   checks.push(`${worker?'Worker':'sync'}: actual coverage, mixed-face fallback, SHA-256 identity, same-name isolation, duplicate/conflict/index errors, legacy fallback, PDF revision and resource release; preflight root/Form/pattern/mask/Type3, partial/resolved selections, scoped stale/foreign/closed rejection and no-copy redaction`);
  }
  return {checks};
 });report.checks=result.checks;report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;}
finally{await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,checks:report.checks,errors:report.errors}));}
