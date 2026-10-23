import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),root=resolve(repo,'web'),out=resolve(repo,'output/webassembly/outline-recovery');await mkdir(out,{recursive:true});
function fixture(kind){
 const title=kind==='odd'?Buffer.from('<FEFF0041FF>'):kind==='delimiter'?Buffer.from([40,254,255,89,41,98,77,41]):Buffer.from('<FEFF4E2D6587>');
 const bodies=[Buffer.from('<< /Type /Catalog /Pages 2 0 R /Outlines 5 0 R >>'),Buffer.from('<< /Type /Pages /Kids [3 0 R] /Count 1 >>'),Buffer.from('<< /Type /Page /Parent 2 0 R /MediaBox [0 0 20 20] /Contents 4 0 R >>'),Buffer.from('<< /Length 23 >>\nstream\n1 0 0 rg 0 0 20 20 re f\nendstream'),Buffer.from('<< /Type /Outlines /First 6 0 R /Last 6 0 R /Count 1 >>'),Buffer.concat([Buffer.from('<< /Parent 5 0 R /Title '),title,Buffer.from(kind==='unsupported'?' /A << /S /JavaScript /JS (throw) >> >>':' /Dest [3 0 R /Fit] >>')])];
 const chunks=[Buffer.from('%PDF-1.7\n%\xe2\xe3\xcf\xd3\n','latin1')],offsets=[];let size=chunks[0].length;
 for(const [index,body]of bodies.entries()){offsets.push(size);const b=Buffer.concat([Buffer.from(`${index+1} 0 obj\n`),body,Buffer.from('\nendobj\n')]);chunks.push(b);size+=b.length;}
 chunks.push(Buffer.from(`xref\n0 7\n0000000000 65535 f \n${offsets.map(n=>`${String(n).padStart(10,'0')} 00000 n \n`).join('')}trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n${size}\n%%EOF\n`));return Buffer.concat(chunks);
}
const server=createServer(async(req,res)=>{try{const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);const file=resolve(root,path==='/'?'index.html':'.'+path);if(!file.startsWith(root+sep)){res.writeHead(403).end();return;}res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.css':'text/css'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));}catch{res.writeHead(404).end();}});await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser,page;const report={status:'failed',checks:[],errors:[]};
try{
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});page=await browser.newPage();page.on('pageerror',e=>report.errors.push(e.message));await page.goto(`http://127.0.0.1:${server.address().port}`);await page.waitForFunction(()=>!document.querySelector('#pdf').disabled);
 const open=async kind=>{await page.locator('#pdf').setInputFiles({name:kind+'.pdf',mimeType:'application/pdf',buffer:fixture(kind)});await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 / 1'));};
 for(const kind of ['odd','delimiter','unsupported']){
  await open(kind);await page.locator('[data-tab=outline]').click();assert.ok(await page.locator('#outline-warning').isVisible());assert.equal(await page.locator('#outline button').count(),0);assert.ok(await page.locator('.outline-empty').isHidden());assert.match(await page.locator('#status').textContent(),/书签不可用/);
  const pixels=await page.locator('#canvas').evaluate(canvas=>{const p=canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data;return [...p].some((v,i)=>i%4!==3&&v<100);});assert.ok(pixels,'body still paints');
 }
 report.checks.push('odd UTF-16, malformed literal delimiters and unsupported actions show an explicit bookmark warning while body rendering continues');
 await open('valid');assert.ok(await page.locator('#outline-warning').isHidden());assert.equal(await page.locator('#outline button').textContent(),'中文');await page.locator('#outline button').click();await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 / 1'));report.checks.push('valid Unicode bookmarks and navigation remain available after switching files');
 await page.evaluate(async()=>{const {WorkerPdfDocument}=await import('/dist/worker-client.js'),{PdfApiError}=await import('/dist/client.js');const original=WorkerPdfDocument.prototype.outlines;window.__outlineOriginal=original;window.__holdOutline=true;WorkerPdfDocument.prototype.outlines=function(){if(window.__holdOutline){window.__holdOutline=false;return new Promise((_,reject)=>{window.__releaseOutline=()=>reject(new PdfApiError('format','late invalid UTF-16','outlines'));});}return original.call(this);};});
 await page.locator('#pdf').setInputFiles({name:'late.pdf',mimeType:'application/pdf',buffer:fixture('odd')});await page.waitForFunction(()=>typeof window.__releaseOutline==='function');await open('valid');await page.evaluate(async()=>{window.__releaseOutline();await new Promise(resolve=>setTimeout(resolve,50));});assert.ok(await page.locator('#outline-warning').isHidden());assert.equal(await page.locator('#outline button').textContent(),'中文');report.checks.push('late bookmark failure from a replaced document cannot poison the current reader');
 await page.evaluate(async()=>{const {WorkerPdfDocument}=await import('/dist/worker-client.js'),{PdfApiError}=await import('/dist/client.js');WorkerPdfDocument.prototype.outlines=function(){return Promise.reject(new PdfApiError('resource_limit','test outline quota','outlines'));};});
 await page.locator('#pdf').setInputFiles({name:'quota.pdf',mimeType:'application/pdf',buffer:fixture('valid')});await page.waitForFunction(()=>document.querySelector('#status').textContent==='test outline quota');assert.ok(await page.locator('#outline-warning').isHidden());assert.ok(await page.locator('#app').getAttribute('data-error')==='true');report.checks.push('resource-limit failures remain fatal and are not disguised as recoverable bookmark metadata');
 assert.equal(report.errors.length,0);report.status='passed';
}catch(error){report.errors.push(error.stack);process.exitCode=1;}finally{await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));}
