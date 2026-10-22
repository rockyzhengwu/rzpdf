import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),out=resolve(repo,'output/webassembly/import-browser');await mkdir(out,{recursive:true});
const server=createServer(async(req,res)=>{const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname),file=path.startsWith('/fixtures/')?resolve(repo,'output/webassembly',path.slice(1)):resolve(repo,'web',path==='/'?'index.html':path.slice(1));if(!file.startsWith(repo)){res.writeHead(403).end();return;}try{res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.pdf':'application/pdf'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));}catch{res.writeHead(404).end();}});await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser,page;const report={status:'failed',checks:[],errors:[]};
try{
 const hash=data=>createHash('sha256').update(data).digest('hex');report.source=JSON.parse(await readFile(resolve(repo,'output/webassembly/wa03-build.json'))).source;report.artifacts={};
 for(const name of ['rzpdf_web.mjs','rzpdf_web.wasm','client.js','worker-client.js','protocol.js','interaction.js','pdf-worker.js','editor.js','demo.js'])report.artifacts[name]=hash(await readFile(resolve(repo,'web/dist',name)));
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});page=await browser.newPage();page.on('pageerror',e=>report.errors.push(e.stack));await page.goto(`http://127.0.0.1:${server.address().port}`);
 const result=await page.evaluate(async()=>{
  const {createEngine}=await import('/dist/client.js'),{createWorkerEngine}=await import('/dist/worker-client.js');const bytes=async(name)=>new Uint8Array(await(await fetch('/fixtures/'+name+'.pdf')).arrayBuffer());
  const check=(v,message)=>{if(!v)throw Error(message);},reject=async(action,codes)=>{try{await action();}catch(e){check(codes.includes(e.code),`Expected ${codes}, got ${e.code}: ${e.message}`);return;}throw Error('Expected rejection '+codes);};
  const artifacts=[],checks=[];
  for(const worker of [false,true]){
   const engine=await(worker?createWorkerEngine():createEngine()),target=await engine.open(await bytes('content-edit')),source=await engine.open(await bytes('interaction'));
   const sourceState=await source.state(),targetState=await target.state(),sourceImage=await source.render(0);
   const first={operation:'importPages',stamp:targetState.stamp,source:sourceState.stamp,pages:[0],insertion:1,namePrefix:'external'};
   await reject(()=>target.apply({...first,source:{...sourceState.stamp,engine:'foreign-module'}}),['invalid_handle']);
   await reject(()=>target.apply({...first,source:targetState.stamp}),['invalid_argument']);
   await reject(()=>target.apply({...first,pages:[]}),['invalid_argument']);
   await target.apply(first);await reject(()=>target.apply(first),['invalid_handle']);check(target.pageCount===3,'Imported page count missing');check(JSON.stringify((await source.state()).stamp)===JSON.stringify(sourceState.stamp),'Source revision changed');
   let state=await target.state();const before=await target.save();await reject(()=>target.apply({...first,stamp:state.stamp,insertion:3}),['format','unsupported']);const after=await target.save();check(before.bytes.length===after.bytes.length&&before.bytes.every((v,i)=>v===after.bytes[i]),'Conflict partially imported data');check((await target.state()).commitRevision===state.commitRevision,'Conflict committed');
   let fields=await source.formFields();await source.apply({operation:'setFormValue',stamp:fields.stamp,object:fields.records.find(f=>f.name==='name').object,value:{kind:'text',text:'Source changed'}});
   state=await target.state();await reject(()=>target.apply({...first,stamp:state.stamp,insertion:3,namePrefix:'stale'}),['invalid_handle']);
   const latest=await source.state();await source.close();await reject(()=>target.apply({...first,stamp:state.stamp,source:latest.stamp,insertion:3,namePrefix:'closed'}),['invalid_handle']);
   const imported=await target.render(1);check(imported.pixels.length===sourceImage.pixels.length&&imported.pixels.every((v,i)=>v===sourceImage.pixels[i]),'Source change/close damaged imported appearance');
   fields=await target.formFields();const field=fields.records.find(f=>f.name==='external_name');check(field.details.value.text==='Old'&&field.widgets[0].page===1,'Imported field ownership/value wrong');await target.apply({operation:'setFormValue',stamp:fields.stamp,object:field.object,value:{kind:'text',text:'Edited import'}});
   const navigation=await engine.open(await bytes('page-import-navigation')),navigationState=await navigation.state();state=await target.state();
   const nav={operation:'importPages',stamp:state.stamp,source:navigationState.stamp,pages:[0],insertion:3,namePrefix:'nav',includeNavigation:true};const originalCommit=state.commitRevision;await reject(()=>target.apply(nav),['unsupported','format']);check((await target.state()).commitRevision===originalCommit,'Partial navigation import committed');
   state=await target.state();await target.apply({...nav,stamp:state.stamp,pages:[1,0]});await navigation.close();check(target.pageCount===5,'Navigation page count wrong');
   const outlines=await target.outlines(),links=await target.links(4);check(outlines.some(o=>o.title==='Imported second'&&o.target.page===3),'Outline target not remapped');check(links.some(l=>l.target.kind==='page'&&l.target.page===3),'Link target not remapped');
   const saved=await target.save(),reopened=await engine.open(saved.bytes,{mode:'strict'}),pixels=[];check(reopened.pageCount===5,'Reopened import count wrong');check((await reopened.formFields()).records.find(f=>f.name==='external_name').details.value.text==='Edited import','Imported field edit not saved');
   for(let i=0;i<5;i++){const a=await target.render(i),b=await reopened.render(i);check(a.pixels.length===b.pixels.length&&a.pixels.every((v,index)=>v===b.pixels[index]),'Import reopen pixels changed');pixels.push(Array.from(b.pixels));}
   artifacts.push({name:worker?'import-worker':'import-sync',bytes:Array.from(saved.bytes),pixels});await reopened.close();await target.close();engine.dispose();checks.push(`${worker?'Worker':'sync'} import: namespaces, stale/foreign/closed source, rollback, source isolation/close, field edits, navigation and strict reopen`);
  }
  const engine=await createWorkerEngine(),target=await engine.open(await bytes('content-edit'));
  for(const [name,password,code]of [['content-edit-no-copy','user','permission'],['content-edit-encrypted','user','unsupported'],['page-import-newer',undefined,'unsupported'],['page-import-signed',undefined,'unsupported']]){
   const source=await engine.open(await bytes(name),password?{password}:{}),sourceState=await source.state(),state=await target.state();
   await reject(()=>target.apply({operation:'importPages',stamp:state.stamp,source:sourceState.stamp,pages:[0],insertion:2,namePrefix:'negative'}),[code]);check((await target.state()).commitRevision===state.commitRevision,'Rejected source changed target');await source.close();
  }
  const denied=await engine.open(await bytes('content-edit-denied'),{password:'user'}),source=await engine.open(await bytes('interaction')),sourceState=await source.state();let state=await denied.state();await reject(()=>denied.apply({operation:'importPages',stamp:state.stamp,source:sourceState.stamp,pages:[0],insertion:2,namePrefix:'denied'}),['permission']);await denied.close();
  const encrypted=await engine.open(await bytes('content-edit-encrypted'),{password:'user'});state=await encrypted.state();await encrypted.apply({operation:'importPages',stamp:state.stamp,source:sourceState.stamp,pages:[0],insertion:2,namePrefix:'secure'});await source.close();const saved=await encrypted.save();await reject(()=>engine.open(saved.bytes),['password_required']);const unlocked=await engine.open(saved.bytes,{password:'user'});check(unlocked.pageCount===3&&(await unlocked.formFields()).records.some(f=>f.name==='secure_name'),'Encrypted target lost imported fields');artifacts.push({name:'import-encrypted',bytes:Array.from(saved.bytes),pixels:[]});
  for(const d of [target,encrypted,unlocked])await d.close();const stats=await engine.statistics();check(!stats.handles&&!stats.retainedBytes&&!stats.documents,'Import retained resources');engine.dispose();checks.push('source copy/encryption/version/signature refusals, target permissions, encrypted target preserved, resources released');return {artifacts,checks};
 });
 report.checks=result.checks;for(const item of result.artifacts){await writeFile(resolve(out,item.name+'.pdf'),new Uint8Array(item.bytes));for(const [index,pixels]of item.pixels.entries())await writeFile(resolve(out,`${item.name}-${index}.rgba`),new Uint8Array(pixels));}
 const ready=()=>page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第')&&!!document.querySelector('#save-pdf')&&!document.querySelector('#save-pdf').disabled);
 await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/content-edit.pdf'));await ready();
 await page.getByLabel('导入 PDF 文件',{exact:true}).setInputFiles(resolve(repo,'output/webassembly/fixtures/interaction.pdf'));await page.getByLabel('导入名称前缀',{exact:true}).fill('ui');await page.getByRole('button',{name:'导入选定页面',exact:true}).click();
 await page.waitForFunction(()=>document.querySelector('#page').max==='3'&&!document.querySelector('#save-pdf')?.disabled);
 await page.locator('#page').fill('2');await page.locator('#render').click();await ready();assert.equal(await page.getByLabel('name',{exact:true}).count(),0);assert.equal(await page.getByLabel('ui_name',{exact:true}).inputValue(),'Old');
 const field=page.getByLabel('ui_name',{exact:true});await field.fill('UI imported');await field.dispatchEvent('change');await ready();
 const download=page.waitForEvent('download');await page.locator('#save-pdf').click();await(await download).saveAs(resolve(out,'import-reader.pdf'));assert.equal(report.errors.length,0);report.checks.push('reader local import, navigation to imported page, field filling and download');
 await page.evaluate(()=>{const original=File.prototype.arrayBuffer;File.prototype.arrayBuffer=function(){if(this.name==='interaction.pdf'&&window.__holdImport)return new Promise((resolve,reject)=>{window.__releaseImport=()=>original.call(this).then(resolve,reject);});return original.call(this);};window.__holdImport=true;});
 await page.getByLabel('导入 PDF 文件',{exact:true}).setInputFiles(resolve(repo,'output/webassembly/fixtures/interaction.pdf'));await page.getByRole('button',{name:'导入选定页面',exact:true}).click();await page.waitForFunction(()=>typeof window.__releaseImport==='function');
 await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/page-import-navigation.pdf'));await ready();
 await page.evaluate(async()=>{await window.__releaseImport();window.__holdImport=false;await Promise.resolve();});
 assert.equal(await page.locator('#page').getAttribute('max'),'2');assert.equal(await page.getByLabel('ui_name',{exact:true}).count(),0);assert.equal(report.errors.length,0);report.checks.push('late source file read after switching targets does not import or write stale errors');report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;try{await page?.screenshot({path:resolve(out,'failure.png'),fullPage:true});report.failure=await page?.evaluate(()=>({status:document.querySelector('#status')?.textContent,page:document.querySelector('#page')?.value,max:document.querySelector('#page')?.max}));}catch{}}
finally{await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify({status:report.status,checks:report.checks,errors:report.errors,failure:report.failure}));}
