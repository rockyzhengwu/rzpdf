import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
const repo=fileURLToPath(new URL('../../',import.meta.url)),root=resolve(repo,'web'),out=resolve(repo,'output/webassembly/large-input');
await mkdir(out,{recursive:true});
const bodies=['<< /Type /Catalog /Pages 2 0 R /RzPadding 5 0 R >>','<< /Type /Pages /Kids [3 0 R] /Count 1 >>','<< /Type /Page /Parent 2 0 R /MediaBox [0 0 20 20] /Contents 4 0 R >>','<< /Length 23 >>\nstream\n1 0 0 rg 0 0 20 20 re f\nendstream'];
const chunks=[Buffer.from('%PDF-1.7\n%\xe2\xe3\xcf\xd3\n','latin1')],offsets=[];let size=chunks.reduce((n,b)=>n+b.length,0);
for(const [index,body] of bodies.entries()){offsets.push(size);const bytes=Buffer.from(`${index+1} 0 obj\n${body}\nendobj\n`);chunks.push(bytes);size+=bytes.length;}
offsets.push(size);const padding=[Buffer.from(`5 0 obj\n<< /Length ${9*1024*1024} >>\nstream\n`),Buffer.alloc(9*1024*1024,88),Buffer.from('\nendstream\nendobj\n')];chunks.push(...padding);size+=padding.reduce((n,b)=>n+b.length,0);
chunks.push(Buffer.from(`xref\n0 6\n0000000000 65535 f \n${offsets.map(n=>`${String(n).padStart(10,'0')} 00000 n \n`).join('')}trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n${size}\n%%EOF\n`));
const fixture=Buffer.concat(chunks);
const server=createServer(async(req,res)=>{try{const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);if(path==='/large.pdf'){res.setHeader('Content-Type','application/pdf');res.end(fixture);return;}const file=resolve(root,path==='/'?'index.html':'.'+path);if(!file.startsWith(root+sep)){res.writeHead(403).end();return;}res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.css':'text/css'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));}catch{res.writeHead(404).end();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser,page;const report={status:'failed',inputBytes:fixture.length,checks:[],errors:[]};
try{
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});page=await browser.newPage({acceptDownloads:true});page.on('pageerror',e=>report.errors.push(e.message));await page.goto(`http://127.0.0.1:${server.address().port}`);await page.waitForFunction(()=>!document.querySelector('#pdf').disabled);
 report.checks=await page.evaluate(async()=>{
  const {createEngine}=await import('/dist/client.js'),{createWorkerEngine}=await import('/dist/worker-client.js');const bytes=new Uint8Array(await(await fetch('/large.pdf')).arrayBuffer()),checks=[];
  for(const worker of [false,true]){const engine=await(worker?createWorkerEngine():createEngine());try{const doc=await engine.open(bytes);if(doc.pageCount!==1)throw Error('page count');const capabilities=await doc.capabilities();if(capabilities.maxInputBytes!==2147483648||capabilities.maxBufferBytes!==64*1024*1024)throw Error('incorrect input budget capabilities');const image=await doc.render(0);if(!image.pixels.every((v,i)=>v===[255,0,0,255][i%4]))throw Error('large PDF rendered incorrectly');const saved=await doc.save();if(saved.bytes.length<=8*1024*1024)throw Error('large source save was truncated');const reopened=await engine.open(saved.bytes);if(reopened.pageCount!==1)throw Error('saved source cannot reopen');await reopened.close();await doc.close();const stats=await engine.statistics();if(stats.handles||stats.retainedBytes)throw Error('resources retained');if(bytes.byteLength<=8*1024*1024)throw Error('caller buffer detached');checks.push((worker?'Worker':'sync')+': >8 MiB open, nonblank exact rendering, default save, strict reopen, owned input and release');}finally{engine.dispose();}}
  return checks;
 });
 await page.locator('#pdf').setInputFiles({name:'large.pdf',mimeType:'application/pdf',buffer:fixture});await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 / 1'));assert.equal(await page.locator('#document-name').textContent(),'large.pdf');assert.doesNotMatch(await page.locator('#welcome').textContent(),/最多 8 MiB/);
 const download=page.waitForEvent('download');await page.locator('#download').click();await(await download).saveAs(resolve(out,'download.pdf'));assert.ok((await readFile(resolve(out,'download.pdf'))).length>8*1024*1024);
 report.checks.push('reader file picker and download accept a 9 MiB source');assert.equal(report.errors.length,0);report.status='passed';
}catch(error){report.errors.push(error.stack);process.exitCode=1;}finally{await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));}
