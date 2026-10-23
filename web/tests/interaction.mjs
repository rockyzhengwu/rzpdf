import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),out=resolve(repo,'output/webassembly/interaction-browser');
await mkdir(out,{recursive:true});
const server=createServer(async(req,res)=>{
  const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
  const file=path.startsWith('/fixtures/')?resolve(repo,'output/webassembly',path.slice(1)):resolve(repo,'web',path==='/'?'index.html':path.slice(1));
  if(!file.startsWith(repo)){res.writeHead(403).end();return;}
  try{const data=await readFile(file);res.setHeader('Content-Type',({'.css':'text/css','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.pdf':'application/pdf'})[extname(file)]??'application/octet-stream');res.end(data);}catch{res.writeHead(404).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser,page;
const report={status:'failed',checks:[],errors:[]};
try {
  browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});
  page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}`);await page.locator('.edit-toggle').click();
  const workerResult=await page.evaluate(async()=>{
    const {createWorkerEngine}=await import('/dist/worker-client.js');const engine=await createWorkerEngine();
    const check=(condition,message)=>{if(!condition)throw Error(message);};
    const rejected=async(action,code)=>{try{await action();}catch(e){check(e.code===code,`Expected ${code}, got ${e.code}: ${e.message}`);return;}throw Error(`Expected rejection ${code}`);};
    const fetchPdf=async(name)=>new Uint8Array(await(await fetch('/fixtures/'+name+'.pdf')).arrayBuffer());
    const doc=await engine.open(await fetchPdf('interaction'));
    const other=await engine.open(await fetchPdf('interaction'));
    let fields=await doc.formFields();const name=fields.records.find(f=>f.name==='name');
    const stale={operation:'setFormValue',stamp:fields.stamp,object:name.object,value:{kind:'text',text:'New'}};
    await rejected(()=>other.apply(stale),'invalid_handle');await doc.apply(stale);await rejected(()=>doc.apply(stale),'invalid_handle');
    const values=[['unicode',{kind:'text',text:'世界'}],['accepted',{kind:'checkbox',checked:true}],['selection',{kind:'choice',values:['A','B']}],['radio',{kind:'radio',state:'B'}]];
    for(const [name,value] of values){fields=await doc.formFields();const field=fields.records.find(f=>f.name===name);check(!!field,`missing field ${name}`);await doc.apply({operation:'setFormValue',stamp:fields.stamp,object:field.object,value});}
    let annotations=await doc.annotations(0);const square=annotations.records.find(a=>a.subtype==='Square');
    await doc.apply({operation:'updateAnnotation',stamp:annotations.stamp,page:0,object:square.object,annotation:{...square.annotation,contents:'Changed note'}});
    let state=await doc.state();const created=await doc.apply({operation:'addAnnotation',stamp:state.stamp,page:0,annotation:{kind:'Circle',rect:[200,80,260,120],contents:'Temporary'}});
    await doc.apply({operation:'removeAnnotation',stamp:created.stamp,page:0,object:created.created});
    const before=await doc.render(0);const bare=await doc.render(0,72,{annotations:false});check(before.pixels.some((v,i)=>v!==bare.pixels[i]),'Appearances not rendered');
    await rejected(()=>doc.save({maxBytes:1}),'resource_limit');
    const saved=await doc.save();const reopened=await engine.open(saved.bytes,{mode:'strict'});
    const again=await reopened.formFields();check(again.records.find(f=>f.name==='unicode').details.value.text==='世界','Unicode not saved');check(again.records.find(f=>f.name==='name').details.value.text==='New','Text not saved');
    check(again.records.find(f=>f.name==='accepted').details.value.checked,'Checkbox not saved');
    check((await reopened.annotations(0)).records.find(a=>a.subtype==='Square').annotation.contents==='Changed note','Annotation not saved');
    const after=await reopened.render(0);check(after.pixels.length===before.pixels.length&&after.pixels.every((v,i)=>v===before.pixels[i]),'Reopened appearance changed');
    const mixed=await engine.open(await fetchPdf('interaction-mixed'));const records=(await mixed.formFields()).records;check(records.some(f=>!f.details&&f.unsupportedReason)&&records.some(f=>f.details),'Unsupported field hid supported fields');
    const secure=await engine.open(await fetchPdf('interaction-protected'),{password:'user'});const sf=await secure.formFields();
    await secure.apply({operation:'setFormValue',stamp:sf.stamp,object:sf.records.find(f=>f.name==='name').object,value:{kind:'text',text:'Allowed'}});
    state=await secure.state();await rejected(()=>secure.apply({operation:'addAnnotation',stamp:state.stamp,page:0,annotation:{kind:'Square',rect:[20,20,80,60]}}),'permission');
    const encrypted=await secure.save();await rejected(()=>engine.open(encrypted.bytes),'password_required');const unlocked=await engine.open(encrypted.bytes,{password:'user'});check((await unlocked.state()).permissions.securityRevision===6,'Encryption lost');
    for(const d of [doc,other,reopened,mixed,secure,unlocked])await d.close();engine.dispose();
    return {saved:Array.from(saved.bytes),pixels:Array.from(before.pixels),checks:['Worker fields: text/Unicode/checkbox/choice/radio','stale and foreign stamps rejected','annotation update/create/delete','appearance rendering and strict save/reopen pixel equality','save budget rejection','mixed unsupported fields retained','R6 fill permission, annotation denial and encrypted save']};
  });
  report.checks=workerResult.checks;
  await writeFile(resolve(out,'worker-edited.pdf'),new Uint8Array(workerResult.saved));
  await writeFile(resolve(out,'worker-edited.rgba'),new Uint8Array(workerResult.pixels));
  await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/interaction.pdf'));
  await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 /'));
  const field=page.getByLabel('name',{exact:true});await field.fill('Browser input');await field.dispatchEvent('change');
  await page.waitForFunction(()=>document.querySelector('[aria-label="name"]')?.value==='Browser input'&&!document.querySelector('[aria-label="name"]').disabled);
  const ready=()=>page.waitForFunction(()=>document.querySelector('#save-pdf')&&!document.querySelector('#save-pdf').disabled);
  await page.getByLabel('accepted',{exact:true}).check();await ready();
  await page.getByLabel('radio',{exact:true}).selectOption('B');await ready();
  await page.getByLabel('selection',{exact:true}).selectOption(['0','1']);await ready();
  await page.getByLabel('unicode',{exact:true}).fill('世界');await page.getByLabel('unicode',{exact:true}).dispatchEvent('change');await ready();
  await page.getByLabel('批注内容',{exact:true}).fill('Reader note');await page.getByLabel('批注内容',{exact:true}).dispatchEvent('change');await ready();
  const downloadEvent=page.waitForEvent('download');await page.locator('#save-pdf').click();const download=await downloadEvent;await download.saveAs(resolve(out,'edited.pdf'));
  assert.ok((await readFile(resolve(out,'edited.pdf'))).length>0);report.checks.push('reader text/Unicode/checkbox/radio/multichoice, annotation edit and user download');
  for(const rotation of [0,90,180,270]) {
    await page.locator('#pdf').setInputFiles(resolve(repo,`output/webassembly/fixtures/interaction-rotation-${rotation}.pdf`));
    await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 /'));
    const geometry=await page.evaluate(async(rotation)=>{
      const {createWorkerEngine}=await import('/dist/worker-client.js');const engine=await createWorkerEngine();
      try {const doc=await engine.open(new Uint8Array(await(await fetch(`/fixtures/interaction-rotation-${rotation}.pdf`)).arrayBuffer()));return {info:await doc.pageInfo(0),widget:(await doc.formFields()).records.find(f=>f.name==='name').widgets[0]};}finally{engine.dispose();}
    },rotation);
    for(const percent of [50,100,200]) {
      await page.locator('#zoom').fill(String(percent));await page.locator('#render').click();
      await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 /')&&!!document.querySelector('#widgets [aria-label="页面表单 name"]'));
      const delta=await page.evaluate(({geometry,percent})=>{
        const input=document.querySelector('#widgets [aria-label="页面表单 name"]'),view=document.querySelector('#page-view').getBoundingClientRect(),actual=input.getBoundingClientRect();
        const [a,b,c,d,e,f]=geometry.info.userToDisplay,[x0,y0,x1,y1]=geometry.widget.rect,scale=percent/100;
        const points=[[x0,y0],[x1,y0],[x1,y1],[x0,y1]].map(([x,y])=>[(a*x+c*y+e)*scale,(b*x+d*y+f)*scale]);
        const xs=points.map(p=>p[0]),ys=points.map(p=>p[1]);return Math.max(Math.abs(actual.x-view.x-Math.min(...xs)),Math.abs(actual.y-view.y-Math.min(...ys)),Math.abs(actual.width-(Math.max(...xs)-Math.min(...xs))),Math.abs(actual.height-(Math.max(...ys)-Math.min(...ys))));
      },{geometry,percent});assert.ok(delta<.6,`Widget placement error ${rotation}/${percent}: ${delta}`);
    }
    const widget=page.getByLabel('页面表单 name',{exact:true});await widget.fill(`Angle${rotation}`);await widget.dispatchEvent('change');
    await page.waitForFunction((value)=>document.querySelector('#editor [aria-label="name"]')?.value===value&&!document.querySelector('#save-pdf').disabled,`Angle${rotation}`);
    await page.getByLabel('页面表单 accepted',{exact:true}).check();await ready();
    await page.getByLabel('页面表单 radio B',{exact:true}).check();await ready();
    assert.equal(await page.getByLabel('radio',{exact:true}).inputValue(),'B');
  }
  for(const flags of [12,20,28]) {
    await page.locator('#pdf').setInputFiles(resolve(repo,`output/webassembly/fixtures/interaction-flags-${flags}.pdf`));
    await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 /'));
    const delta=await page.evaluate(async(flags)=>{
      const {createWorkerEngine}=await import('/dist/worker-client.js');const engine=await createWorkerEngine();
      try {
        const doc=await engine.open(new Uint8Array(await(await fetch(`/fixtures/interaction-flags-${flags}.pdf`)).arrayBuffer()));
        const info=await doc.pageInfo(0),widget=(await doc.formFields()).records.find(f=>f.name==='name').widgets[0];
        const actual=document.querySelector('#widgets [aria-label="页面表单 name"]').getBoundingClientRect(),view=document.querySelector('#page-view').getBoundingClientRect();
        const [a,b,c,d,e,f]=info.userToDisplay,[x0,y0,x1,y1]=widget.rect,scale=Number(document.querySelector('#zoom').value)/100,factor=(flags&8)?1:scale;
        const width=((flags&16)?x1-x0:y1-y0)*info.userUnit*factor,height=((flags&16)?y1-y0:x1-x0)*info.userUnit*factor;
        const expectedX=(a*x0+c*y1+e)*scale-((flags&16)?0:width),expectedY=(b*x0+d*y1+f)*scale;
        return {delta:Math.max(Math.abs(actual.x-view.x-expectedX),Math.abs(actual.y-view.y-expectedY),Math.abs(actual.width-width),Math.abs(actual.height-height)),actual:{x:actual.x-view.x,y:actual.y-view.y,width:actual.width,height:actual.height,css:document.querySelector('#widgets [aria-label="页面表单 name"]').style.cssText},expected:{x:expectedX,y:expectedY,width,height},widget,info,scale};
      }finally{engine.dispose();}
    },flags);assert.ok(delta.delta<.6,`NoZoom/NoRotate placement error ${flags}: ${JSON.stringify(delta)}`);
    await page.getByLabel('页面表单 name',{exact:true}).fill('Flags');await page.getByLabel('页面表单 name',{exact:true}).dispatchEvent('change');await ready();
  }
  report.checks.push('real NoZoom, NoRotate and combined flags at 200%/90 degrees/UserUnit 1.5');
  {
    const count=await page.getByLabel('批注内容',{exact:true}).count();
    await page.getByLabel('页面批注工具',{exact:true}).selectOption('FreeText');await page.getByLabel('绘制批注内容',{exact:true}).fill('This text cannot fit the tiny rectangle');
    const box=await page.locator('#widgets').evaluate(element=>{const b=element.getBoundingClientRect(),v=document.querySelector('#viewport').getBoundingClientRect();return {x:Math.max(b.left,v.left,0)+35,y:Math.max(b.top,v.top,0)+35};});
    await page.mouse.move(box.x,box.y);await page.mouse.down();await page.mouse.move(box.x+50,box.y+35);await page.mouse.up();
    await page.waitForFunction(()=>document.querySelector('#status').textContent.includes('does not fit')&&!document.querySelector('#save-pdf').disabled);
    assert.equal(await page.getByLabel('批注内容',{exact:true}).count(),count);
    await page.getByLabel('页面批注工具',{exact:true}).selectOption('Square');
    await page.mouse.move(box.x,box.y);await page.mouse.down();await page.mouse.move(box.x+20,box.y+20);
    await page.locator('#widgets').dispatchEvent('pointercancel',{pointerId:1});await page.mouse.up();
    assert.equal(await page.getByLabel('批注内容',{exact:true}).count(),count);assert.equal(await page.getByLabel('页面批注工具',{exact:true}).inputValue(),'Square');
    report.checks.push('drawing overflow rollback refresh and pointercancel without submission');
  }
  for(const kind of ['Text','Square','Circle','FreeText','Highlight','Line','Ink']) {
    const before=await page.getByLabel('批注内容',{exact:true}).count();
    await page.getByLabel('页面批注工具',{exact:true}).selectOption(kind);
    await page.getByLabel('绘制批注内容',{exact:true}).fill(kind==='FreeText'?'X':'Drawn '+kind);
    const box=await page.locator('#widgets').evaluate(element=>{const b=element.getBoundingClientRect(),v=document.querySelector('#viewport').getBoundingClientRect();return {x:Math.max(b.left,v.left,0)+35,y:Math.max(b.top,v.top,0)+35};});
    await page.mouse.move(box.x,box.y);await page.mouse.down();await page.mouse.move(box.x+50,box.y+35,{steps:5});await page.mouse.up();
    await page.waitForFunction((count)=>document.querySelectorAll('[aria-label="批注内容"]').length===count&&!document.querySelector('#save-pdf').disabled,before+1);
  }
  const annotationDownload=page.waitForEvent('download');await page.locator('#save-pdf').click();await(await annotationDownload).saveAs(resolve(out,'drawn.pdf'));
  report.checks.push('real pointer tools: note, square, circle, free text, highlight, line, ink and download');
  report.checks.push('page Widget placement and real filling at four rotations, three zooms, CropBox and UserUnit');
  await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/interaction-visibility.pdf'));
  await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 /'));
  assert.equal(await page.getByLabel('页面表单 name',{exact:true}).count(),0);
  assert.equal(await page.getByLabel('页面表单 accepted',{exact:true}).count(),0);
  assert.equal(await page.getByLabel('页面表单 unicode',{exact:true}).count(),0);
  assert.ok(await page.getByLabel('页面表单 radio A',{exact:true}).isDisabled());
  const combo=page.getByLabel('页面表单 editable',{exact:true});await combo.fill('Custom');await combo.dispatchEvent('change');
  await page.waitForFunction(()=>document.querySelector('#editor [aria-label="editable"]')?.value==='Custom'&&!document.querySelector('#save-pdf').disabled);
  assert.equal(await page.getByLabel('页面表单 unison A',{exact:true}).count(),2);
  assert.ok(await page.getByLabel('页面表单 unison A',{exact:true}).nth(0).isChecked());assert.ok(await page.getByLabel('页面表单 unison A',{exact:true}).nth(1).isChecked());
  await page.getByLabel('页面表单 unison B',{exact:true}).check();await ready();
  assert.ok(await page.getByLabel('页面表单 unison B',{exact:true}).isChecked());assert.ok(!await page.getByLabel('页面表单 unison A',{exact:true}).nth(0).isChecked());
  report.checks.push('hidden/NoView widgets omitted, read-only widgets disabled, editable combo custom value and radio InUnison');
  // Fit is measured against the actual scroll container, including padding and scrollbar.
  const fitted=async(kind)=>{
    await page.locator('#fit').selectOption(kind);
    await page.waitForFunction(kind=>{
      const v=document.querySelector('#viewport'),p=document.querySelector('#page-view'),s=getComputedStyle(v);
      const width=v.clientWidth-parseFloat(s.paddingLeft)-parseFloat(s.paddingRight);
      const height=parseFloat(s.maxHeight)-parseFloat(s.paddingTop)-parseFloat(s.paddingBottom);
      const b=p.getBoundingClientRect();
      return document.querySelector('#status').textContent.startsWith('第 ')&&document.querySelector('#save-pdf')&&!document.querySelector('#save-pdf').disabled&&
        (kind==='width'?Math.abs(b.width-width)<1:b.width<=width+1&&b.height<=height+1&&(Math.abs(b.width-width)<1||Math.abs(b.height-height)<1));
    },kind);
  };
  await fitted('width');
  await page.setViewportSize({width:980,height:700});await fitted('width');
  await fitted('page');
  await page.setViewportSize({width:980,height:400});
  await page.waitForFunction(()=>{const v=document.querySelector('#viewport'),p=document.querySelector('#page-view'),s=getComputedStyle(v);return Math.abs(p.getBoundingClientRect().height-(parseFloat(s.maxHeight)-parseFloat(s.paddingTop)-parseFloat(s.paddingBottom)))<1;});
  await page.locator('#zoom').fill('125');assert.equal(await page.locator('#fit').inputValue(),'manual');
  await page.locator('#render').click();await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 ')&&document.querySelector('#save-pdf')&&!document.querySelector('#save-pdf').disabled);
  report.checks.push('fit width/page, container and window resize, manual zoom exits fit');
  report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;try{await page?.screenshot({path:resolve(out,'failure-widget.png'),fullPage:true});report.failure=await page?.evaluate(()=>({status:document.querySelector('#status')?.textContent,fit:document.querySelector('#fit')?.value,zoom:document.querySelector('#zoom')?.value,pageSize:[document.querySelector('#page-view')?.offsetWidth,document.querySelector('#page-view')?.offsetHeight],viewportSize:[document.querySelector('#viewport')?.clientWidth,document.querySelector('#viewport')?.clientHeight],annotations:[...document.querySelectorAll('[aria-label="批注内容"]')].map(e=>e.value),tool:document.querySelector('[aria-label="页面批注工具"]')?.value}));}catch{}}
finally {await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));}
