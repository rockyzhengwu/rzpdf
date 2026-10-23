import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
const repo=fileURLToPath(new URL('../../',import.meta.url)),out=resolve(repo,'output/webassembly/content-browser');await mkdir(out,{recursive:true});
const server=createServer(async(req,res)=>{const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);const file=path.startsWith('/fixtures/')?resolve(repo,'output/webassembly',path.slice(1)):resolve(repo,'web',path==='/'?'index.html':path.slice(1));if(!file.startsWith(repo)){res.writeHead(403).end();return;}try{res.setHeader('Content-Type',({'.css':'text/css','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.pdf':'application/pdf'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));}catch{res.writeHead(404).end();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));let browser,page;
const report={status:'failed',checks:[],errors:[]};
try{
  const digest=data=>createHash('sha256').update(data).digest('hex');report.fixture_sha256={};
  for(const name of ['content-edit.pdf','content-edit-rotated.pdf','content-edit-inline.pdf','content-edit-denied.pdf','content-edit-no-copy.pdf','content-edit-encrypted.pdf','content-edit-metadata-limit.pdf','content-replacement.png','content-replacement.jpg'])report.fixture_sha256[name]=digest(await readFile(resolve(repo,'output/webassembly/fixtures',name)));
  report.artifacts={};for(const name of ['rzpdf_web.mjs','rzpdf_web.wasm','client.js','worker-client.js','protocol.js','pdf-worker.js','interaction.js','editor.js','content-editor.js','geometry.js','demo.js'])report.artifacts[name]=digest(await readFile(resolve(repo,'web/dist',name)));
  report.source=JSON.parse(await readFile(resolve(repo,'output/webassembly/wa03-build.json'))).source;
  browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});page=await browser.newPage();page.on('pageerror',error=>report.errors.push(error.stack));await page.goto(`http://127.0.0.1:${server.address().port}`);await page.locator('.edit-toggle').click();
  const result=await page.evaluate(async()=>{
    const {createWorkerEngine}=await import('/dist/worker-client.js'),{createEngine}=await import('/dist/client.js');
    const bytes=async(name)=>new Uint8Array(await(await fetch('/fixtures/'+name)).arrayBuffer());
    const source=await bytes('content-edit.pdf'),png=await bytes('content-replacement.png'),jpeg=await bytes('content-replacement.jpg');
    const check=(v,message)=>{if(!v)throw Error(message);};
    const rejected=async(action,code)=>{try{await action();}catch(e){check(e.code===code,`Expected ${code}, got ${e.code}: ${e.message}`);return;}throw Error('Expected rejection '+code);};
    const text=content=>content.characters.map(c=>c.unicode??'�').join('');
    const artifacts=[],checks=[];
    for(const worker of [false,true]) {
      const engine=await(worker?createWorkerEngine():createEngine());const doc=await engine.open(source),foreign=await engine.open(source);
      const apply=(d,request,asset)=>worker?d.apply(request,{asset}):d.apply(request,asset);
      let content=await doc.content(0);check(text(content)==='AlphaTailAdjusted你好NestedNested','Source content differs');
      const tail=content.characters[5].position;
      const command={operation:'replaceText',stamp:content.stamp,page:0,start:0,end:5,text:'New'};
      await rejected(()=>apply(foreign,command),'invalid_handle');await apply(doc,command);await rejected(()=>apply(doc,command),'invalid_handle');
      content=await doc.content(0);check(JSON.stringify(content.characters[3].position)===JSON.stringify(tail),'Advance compensation moved Tail');
      let state=await doc.state();const commit=state.commitRevision;await rejected(()=>apply(doc,{operation:'replaceText',stamp:state.stamp,page:0,start:0,end:3,text:'中文'}),'unsupported');check((await doc.state()).commitRevision===commit,'Failed font edit committed');
      content=await doc.content(0);let index=text(content).indexOf('Adjusted');await apply(doc,{operation:'replaceText',stamp:content.stamp,page:0,start:index,end:index+8,text:'TJ'});
      content=await doc.content(0);index=text(content).indexOf('你好');await apply(doc,{operation:'replaceText',stamp:content.stamp,page:0,start:index,end:index+2,text:'世界'});
      content=await doc.content(0);index=text(content).indexOf('Nested');await apply(doc,{operation:'replaceText',stamp:content.stamp,page:0,start:index,end:index+6,text:'Changed'});
      content=await doc.content(0);check(text(content)==='NewTailTJ世界ChangedNested','TJ/Unicode/shared nested text isolation failed');
      const before=content.images.map(i=>i.matrix);await apply(doc,{operation:'transformImage',stamp:content.stamp,page:0,index:1,matrix:[1,0,0,1,11,-7]});
      content=await doc.content(0);check(Math.abs(content.images[1].matrix[4]-before[1][4]-11)<.001&&Math.abs(content.images[1].matrix[5]-before[1][5]+7)<.001,'Nested display delta wrong');check(JSON.stringify(content.images[0].matrix)===JSON.stringify(before[0])&&JSON.stringify(content.images[2].matrix)===JSON.stringify(before[2]),'Nested edit altered neighbor');
      await apply(doc,{operation:'copyPage',stamp:content.stamp,page:0,insertion:2});check(doc.pageCount===3,'Copy did not refresh pageCount');
      const copy=await doc.render(2);content=await doc.content(0);
      await apply(doc,{operation:'replaceImage',stamp:content.stamp,page:0,index:1,format:'png'},png);
      content=await doc.content(0);await apply(doc,{operation:'replaceImage',stamp:content.stamp,page:0,index:0,format:'jpeg'},jpeg);check(png.byteLength>0&&jpeg.byteLength>0,'Caller image bytes detached');
      const copyAfter=await doc.render(2);check(copyAfter.pixels.every((v,i)=>v===copy.pixels[i]),'Image replacement polluted copied page');
      state=await doc.state();const beforeBad=await doc.save();await rejected(()=>apply(doc,{operation:'replaceImage',stamp:state.stamp,page:0,index:0,format:'png'},new Uint8Array([1,2,3])),'format');
      const afterBad=await doc.save();check(afterBad.bytes.length===beforeBad.bytes.length&&afterBad.bytes.every((v,i)=>v===beforeBad.bytes[i]),'Failed image replacement changed saved bytes');
      content=await doc.content(0);await rejected(()=>apply(doc,{operation:'replaceText',stamp:content.stamp,page:0,start:0,end:5,text:'bad cross event'}),'unsupported');
      content=await doc.content(0);await apply(doc,{operation:'removeImage',stamp:content.stamp,page:0,index:2});check((await doc.content(0)).images.length===2,'Remove image draw failed');
      state=await doc.state();await apply(doc,{operation:'reorderPages',stamp:state.stamp,order:[2,1,0]});state=await doc.state();await apply(doc,{operation:'deletePage',stamp:state.stamp,page:1});check(doc.pageCount===2,'Delete did not refresh pageCount');
      state=await doc.state();await apply(doc,{operation:'rotatePage',stamp:state.stamp,page:1,degrees:90});check((await doc.pageInfo(1)).rotation===90,'Page rotation missing');
      const saved=await doc.save(),reopened=await engine.open(saved.bytes,{mode:'strict'});check(reopened.pageCount===2,'Saved page count wrong');
      const pixels=[];for(let page=0;page<2;page++){const a=await doc.render(page),b=await reopened.render(page);check(a.pixels.length===b.pixels.length&&a.pixels.every((v,i)=>v===b.pixels[i]),'Reopened pixels changed');pixels.push(Array.from(b.pixels));}
      artifacts.push({kind:worker?'worker':'sync',bytes:Array.from(saved.bytes),pixels});checks.push(`${worker?'Worker':'sync'}: text/TJ/Unicode, nested source isolation, image transform/PNG/JPEG/remove, page copy/order/delete/rotate, stale/foreign/error rollback, strict reopen pixels`);
      for(const d of [doc,foreign,reopened])await d.close();engine.dispose();
    }
    const engine=await createWorkerEngine();
    const rotated=await engine.open(await bytes('content-edit-rotated.pdf'));
    for(const imageIndex of [1,0]) {
      const current=await rotated.content(0),before=current.images.map(i=>i.matrix),display=[.5,.25,-.4,1.2,10,-20];
      await rotated.apply({operation:'transformImage',stamp:current.stamp,page:0,index:imageIndex,matrix:display});const updated=await rotated.content(0);
      for(const [x,y] of [[0,0],[1,0],[1,1],[0,1]]) {
        const [a,b,c,d,e,f]=before[imageIndex],px=a*x+c*y+e,py=b*x+d*y+f,[da,db,dc,dd,de,df]=display,[aa,ab,ac,ad,ae,af]=updated.images[imageIndex].matrix;
        check(Math.abs(aa*x+ac*y+ae-(da*px+dc*py+de))<.01&&Math.abs(ab*x+ad*y+af-(db*px+dd*py+df))<.01,'Rotated/CropBox/UserUnit image affine transform wrong');
      }
      check(updated.images.every((image,index)=>index===imageIndex||JSON.stringify(image.matrix)===JSON.stringify(before[index])),'Affine edit changed neighboring draw');
    }
    await rotated.close();checks.push('top-level and nested image affine transforms under Rotate/CropBox/UserUnit');
    const inline=await engine.open(await bytes('content-edit-inline.pdf'));let inlineContent=await inline.content(0);const neighbor=inlineContent.images[2].matrix;
    await inline.apply({operation:'replaceImage',stamp:inlineContent.stamp,page:0,index:1,format:'png'},{asset:png});inlineContent=await inline.content(0);check(JSON.stringify(inlineContent.images[2].matrix)===JSON.stringify(neighbor),'Inline replacement changed neighbor');
    await inline.apply({operation:'transformImage',stamp:inlineContent.stamp,page:0,index:2,matrix:[1,0,0,1,5,3]});inlineContent=await inline.content(0);check(Math.abs(inlineContent.images[2].matrix[4]-neighbor[4]-5)<.001,'Inline transform wrong');
    await inline.apply({operation:'removeImage',stamp:inlineContent.stamp,page:0,index:2});check((await inline.content(0)).images.length===2,'Inline remove failed');await inline.close();checks.push('nested inline image replacement/transform/removal');
    const denied=await engine.open(await bytes('content-edit-denied.pdf'),{password:'user'});let state=await denied.state();await rejected(()=>denied.apply({operation:'replaceText',stamp:state.stamp,page:0,start:0,end:5,text:'Denied'}),'permission');check((await denied.state()).stamp.revision===state.stamp.revision,'Permission refusal changed revision');
    const noCopy=await engine.open(await bytes('content-edit-no-copy.pdf'),{password:'user'});await rejected(()=>noCopy.content(0),'permission');await noCopy.render(0);
    const encrypted=await engine.open(await bytes('content-edit-encrypted.pdf'),{password:'user'});let content=await encrypted.content(0);await encrypted.apply({operation:'replaceText',stamp:content.stamp,page:0,start:0,end:5,text:'Secure'});content=await encrypted.content(0);await encrypted.apply({operation:'replaceImage',stamp:content.stamp,page:0,index:0,format:'png'},{asset:png});const saved=await encrypted.save();await rejected(()=>engine.open(saved.bytes),'password_required');const unlocked=await engine.open(saved.bytes,{password:'user'});check(text(await unlocked.content(0)).startsWith('Secure'),'Encrypted edit not preserved');
    for(const d of [denied,noCopy,encrypted,unlocked])await d.close();const stats=await engine.statistics();check(stats.handles===0&&stats.retainedBytes===0&&stats.documents===0,'Edit resources retained after close');engine.dispose();checks.push('R6 modify/copy refusals, encrypted text+image save, close releases tracked resources');
    return {checks,artifacts};
  });
  report.checks=result.checks;for(const artifact of result.artifacts){await writeFile(resolve(out,`content-${artifact.kind}.pdf`),new Uint8Array(artifact.bytes));for(const [index,pixels] of artifact.pixels.entries())await writeFile(resolve(out,`content-${artifact.kind}-${index}.rgba`),new Uint8Array(pixels));}
  await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/content-edit.pdf'));await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 /')&&!!document.querySelector('#save-pdf'));
  const ready=()=>page.waitForFunction(()=>!!document.querySelector('#save-pdf')&&!document.querySelector('#save-pdf').disabled&&document.querySelector('#status').textContent.startsWith('第'));
  await page.locator('[data-tab=search]').click();await page.locator('#query').fill('Alpha');await page.locator('#find').click();await page.waitForFunction(()=>document.querySelectorAll('#results button').length===1);
  await page.getByLabel('编辑文字段',{exact:true}).selectOption({label:'Alpha'});await page.getByLabel('替换文字',{exact:true}).fill('UI text');await page.getByRole('button',{name:'替换选中文字',exact:true}).click();await ready();assert.ok((await page.locator('#text-layer').textContent()).startsWith('UI textTail'));assert.equal(await page.locator('#results button').count(),0);
  await page.evaluate(()=>{[...document.querySelectorAll('button')].find(b=>b.textContent==='复制当前页').click();document.querySelector('#page').value='2';document.querySelector('#render').click();});
  await ready();assert.equal(await page.locator('#page').getAttribute('max'),'3');
  await page.locator('#page').fill('1');await page.locator('#render').click();await ready();
  await page.getByLabel('编辑图片',{exact:true}).selectOption('1');await page.getByLabel('图片水平移动',{exact:true}).fill('12');await page.getByLabel('图片垂直移动',{exact:true}).fill('-8');await page.getByRole('button',{name:'移动图片',exact:true}).click();await ready();assert.equal(await page.getByLabel('编辑图片',{exact:true}).inputValue(),'1');
  await page.getByLabel('替换图片文件',{exact:true}).setInputFiles(resolve(repo,'output/webassembly/fixtures/content-replacement.png'));await page.getByRole('button',{name:'替换图片',exact:true}).click();await ready();
  await page.getByRole('button',{name:'顺时针旋转页面',exact:true}).click();await ready();await page.getByRole('button',{name:'页面后移',exact:true}).click();await ready();
  await page.locator('#page').fill('3');await page.locator('#render').click();await ready();await page.getByRole('button',{name:'删除当前页',exact:true}).click();await ready();assert.equal(await page.locator('#page').getAttribute('max'),'2');
  const download=page.waitForEvent('download');await page.locator('#save-pdf').click();await(await download).saveAs(resolve(out,'content-reader.pdf'));report.checks.push('reader text/image edits, page copy/rotation/order/delete, refreshed page count and download');
  await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/content-edit-metadata-limit.pdf'));await ready();
  assert.ok((await page.getByRole('region',{name:'页面内容编辑'}).textContent()).includes('budget'));assert.equal(await page.locator('#canvas').getAttribute('width'),'300');
  report.checks.push('content metadata limit keeps reader rendering/page operations/save available');
  await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/content-edit.pdf'));await ready();
  await page.getByRole('button',{name:'复制当前页',exact:true}).click();
  await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/content-edit-rotated.pdf'));await ready();assert.equal(await page.locator('#page').getAttribute('max'),'2');
  assert.ok((await page.locator('#text-layer').textContent()).startsWith('AlphaTail'));assert.equal(report.errors.length,0);
  report.checks.push('editing concurrent navigation/file switch preserves current page state without unhandled rejections');report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;try{await page?.screenshot({path:resolve(out,'failure.png'),fullPage:true});report.failure=await page?.evaluate(()=>({status:document.querySelector('#status')?.textContent,text:document.querySelector('#text-layer')?.textContent}));}catch{}}
finally{await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));}
