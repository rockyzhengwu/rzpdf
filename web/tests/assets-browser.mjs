import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdtemp,rm} from 'node:fs/promises';
const repo=fileURLToPath(new URL('../../',import.meta.url)),out=resolve(repo,'output/webassembly/assets-browser');
await mkdir(out,{recursive:true});

const temporary=await mkdtemp(resolve(repo,'output/webassembly/.assets-browser-')),bundle=resolve(temporary,'fonts-package');
const hash=data=>createHash('sha256').update(data).digest('hex');
const sources=[['CJK','pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf','docs/licenses/NotoSansSC-OFL.txt','OFL-1.1','truetype',0,0x65b0],['Symbol','pdf/tests/fixtures/synthetic-symbol.ttf','LICENSE','AGPL-3.0-only','truetype',0,0xd7],['Collection0','output/webassembly/fixtures/matrix-two-faces.ttc','LICENSE','AGPL-3.0-only','collection',0,65],['Collection1','output/webassembly/fixtures/matrix-two-faces.ttc','LICENSE','AGPL-3.0-only','collection',1,65]];
const entries=[];
for(const [id,file,licenseFile,spdx,format,index,point] of sources){const data=await readFile(resolve(repo,file));entries.push({resource:{id,version:'package-browser-1',format,sha256:hash(data),sizeBytes:data.length,font:{family:id,aliases:[format==='collection'?'MissingMatrix':'MissingFace'],weight:400,italic:false,serif:false,index},license:{spdx},ranges:[[point,format==='collection'?66:point]]},file:resolve(repo,file),licenseFile:resolve(repo,licenseFile)});}
const config=resolve(temporary,'input.json');await writeFile(config,JSON.stringify({version:1,entries}));
execFileSync(process.execPath,[resolve(repo,'scripts/package_web_assets.mjs'),'--output',bundle,'--config',config],{cwd:repo,timeout:60000});
const inventory=JSON.parse(await readFile(resolve(bundle,'assets.json')));assert.equal(inventory.optional.resources,4);assert.equal(inventory.optional.uniquePrograms,3);
const requests=[];
const server=createServer(async(req,res)=>{
  try{
    const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
    const base=path.startsWith('/assets/')?bundle: path.startsWith('/fixtures/')?resolve(repo,'output/webassembly'):resolve(repo,'web');
    const relative=path.startsWith('/assets/')?path.slice(8):path.startsWith('/fixtures/')?path.slice(1):path==='/'?'index.html':path.slice(1);
    const file=resolve(base,relative);if(!file.startsWith(base+'/')){res.writeHead(403).end();return;}
    if(path.startsWith('/assets/'))requests.push(path);
    const data=await readFile(file);res.setHeader('Content-Type',({'.css':'text/css','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.json':'application/json','.txt':'text/plain'})[extname(file)]??'application/octet-stream');res.setHeader('Content-Length',data.length);res.end(data);
  }catch{res.writeHead(404).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser,page;
const report={status:'failed',checks:[],errors:[]};
try {
  browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true,args:['--no-sandbox']});
  page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}`);


  report.browser=await browser.version();report.source=JSON.parse(await readFile(resolve(repo,'output/webassembly/wa03-build.json'))).source;
  report.assets={wasm:inventory.wasm,builtinCounts:inventory.builtinCounts,optional:inventory.optional};
  const checked=await page.evaluate(async()=>{
    const {FontResourceService}=await import('/dist/font-resources.js'),{createWorkerEngine}=await import('/dist/worker-client.js');
    const root=new URL('/assets/fonts/',location.href).href,manifest=await(await fetch('/assets/fonts/manifest.json')).json(),assets=await(await fetch('/assets/assets.json')).json();
    const check=(value,message)=>{if(!value)throw Error(message);};
    check(manifest.resources.length===4,'missing resources');check(manifest.resources[2].url===manifest.resources[3].url,'collection bytes duplicated');
    const notices=await(await fetch('/assets/fonts/licenses.json')).json();
    for(const entry of notices.resources){const response=await fetch('/assets/'+entry.file),bytes=await response.arrayBuffer();check(response.ok&&bytes.byteLength>0,'license text missing');const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');check(digest===entry.sha256,'license text changed');}
    for(const file of assets.files.filter(f=>f.file.startsWith('notices/font-data/'))){const response=await fetch('/assets/'+file.file),bytes=await response.arrayBuffer();check(response.ok&&bytes.byteLength===file.rawBytes,'builtin notice absent');const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),b=>b.toString(16).padStart(2,'0')).join('');check(digest===file.sha256,'builtin notice changed');}
    const input=new Uint8Array(await(await fetch('/fixtures/font-service.pdf')).arrayBuffer()),cacheName='rzpdf-fonts-v1-packaged-assets-browser';await caches.delete(cacheName);
    let engine=await createWorkerEngine(),doc=await engine.open(input),service=new FontResourceService(manifest,{root,cacheName});
    check((await service.ensure(doc,0)).complete,'packaged online resources incomplete');const expected=(await doc.render(0)).pixels;check(expected.some((v,i)=>i%4!==3&&v<200),'blank font output');
    service.dispose();await doc.close();engine.dispose();
    engine=await createWorkerEngine();doc=await engine.open(input);service=new FontResourceService(manifest,{root,cacheName,network:false});
    check((await service.ensure(doc,0)).complete,'packaged offline resources incomplete');const actual=(await doc.render(0)).pixels;
    check(actual.length===expected.length&&actual.every((v,i)=>v===expected[i]),'offline package output differs');service.dispose();await doc.close();engine.dispose();await caches.delete(cacheName);
    const results=[];
    for(const resource of manifest.resources.filter(r=>r.format==='collection')){
      engine=await createWorkerEngine();doc=await engine.open(new Uint8Array(await(await fetch('/fixtures/matrix-missing.pdf')).arrayBuffer()));service=new FontResourceService({version:1,resources:[resource]},{root,persistent:false});
      check((await service.ensure(doc,0)).complete,'collection face unavailable');const image=await doc.render(0);let ink=0;for(let i=0;i<image.pixels.length;i+=4)if(image.pixels[i]<200)ink++;
      check(ink>0,'blank collection face');results.push({index:resource.font.index,ink});service.dispose();await doc.close();engine.dispose();
    }
    check(results[0].ink!==results[1].ink,'collection index not preserved');
    return {collectionFaces:results,licenseResources:notices.resources.length,builtinNotices:assets.files.filter(f=>f.file.startsWith('notices/font-data/')).length};
  });
  report.validation=checked;report.requests=requests;report.checks.push('actual packaged HTTP resources load in Worker, fresh-engine offline rendering matches, no blank output');
  report.checks.push('both TTC indices render distinct glyphs from one packaged program; all copied notices remain retrievable');
  report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;try{await page?.screenshot({path:resolve(out,'failure-widget.png'),fullPage:true});report.failure=await page?.evaluate(()=>({status:document.querySelector('#status')?.textContent,fit:document.querySelector('#fit')?.value,zoom:document.querySelector('#zoom')?.value,pageSize:[document.querySelector('#page-view')?.offsetWidth,document.querySelector('#page-view')?.offsetHeight],viewportSize:[document.querySelector('#viewport')?.clientWidth,document.querySelector('#viewport')?.clientHeight],annotations:[...document.querySelectorAll('[aria-label="批注内容"]')].map(e=>e.value),tool:document.querySelector('[aria-label="页面批注工具"]')?.value}));}catch{}}
finally {await browser?.close();await new Promise(done=>server.close(done));await rm(temporary,{recursive:true,force:true});await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));}
