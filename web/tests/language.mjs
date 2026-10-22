import {chromium} from 'playwright-core';
import {createServer} from 'node:http';import {readFile,writeFile,mkdir} from 'node:fs/promises';import {resolve,extname} from 'node:path';import {fileURLToPath} from 'node:url';import {createHash} from 'node:crypto';import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),out=resolve(repo,'output/webassembly/language-browser');await mkdir(out,{recursive:true});
const server=createServer(async(req,res)=>{const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname),file=path.startsWith('/fixtures/')?resolve(repo,'output/webassembly',path.slice(1)):resolve(repo,'web',path==='/'?'index.html':path.slice(1));if(!file.startsWith(repo)){res.writeHead(403).end();return;}try{res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.pdf':'application/pdf'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));}catch{res.writeHead(404).end();}});await new Promise(done=>server.listen(0,'127.0.0.1',done));let browser,page;const report={status:'failed',checks:[],errors:[]};
try{
 const hash=b=>createHash('sha256').update(b).digest('hex');report.source=JSON.parse(await readFile(resolve(repo,'output/webassembly/wa03-build.json'))).source;report.artifacts={};for(const name of ['rzpdf_web.mjs','rzpdf_web.wasm','client.js','fonts.js','font-resources.js','worker-client.js','protocol.js','interaction.js','pdf-worker.js','content-editor.js','editor.js','demo.js'])report.artifacts[name]=hash(await readFile(resolve(repo,'web/dist',name)));
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});page=await browser.newPage();page.on('pageerror',e=>report.errors.push(e.stack));await page.goto(`http://127.0.0.1:${server.address().port}`);

 const result=await page.evaluate(async()=>{
  const {createEngine}=await import('/dist/client.js'),{createWorkerEngine}=await import('/dist/worker-client.js'),{FontResourceService}=await import('/dist/font-resources.js');
  const read=async name=>new Uint8Array(await(await fetch('/fixtures/'+name)).arrayBuffer()),input=await read('language.pdf'),ja=await read('language-ja.ttf'),zh=await read('language-zh.ttf');
  const hash=async b=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',b)),b=>b.toString(16).padStart(2,'0')).join('');const jaHash=await hash(ja),zhHash=await hash(zh);const check=(v,m)=>{if(!v)throw Error(m);},checks=[],artifacts=[];
  const entry=(id,data,sha256,language)=>({id,version:'locale-1',format:'truetype',sha256,sizeBytes:data.length,font:{family:id,weight:400,italic:false,serif:false,languages:[language]},license:{spdx:'AGPL-3.0-only'},ranges:[[215,215]]});
  const manifest={version:1,resources:[entry('Japanese',ja,jaHash,'ja-JP'),entry('Traditional',zh,zhHash,'zh-Hant')]};
  for(const worker of [false,true]){
   const engine=await(worker?createWorkerEngine():createEngine()),doc=await engine.open(input),events=[];
   const missing=await doc.fontRequirements(0);check(!missing.complete,'missing locale input unexpectedly resolved');check(missing.fonts.some(f=>f.language==='ja-JP')&&missing.fonts.some(f=>f.language==='zh-Hant-HK')&&missing.fonts.some(f=>f.language===null),'language scopes missing');
   const loaded=[];const service=new FontResourceService(manifest,{persistent:false,load:async e=>{loaded.push(e.id);return e.id==='Japanese'?ja:zh;},onEvent:e=>events.push(e)});
   const ready=await service.ensure(doc,0);check(ready.complete,'locale resources unresolved');check(loaded.join(',')==='Japanese,Traditional','stopped at first coverage instead of improving locale');
   for(const font of ready.fonts){const expected=font.language?.startsWith('zh')?zhHash:jaHash;check(font.selected.length===1&&font.selected[0].sha256===expected,'wrong selected locale: '+font.language);check(font.codepoints.join(',')==='215','missing proven codepoint diagnostics');}
   const mixed=await doc.render(0);await engine.clearFonts();await engine.setSubstituteFont(ja);const pureJa=await doc.render(0);await engine.setSubstituteFont(zh);const pureZh=await doc.render(0);
   const expected=pureJa.pixels.slice();for(const x of [60,140,180])for(let y=0;y<mixed.height;y++){const start=(y*mixed.width+x)*4,end=start+30*4;expected.set(pureZh.pixels.subarray(start,end),start);}
   check(mixed.pixels.every((v,i)=>v===expected[i]),'mixed locale pixels differ from independent explicit-face regions');check(pureJa.pixels.some((v,i)=>v!==pureZh.pixels[i]),'synthetic faces are not visually distinct');
   artifacts.push({name:worker?'worker':'sync',width:mixed.width,height:mixed.height,mixed:Array.from(mixed.pixels),japanese:Array.from(pureJa.pixels),traditional:Array.from(pureZh.pixels)});
   await doc.close();await engine.clearFonts();await engine.setSubstituteFont();service.dispose();const stats=await engine.statistics();check(stats.handles===0&&stats.retainedBytes===0,'locale resources retained');engine.dispose();
   checks.push(`${worker?'Worker':'sync'}: Catalog/BDC/Form language grouping, two same-coverage faces, alias-free locale improvement after coverage, selected identity/preferences and independent explicit-face pixel equality`);
  }return {checks,artifacts,fonts:{jaHash,zhHash}};
 });report.checks=result.checks;report.fonts=result.fonts;
 for(const item of result.artifacts){for(const kind of ['mixed','japanese','traditional'])await writeFile(resolve(out,item.name+'-'+kind+'.rgba'),new Uint8Array(item[kind]));}report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;}
finally{await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,checks:report.checks,errors:report.errors}));}
