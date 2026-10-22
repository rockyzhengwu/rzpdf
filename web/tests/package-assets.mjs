import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir,mkdtemp,rm,readdir,stat} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {tmpdir} from 'node:os';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {packageAssets,cmapBytes} from '../../scripts/package_web_assets.mjs';
const repo=resolve(fileURLToPath(new URL('../../',import.meta.url))),temp=await mkdtemp(join(tmpdir(),'rzpdf-assets-test-'));
const hash=data=>createHash('sha256').update(data).digest('hex');
const json=value=>JSON.stringify(value,null,2)+'\n';
const leb=n=>{const bytes=[];do{const part=n&127;n=Math.floor(n/128);bytes.push(part|(n?128:0));}while(n);return Buffer.from(bytes);};
const checkAbsent=path=>assert.rejects(stat(path),e=>e.code==='ENOENT');
try{
  assert.equal(cmapBytes(Buffer.from('%ignored\r\nA\r\n\nB')).toString(),'A\n\nB\n');
  const payload=[];
  for(const group of ['fonts','afm','cmaps'])for(const name of (await readdir(join(repo,'font-data',group))).sort()){
    const raw=await readFile(join(repo,'font-data',group,name));payload.push(group==='cmaps'?cmapBytes(raw):raw,Buffer.from('\nrzpdf-test-boundary\n'));
  }
  const makeModule=extra=>{const body=Buffer.concat([Buffer.from([0]),...payload,...extra]);return Buffer.concat([Buffer.from([0,97,115,109,1,0,0,0,0]),leb(body.length),body]);};
  const wasm=join(temp,'inventory-fixture.wasm'),module=makeModule([]);assert.ok(WebAssembly.validate(module));await writeFile(wasm,module);
  const buildReport=join(temp,'build.json');await writeFile(buildReport,json({status:'passed',task:'WA-03',target:'wasm32-unknown-emscripten',artifacts:{'rzpdf_web.wasm':hash(module)}}));
  const font=join(repo,'output/webassembly/fixtures/matrix-two-faces.ttc'),bytes=await readFile(font),license=join(repo,'LICENSE');
  const entry=index=>({resource:{id:'face-'+index,version:'1',format:'collection',sha256:hash(bytes),sizeBytes:bytes.length,font:{family:'Geometry',weight:400,italic:false,serif:false,index},license:{spdx:'AGPL-3.0-only'}},file:font,licenseFile:license});
  const config=join(temp,'config.json');await writeFile(config,json({version:1,entries:[entry(0),entry(1)]}));
  const options={wasm,buildReport,config};const output=join(temp,'bundle');const report=await packageAssets({...options,output});
  assert.deepEqual(report.builtinCounts,{fonts:14,afm:14,cmaps:65});assert.equal(report.optional.uniquePrograms,1);assert.equal(report.optional.resources,2);assert.equal(report.optional.uniqueRawBytes,bytes.length);
  const manifest=JSON.parse(await readFile(join(output,'fonts/manifest.json')));assert.equal(manifest.resources[0].url,manifest.resources[1].url);assert.deepEqual(manifest.resources.map(r=>r.font.index),[0,1]);
  assert.ok(!(await readFile(join(output,'assets.json'),'utf8')).includes(temp),'private configuration path leaked');
  for(const record of report.files){const data=await readFile(join(output,record.file));assert.equal(data.length,record.rawBytes);assert.equal(hash(data),record.sha256);}
  assert.deepEqual(JSON.parse(await readFile(join(output,'assets.json'))),report,'returned and deployed inventories differ');
  const second=join(temp,'bundle-2');await packageAssets({...options,output:second});assert.deepEqual(await readFile(join(second,'assets.json')),await readFile(join(output,'assets.json')),'asset inventory is not reproducible');
  await writeFile(join(output,'keep.txt'),'user-owned');await assert.rejects(packageAssets({...options,output}),/already exists/);assert.equal(await readFile(join(output,'keep.txt'),'utf8'),'user-owned');
  const broken=async(entries,pattern)=>{await writeFile(config,json({version:1,entries}));const output=join(temp,'rejected');await assert.rejects(packageAssets({...options,output}),pattern);await checkAbsent(output);assert.equal((await readdir(temp)).filter(n=>n.startsWith('.rzpdf-assets-')).length,0);};
  const badHash=entry(0);badHash.resource.sha256='0'.repeat(64);await broken([badHash],/hash, length or format/);
  const missing=entry(0);missing.licenseFile=join(temp,'no-license');await broken([missing],/ENOENT/);
  const empty=join(temp,'empty.txt');await writeFile(empty,'');const noNotice=entry(0);noNotice.licenseFile=empty;await broken([noNotice],/nonempty/);
  const badMeta=entry(0);badMeta.resource.font.family='';await broken([badMeta],/metadata/);
  const pfb=join(repo,'font-data/fonts/n022003l.pfb'),pbytes=await readFile(pfb);
  const duplicate={...entry(0),resource:{...entry(0).resource,format:'type1',sha256:hash(pbytes),sizeBytes:pbytes.length},file:pfb};
  await broken([duplicate],/duplicates an embedded/);
  await writeFile(config,json({version:1,entries:[duplicate]}));const explicit=await packageAssets({...options,output:join(temp,'explicit-duplicate'),allowEmbeddedDuplicates:true});assert.equal(explicit.optional.programs[0].duplicatesEmbedded,true);
  await writeFile(buildReport,json({status:'failed',artifacts:{'rzpdf_web.wasm':hash(module)}}));await assert.rejects(packageAssets({...options,output:join(temp,'bad-build')}),/passed WA-03/);await checkAbsent(join(temp,'bad-build'));
  console.log('Font assets: runtime metadata, exact embedded identity, TTC dedup, copied notices, reproducibility, non-overwrite and atomic failure checks passed');
}finally{await rm(temp,{recursive:true,force:true});}
