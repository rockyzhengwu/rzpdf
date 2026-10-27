// Actual WASM/Worker policy, cache identity and tiled-output acceptance.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {resolve, extname, basename} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';

const root = fileURLToPath(new URL('../../', import.meta.url));
const fixtures = resolve(root, 'output/webassembly/target-decode-fixtures');
const output = resolve(root, 'output/webassembly/image-decode-browser');
await mkdir(output, {recursive: true});
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const manifest = JSON.parse(await readFile(resolve(fixtures, 'manifest.json')));
assert.equal(hash(await readFile(resolve(fixtures, 'fit.pdf'))), manifest.sha256);
const report = {status: 'failed', checks: [], errors: [], fixture: manifest, artifacts: {}};
const server = createServer(async (request, response) => {
  try {
    const path = new URL(request.url, 'http://localhost').pathname;
    if (path === '/') { response.end('<!doctype html><title>Image decode policy</title>'); return; }
    const file = path === '/fit.pdf' ? resolve(fixtures, 'fit.pdf')
      : path.startsWith('/dist/') && basename(path) === path.slice(6)
        ? resolve(root, 'web/dist', basename(path)) : undefined;
    if (!file) { response.writeHead(404).end(); return; }
    response.setHeader('Content-Type', ({'.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm'})[extname(file)] ?? 'application/octet-stream');
    response.end(await readFile(file));
  } catch { response.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
let browser;
try {
  for (const file of ['rzpdf_web.wasm', 'rzpdf_web.mjs', 'client.js', 'worker-client.js', 'interaction.js']) {
    report.artifacts[file] = hash(await readFile(resolve(root, 'web/dist', file)));
  }
  browser = await chromium.launch({executablePath: process.env.RZPDF_CHROMIUM, headless: true});
  report.browser = browser.version();
  const page = await browser.newPage();
  page.on('pageerror', error => report.errors.push(error.stack));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  report.checks = await page.evaluate(async () => {
    const {createEngine} = await import('/dist/client.js');
    const {createWorkerEngine} = await import('/dist/worker-client.js');
    const equal = (a, b) => a.length === b.length && a.every((v, i) => v === b[i]);
    const check = (value, message) => { if (!value) throw Error(message); };
    const checks = [], snapshots = [];
    const bytes = new Uint8Array(await (await fetch('/fit.pdf')).arrayBuffer());
    for (const worker of [false, true]) {
      const engine = await (worker ? createWorkerEngine() : createEngine());
      try {
        const doc = await engine.open(bytes.slice());
        check((await doc.capabilities()).imageDecode.supportedPolicies.includes('target'), 'policy capability');
        const full = await doc.render(0, 72, {annotations: false});
        const fullStats = await doc.cacheStatistics();
        await doc.clearRenderCaches();
        const target = await doc.render(0, 72, {annotations: false, imageDecode: 'target'});
        const targetStats = await doc.cacheStatistics();
        check(target.width === full.width && target.height === full.height, 'policy changed page dimensions');
        check(targetStats.pageEstimatedBytes < fullStats.pageEstimatedBytes / 8, 'JPEG was not reduced before retention');
        const beforeHit = await doc.cacheStatistics();
        const repeat = await doc.render(0, 72, {annotations: false, imageDecode: 'target'});
        check(equal(target.pixels, repeat.pixels), 'target replay differs');
        check((await doc.cacheStatistics()).pageHits > beforeHit.pageHits, 'target cache miss');
        const beforeFull = await doc.cacheStatistics();
        const original = await doc.render(0, 72, {annotations: false});
        check(equal(full.pixels, original.pixels), 'target polluted default pixels');
        check((await doc.cacheStatistics()).pageMisses > beforeFull.pageMisses, 'policies share cache identity');
        const assembled = new Uint8Array(target.pixels.length);
        for (let y = 0; y < target.height; y += 64) for (let x = 0; x < target.width; x += 64) {
          const area = {x, y, width: Math.min(64, target.width - x), height: Math.min(64, target.height - y)};
          const tile = await doc.renderRegion(0, area, 72, {annotations: false, imageDecode: 'target'});
          for (let row = 0; row < area.height; row++) {
            assembled.set(tile.pixels.subarray(row * area.width * 4, (row + 1) * area.width * 4), ((y + row) * target.width + x) * 4);
          }
        }
        check(equal(assembled, target.pixels), 'target tiles differ from full output');
        const area = {x: 0, y: 0, width: 64, height: 64};
        const highTarget = await doc.renderRegion(0, area, 576, {annotations: false, imageDecode: 'target'});
        const highFull = await doc.renderRegion(0, area, 576, {annotations: false});
        check(equal(highTarget.pixels, highFull.pixels), 'native-resolution fallback differs');
        await doc.fontRequirements(0, {imageDecode: 'target'});
        await doc.clearRenderCaches();
        const cleared = await doc.render(0, 72, {annotations: false, imageDecode: 'target'});
        check(equal(cleared.pixels, target.pixels), 'cleared target differs');
        snapshots.push({full: full.pixels, target: target.pixels});
        checks.push({transport: worker ? 'worker' : 'sync', fullEstimatedBytes: fullStats.pageEstimatedBytes,
          targetEstimatedBytes: targetStats.pageEstimatedBytes, cacheIdentity: true, tileEquality: true, nativeResolutionFallback: true});
        await doc.close();
        check((await engine.statistics()).handles === 0, 'native handles leaked');
      } finally { engine.dispose(); }
    }
    check(equal(snapshots[0].full, snapshots[1].full), 'default transport mismatch');
    check(equal(snapshots[0].target, snapshots[1].target), 'target transport mismatch');
    return checks;
  });
  assert.equal(report.checks.length, 2);
  assert.deepEqual(report.errors, []);
  report.status = 'passed';
} catch (error) {
  report.errors.push(error.stack);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await new Promise(done => server.close(done));
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
}
