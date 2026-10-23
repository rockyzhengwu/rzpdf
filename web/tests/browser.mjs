// WA-T02/T03: real browser execution and native pixel comparison.
import { chromium } from 'playwright-core';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve, dirname, extname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repo = resolve(root, '..');
const out = resolve(repo, 'output/webassembly/wa03-browser');
await mkdir(out, {recursive:true});
const report = { task:'WA-03', acceptance_cases:['WA-T02','WA-T03'], status:'failed', p0_complete:false,
  generated_at:new Date().toISOString(), platform:process.platform, node:process.version,
  pixel_threshold:{max_channel_difference:0, differing_channels:0, overrides:"per-case explicit manifest threshold"}, cases:[], errors:[], requests:[] };
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
let browser, server;
try {
  const nativeBinary=resolve(repo,'target/debug/examples/rgba_reference');
  report.native_reference_sha256=hash(await readFile(nativeBinary));
  const sharedPaths=[...execFileSync('ldd',[nativeBinary],{encoding:'utf8'}).matchAll(/=> (\/\S+) /g)].map(m=>m[1]).sort();
  const shared=[];
  for(const path of sharedPaths)shared.push([path,hash(await readFile(path))]);
  report.native_shared_libraries=shared;
  const runtimeHash=hash(JSON.stringify(shared));
  report.asset_manifests={
    fonts:JSON.parse(await readFile(resolve(repo,'pdf/tests/fixtures/v07/manifest.json'))),
    codecs:JSON.parse(await readFile(resolve(repo,'pdf/tests/fixtures/v06/manifest.json'))),
    security:JSON.parse(await readFile(resolve(repo,'pdf/tests/fixtures/r4/manifest.json'))),
    bitmap_sha256:hash(await readFile(resolve(repo,'pdf/tests/fixtures/v06/mono.samples')))
  };
  const manifest = JSON.parse(await readFile(resolve(repo,'output/webassembly/fixtures/manifest.json')));
  if(process.env.RZPDF_PRIVATE_MANIFEST)Object.assign(manifest,JSON.parse(await readFile(process.env.RZPDF_PRIVATE_MANIFEST)));
  const routes = new Map([["/password-fixture",resolve(repo,"pdf/tests/fixtures/r4/password-user.pdf")],["/worker-heavy-fixture",resolve(repo,"output/webassembly/fixtures/worker-heavy.pdf")],["/geometry-font",resolve(repo,"pdf/tests/fixtures/v07/geometry.ttf")]]);
  for (const [id, fixture] of Object.entries(manifest)) {
    const threshold=fixture.pixel_threshold || {max_channel_difference:0,differing_channels:0};
    if(!Number.isInteger(threshold.max_channel_difference)||threshold.max_channel_difference<0||threshold.max_channel_difference>255||!Number.isInteger(threshold.differing_channels)||threshold.differing_channels<0)throw Error("invalid pixel threshold");
    fixture.pixel_threshold=threshold;
    const input = resolve(repo,fixture.path);
    if (hash(await readFile(input)) !== fixture.sha256) throw Error('fixture hash mismatch');
    const reference = resolve(out,id); await mkdir(reference,{recursive:true});
    const key=[fixture.sha256,fixture.font_sha256||'',report.native_reference_sha256,runtimeHash].join(':');
    let cached;
    try{cached=JSON.parse(await readFile(resolve(reference,'identity.json')));}catch{}
    let valid=cached?.key===key;
    if(valid)for(const [file,expected] of Object.entries(cached.files)){
      try{if(hash(await readFile(resolve(reference,file)))!==expected)valid=false;}catch{valid=false;}
    }
    if(!valid){
      execFileSync(nativeBinary,[input,reference],{env:{...process.env,RZPDF_SUBSTITUTE_FONT:fixture.font_path}});
      const saved=JSON.parse(await readFile(resolve(reference,'pages.json')));
      const files={};
      for(const file of ['pages.json',...saved.map((_,i)=>`${i}.rgba`)])files[file]=hash(await readFile(resolve(reference,file)));
      await writeFile(resolve(reference,'identity.json'),JSON.stringify({key,files},null,2));
    }
    if(fixture.font_path){
      const font=resolve(repo,fixture.font_path);
      if(hash(await readFile(font))!==fixture.font_sha256)throw Error('font hash mismatch');
      routes.set('/fonts/'+id,font);
    }
    const pages = JSON.parse(await readFile(resolve(reference,'pages.json')));
    fixture.pages = pages;
    if(id==='native-route') {
      const raw=await readFile(resolve(reference,'0.rgba'));
      const mono=await readFile(resolve(repo,'pdf/tests/fixtures/v06/mono.samples'));
      if(raw.length!==mono.length*4 || raw.some((v,i)=>v!==(i%4===3?255:mono[Math.floor(i/4)])))throw Error('native route bitmap oracle differs');
      report.native_bitmap_oracle=true;
    }
    if(id==='folded-patch') {
      const raw=await readFile(resolve(reference,'0.rgba'));
      for(let y=2;y<30;y++)for(let x=0;x<30;x++) {
        const expected=Math.round((1+Math.sqrt(x/30))*0.5*255),offset=(y*32+x)*4;
        if(Math.abs(raw[offset]-expected)>4 || raw[offset]!==raw[offset+1] || raw[offset+1]!==raw[offset+2] || raw[offset+3]!==255)throw Error('folded patch root/color oracle differs');
      }
      report.folded_patch_oracle=true;
    }
    if(id==='default-hidden-layer') {
      const raw=await readFile(resolve(reference,'0.rgba'));
      for(let y=0;y<32;y++)for(let x=0;x<32;x++) {
        const expected=x<16?[255,255,255,255]:[0,255,0,255],offset=(y*32+x)*4;
        if(expected.some((v,c)=>raw[offset+c]!==v))throw Error('default optional-content configuration was not applied');
      }
      report.default_layer_oracle=true;
    }
    if(id==='inline-ccitt') {
      const raw=await readFile(resolve(reference,'0.rgba'));
      const mono=await readFile(resolve(repo,'pdf/tests/fixtures/v06/mono.samples'));
      if(raw.length!==mono.length*4 || raw.some((v,i)=>v!==(i%4===3?255:mono[Math.floor(i/4)])))throw Error('inline CCITT sample oracle differs');
      report.inline_ccitt_oracle=true;
    }
    if(id==='gray-mask-group') {
      const raw=await readFile(resolve(reference,'0.rgba'));
      const first=[...raw.subarray((16*32+8)*4,(16*32+8)*4+4)],second=[...raw.subarray((16*32+24)*4,(16*32+24)*4+4)];
      if(![126,127,128,129].includes(first[0]) || first[0]!==first[1] || first[2]!==255 || first[3]!==255 || second.some(v=>v!==255))throw Error('Gray luminosity group oracle differs');
      report.gray_mask_group_oracle=true;
    }
    // Allow one LSB for alpha quantization plus one for Skia sRGB readback;
    // native/Wasm comparison below still requires zero differing channels.
    if(id==='type3-alpha') {
      const raw=await readFile(resolve(reference,'0.rgba'));
      const pixel=x=>[...raw.subarray((20*32+x)*4,(20*32+x)*4+4)];
      const first=pixel(5),overlap=pixel(12);
      if(first[0]!==255 || ![126,127,128,129].includes(first[1]) || first[1]!==first[2] || first[3]!==255 || JSON.stringify(first)!==JSON.stringify(overlap))throw Error('Type3 opacity must apply once across overlapping glyph paints');
      report.type3_alpha_oracle=true;
    }
    if(id==='fine-soft-mask' || id==='jpx-pdf-indices') {
      const raw=await readFile(resolve(reference,'0.rgba'));
      const palette=[[255,0,0,255],[0,255,0,255],[0,0,255,255],[255,255,255,255]];
      for(let y=0;y<32;y++)for(let x=0;x<32;x++) {
        const expected=id==='fine-soft-mask' ? palette[(Math.floor(x/2)+Math.floor(y/2))%2] : palette[Math.floor(x/8)];
        const offset=(y*32+x)*4;
        if(expected.some((v,c)=>raw[offset+c]!==v))throw Error(id+' source-pixel oracle differs');
      }
      report[id+'-oracle']=true;
    }
    if(id==='jpx-large-tile') {
      const raw=await readFile(resolve(reference,'0.rgba'));
      for(let y=0;y<32;y++)for(let x=0;x<32;x++) {
        const offset=(y*32+x)*4;
        const expected=[Math.floor(x/8)*85,Math.floor(y/8)*85,64,255];
        if(expected.some((value,channel)=>raw[offset+channel]!==value))throw Error('oversized JPX tile source-pixel oracle differs');
      }
      report.jpx_large_tile_oracle=true;
    }
    if(id==='compat-regressions') {
      const first=await readFile(resolve(reference,'0.rgba'));
      const pixel=(x,y)=>[...first.subarray((y*32+x)*4,(y*32+x)*4+4)];
      if(JSON.stringify(pixel(8,24))!==JSON.stringify([0,255,255,255]) || JSON.stringify(pixel(24,8))!==JSON.stringify([0,0,0,255]))throw Error('transparent page backdrop oracle differs');
      const fine=await readFile(resolve(reference,'1.rgba'));
      const colors=new Set();for(let i=0;i<fine.length;i+=4)colors.add([...fine.subarray(i,i+4)].join(','));
      if(!colors.has('0,0,255,255') || !colors.has('255,255,255,255'))throw Error('fine stencil resolution oracle differs');
      const thin=await readFile(resolve(reference,'5.rgba'));
      if(![11,12].some(y=>thin[(y*32+10)*4]<240))throw Error('subpixel stroke coverage oracle differs');
      report.compatibility_oracles={page_backdrop:true,fine_stencil:true,subpixel_stroke:true};
    }
    routes.set('/fixtures/'+id, input);
    for (let i=0;i<pages.length;i++) routes.set(`/references/${id}/${i}`,resolve(reference,`${i}.rgba`));
  }
  server = createServer(async (request,response) => {
    try {
      const path = decodeURIComponent(new URL(request.url,'http://localhost').pathname);
      let file = routes.get(path);
      if (!file) {
        file = resolve(root,'.'+(path==='/'?'/index.html':path));
        if (!file.startsWith(root+sep)) throw Error('invalid path');
      }
      const bytes = await readFile(file);
      const type = {'.html':'text/html','.css':'text/css','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.json':'application/json'}[extname(file)] || 'application/octet-stream';
      response.writeHead(200,{'Content-Type':type,'Cache-Control':'no-store'}); response.end(bytes);
    } catch { response.writeHead(404); response.end(); }
  });
  await new Promise(done=>server.listen(0,'127.0.0.1',done));
  const origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM,headless:true});
  report.browser = browser.version();
  const page = await browser.newPage();
  page.on('console',m=>{if(m.text().startsWith('checking '))console.log(m.text());});
  page.on('pageerror',e=>report.errors.push(String(e)));
  page.on('request',r=>report.requests.push(r.url()));
  await page.addInitScript(()=>{
    window.__rzWorkerTrace=[];
    const original=Worker.prototype.postMessage;
    Worker.prototype.postMessage=function(message,...rest){
      if(message?.operation){window.__rzWorkerTrace.push({operation:message.operation,session:message.session,id:message.id});if(window.__rzWorkerTrace.length>256)window.__rzWorkerTrace.shift();}
      const result=original.call(this,message,...rest);
      if(['render','renderRegion'].includes(message?.operation)&&window.__rzCancelNextRender){window.__rzCancelNextRender=false;queueMicrotask(()=>document.querySelector('#cancel').click());}
      return result;
    };
  });
  await page.goto(origin);
  await page.waitForFunction(()=>!document.querySelector('#pdf').disabled,{timeout:120000});
  await page.locator('#pdf').setInputFiles(resolve(repo,manifest.composite.path));
  await page.waitForFunction(()=>document.querySelector('#status').textContent.includes('/ 1'),{timeout:60000});
  report.reader_worker = await page.evaluate(()=>({main_thread_wasm_resources:performance.getEntriesByType('resource').filter(r=>r.name.endsWith('.wasm')).length}));
  if(report.reader_worker.main_thread_wasm_resources)throw Error('reader initialized Wasm on the main thread');
  report.canvas = await page.evaluate(async () => {
    const expected = new Uint8Array(await (await fetch('/references/composite/0')).arrayBuffer());
    const c = document.querySelector('canvas');
    const actual = c.getContext('2d').getImageData(0,0,c.width,c.height).data;
    if(actual.length!==expected.length || actual.some((v,i)=>v!==expected[i]))throw Error('demo canvas pixels differ');
    return {width:c.width,height:c.height,exact_match:true};
  });
  await page.screenshot({path:resolve(out,'demo.png'),fullPage:true});
  const checked = await page.evaluate(async ({manifest,diagnostic}) => {
    const {createEngine} = await import('/dist/client.js');
    const engine = await createEngine();
    const results = [];
    for (const [id,fixture] of Object.entries(manifest)) {
      const bytes = new Uint8Array(await (await fetch('/fixtures/'+id)).arrayBuffer());
      if(fixture.font_path) engine.setSubstituteFont(new Uint8Array(await (await fetch('/fonts/'+id)).arrayBuffer()));
      else engine.setSubstituteFont();
      const doc = engine.open(bytes);
      try {
        if (doc.pageCount !== fixture.pages.length) throw Error('page count mismatch');
        for (let index=0;index<doc.pageCount;index++) {
          if(index%25===0)console.log(`checking ${id}: ${index+1}/${doc.pageCount}`);
          const image = doc.render(index);
          const expected = new Uint8Array(await (await fetch(`/references/${id}/${index}`)).arrayBuffer());
          if (image.width!==fixture.pages[index].width || image.height!==fixture.pages[index].height || image.pixels.length!==expected.length) throw Error('RGBA dimensions mismatch');
          let differences=0,maxDifference=0,nonwhite=0;
          for (let i=0;i<expected.length;i++) {
            const d=Math.abs(image.pixels[i]-expected[i]);
            if (d) differences++; maxDifference=Math.max(maxDifference,d);
            if(i%4!==3 && image.pixels[i]<240)nonwhite++;
          }
          const sha = [...new Uint8Array(await crypto.subtle.digest('SHA-256',image.pixels))].map(x=>x.toString(16).padStart(2,'0')).join('');
          results.push({id,page:index,width:image.width,height:image.height,differences,maxDifference,nonwhite,rgba_sha256:sha,source_sha256:fixture.sha256,font_sha256:fixture.font_sha256 || null,pixel_threshold:fixture.pixel_threshold});
          if ((!diagnostic && (maxDifference>fixture.pixel_threshold.max_channel_difference || differences>fixture.pixel_threshold.differing_channels)) || !nonwhite) throw Error(`pixel check failed ${id}/${index}: ${differences}, max ${maxDifference}`);
        }
      } finally {doc.close();}
      let rejected=false; try {doc.render();} catch {rejected=true;}
      if (!rejected) throw Error('closed document was accepted');
    }
    const features = engine.open(new Uint8Array(await (await fetch('/fixtures/web-features')).arrayBuffer()));
    let invalidPage=false;try{features.render(-1);}catch(e){invalidPage=e.code==='invalid_argument' && e.operation==='render';}
    if(!invalidPage)throw Error('TS argument errors were not structured');
    const first = features.extractText(0);
    if(first.characters.map(c=>c.unicode ?? "\uFFFD").join('')!=='Web text')throw Error('hidden text leaked into extraction');
    if(first.characters.some(c=>!c.emToDisplay || !c.advance))throw Error('missing character geometry');
    const hits=features.searchText("page");
    if(hits.matches.length!==1 || hits.matches[0].page!==1 || hits.truncated || features.searchText("HIDDEN").matches.length)throw Error("cross-page/visible search differs");
    const second=features.pageInfo(1);
    if(second.width!==100 || second.height!==160 || second.rotation!==90)throw Error('rotated page info differs');
    const outlines=features.outlines(),links=features.links(0);
    if(outlines.length!==1 || outlines[0].target.page!==1 || links.length!==3 || links[0].target.page!==1 || links[1].target.uri!=='https://example.org/')throw Error('navigation metadata differs');
    for(let i=0;i<12;i++)features.extractText(0);
    features.close();
    const encrypted=new Uint8Array(await (await fetch('/password-fixture')).arrayBuffer());
    let required=false,incorrect=false;
    try{engine.open(encrypted);}catch(e){required=e.code==='password_required' && e.operation==='open';}
    try{engine.open(encrypted,{password:'never-log-this-password'});}catch(e){incorrect=e.code==='password_invalid' && !e.message.includes('never-log');}
    if(!required || !incorrect)throw Error('password errors were not structured');
    const unlocked=engine.open(encrypted,{password:'synthetic-user'});
    unlocked.render();unlocked.close();
    const m=engine.module;
    if (m._rz_alloc(0)!==0 || m._rz_render(0,0,NaN)!==0) throw Error('invalid input was accepted');
    const h=m._rz_alloc(8);
    if(!h || m._rz_page_count(h)!==0 || m._rz_release(h)!==1 || m._rz_release(h)!==0) throw Error('handle validation failed');
    let bad=false;try {engine.open(new Uint8Array([1,2,3]));} catch {bad=true;}
    if(!bad)throw Error('malformed PDF accepted');
    // Failure must not poison the session or leak all available slots.
    for(let i=0;i<12;i++) { const d=engine.open(new Uint8Array(await (await fetch('/fixtures/composite')).arrayBuffer()));d.render();d.close(); }
    const fresh=await createEngine();
    const f=fresh.module;
    const doc=fresh.open(new Uint8Array(await (await fetch('/fixtures/composite')).arrayBuffer()));
    const owned=doc.render().pixels;const snapshot=owned.slice();
    const before=f.HEAPU8.byteLength;const old=f.HEAPU8.buffer;const handles=[];
    try {
      for(let i=0;i<4;i++){const h=f._rz_alloc(8*1024*1024);if(!h)throw Error('growth allocation failed');handles.push(h);}
      if(f.HEAPU8.byteLength<=before || old===f.HEAPU8.buffer)throw Error('memory growth was not exercised');
      if(owned.some((v,i)=>v!==snapshot[i]))throw Error('owned pixels changed after memory growth');
    }finally { for(const h of handles)fresh.release(h); }
    doc.render();doc.close();
    if(f._rz_alloc(64*1024*1024+1)!==0)throw Error('oversized input accepted');
    const slots=[];
    try {
      for(let i=0;i<8;i++){const h=f._rz_alloc(1);if(!h)throw Error('slot leak');slots.push(h);}
      if(f._rz_alloc(1)!==0)throw Error('handle cap not enforced');
    }finally{for(const h of slots)fresh.release(h);}
    const disposable=await createEngine(),disposeDoc=disposable.open(new Uint8Array(await (await fetch('/fixtures/composite')).arrayBuffer()));
    const disposePixels=disposeDoc.render().pixels,disposeSnapshot=disposePixels.slice();
    disposable.setSubstituteFont(new Uint8Array(await (await fetch("/geometry-font")).arrayBuffer()));
    if(!disposable.statistics().handles || !disposable.statistics().retainedBytes)throw Error('dispose setup did not retain a document');
    disposable.dispose();disposable.dispose();
    if(disposable.module._rz_handle_count() || disposable.module._rz_retained_bytes())throw Error('native dispose leaked tracked resources');
    let disposed=false;try{disposeDoc.render();}catch(error){disposed=error.code==='invalid_handle';}
    if(!disposed || disposePixels.some((v,i)=>v!==disposeSnapshot[i]))throw Error('dispose ownership contract failed');
    disposeDoc.close();engine.dispose();fresh.dispose();
    return {pages:results,lifecycle:{close_reopen:12,memory_before:before,memory_after:f.HEAPU8.byteLength,
      owned_copy_survived_growth:true,handle_cap:true,malformed_recovery:true}};
  },{manifest,diagnostic:process.env.RZPDF_DIAGNOSTIC === "1"});
  report.worker = await page.evaluate(async ({manifest})=>{
    const {createWorkerEngine}=await import('/dist/worker-client.js');
    const engine=await createWorkerEngine(),results=[];
    const bytes=async route=>new Uint8Array(await (await fetch(route)).arrayBuffer());
    const rejects=async(promise,code)=>{try{await promise;}catch(error){if(error.code===code)return error;throw error;}throw Error('expected '+code);};
    const raw=new Worker('/dist/pdf-worker.js',{type:'module'}),rawSession=crypto.randomUUID();
    const rawRequest=(id,operation,args=[],session=rawSession,document)=>new Promise((resolve,reject)=>{
      const timer=setTimeout(()=>reject(Error('raw Worker protocol timed out')),30000);
      const listen=event=>{const r=event.data;if(r.id===id&&r.session===session&&r.kind!=='started'){clearTimeout(timer);raw.removeEventListener('message',listen);resolve(r);}};
      raw.addEventListener('message',listen);raw.postMessage({protocol:2,session,id,operation,args,document});
    });
    if((await rawRequest(0,'initialize')).kind!=='ready')throw Error('raw initialization failed');
    raw.postMessage(null);
    const wrong=await rawRequest(1,'statistics',[],'wrong-session');
    if(wrong.error?.code!=='invalid_handle'||wrong.error?.fatal)throw Error('cross-session request accepted');
    const bad=await rawRequest(1,'open',[new Uint8Array([1]),{password:17}]);
    if(bad.error?.code!=='invalid_argument'||bad.error?.fatal)throw Error('bad arguments poisoned Worker');
    if((await rawRequest(1,'statistics')).value.handles!==0)throw Error('malformed request retained state');
    if((await rawRequest(1,'statistics')).error?.code!=='invalid_handle')throw Error('replayed task accepted');
    const opened=(await rawRequest(2,'open',[await bytes('/fixtures/web-features'),{}])).value;
    if((await rawRequest(3,'pageInfo',[0],rawSession,opened.document)).value.width!==160)throw Error('protocol failure prevented later valid request');
    await rawRequest(4,'close',[],rawSession,opened.document);
    if((await rawRequest(5,'pageInfo',[0],rawSession,opened.document)).error?.code!=='invalid_handle')throw Error('released protocol document was accepted');
    if((await rawRequest(6,'statistics')).value.handles!==0)throw Error('raw Worker close leaked');
    raw.terminate();
    const workerCases=Object.entries(manifest);
    for(const [id,fixture] of workerCases) {
      console.log('checking Worker '+id);
      await engine.setSubstituteFont(fixture.font_path?await bytes('/fonts/'+id):undefined);
      const source=await bytes('/fixtures/'+id),length=source.length;
      const doc=await engine.open(source);
      if(source.length!==length)throw Error('caller PDF buffer was detached');
      try {
        for(let page=0;page<doc.pageCount;page++) {
          const image=await doc.render(page),expected=await bytes(`/references/${id}/${page}`);
          if(image.width!==fixture.pages[page].width||image.height!==fixture.pages[page].height||image.pixels.length!==expected.length||image.pixels.some((v,i)=>v!==expected[i]))throw Error('Worker native pixels differ: '+id+'/'+page);
          results.push({id,page,width:image.width,height:image.height,differing_channels:0});
        }
      }finally{await doc.close();await doc.close();}
    }
    await engine.setSubstituteFont(await bytes("/geometry-font"));
    if(!(await engine.statistics()).retainedBytes)throw Error("Worker font was not retained");
    await engine.setSubstituteFont();
    const empty=await engine.statistics();if(empty.handles||empty.retainedBytes||empty.documents)throw Error('Worker close leaked tracked resources');
    const source=await bytes('/fixtures/web-features'),first=await engine.open(source),second=await engine.open(source);
    if((await first.extractText(0)).characters.map(c=>c.unicode??'').join('')!=='Web text'||(await second.searchText('page')).matches[0].page!==1)throw Error('Worker text/search differs');
    if((await first.outlines())[0].target.page!==1||(await second.links(0)).length!==3)throw Error('Worker navigation differs');
    await first.close();await second.close();
    const heavy=await engine.open(await bytes('/worker-heavy-fixture')),other=await engine.open(source),originalSession=engine.sessionId;
    const activeController=new AbortController(),queuedController=new AbortController();
    let startedResolve;const started=new Promise(resolve=>{startedResolve=resolve;});
    const start=event=>{if(event.detail.operation==='render'){engine.removeEventListener('taskstart',start);startedResolve();}};
    engine.addEventListener('taskstart',start);
    let completed=false;
    const active=heavy.render(0,144,{signal:activeController.signal});const activeOutcome=active.then(()=>{completed=true;return null;},error=>error);
    await started;
    const queued=other.pageInfo(0,{signal:queuedController.signal});queuedController.abort();await rejects(queued,'cancelled');
    if(engine.sessionId!==originalSession)throw Error('queued abort killed active session');
    let ticks=0;const heartbeat=setInterval(()=>ticks++,5);
    await new Promise(resolve=>setTimeout(resolve,50));clearInterval(heartbeat);
    if(completed||ticks<3)throw Error('heavy render did not remain active while main thread ticked');
    activeController.abort();const cancelled=await activeOutcome;
    if(cancelled?.code!=='cancelled'||!cancelled.taskId||cancelled.session!==originalSession)throw Error('active cancellation failed');
    if(engine.sessionId===originalSession)throw Error('hard cancel kept the old session');
    await rejects(other.pageInfo(0),'invalid_handle');await heavy.close();await other.close();
    const reopened=await engine.open(await bytes('/fixtures/composite'));
    const owned=(await reopened.render()).pixels,snapshot=owned.slice();
    await reopened.close();
    for(let i=0;i<12;i++){const d=await engine.open(await bytes('/fixtures/composite'));await d.render();await d.close();}
    if(owned.some((v,i)=>v!==snapshot[i]))throw Error('transferred pixels changed after later tasks');
    const after=await engine.statistics();if(after.handles||after.retainedBytes||after.documents)throw Error('reopen leaked Worker resources');
    const closing=await engine.open(source),inFlight=closing.render(),closed=inFlight.then(()=>null,error=>error);
    await closing.close();if((await closed)?.code!=='cancelled')throw Error('close did not cancel its active task');
    const doc=await engine.open(source),render=doc.render(),outcome=render.then(()=>null,error=>error);
    engine.dispose();engine.dispose();if((await outcome)?.code!=='cancelled')throw Error('dispose did not cancel');
    await rejects(engine.open(source),'invalid_handle');
    const faultEngine=await createWorkerEngine({workerUrl:new URL('/tests/fixtures/trap-worker.mjs',location.href)});
    const faultSource=await bytes('/fixtures/composite'),faultDoc=await faultEngine.open(faultSource,{mode:'compatible'}),faultSession=faultEngine.sessionId;
    await rejects(faultDoc.render(),'internal');
    if(faultEngine.sessionId===faultSession)throw Error('Wasm trap did not invalidate Worker');
    await rejects(faultDoc.pageInfo(0),'invalid_handle');await faultDoc.close();
    const afterTrap=await faultEngine.open(faultSource),trapPixels=(await afterTrap.render()).pixels,trapExpected=await bytes('/references/composite/0');
    if(trapPixels.some((v,i)=>v!==trapExpected[i]))throw Error('render after trap/reopen differs');
    await afterTrap.close();faultEngine.dispose();
    return {status:'passed',pages:results,controlled_real_wasm_trap_recovery:true,protocol_wrong_session_replay_and_malformed_rejected:true,main_thread_heartbeat_ticks:ticks,queued_abort_preserves_session:true,active_abort_invalidates_session:true,close_cancels_active:true,dispose_cancels:true,stale_handles_rejected:true,tracked_resources_after_close:after,transferred_pixels_owned:true,close_reopen:12};
  },{manifest});
  report.cases=checked.pages;report.lifecycle=checked.lifecycle;
  console.log('checking reader UI: opening');
  await page.locator('#pdf').setInputFiles(resolve(repo,manifest['web-features'].path));
  await page.waitForFunction(()=>document.querySelector('#status').textContent.includes('/ 2'));
  if(await page.locator('#text-layer').textContent()!=='Web text')throw Error('demo text selection layer differs');
  const textBox=await page.locator('#page-view').boundingBox();
  await page.mouse.move(textBox.x+10,textBox.y+25);await page.mouse.down();
  await page.mouse.move(textBox.x+70,textBox.y+25,{steps:15});await page.mouse.up();
  const selected=await page.evaluate(()=>window.getSelection().toString());
  if(selected!=='Web text')throw Error('pointer text selection differs: '+JSON.stringify(selected));
  await page.evaluate(()=>window.getSelection().removeAllRanges());
  if(await page.locator('#links a').count()!==2)throw Error('unsafe URI was not filtered');
  console.log('checking reader UI: outline');
  await page.locator('[data-tab=outline]').click();await page.locator('#outline button').click();
  await page.waitForFunction(()=>document.querySelector('#page').value==='2' && document.querySelector('#status').textContent.startsWith('第 2 /'));
  console.log('checking reader UI: search');
  await page.locator('[data-tab=search]').click();await page.locator('#query').fill('Web');await page.locator('#find').click();
  await page.locator('#results button').click();
  await page.waitForFunction(()=>document.querySelector('#page').value==='1' && document.querySelectorAll('#highlights polygon').length===3);
  console.log('checking reader UI: links');
  await page.locator('#links a[aria-label="转到第 2 页"]').click();
  await page.waitForFunction(()=>document.querySelector('#page').value==='2' && document.querySelector('#status').textContent.startsWith('第 2 /'));
  console.log('checking reader UI: password');
  await page.locator('#pdf').setInputFiles(resolve(repo,'pdf/tests/fixtures/r4/password-user.pdf'));
  await page.locator('#unlock').waitFor({state:'visible'});
  await page.locator('#password').fill('wrong-password');await page.locator('#unlock').click();
  await page.waitForFunction(()=>document.querySelector('#status').textContent.includes('invalid document password'));
  await page.locator('#password').fill('synthetic-user');await page.locator('#unlock').click();
  await page.waitForFunction(()=>document.querySelector('#status').textContent.includes('/ 1'));
  if(await page.locator('#password').inputValue()!=='')throw Error('password field was retained');
  if(!await page.locator('#find').isDisabled() || await page.locator('#text-layer').textContent()!=='')throw Error('copy restriction was not applied in reader UI');
  console.log('checking reader UI: rapid navigation and cancellation');
  await page.locator('#pdf').setInputFiles(resolve(repo,manifest['web-features'].path));
  await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 / 2'));
  await page.evaluate(()=>{for(const value of ['2','1','2','1']){document.querySelector('#page').value=value;document.querySelector('#render').click();}});
  await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 / 2'));
  await page.waitForFunction(()=>document.querySelector('#cancel').disabled);
  if(await page.locator('#text-layer').textContent()!=='Web text')throw Error('late navigation replaced the newest page');
  await page.locator('#zoom').fill('200');
  await page.evaluate(()=>{window.__rzCancelNextRender=true;});
  await page.locator('#pdf').setInputFiles(resolve(repo,'output/webassembly/fixtures/worker-heavy.pdf'));
  await page.waitForFunction(()=>document.querySelector('#status').textContent.includes('会话已结束'));
  await new Promise(resolve=>setTimeout(resolve,100));
  const cancelledUI=await page.evaluate(()=>({width:document.querySelector('canvas').width,text:document.querySelector('#text-layer').textContent,status:document.querySelector('#status').textContent}));
  if(cancelledUI.width!==1 || cancelledUI.text || !cancelledUI.status.includes('会话已结束'))throw Error('late result replaced cancelled UI');
  await page.locator('#zoom').fill('10');await page.locator('#reopen').click();
  await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 / 1'));
  if(await page.locator('canvas').evaluate(c=>c.width)!==60)throw Error('explicit reopen after cancellation failed');
  await page.evaluate(()=>{window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:true}));window.dispatchEvent(new PageTransitionEvent('pageshow',{persisted:true}));});
  await page.waitForFunction(()=>!document.querySelector('#pdf').disabled&&document.querySelector('#status').textContent.includes('已恢复'));
  await page.locator('#reopen').click();await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('第 1 / 1'));
  report.reader_ui={selectable_text:true,search_highlights:true,outline_navigation:true,internal_link_navigation:true,unsafe_uri_filtered:true,password_retry:true,copy_permission:true,rapid_navigation_latest_wins:true,cancel_clears_late_results:true,explicit_reopen:true,page_transition_session_recovery:true};
  report.cases=checked.pages;report.lifecycle=checked.lifecycle;
  report.web_features={text:true,search:true,hidden_text_filtered:true,character_geometry:true,rotated_page_info:true,navigation:true,password_retry:true,structured_errors:true};
  if (report.errors.length) throw Error('uncaught browser errors');
  if (report.requests.some(url=>!url.startsWith(origin+'/'))) throw Error('unexpected external request');
  report.status=process.env.RZPDF_DIAGNOSTIC === '1' ? 'diagnostic' : 'passed'; report.p0_complete=report.status==='passed';
  if(report.status==='diagnostic')process.exitCode=2;
  report.pending=['WA-04/05 stable ABI and full lifecycle matrix','WA-06/07 complete font assets and fallback','WA-09 exact fit/rotation/tiling','WA-10 annotations/forms/edit/save','WA-11 Firefox/WebKit and corpus matrix'];
  const build=JSON.parse(await readFile(resolve(repo,'output/webassembly/wa03-build.json')));
  if(build.status!=='passed')throw Error('Wasm build report is not passed');
  report.source=build.source;report.toolchain_lock=build.toolchain_lock;
  for(const [path,expected] of Object.entries(build.artifacts))if(hash(await readFile(path))!==expected)throw Error('build artifact changed');
  report.artifacts={};
  for(const name of ['rzpdf_web.mjs','rzpdf_web.wasm','client.js','demo.js','worker-client.js','protocol.js','pdf-worker.js','text.js','interaction.js','editor.js','geometry.js','content-editor.js'])report.artifacts[name]=hash(await readFile(resolve(root,'dist',name)));
} catch(error) { report.status='failed';report.p0_complete=false;report.errors.push(String(error));report.failure_stack=error.stack;
  try { const active=browser?.contexts()[0]?.pages()[0];if(active){await active.screenshot({path:resolve(out,'failure.png'),fullPage:true});report.failure_ui=await active.evaluate(()=>({status:document.querySelector('#status')?.textContent,page:document.querySelector('#page')?.value,results:document.querySelector('#results')?.textContent,textRects:[...document.querySelectorAll('#text-layer span')].slice(0,16).map(s=>{const r=s.getBoundingClientRect();return {text:s.textContent,x:r.x,y:r.y,w:r.width,h:r.height,transform:s.style.transform};}),selection:{anchor:window.getSelection()?.anchorNode?.textContent,focus:window.getSelection()?.focusNode?.textContent}}));} } catch {}
  process.exitCode=1; }
finally {
  await browser?.close(); await new Promise(done=>server?server.close(done):done());
  await writeFile(resolve(out,'report.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify({status:report.status,pages:report.cases.length,errors:report.errors,report:resolve(out,'report.json')}));
}
