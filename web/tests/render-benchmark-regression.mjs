// Correctness only: native timing belongs to scripts/benchmark_pdf.py.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {resolve, extname, sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';

const root = fileURLToPath(new URL('../../', import.meta.url));
const corpus = resolve(process.env.RZPDF_BENCHMARK_CORPUS ?? resolve(root, 'output/benchmark-corpus'));
const native = resolve(process.env.RZPDF_BENCHMARK_NATIVE ?? resolve(root, 'output/benchmark-run/quality.json'));
const output = resolve(root, 'output/webassembly/render-benchmark-regression');
const web = resolve(root, 'web');
await mkdir(output, {recursive: true});
const manifest = JSON.parse(await readFile(resolve(corpus, 'manifest.json')));
const expected = JSON.parse(await readFile(native)).map(row => row.validation
  ? {case: row.case, dpi: row.dpi, ...row.validation.rzpdf} : row);
const inkStats = (pixels, width, height) => {
  let count=0, left=width, top=height, right=-1, bottom=-1;
  for(let y=0;y<height;y++)for(let x=0;x<width;x++) {
    const i=(y*width+x)*4;
    if(Math.min(pixels[i],pixels[i+1],pixels[i+2])<245) {
      count++;left=Math.min(left,x);right=Math.max(right,x);top=Math.min(top,y);bottom=Math.max(bottom,y);
    }
  }
  return {count,bounds:[left,top,right,bottom]};
};
const cases = [];
for (const item of manifest.cases) {
  for (const dpi of [72, 144]) {
    const row = expected.find(r => r.case === item.name && r.dpi === dpi);
    assert.equal(row?.status, 'ok', `missing native output: ${item.name}/${dpi}`);
    cases.push({name: item.name, dpi, nonblank: item.nonblank,
      // Native uses full-resolution JPEG where it fits; the browser's default
      // image budget deliberately selects a reduced JPEG for the large scan.
      exact: item.browser_exact ?? (item.name !== 'jpeg-scan' && !/^(text-|ttf-|cjk-)/.test(item.name)),
      text: item.browser_text ?? /^(text-|ttf-|cjk-)/.test(item.name),
      ink: inkStats(await readFile(row.outputs[0].file), row.width, row.height),
      width: row.width, height: row.height,
      sha256: createHash('sha256').update(await readFile(row.outputs[0].file)).digest('hex')});
  }
}
const server = createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    const name = manifest.cases.find(c => pathname === `/corpus/${c.name}.pdf`)?.name;
    const path = name ? resolve(corpus, `${name}.pdf`) : resolve(web, '.' + (pathname === '/' ? '/index.html' : pathname));
    if (!name && !path.startsWith(web + sep)) { response.writeHead(403).end(); return; }
    response.setHeader('Content-Type', ({'.mjs':'text/javascript','.js':'text/javascript','.wasm':'application/wasm','.html':'text/html'})[extname(path)] ?? 'application/octet-stream');
    response.end(await readFile(path));
  } catch { response.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const report = {status: 'failed', checks: [], errors: [], artifacts: {}};
let browser;
try {
  for (const name of ['rzpdf_web.wasm','rzpdf_web.mjs','client.js','worker-client.js','pdf-worker.js']) {
    report.artifacts[name] = createHash('sha256').update(await readFile(resolve(web,'dist',name))).digest('hex');
  }
  browser = await chromium.launch({executablePath: process.env.RZPDF_CHROMIUM, headless: true});
  report.browser = await browser.version();
  const page = await browser.newPage();
  page.on('pageerror', error => report.errors.push(error.stack));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  report.checks = await page.evaluate(async cases => {
    const {createEngine} = await import('/dist/client.js');
    const {createWorkerEngine} = await import('/dist/worker-client.js');
    const results = [];
    const directHashes = new Map();
    const inkStats = (pixels, width, height) => {
  let count=0, left=width, top=height, right=-1, bottom=-1;
  for(let y=0;y<height;y++)for(let x=0;x<width;x++) {
    const i=(y*width+x)*4;
    if(Math.min(pixels[i],pixels[i+1],pixels[i+2])<245) {
      count++;left=Math.min(left,x);right=Math.max(right,x);top=Math.min(top,y);bottom=Math.max(bottom,y);
    }
  }
  return {count,bounds:[left,top,right,bottom]};
};

    for (const transport of ['sync', 'worker']) {
      const engine = await (transport === 'sync' ? createEngine() : createWorkerEngine());
      try {
        for (const item of cases) {
          const result = {name: item.name, dpi: item.dpi, transport, exact: item.exact};
          let doc;
          try {
            doc = await engine.open(new Uint8Array(await (await fetch(`/corpus/${item.name}.pdf`)).arrayBuffer()), {mode: 'compatible'});
            const image = await doc.render(0, item.dpi, {annotations: false, includeWidgets: false});
            const sha256 = [...new Uint8Array(await crypto.subtle.digest('SHA-256', image.pixels))].map(v=>v.toString(16).padStart(2,'0')).join('');
            if (image.width !== item.width || image.height !== item.height) throw Error('dimensions differ from native');
            if (item.nonblank && !image.pixels.some((v,i)=>i%4!==3 && v<240)) throw Error('unexpected blank image');
            if (item.exact && sha256 !== item.sha256) throw Error('pixels differ from native');
            // CoreText and FreeType use different bitmap glyph hinting. Require
            // bounded text geometry/coverage rather than identical antialiasing.
            const ink = inkStats(image.pixels,image.width,image.height);
            if(item.text && (ink.bounds.some((v,i)=>Math.abs(v-item.ink.bounds[i])>1)
              || ink.count/item.ink.count<0.75 || ink.count/item.ink.count>1.3334)) throw Error('text geometry/coverage differs from native');
            const key=`${item.name}/${item.dpi}`;
            if(transport==='sync')directHashes.set(key,sha256);
            else if(sha256!==directHashes.get(key))throw Error('Worker output differs from direct Wasm');
            const repeated = await doc.render(0, item.dpi, {annotations: false, includeWidgets: false});
            const digest = async pixels => [...new Uint8Array(await crypto.subtle.digest('SHA-256', pixels))].map(v=>v.toString(16).padStart(2,'0')).join('');
            if(await digest(repeated.pixels)!==sha256)throw Error('cached replay changed pixels');
            const caches=await doc.cacheStatistics();
            if(caches.renderSessionEstimatedBytes>caches.renderSessionByteLimit)throw Error('session exceeded cache budget');
            if(item.text&&caches.renderSessionFontHits===0)throw Error('embedded font not reused across requests');
            if(['jpeg-small','flate-image'].includes(item.name)&&caches.renderSessionImageHits===0)throw Error('image not reused across requests');
            if(item.text&&caches.renderSessionFontManagerEntries!==1)throw Error('font manager lifetime not shared');
            const cleared=await doc.clearRenderCaches();
            if(cleared.renderSessionEstimatedBytes!==0||cleared.renderSessionFontEntries!==0||cleared.renderSessionFontManagerEntries!==0
              ||cleared.renderSessionImageEntries!==0||cleared.renderSessionGeometryEntries!==0)throw Error('session clear retained entries');
            const rebuilt=await doc.render(0,item.dpi,{annotations:false,includeWidgets:false});
            if(await digest(rebuilt.pixels)!==sha256)throw Error('cache clear changed pixels');
            result.cacheStatistics=caches;
            result.ink=ink;
            await doc.state();
            result.sha256 = sha256;
            result.passed = true;
          } catch (error) { result.passed = false; result.error = String(error.stack ?? error); }
          finally { await doc?.close(); }
          results.push(result);
        }
      } finally { await engine.dispose(); }
    }
    return results;
  }, cases);
  assert.equal(report.errors.length, 0);
  assert.equal(report.checks.length, cases.length * 2);
  assert.ok(report.checks.every(r => r.passed), JSON.stringify(report.checks.filter(r=>!r.passed)));
  report.status = 'passed';
} catch (error) { report.errors.push(error.stack); process.exitCode = 1; }
finally {
  await browser?.close();
  await new Promise(done => server.close(done));
  await writeFile(resolve(output,'report.json'), JSON.stringify(report,null,2) + '\n');
  console.log(JSON.stringify({status: report.status, checks: report.checks.length, errors: report.errors,
    report: resolve(output,'report.json')}));
}
