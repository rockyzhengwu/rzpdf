// Real optimized Wasm + SDK + Worker; no system font registration or overlays.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';
const root=fileURLToPath(new URL('../../',import.meta.url)),web=resolve(root,'web');
const fixtures=resolve(process.env.RZPDF_FREETEXT_FIXTURES??resolve(root,'output/unicode-freetext-20261028/fixtures'));
const out=resolve(process.env.RZPDF_FREETEXT_OUTPUT??resolve(root,'output/webassembly/unicode-freetext'));
await mkdir(out,{recursive:true});
const names=['base.pdf','allowed.pdf','denied.pdf','font.ttf'];
const server=createServer(async(req,res)=>{
 try{
  const pathname=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
  const fixture=names.find(n=>pathname==='/fixtures/'+n);
  const signed=pathname==='/fixtures/signed.pdf';
  const path=fixture?resolve(fixtures,fixture):signed?resolve(root,'pdf/tests/fixtures/linearized/two-pages-signed.pdf'):resolve(web,'.'+(pathname==='/'?'/index.html':pathname));
  if(!fixture&&!signed&&!path.startsWith(web+sep)){res.writeHead(403).end();return;}
  res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html'})[extname(path)]??'application/octet-stream');res.end(await readFile(path));
 }catch{res.writeHead(404).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
const report={status:'failed',checks:[],errors:[],artifacts:{},fixtures:{}};let browser;
try{
 const digest=b=>createHash('sha256').update(b).digest('hex');
 for(const n of ['rzpdf_web.wasm','rzpdf_web.mjs','client.js','worker-client.js','pdf-worker.js'])report.artifacts[n]=digest(await readFile(resolve(web,'dist',n)));
 for(const n of names)report.fixtures[n]=digest(await readFile(resolve(fixtures,n)));
 report.driverSha256=digest(await readFile(fileURLToPath(import.meta.url)));
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true});report.browser=await browser.version();
 const page=await browser.newPage();page.on('pageerror',e=>report.errors.push(e.stack));
 await page.goto(`http://127.0.0.1:${server.address().port}`);
 const result=await page.evaluate(async()=>{
  const {createEngine}=await import('/dist/client.js');const {createWorkerEngine}=await import('/dist/worker-client.js');
  const check=(v,m)=>{if(!v)throw Error(m);};
  const hash=async bytes=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',bytes))].map(b=>b.toString(16).padStart(2,'0')).join('');
  const bytes=async name=>new Uint8Array(await(await fetch('/fixtures/'+name)).arrayBuffer());
  const fontBytes=await bytes('font.ttf'),font={postscriptName:'NotoSansSC-Regular',capHeight:733,stemV:80,serif:false,omitLayoutAndAuxiliary:true};
  const annotation=(text,row=0,alignment='left')=>({kind:'FreeText',rect:[30,30+row*60,350,80+row*60],contents:text,fontSize:14,alignment,color:[0.1,0.1,0.7]});
  const reject=async(action,code)=>{let error;try{await action();}catch(e){error=e;}check(error?.code===code,`expected ${code}; got ${error?.code}: ${error?.message}`);};
  const snapshot=async(doc,count)=>{const frames=[];for(const dpi of [72,144])for(let index=0;index<count;index++){const image=await doc.render(index,dpi);frames.push({page:index,dpi,width:image.width,height:image.height,sha256:await hash(image.pixels)});}return frames;};
  const atomic=async(doc,action,code)=>{
   const state=await doc.state(),before=(await doc.annotations(0)).records.length,pixels=await hash((await doc.render(0,72)).pixels);
   await reject(()=>action(state.stamp),code);
   check((await doc.state()).commitRevision===state.commitRevision,'failed operation committed');
   check((await doc.annotations(0)).records.length===before,'failed operation published annotation');
   check(await hash((await doc.render(0,72)).pixels)===pixels,'failed operation changed pixels');
  };
  const checks=[],outputs=[],direct=new Map();
  for(const transport of ['sync','worker']){
   const worker=transport==='worker',engine=await(worker?createWorkerEngine():createEngine());
   const apply=(doc,request,asset)=>worker?doc.apply(request,asset?{asset}:{}):doc.apply(request,asset);
   const save=(doc,maxBytes)=>worker?doc.save(maxBytes===undefined?{}:{maxBytes}):doc.save(maxBytes);
   try{
    const doc=await engine.open(await bytes('base.pdf'),{mode:'strict'});
    const geometry=await Promise.all([0,1,2,3].map(i=>doc.pageInfo(i)));
    await atomic(doc,stamp=>apply(doc,{operation:'addAnnotation',stamp,page:0,annotation:annotation('你好')}),'unsupported');
    await atomic(doc,stamp=>apply(doc,{operation:'addUnicodeFreeText',stamp,page:0,annotation:annotation('不存在的字'),font},fontBytes),'font');
    await atomic(doc,stamp=>apply(doc,{operation:'addUnicodeFreeText',stamp,page:0,annotation:annotation('你好'),font},new Uint8Array([1,2,3])),'font');
    const tiny={...annotation('你好'),rect:[20,20,25,25]};
    await atomic(doc,stamp=>apply(doc,{operation:'addUnicodeFreeText',stamp,page:0,annotation:tiny,font},fontBytes),'format');
    for(let index=0;index<4;index++)for(const [row,alignment]of ['left','center','right'].entries()){
     const a=annotation('你好，世界！\nHello PDF',row,alignment);if(row===1)a.opacity=0.65;
     await apply(doc,{operation:'addUnicodeFreeText',stamp:(await doc.state()).stamp,page:index,annotation:a,font},fontBytes);
    }
    check(fontBytes.byteLength>0,'Worker detached caller font asset');
    check(JSON.stringify(await Promise.all([0,1,2,3].map(i=>doc.pageInfo(i))))===JSON.stringify(geometry),'creation changed page geometry');
    const created=await snapshot(doc,4),initial=await save(doc);
    outputs.push({name:transport+'-created.pdf',bytes:Array.from(initial.bytes)});
    const reopened=await engine.open(initial.bytes,{mode:'strict'});await doc.close();
    check(JSON.stringify(await snapshot(reopened,4))===JSON.stringify(created),'strict reopen changed created pixels');
    for(let index=0;index<4;index++){
     const records=(await reopened.annotations(index)).records;
     check(records.length===3&&records.every(r=>r.unicodeFreeText?.repertoire.includes('你')),'reopened bindings missing');
     for(const [row,alignment]of ['right','left','center'].entries()){
      const a={...records[row].annotation,contents:'Hello PDF\n世界',fontSize:13,alignment};
      await apply(reopened,{operation:'updateUnicodeFreeText',stamp:(await reopened.state()).stamp,page:index,object:records[row].object,annotation:a});
     }
    }
    let record=(await reopened.annotations(0)).records[0];
    await atomic(reopened,stamp=>apply(reopened,{operation:'updateAnnotation',stamp,page:0,object:record.object,annotation:record.annotation}),'unsupported');
    await atomic(reopened,stamp=>apply(reopened,{operation:'updateUnicodeFreeText',stamp,page:0,object:record.object,annotation:{...record.annotation,contents:'新增字符'}}),'font');
    const neighbors=await snapshot(reopened,4);
    await apply(reopened,{operation:'updateUnicodeFreeText',stamp:(await reopened.state()).stamp,page:0,object:record.object,annotation:{...record.annotation,contents:'新增字符'},font},fontBytes);
    const after=await snapshot(reopened,4);
    check(after.filter(f=>f.page!==0).every(f=>JSON.stringify(f)===JSON.stringify(neighbors.find(n=>n.page===f.page&&n.dpi===f.dpi))),'new font changed another page');
    await apply(reopened,{operation:'copyPage',stamp:(await reopened.state()).stamp,page:0,insertion:4});
    const source=(await reopened.annotations(0)).records[0],copied=(await reopened.annotations(4)).records[0];
    check(JSON.stringify(source.unicodeFreeText.fontObject)===JSON.stringify(copied.unicodeFreeText.fontObject),'copy did not retain shared immutable font');
    const sourcePixels=await hash((await reopened.render(0,72)).pixels);
    await apply(reopened,{operation:'updateUnicodeFreeText',stamp:(await reopened.state()).stamp,page:4,object:copied.object,annotation:{...copied.annotation,contents:'你好'},font},fontBytes);
    check(await hash((await reopened.render(0,72)).pixels)===sourcePixels,'copy update changed shared original');
    await atomic(reopened,()=>save(reopened,1),'resource_limit');
    const finalPixels=await snapshot(reopened,5),saved=await save(reopened),finalDoc=await engine.open(saved.bytes,{mode:'strict'});
    check(JSON.stringify(await snapshot(finalDoc,5))===JSON.stringify(finalPixels),'strict reopen changed updated pixels');
    check(JSON.stringify(await Promise.all([0,1,2,3].map(i=>finalDoc.pageInfo(i))))===JSON.stringify(geometry),'updates changed original page geometry');
    for(const stage of [{name:'created',frames:created},{name:'updated',frames:finalPixels}]){
     if(!worker)direct.set(stage.name,JSON.stringify(stage.frames));else check(direct.get(stage.name)===JSON.stringify(stage.frames),'direct/Worker pixels differ: '+stage.name);
    }
    outputs.push({name:transport+'-updated.pdf',bytes:Array.from(saved.bytes)});
    await reopened.close();await finalDoc.close();
    const legacy=await engine.open(await bytes('base.pdf'),{mode:'strict'});
    const plain=annotation('Hello PDF');
    const legacyAdded=await apply(legacy,{operation:'addAnnotation',stamp:(await legacy.state()).stamp,page:0,annotation:plain});
    const legacyRecord=(await legacy.annotations(0)).records[0];
    check(legacyRecord.annotation.contents==='Hello PDF'&&!legacyRecord.unicodeFreeText,'legacy Courier projection changed');
    await apply(legacy,{operation:'updateAnnotation',stamp:(await legacy.state()).stamp,page:0,object:legacyAdded.created,annotation:{...plain,contents:'PDF Hello',alignment:'right'}});
    const legacyPixels=await hash((await legacy.render(0,72)).pixels);
    const legacyReopened=await engine.open((await save(legacy)).bytes,{mode:'strict'});
    check((await legacyReopened.annotations(0)).records[0].annotation.contents==='PDF Hello','legacy update lost');
    check(await hash((await legacyReopened.render(0,72)).pixels)===legacyPixels,'legacy save/reopen pixels');
    await legacy.close();await legacyReopened.close();
    const allowed=await engine.open(await bytes('allowed.pdf'),{mode:'strict',password:'user'});
    check(!(await allowed.state()).permissions.modify&&(await allowed.state()).permissions.annotate,'R6 permission fixture');
    await apply(allowed,{operation:'addUnicodeFreeText',stamp:(await allowed.state()).stamp,page:0,annotation:annotation('你好'),font},fontBytes);
    await atomic(allowed,stamp=>apply(allowed,{operation:'replaceText',stamp,page:0,start:0,end:1,text:'X'}),'permission');
    const encrypted=await save(allowed);await reject(()=>engine.open(encrypted.bytes,{mode:'strict'}),'password_required');
    const unlocked=await engine.open(encrypted.bytes,{mode:'strict',password:'owner'});
    check((await unlocked.annotations(0)).records[0].annotation.contents==='你好','encrypted annotation lost');
    check(await hash((await unlocked.render(0)).pixels)===await hash((await allowed.render(0)).pixels),'encrypted reopen pixels');
    outputs.push({name:transport+'-encrypted.pdf',bytes:Array.from(encrypted.bytes)});await unlocked.close();await allowed.close();
    const denied=await engine.open(await bytes('denied.pdf'),{mode:'strict',password:'user'});
    await atomic(denied,stamp=>apply(denied,{operation:'addUnicodeFreeText',stamp,page:0,annotation:annotation('你好'),font},fontBytes),'permission');await denied.close();
    const signed=await engine.open(await bytes('signed.pdf'),{mode:'strict'});
    await atomic(signed,stamp=>apply(signed,{operation:'addUnicodeFreeText',stamp,page:0,annotation:annotation('你好'),font},fontBytes),'unsupported');await signed.close();
    const stats=await engine.statistics();check(stats.handles===0&&stats.retainedBytes===0&&(!worker||stats.documents===0),'retained engine resources: '+JSON.stringify(stats));
    checks.push({transport,created,updated:finalPixels,geometry,sharedIsolation:true,negativeAtomicity:true,permissionIsolation:true,legacyAsciiRoundtrip:true,strictReopen:true});
   }catch(error){throw new Error(`${transport}: ${error.message}`,{cause:error});}finally{engine.dispose();}
  }
  return {checks,outputs};
 });
 report.checks=result.checks;for(const item of result.outputs)await writeFile(resolve(out,item.name),new Uint8Array(item.bytes));
 assert.equal(report.checks.length,2);assert.equal(report.errors.length,0);report.status='passed';
}catch(error){report.errors.push(error.stack);process.exitCode=1;}finally{
 await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));
 console.log(JSON.stringify({status:report.status,transports:report.checks.length,errors:report.errors,report:resolve(out,'report.json')},null,2));
}
