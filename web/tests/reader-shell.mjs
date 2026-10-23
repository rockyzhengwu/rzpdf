import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),root=resolve(repo,'web'),out=resolve(repo,'output/webassembly/reader-shell');
await mkdir(out,{recursive:true});
const server=createServer(async(req,res)=>{try{const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);const file=resolve(root,path==='/'?'index.html':'.'+path);if(!file.startsWith(root+sep)){res.writeHead(403).end();return;}res.setHeader('Content-Type',({'.mjs':'text/javascript','.js':'text/javascript','.css':'text/css','.wasm':'application/wasm','.html':'text/html'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));}catch{res.writeHead(404).end();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser,page;const report={status:'failed',checks:[],errors:[]};
try{
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});
 page=await browser.newPage({viewport:{width:1440,height:1000},acceptDownloads:true});page.on('pageerror',error=>report.errors.push(error.message));
 await page.goto(`http://127.0.0.1:${server.address().port}`);await page.waitForFunction(()=>!document.querySelector('#pdf').disabled);
 assert.ok(await page.locator('#welcome').isVisible());assert.equal(await page.locator('#sidebar-toggle').getAttribute('aria-expanded'),'true');
 await page.screenshot({path:resolve(out,'desktop-welcome.png')});
 await page.locator('#theme-toggle').click();assert.equal(await page.locator('html').getAttribute('data-theme'),'dark');await page.reload();await page.waitForFunction(()=>!document.querySelector('#pdf').disabled);assert.equal(await page.locator('html').getAttribute('data-theme'),'dark');await page.locator('#theme-toggle').click();report.checks.push('desktop welcome, persistent theme and engine ready');
 const open=async path=>{await page.locator('#pdf').setInputFiles(resolve(repo,path));await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 /'));};
 const ready=number=>page.waitForFunction(number=>document.querySelector('#status').textContent.startsWith(`第 ${number} /`)&&document.querySelector('#canvas').getAttribute('aria-busy')==='false',number);
 await open('output/webassembly/fixtures/web-features.pdf');assert.ok(await page.locator('#viewport').isVisible());assert.equal(await page.locator('#page-count').textContent(),'2');assert.equal(await page.locator('#document-name').textContent(),'web-features.pdf');
 await page.locator('#next-page').click();await ready(2);assert.ok(await page.locator('#next-page').isDisabled());await page.locator('#previous-page').click();await ready(1);
 await page.locator('#page').fill('2');await page.locator('#page').press('Enter');await page.locator('#page').blur();await ready(2);
 await page.locator('#viewport').focus();await page.keyboard.press('ArrowLeft');await ready(1);report.checks.push('real PDF, page index, next/previous, direct page input and keyboard navigation');
 await page.locator('#zoom-in').click();await ready(1);assert.equal(await page.locator('#zoom').inputValue(),'125');await page.locator('#rotate').click();await page.waitForFunction(()=>document.querySelector('#page-view').dataset.rotation==='90'&&document.querySelector('#status').textContent.startsWith('第 1 /'));
 await page.locator('#fit').selectOption('page');await ready(1);const fit=await page.evaluate(()=>{const view=document.querySelector('#page-view'),port=document.querySelector('#viewport');return view.offsetHeight<=port.clientHeight&&view.offsetWidth<=port.clientWidth;});assert.ok(fit);report.checks.push('zoom, rotation and fit-page within actual reader viewport');
 await page.keyboard.press('Control+f');assert.ok(await page.locator('#query').isVisible());await page.locator('#query').fill('Web');await page.locator('#find').click();await page.locator('#results button').first().click();await page.waitForFunction(()=>document.querySelectorAll('#highlights polygon').length>0);
 await page.locator('[data-tab=outline]').click();await page.locator('#outline button').first().click();await ready(2);report.checks.push('search shortcut, real search highlights and document outline navigation');
 await page.locator('.edit-toggle').click();assert.ok(await page.locator('#editor').isVisible());await page.locator('#download').waitFor({state:'visible'});await page.waitForFunction(()=>!document.querySelector('#download').disabled);const download=page.waitForEvent('download');await page.locator('#download').click();const saved=await download;assert.ok(saved.suggestedFilename().endsWith('.pdf'));await saved.saveAs(resolve(out,'download.pdf'));
 await page.locator('#view-rotation').selectOption('0');await ready(2);await page.waitForTimeout(250);await ready(2);await page.screenshot({path:resolve(out,'desktop-reader.png')});await page.locator('#close-inspector').click();report.checks.push('editor panel and real PDF download through save API');
}catch(error){report.errors.push(error.stack);process.exitCode=1;}
finally{if(page)try{
 // Narrow layouts and drag-and-drop use the same document lifecycle.
 await page.setViewportSize({width:390,height:844});await page.reload();await page.waitForFunction(()=>!document.querySelector('#pdf').disabled);assert.equal(await page.locator('#sidebar-toggle').getAttribute('aria-expanded'),'false');await page.screenshot({path:resolve(out,'mobile-welcome.png')});
 const data=[...await readFile(resolve(repo,'output/webassembly/fixtures/web-features.pdf'))];await page.evaluate(data=>{const transfer=new DataTransfer();transfer.items.add(new File([new Uint8Array(data)],'dropped.pdf',{type:'application/pdf'}));document.dispatchEvent(new DragEvent('drop',{dataTransfer:transfer,bubbles:true,cancelable:true}));},data);
 await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 /'));assert.equal(await page.locator('#document-name').textContent(),'dropped.pdf');await page.locator('#fit').selectOption('width');await page.waitForFunction(()=>document.querySelector('#canvas').getAttribute('aria-busy')==='false'&&document.querySelector('#status').textContent.startsWith('第 1 /'));await page.waitForTimeout(200);
 assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));await page.screenshot({path:resolve(out,'mobile-reader.png')});await page.locator('#sidebar-toggle').click();await page.locator('[data-tab=search]').click();assert.ok(await page.locator('#query').isVisible());await page.keyboard.press('Escape');assert.ok(await page.locator('#sidebar').isHidden());report.checks.push('390px mobile layout, real file drop, fit width, navigation drawer and Escape');
 await page.locator('#pdf').setInputFiles(resolve(repo,'pdf/tests/fixtures/r4/password-user.pdf'));await page.waitForFunction(()=>!document.querySelector('#unlock').hidden);assert.ok(await page.locator('#password').isVisible());report.checks.push('password-protected PDF automatically reveals settings and password prompt');
 if(!report.errors.length)report.status='passed';
 }catch(error){report.errors.push(error.stack);process.exitCode=1;}
 await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));}
