// Explicit local private-input regression. Never copies the input into the repo.
import {chromium} from 'playwright-core';
import {createServer} from 'node:http';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {resolve,extname,sep} from 'node:path';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
const repo=fileURLToPath(new URL('../../',import.meta.url)),web=resolve(repo,'web');
if(!process.env.RZPDF_PRIVATE_PDF)throw Error('Set RZPDF_PRIVATE_PDF to an explicitly authorized local PDF');
const input=resolve(process.env.RZPDF_PRIVATE_PDF),out=resolve(process.env.RZPDF_PRIVATE_REPORT??resolve(repo,'output/webassembly/private-pages'));
await mkdir(out,{recursive:true});
const bytes=await readFile(input);
const server=createServer(async(req,res)=>{try{
 const path=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
 if(path==='/input.pdf'){res.setHeader('Content-Type','application/pdf');res.end(bytes);return;}
 if(path==='/'){res.setHeader('Content-Type','text/html');res.end('<!doctype html><title>Local PDF diagnostic</title>');return;}
 const file=resolve(web,'.'+path);if(!file.startsWith(web+sep)){res.writeHead(403).end();return;}
 res.setHeader('Content-Type',({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm'})[extname(file)]??'application/octet-stream');res.end(await readFile(file));
}catch{res.writeHead(404).end();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
const report={status:'failed',inputSha256:createHash('sha256').update(bytes).digest('hex'),cases:[],errors:[],artifacts:{}};
for (const name of ['rzpdf_web.wasm','rzpdf_web.mjs','client.js','worker-client.js','pdf-worker.js']) {
 report.artifacts[name]=createHash('sha256').update(await readFile(resolve(web,'dist',name))).digest('hex');
}
report.driverSha256=createHash('sha256').update(await readFile(fileURLToPath(import.meta.url))).digest('hex');
let browser;
try{
 browser=await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true});
 report.browser=await browser.version();
 const page=await browser.newPage();page.setDefaultTimeout(120000);
 page.on('pageerror',e=>report.errors.push(e.stack));
 await page.goto(`http://127.0.0.1:${server.address().port}`);
 const indices=(process.env.RZPDF_PRIVATE_PAGES??'0,1,2').split(',').map(Number);
 for(const transport of ['sync','worker'])for(const index of indices){
  const result=await page.evaluate(async ({index,transport})=>{
   const {PdfEngine}=await import('/dist/client.js');
   const stderr=[],traps=[];let module,engine;
   if(transport==='sync'){
    const {default:factory}=await import('/dist/rzpdf_web.mjs');
    module=await factory({printErr:line=>stderr.push(String(line))});
    for(const key of ['_rz_render_page','_rz_query']){const original=module[key];module[key]=(...args)=>{try{return original(...args);}catch(error){traps.push(String(error.stack??error));throw error;}};}
    engine=new PdfEngine(module);
   }else{
    const {createWorkerEngine}=await import('/dist/worker-client.js');
    engine=await createWorkerEngine();
   }
   const result={index,transport,stderr,traps,operations:{}};
   try{
    const doc=await engine.open(new Uint8Array(await(await fetch('/input.pdf')).arrayBuffer()),{mode:'compatible'});
    for(const operation of ['render','state','text','content','clearCaches','renderAgain','stateAgain'])try{
     const started=performance.now();
     const render=operation==='render'||operation==='renderAgain';
     const value=await(render?doc.render(index,36,{annotations:false,includeWidgets:false}):operation.startsWith('state')?doc.state():operation==='text'?doc.extractText(index):operation==='clearCaches'?doc.clearRenderCaches():doc.content(index));
     result.operations[operation]={ok:true,milliseconds:Math.round(performance.now()-started),...(render?{width:value.width,height:value.height,ink:value.pixels.some((v,i)=>i%4!==3&&v<240)}:operation==='text'?{characters:value.characters.length}:{})};
     if(operation==='render'){
      const canvas=document.createElement('canvas');canvas.width=value.width;canvas.height=value.height;
      canvas.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(value.pixels),value.width,value.height),0,0);
      result.png=canvas.toDataURL('image/png').split(',')[1];
     }
    }catch(e){result.operations[operation]={ok:false,code:e.code,message:e.message,resource:e.resource};}
    if(module)result.linearMemoryBytes=module.HEAPU8.byteLength;
    await doc.close();
   }finally{engine.dispose();}
   return result;
  },{index,transport});
  if(result.png){await writeFile(resolve(out,`${transport}-page-${index+1}.png`),Buffer.from(result.png,'base64'));delete result.png;}
  report.cases.push(result);console.log(JSON.stringify(result));
 }
 report.status=!report.errors.length&&report.cases.every(c=>Object.values(c.operations).every(v=>v.ok)&&!c.traps.length)?'passed':'failed';
 if(report.status!=='passed')process.exitCode=1;
}catch(e){report.errors.push(e.stack);process.exitCode=1;}
finally{await browser?.close();await new Promise(done=>server.close(done));await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2));}
