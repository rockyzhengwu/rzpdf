// Real Wasm/SDK/Worker acceptance for ordinary rewrites of linearized inputs.
// Build the pinned Wasm module and TypeScript first; no downloads or fixtures
// outside the committed project-authored corpus are required.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {resolve, extname, sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';

const root = fileURLToPath(new URL('../../', import.meta.url));
const web = resolve(root, 'web');
const fixtures = resolve(root, 'pdf/tests/fixtures/linearized');
const out = resolve(process.env.RZPDF_LINEARIZED_OUTPUT ?? resolve(root, 'output/webassembly/linearized-save'));
const names = ['two-pages.pdf', 'two-pages-objects.pdf', 'two-pages-overflow.pdf', 'two-pages-r6.pdf', 'two-pages-signed.pdf'];
await mkdir(out, {recursive: true});
const server = createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    const fixture = names.find(name => pathname === `/fixtures/${name}`);
    const path = fixture ? resolve(fixtures, fixture) : resolve(web, '.' + (pathname === '/' ? '/index.html' : pathname));
    if (!fixture && !path.startsWith(web + sep)) { response.writeHead(403).end(); return; }
    response.setHeader('Content-Type', ({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html','.pdf':'application/pdf'})[extname(path)] ?? 'application/octet-stream');
    response.end(await readFile(path));
  } catch { response.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const report = {status:'failed', checks:[], errors:[], artifacts:{}, fixtures:{}};
let browser;
try {
  const digest = bytes => createHash('sha256').update(bytes).digest('hex');
  for (const name of ['rzpdf_web.wasm','rzpdf_web.mjs','client.js','worker-client.js','pdf-worker.js']) {
    report.artifacts[name] = digest(await readFile(resolve(web, 'dist', name)));
  }
  for (const name of names) report.fixtures[name] = digest(await readFile(resolve(fixtures, name)));
  report.driverSha256 = digest(await readFile(fileURLToPath(import.meta.url)));
  browser = await chromium.launch({executablePath:process.env.RZPDF_CHROMIUM, headless:true});
  report.browser = await browser.version();
  const page = await browser.newPage();
  page.on('pageerror', error => report.errors.push(error.stack));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  const result = await page.evaluate(async names => {
    const {createEngine} = await import('/dist/client.js');
    const {createWorkerEngine} = await import('/dist/worker-client.js');
    const checks = [], saved = [];
    const direct = new Map();
    const check = (condition, message) => { if (!condition) throw Error(message); };
    const hash = async bytes => [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))].map(n => n.toString(16).padStart(2,'0')).join('');
    const reject = async (action, code) => {
      let caught;
      try { await action(); } catch (error) { caught = error; }
      check(caught?.code === code, `expected ${code}, received ${caught?.code}: ${caught?.message}`);
    };
    const snapshot = async doc => {
      const result = [];
      for (const dpi of [72,144]) for (const index of [0,1]) {
        const image = await doc.render(index, dpi);
        result.push({page:index, dpi, width:image.width, height:image.height, sha256:await hash(image.pixels)});
      }
      return result;
    };
    for (const transport of ['sync','worker']) {
      const worker = transport === 'worker';
      const engine = await (worker ? createWorkerEngine() : createEngine());
      try {
        for (const name of names) {
          try {
            const bytes = new Uint8Array(await (await fetch(`/fixtures/${name}`)).arrayBuffer());
            const encrypted = name === 'two-pages-r6.pdf';
            if (encrypted) {
              await reject(() => engine.open(bytes, {mode:'strict'}), 'password_required');
              await reject(() => engine.open(bytes, {mode:'strict',password:'wrong'}), 'password_invalid');
            }
            const doc = await engine.open(bytes, {mode:'strict', ...(encrypted ? {password:'synthetic-user'} : {})});
            const save = maxBytes => worker ? doc.save(maxBytes === undefined ? {} : {maxBytes}) : doc.save(maxBytes);
            const original = await snapshot(doc);
            if (name === 'two-pages-signed.pdf') {
              const state = await doc.state();
              await reject(() => save(), 'unsupported');
              check((await doc.state()).stamp.revision === state.stamp.revision, 'signed save refusal changed revision');
              await reject(() => doc.apply({operation:'rotatePage',stamp:state.stamp,page:0,degrees:90}), 'unsupported');
              check((await doc.state()).commitRevision === state.commitRevision, 'signed refusals committed changes');
              check(JSON.stringify(await snapshot(doc)) === JSON.stringify(original), 'signed refusals changed pixels');
              await doc.close();
              checks.push({transport,name,signedRewriteRefused:true,sessionSurvives:true});
              continue;
            }
            let state = await doc.state();
            await doc.apply({operation:'rotatePage',stamp:state.stamp,page:0,degrees:90});
            state = await doc.state();
            await doc.apply({operation:'addAnnotation',stamp:state.stamp,page:1,annotation:{kind:'Square',rect:[90,90,160,160],color:[0,0,1],contents:'linearized save regression'}});
            const edited = await snapshot(doc);
            check(edited.some((value,i) => value.sha256 !== original[i].sha256), 'edits must visibly change the pages');
            state = await doc.state();
            await reject(() => save(1), 'resource_limit');
            check((await doc.state()).stamp.revision === state.stamp.revision, 'failed save changed revision');
            const output = await save();
            check((await doc.state()).stamp.revision === state.stamp.revision, 'successful save changed revision');
            check(!new TextDecoder('latin1').decode(output.bytes).includes('/Linearized'), 'stale linearization declaration');
            if (encrypted) await reject(() => engine.open(output.bytes, {mode:'strict'}), 'password_required');
            const reopened = await engine.open(output.bytes, {mode:'strict', ...(encrypted ? {password:'synthetic-owner'} : {})});
            check((await reopened.state()).pageCount === 2, 'reopened page count');
            check((await reopened.state()).permissions.securityRevision === (encrypted ? 6 : null), 'protection changed');
            check((await reopened.annotations(1)).records.some(a => a.annotation?.contents === 'linearized save regression'), 'annotation edit was lost');
            const after = await snapshot(reopened);
            check(JSON.stringify(after) === JSON.stringify(edited), 'strict reopen changed edited pixels');
            const key = name;
            if (!worker) direct.set(key, JSON.stringify(after));
            else check(direct.get(key) === JSON.stringify(after), 'Worker differs from direct Wasm');
            saved.push({name:`${transport}-${name}`,bytes:Array.from(output.bytes)});
            checks.push({transport,name,original,edited,after,strictReopen:true,saveBudgetRecovery:true});
            await reopened.close(); await doc.close();
          } catch (error) { throw new Error(`${transport}/${name}: ${error.message}`, {cause:error}); }
        }
        const stats = await engine.statistics();
        check(stats.handles === 0 && stats.retainedBytes === 0 && (!worker || stats.documents === 0), `resources retained after close: ${JSON.stringify(stats)}`);
      } finally { engine.dispose(); }
    }
    return {checks,saved};
  }, names);
  report.checks = result.checks;
  for (const artifact of result.saved) await writeFile(resolve(out, artifact.name), new Uint8Array(artifact.bytes));
  assert.equal(report.checks.length, 10);
  assert.equal(report.errors.length, 0);
  report.status = 'passed';
} catch (error) { report.errors.push(error.stack); process.exitCode = 1; }
finally {
  await browser?.close();
  await new Promise(done => server.close(done));
  await writeFile(resolve(out, 'report.json'), JSON.stringify(report,null,2));
  console.log(JSON.stringify({status:report.status,cases:report.checks.length,errors:report.errors,report:resolve(out,'report.json')},null,2));
}
