// Real Wasm/Worker acceptance, deliberately separate from resources.mjs stubs.
// Requires a freshly built module and resource_scan fixtures; never installs tools.
import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
const repo=fileURLToPath(new URL('../../',import.meta.url));
const fixtures=resolve(process.env.RZPDF_RESOURCE_FIXTURES??resolve(repo,'output/webassembly/resource-fixtures'));
const out=resolve(repo,'output/webassembly/resource-browser');await mkdir(out,{recursive:true});
const web=resolve(repo,'web');
const server=createServer(async(req,res)=>{try{
  const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
  const file=path==='/scan.pdf'||path==='/solid.pdf'?resolve(fixtures,path.slice(1)):resolve(web,'.'+(path==='/'?'/index.html':path));
  if(!file.startsWith(web+sep)&&!['scan.pdf','solid.pdf'].some(name=>file===resolve(fixtures,name))){res.writeHead(403).end();return;}
  res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));
}catch{res.writeHead(404).end();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let browser;const report={status:'failed',checks:[],errors:[]};
try{
  report.node=process.version;
  report.artifacts={};
  for(const name of ['rzpdf_web.wasm','rzpdf_web.mjs','client.js','worker-client.js','pdf-worker.js','protocol.js','raster.js']) {
    report.artifacts[name]=createHash('sha256').update(await readFile(resolve(web,'dist',name))).digest('hex');
  }
  report.fixtures={};
  for(const name of ['scan.pdf','solid.pdf'])report.fixtures[name]=createHash('sha256').update(await readFile(resolve(fixtures,name))).digest('hex');
  report.driverSha256=createHash('sha256').update(await readFile(fileURLToPath(import.meta.url))).digest('hex');
  browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true});
  report.browser=await browser.version();
  const page=await browser.newPage();page.setDefaultTimeout(120000);
  page.on('pageerror',e=>report.errors.push(e.message));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  report.checks=await page.evaluate(async()=>{
    const {createEngine}=await import('/dist/client.js');
    const {createWorkerEngine}=await import('/dist/worker-client.js');
    const checks=[],check=(v,m)=>{if(!v)throw Error(m);};
    const solid=new Uint8Array(await(await fetch('/solid.pdf')).arrayBuffer());
    const scan=new Uint8Array(await(await fetch('/scan.pdf')).arrayBuffer());
    for(const worker of [false,true]){
      const engine=await(worker?createWorkerEngine():createEngine());
      try{
        check((await engine.capabilities()).maxRegionDpi===1152,'engine capabilities before open');
        const doc=await engine.open(solid,{resourceProfile:'conservative'});
        check((await doc.capabilities()).maxImagePixels===2000000,'effective profile');
        for(const dpi of [576,1152]){
          const image=await doc.renderRegion(0,{x:0,y:0,width:64,height:64},dpi);
          check(image.pixels.every((v,i)=>v===(i%4===3?255:0)),'high-DPI tile independent solid oracle');
          await doc.renderRegion(0,{x:8,y:8,width:32,height:32},dpi);
          check((await doc.cacheStatistics()).pageHits>0,'prepared cache hit');
          check((await doc.clearRenderCaches()).pageEntries===0,'cache release');
          check(image.pixels[3]===255,'caller output retained after clear');
        }
        await doc.close();
        const scanning=await engine.open(scan);
        // The preview's font/text metadata must not force full-size images.
        await scanning.fontRequirements(0);await scanning.extractText(0);
        const image=await scanning.renderRegion(0,{x:0,y:0,width:64,height:64},144);
        check(image.pixels.every((v,i)=>i%4===3?v===255:Math.abs(v-120)<=3),'large scan native reduced decode');
        let rejected=false;
        try{await scanning.renderRegion(0,{x:0,y:0,width:32,height:32},576);}
        catch(e){check(e.code==='resource_limit'&&e.resource?.reason==='image_pixels','typed image failure through transport');rejected=true;}
        check(rejected,'large requested decode must not bypass budget');
        await scanning.renderRegion(0,{x:0,y:0,width:32,height:32},72);
        await scanning.close();check((await engine.statistics()).handles===0,'all native handles released');
        checks.push({transport:worker?'worker':'sync',highDpi:true,reducedScan:true,errorMetadata:true,caches:true});
      }finally{engine.dispose();}
    }
    return checks;
  });
  assert.equal(report.checks.length,2);assert.equal(report.errors.length,0);report.status='passed';
}catch(e){report.errors.push(e.stack);process.exitCode=1;}
finally{await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));}
