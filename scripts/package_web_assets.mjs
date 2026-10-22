#!/usr/bin/env node
/** Offline font/CMap delivery inventory; does not publish or install anything. */
import {open,readFile,writeFile,mkdir,mkdtemp,rename,rm,rmdir,stat} from 'node:fs/promises';
import {resolve,dirname,extname,join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';
import {readdir} from 'node:fs/promises';
const REPO=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const MAX_FONT=32*1024*1024,MAX_PROGRAMS=64*1024*1024;
const hash=data=>createHash('sha256').update(data).digest('hex');
const json=value=>Buffer.from(JSON.stringify(value,null,2)+'\n');
const notices=['NOTICE.md','cmap-provenance.json','licenses/gsfonts-COPYING','licenses/gsfonts-README','licenses/gsfonts-README.tweaks','licenses/Adobe-Core14-MustRead.html','licenses/Adobe-CMap-LICENSE.md','licenses/Adobe-CMap-NOTICES.txt'];
const licenseRefs={fonts:['notices/font-data/NOTICE.md','notices/font-data/licenses/gsfonts-COPYING','notices/font-data/licenses/gsfonts-README','notices/font-data/licenses/gsfonts-README.tweaks'],afm:['notices/font-data/NOTICE.md','notices/font-data/licenses/Adobe-Core14-MustRead.html'],cmaps:['notices/font-data/NOTICE.md','notices/font-data/cmap-provenance.json','notices/font-data/licenses/Adobe-CMap-LICENSE.md','notices/font-data/licenses/Adobe-CMap-NOTICES.txt']};
async function bounded(path,limit){
  const file=await open(path,'r');
  try{
    const before=await file.stat();if(!before.isFile()||before.size<1||before.size>limit)throw Error('Input must be a nonempty regular file within its size limit');
    const buffer=Buffer.alloc(before.size+1);let used=0;
    while(used<buffer.length){const {bytesRead}=await file.read(buffer,used,buffer.length-used,null);if(!bytesRead)break;used+=bytesRead;}
    if(used!==before.size||(await file.stat()).size!==before.size)throw Error('Input changed while reading');
    return buffer.subarray(0,used);
  }finally{await file.close();}
}
export function cmapBytes(bytes){
  const source=new TextDecoder('utf-8',{fatal:true}).decode(bytes),lines=source.split('\n'),out=[];
  for(let i=0;i<lines.length;i++){
    if(i===lines.length-1&&lines[i]==='')continue;
    const line=i<lines.length-1&&lines[i].endsWith('\r')?lines[i].slice(0,-1):lines[i];
    if(!line.startsWith('%'))out.push(line+'\n');
  }return Buffer.from(out.join(''));
}
export function occurrences(haystack,needle){
  if(!needle.length)throw Error('Empty embedded resource');let count=0,offset=0;
  for(;;){const found=haystack.indexOf(needle,offset);if(found<0)return count;count++;offset=found+1;}
}
function format(bytes,kind){
  const tag=bytes.subarray(0,4).toString('latin1'),tt=bytes.length>=4&&(bytes.readUInt32BE(0)===0x00010000||tag==='true');
  return kind==='truetype'?tt:kind==='opentype'?tt||tag==='OTTO':kind==='collection'?tag==='ttcf':kind==='type1'?bytes.length>=2&&(bytes[0]===128&&bytes[1]===1||bytes[0]===37&&bytes[1]===33):kind==='cff'?bytes.length>=4&&[1,2].includes(bytes[0])&&bytes[2]>=4:false;
}
const extension={truetype:'.ttf',opentype:'.otf',collection:'.ttc',type1:'.pfb',cff:'.cff'};
export async function packageAssets({output,wasm=join(REPO,'web/dist/rzpdf_web.wasm'),config,buildReport=join(REPO,'output/webassembly/wa03-build.json'),allowEmbeddedDuplicates=false}){
  if(!output)throw Error('An explicit output directory is required');output=resolve(output);
  try{await stat(output);throw Error('Output already exists');}catch(error){if(error.code!=='ENOENT')throw error;}
  const wasmBytes=await bounded(wasm,256*1024*1024);if(!wasmBytes.subarray(0,8).equals(Buffer.from([0,97,115,109,1,0,0,0])))throw Error('Invalid Wasm module');
  if(!WebAssembly.validate(wasmBytes))throw Error('Wasm validation failed');
  const build=JSON.parse((await bounded(buildReport,2*1024*1024)).toString('utf8'));
  const expected=Object.entries(build.artifacts??{}).find(([name])=>name.endsWith('rzpdf_web.wasm'))?.[1];
  if(build.status!=='passed'||build.task!=='WA-03'||build.target!=='wasm32-unknown-emscripten'||expected!==hash(wasmBytes))throw Error('Wasm does not match a passed WA-03 build');
  const builtins=[],embeddedHashes=new Set();
  for(const group of ['fonts','afm','cmaps']){
    const folder=join(REPO,'font-data',group);
    for(const name of (await readdir(folder)).sort()){
      if(!(await stat(join(folder,name))).isFile())continue;
      if(group==='fonts'&&extname(name)!=='.pfb'||group==='afm'&&extname(name)!=='.afm')continue;
      const raw=await bounded(join(folder,name),8*1024*1024),data=group==='cmaps'?cmapBytes(raw):raw,count=occurrences(wasmBytes,data);
      if(count!==1)throw Error(`Embedded ${group} resource ${name} occurs ${count} times; expected one exact byte sequence`);
      if(group==='fonts')embeddedHashes.add(hash(data));
      builtins.push({kind:group,name,sourceSha256:hash(raw),sourceBytes:raw.length,embeddedSha256:hash(data),embeddedBytes:data.length,exactByteOccurrences:count,storage:'embedded-in-wasm',notices:licenseRefs[group]});
    }
  }
  const wasmMeasure={sha256:hash(wasmBytes),rawBytes:wasmBytes.length,gzipLevel9Bytes:gzipSync(wasmBytes,{level:9}).length,copiedIntoAssetBundle:false};
  let entries=[];
  if(config){const parsed=JSON.parse((await bounded(config,1024*1024)).toString('utf8'));
    if(!parsed||parsed.version!==1||Object.keys(parsed).some(k=>!['version','entries'].includes(k))||!Array.isArray(parsed.entries)||parsed.entries.length>128)throw Error('Invalid local asset configuration');entries=parsed.entries;
  }
  const {FontResourceService}=await import(pathToFileURL(join(REPO,'web/dist/font-resources.js')).href);
  const resources=[],licenseRecords=[],programs=new Map(),licenseFiles=new Map();let uniqueBytes=0;
  const base=config?dirname(resolve(config)):REPO;
  // Validate all metadata with the actual runtime before reading program files.
  for(const entry of entries){if(!entry||Object.keys(entry).some(k=>!['resource','file','licenseFile'].includes(k))||typeof entry.file!=='string'||!entry.file||typeof entry.licenseFile!=='string'||!entry.licenseFile)throw Error('Each resource needs explicit font and license files');resources.push({...entry.resource,url:hash(Buffer.from(String(entry.resource?.id)))+'.font'});}
  const admission=new FontResourceService({version:1,resources},{root:'https://rzpdf.invalid/fonts/',network:false,persistent:false});admission.dispose();
  await mkdir(dirname(output),{recursive:true});const staging=await mkdtemp(join(dirname(output),'.rzpdf-assets-'));let reserved=false;
  try{
    const inventory=[];
    const emit=async(name,data)=>{const target=join(staging,name);await mkdir(dirname(target),{recursive:true});await writeFile(target,data,{flag:'wx'});inventory.push({file:name,sha256:hash(data),rawBytes:data.length,gzipLevel9Bytes:gzipSync(data,{level:9}).length});};
    for(const name of notices)await emit('notices/font-data/'+name,await bounded(join(REPO,'font-data',name),1024*1024));
    for(const [index,entry] of entries.entries()){
      const resource=resources[index],data=await bounded(resolve(base,entry.file),MAX_FONT);
      if(data.length!==resource.sizeBytes||hash(data)!==resource.sha256||!format(data,resource.format))throw Error('Font hash, length or format does not match configuration');
      const duplicate=embeddedHashes.has(resource.sha256)||wasmBytes.indexOf(data)>=0;if(duplicate&&!allowEmbeddedDuplicates)throw Error('Optional font duplicates an embedded program; explicit opt-in required');
      const notice=await bounded(resolve(base,entry.licenseFile),1024*1024);new TextDecoder('utf-8',{fatal:true}).decode(notice);if(notice.includes(0))throw Error('License must be UTF-8 text');
      const noticeHash=hash(notice),noticeFile='notices/optional/'+noticeHash+'.txt';
      if(!licenseFiles.has(noticeHash)){await emit(noticeFile,notice);licenseFiles.set(noticeHash,true);}
      let file=programs.get(resource.sha256)?.file;
      if(!file){uniqueBytes+=data.length;if(uniqueBytes>MAX_PROGRAMS)throw Error('Unique optional fonts exceed 64 MiB');file=resource.sha256+extension[resource.format];await emit('fonts/'+file,data);programs.set(resource.sha256,{file,sha256:resource.sha256,rawBytes:data.length,gzipLevel9Bytes:gzipSync(data,{level:9}).length,duplicatesEmbedded:duplicate});}
      resources[index]={...resource,url:file};licenseRecords.push({id:resource.id,file:noticeFile,sha256:noticeHash,metadata:resource.license});
    }
    const manifest={version:1,resources};const validation=new FontResourceService(manifest,{root:'https://rzpdf.invalid/fonts/',network:false,persistent:false});validation.dispose();
    const manifestBytes=json(manifest);if(manifestBytes.length>1024*1024)throw Error('Generated font manifest exceeds the reader 1 MiB limit');
    await emit('fonts/manifest.json',manifestBytes);await emit('fonts/licenses.json',json({version:1,resources:licenseRecords}));
    await emit('builtins.json',json({schemaVersion:1,kind:'embedded-font-and-cmap-inventory',wasm:wasmMeasure,records:builtins,occurrenceScope:'exact original font/AFM bytes and comment-stripped CMap bytes; not an analysis of alternative encodings'}));
    const report={schemaVersion:1,kind:'rzpdf-font-assets',wasm:wasmMeasure,build:{target:build.target,sourceRevision:build.source?.source_revision??null,trackedChanges:build.source?.tracked_changes??null,sourceTreeSha256:hash(json(build.source?.source_sha256??{})),toolchainSha256:hash(json(build.toolchain_lock??{}))},packagerSha256:hash(await bounded(fileURLToPath(import.meta.url),1024*1024)),inventoryScope:'All bundle members except assets.json itself',validationScope:'Runtime metadata validation, format magic, hashes and exact embedded byte occurrences; native face/coverage validation happens on registration',builtinCounts:Object.fromEntries(['fonts','afm','cmaps'].map(kind=>[kind,builtins.filter(r=>r.kind===kind).length])),optional:{manifest:'fonts/manifest.json',licenses:'fonts/licenses.json',resources:resources.length,uniquePrograms:programs.size,uniqueRawBytes:uniqueBytes,programs:[...programs.values()]},files:inventory,scope:'Font and CMap assets/notices only. This is not a complete application, dependency-license or publication acceptance.',compression:'gzip level 9 measured independently; no compressed variants emitted and HTTP wire sizes depend on server configuration'};
    await writeFile(join(staging,'assets.json'),json(report),{flag:'wx'});
    await mkdir(output);reserved=true;await rename(staging,output);reserved=false;return report;
  }finally{await rm(staging,{recursive:true,force:true});if(reserved)await rmdir(output).catch(()=>{});}
}
async function main(){
  const options={};
  for(let i=2;i<process.argv.length;i++){const arg=process.argv[i];if(arg==='--allow-embedded-duplicates')options.allowEmbeddedDuplicates=true;else if(['--output','--wasm','--config','--build-report'].includes(arg)&&process.argv[i+1])options[arg==='--build-report'?'buildReport':arg.slice(2)]=process.argv[++i];else throw Error('Usage: node scripts/package_web_assets.mjs --output NEW_DIRECTORY [--wasm FILE] [--config JSON] [--build-report JSON] [--allow-embedded-duplicates]');}
  const report=await packageAssets(options);console.log(JSON.stringify({status:'passed',builtinCounts:report.builtinCounts,optionalPrograms:report.optional.uniquePrograms,wasm:report.wasm}));
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url))main().catch(error=>{console.error(error.message);process.exitCode=1;});
