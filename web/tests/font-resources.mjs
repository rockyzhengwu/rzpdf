import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';import {createHash} from 'node:crypto';
import {FontResourceService} from '../dist/font-resources.js';
import {fontLanguageRank} from '../dist/fonts.js';import {PdfApiError} from '../dist/client.js';
const cjk=new Uint8Array(await readFile(new URL('../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf',import.meta.url))),symbol=new Uint8Array(await readFile(new URL('../../pdf/tests/fixtures/synthetic-symbol.ttf',import.meta.url)));
const hash=b=>createHash('sha256').update(b).digest('hex'),tick=()=>new Promise(r=>setTimeout(r,0));
const makeEntry=(id,data,point)=>({id,version:'test-1',format:'truetype',sha256:hash(data),sizeBytes:data.length,font:{family:id,aliases:['MissingFace'],weight:400,italic:false,serif:false},license:{spdx:id==='CJK'?'OFL-1.1':'AGPL-3.0-only'},ranges:[[point,point]],url:id+'.ttf'});
const manifest=()=>({version:1,resources:[makeEntry('CJK',cjk,0x65b0),makeEntry('Symbol',symbol,0xd7)]});
const sources={CJK:cjk,Symbol:symbol};
const reject=async(p,code)=>assert.rejects(p,e=>e.code===code);
function invalid(fn){assert.throws(fn,e=>e.code==='invalid_argument');}
async function until(fn){for(let i=0;i<100;i++){if(fn())return;await tick();}throw Error('Condition did not become true');}
function deferred(){let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};}
class Doc {
  closed=false;revision='0';epoch='0';id=1;registrations=[];covered=new Set();wanted=[0x65b0,0xd7];blocked=false;redacted=false;neverProgress=false;preflights=0;
  stamp(){return {engine:'mock',document:this.id,revision:this.revision,fontEpoch:this.epoch};}
  fontRequirements(page,options={},task={}){if(this.closed)throw new PdfApiError('invalid_handle','closed','fontRequirements');this.preflights++;const missing=this.wanted.filter(c=>!this.covered.has(c));return {stamp:this.stamp(),page,renderOptions:options,codepointsRedacted:this.redacted,complete:!missing.length&&!this.blocked,fonts:[{fontId:'font-0',name:'ABCDEF+MissingFace',weight:400,italic:false,serif:false,missingCount:this.blocked?0:missing.length,missingCodepoints:this.blocked||this.redacted?[]:missing,missingUnicode:this.blocked?1:0,needsShaping:0,selected:[]}]};}
  async registerFont(bytes,font,stamp,task={}){if(task.signal?.aborted)throw new PdfApiError('cancelled','cancelled','registerFont');if(this.closed||JSON.stringify(stamp)!==JSON.stringify(this.stamp()))throw new PdfApiError('invalid_handle','stale','registerFont');const sha256=hash(bytes);this.registrations.push({sha256,font:structuredClone(font),stamp:structuredClone(stamp)});if(!this.neverProgress){if(sha256===hash(cjk))this.covered.add(0x65b0);if(sha256===hash(symbol))this.covered.add(0xd7);}return {sha256,index:font.index??0};}
}
class Cache {
  rows=new Map();holdPut;putStarted=false;
  async match(request){return this.rows.get(typeof request==='string'?request:request.url)?.clone();}
  async keys(){return [...this.rows.keys()].map(u=>new Request(u));}
  async put(request,response){this.putStarted=true;if(this.holdPut)await this.holdPut.promise;this.rows.delete(typeof request==='string'?request:request.url);this.rows.set(typeof request==='string'?request:request.url,response.clone());}
  async delete(request){return this.rows.delete(typeof request==='string'?request:request.url);}
}
class Storage {
  stores=new Map();next;
  async open(name){if(!this.stores.has(name))this.stores.set(name,this.next??new Cache());return this.stores.get(name);}
  async delete(name){return this.stores.delete(name);}
}
const checks=[];
// Validate before I/O and own configuration; PDF names never become URLs.
{
  const bad=manifest();delete bad.resources[0].license;invalid(()=>new FontResourceService(bad,{load:async()=>cjk}));
  invalid(()=>new FontResourceService({...manifest(),version:2},{load:async()=>cjk}));
  invalid(()=>new FontResourceService(manifest(),{root:'https://example.test/fonts'}));
  for(const url of ['../secret.ttf','https://other.test/x.ttf','%252e%252e/secret.ttf','http://[','https://user:pass@example.test/fonts/x.ttf']){const m=manifest();m.resources[0].url=url;invalid(()=>new FontResourceService(m,{root:'https://example.test/fonts/'}));}
  const duplicate=manifest();duplicate.resources.push({...duplicate.resources[0],id:'Other'});invalid(()=>new FontResourceService(duplicate,{load:async()=>cjk}));
  invalid(()=>new FontResourceService(manifest(),{load:async()=>cjk,memoryBytes:65*1024*1024}));
  const original=manifest(),events=[];const service=new FontResourceService(original,{persistent:false,load:async e=>{assert.equal(e.font.aliases[0],'MissingFace');assert.ok(Object.isFrozen(e.font.aliases));return sources[e.id];},onEvent:e=>{events.push(e);throw Error('observer');}});original.resources[0].font.aliases[0]='Changed';original.resources[0].license.spdx='Changed';
  const doc=new Doc();assert.ok((await service.ensure(doc,0)).complete);assert.equal(doc.registrations.length,2);assert.ok(events.some(e=>e.type==='registered'));assert.equal(service.statistics().reservedBytes,0);service.dispose();checks.push('manifest/root/license/limits validation, configuration ownership, observer isolation');
}
// Shared fetches hold independent subscriptions; cancelling one never kills the other.
{
  const pending=new Map(),calls=[],events=[];const service=new FontResourceService(manifest(),{persistent:false,load:(entry,signal)=>{calls.push(entry.id);const d=deferred();pending.set(entry.id,{...d,signal});return d.promise;},onEvent:e=>events.push(e)});
  const one=new Doc(),two=new Doc(),controller=new AbortController();const a=service.ensure(one,0,{}, {signal:controller.signal}),b=service.ensure(two,0);const outcomes=Promise.allSettled([a,b]);await until(()=>pending.has('CJK'));assert.equal(calls.length,1);controller.abort();await tick();assert.equal(pending.get('CJK').signal.aborted,false);pending.get('CJK').resolve(cjk);await until(()=>pending.has('Symbol'));pending.get('Symbol').resolve(symbol);const result=await outcomes;assert.equal(result[0].reason.code,'cancelled');assert.ok(result[1].value.complete);assert.equal(one.registrations.length,0);assert.deepEqual(calls,['CJK','Symbol']);assert.equal(service.statistics().inflight,0);assert.equal(service.statistics().reservedBytes,0);service.dispose();checks.push('coalescing and independent subscriber cancellation');
}
// Last cancellation drops even a loader that ignores its signal; late results cannot register/cache.
{
  const pending=deferred();let signal;const service=new FontResourceService(manifest(),{persistent:false,load:(_,s)=>{signal=s;return pending.promise;}});const doc=new Doc(),controller=new AbortController();const work=service.ensure(doc,0,{}, {signal:controller.signal});const outcome=reject(work,'cancelled');await until(()=>signal);controller.abort();await outcome;await until(()=>service.statistics().activeLoads===0);assert.ok(signal.aborted);pending.resolve(cjk);await tick();assert.equal(doc.registrations.length,0);assert.equal(service.statistics().memoryBytes,0);assert.equal(service.statistics().reservedBytes,0);service.dispose();checks.push('last subscriber abort and ignored-cancellation late result suppression');
}
{
  const events=[];const service=new FontResourceService(manifest(),{persistent:false,timeoutMs:100,load:()=>new Promise(()=>{}),onEvent:e=>events.push(e)});const doc=new Doc();await reject(service.ensure(doc,0),'resource_limit');await until(()=>service.statistics().activeLoads===0);assert.equal(doc.registrations.length,0);assert.equal(service.statistics().reservedBytes,0);assert.ok(events.some(e=>e.type==='failed'&&e.code==='resource_limit'));service.dispose();checks.push('bounded load timeout without native registration');
}
// A document change during download must use the old stamp and fail, never silently refresh it.
for(const change of [doc=>doc.closed=true,doc=>doc.revision='1',doc=>doc.epoch='1']){
  const pending=deferred();let started=false;const service=new FontResourceService(manifest(),{persistent:false,load:()=>{started=true;return pending.promise;}}),doc=new Doc();const work=service.ensure(doc,0),outcome=reject(work,'invalid_handle');await until(()=>started);change(doc);pending.resolve(cjk);await outcome;assert.equal(doc.registrations.length,0);assert.equal(service.statistics().memoryBytes,0);service.dispose();
}checks.push('closed/revised/font-epoch stale registration suppression');
{
  for(const mode of ['hash','length','format']){const m=manifest();if(mode==='format')m.resources[0].format='type1';const service=new FontResourceService(m,{persistent:false,load:async()=>mode==='length'?cjk.subarray(1):mode==='hash'?new Uint8Array(cjk.length):cjk});const doc=new Doc();await reject(service.ensure(doc,0),'font');assert.equal(doc.registrations.length,0);assert.equal(service.statistics().memoryBytes,0);service.dispose();}
  let calls=0;const mutable=cjk.slice(),service=new FontResourceService(manifest(),{persistent:false,load:async entry=>{calls++;return entry.id==='CJK'?mutable:symbol;},onEvent:e=>{if(e.type==='loaded'&&e.resourceId==='CJK')mutable.fill(0);}});assert.ok((await service.ensure(new Doc(),0)).complete);assert.equal(mutable[0],0);assert.ok((await service.ensure(new Doc(),0)).complete);assert.equal(calls,2);service.dispose();checks.push('hash/length/format refusal, callback byte ownership and verified memory hits');
}
{
  const storage=new Storage(),service=new FontResourceService(manifest(),{cacheStorage:storage,load:async e=>sources[e.id]});assert.ok((await service.ensure(new Doc(),0)).complete);service.dispose();let calls=0;const events=[];
  const offline=new FontResourceService(manifest(),{cacheStorage:storage,network:false,load:async()=>{calls++;throw Error('network forbidden');},onEvent:e=>events.push(e)});const redacted=new Doc();redacted.redacted=true;assert.ok((await offline.ensure(redacted,0)).complete);assert.equal(calls,0);assert.equal(events.filter(e=>e.type==='cache-hit'&&e.source==='persistent').length,2);offline.dispose();
  const cache=await storage.open('rzpdf-fonts-v1-default');const first=[...cache.rows.keys()][0];const old=cache.rows.get(first);const length=Number(old.headers.get('content-length'));cache.rows.set(first,new Response(new Uint8Array(length),{headers:old.headers}));
  const broken=new FontResourceService(manifest(),{cacheStorage:storage,network:false,load:async()=>{throw Error('forbidden');}}),doc=new Doc();await reject(broken.ensure(doc,0),'font');assert.equal(doc.registrations.length,0);assert.equal(cache.rows.has(first),false);await broken.clearCache();assert.equal(storage.stores.size,0);broken.dispose();checks.push('persistent reload offline, redacted requests, corrupt-cache revalidation/eviction and namespace clear');
}
{
  const storage=new Storage(),capacity=Math.max(cjk.length,symbol.length),service=new FontResourceService(manifest(),{cacheStorage:storage,memoryBytes:capacity,persistentBytes:capacity,load:async e=>sources[e.id]});assert.ok((await service.ensure(new Doc(),0)).complete);const cache=await storage.open('rzpdf-fonts-v1-default');assert.equal(cache.rows.size,1);assert.ok(service.statistics().memoryBytes<=capacity);const memoryOnly=new Doc();memoryOnly.wanted=[0xd7];assert.ok((await service.ensure(memoryOnly,0)).complete);service.dispose();checks.push('bounded LRU memory and persistent eviction');
}
// Clear waits for an already started cache write; its late completion cannot repopulate the namespace.
{
  const storage=new Storage(),cache=new Cache(),hold=deferred();cache.holdPut=hold;storage.next=cache;const service=new FontResourceService(manifest(),{cacheStorage:storage,load:async e=>sources[e.id]}),doc=new Doc();const work=service.ensure(doc,0),outcome=reject(work,'cancelled');await until(()=>cache.putStarted);let cleared=false;const clearing=service.clearCache().then(()=>{cleared=true;});await tick();assert.equal(cleared,false);hold.resolve();await outcome;await clearing;assert.equal(storage.stores.size,0);assert.equal(service.statistics().memoryBytes,0);assert.equal(service.statistics().reservedBytes,0);service.dispose();checks.push('cache write/clear ordering and cancellation cleanup');
}
{
  let calls=0;const service=new FontResourceService(manifest(),{persistent:false,maxLoads:1,load:async e=>{calls++;return sources[e.id];}}),doc=new Doc();doc.neverProgress=true;assert.equal((await service.ensure(doc,0)).complete,false);assert.equal(calls,1);assert.equal(doc.preflights,2);service.dispose();
  const blocked=new FontResourceService(manifest(),{persistent:false,load:async()=>{throw Error('must not guess');}}),unmapped=new Doc();unmapped.blocked=true;assert.equal((await blocked.ensure(unmapped,0)).complete,false);blocked.dispose();
  const pending=deferred(),limited=new FontResourceService(manifest(),{persistent:false,load:()=>pending.promise});const jobs=Array.from({length:16},()=>limited.ensure(new Doc(),0));const done=Promise.allSettled(jobs);await reject(limited.ensure(new Doc(),0),'resource_limit');limited.dispose();assert.ok((await done).every(r=>r.status==='rejected'&&r.reason.code==='cancelled'));await until(()=>limited.statistics().activeLoads===0);assert.equal(limited.statistics().reservedBytes,0);await reject(limited.ensure(new Doc(),0),'invalid_handle');checks.push('bounded attempts, blocked mapping no-I/O, request admission and disposal');
}
// An observer can cancel synchronously before a synchronous native call starts.
{
  let service;const doc=new Doc();doc.registerFont=function(){this.registrations.push('late');return {sha256:hash(cjk),index:0};};
  service=new FontResourceService(manifest(),{persistent:false,load:async e=>sources[e.id],onEvent:e=>{if(e.type==='loaded')service.cancel();}});
  await reject(service.ensure(doc,0),'cancelled');assert.equal(doc.registrations.length,0);service.dispose();checks.push('observer cancellation before synchronous registration');
}
// A maximum-size cached file must remain usable offline within the shared byte budget.
{
  const large=new Uint8Array(32*1024*1024);large.set(cjk);const entry=makeEntry('Large',large,0x65b0),m={version:1,resources:[entry]};let calls=0;
  const service=new FontResourceService(m,{persistent:false,load:async()=>{calls++;return large;}});
  const make=()=>{const d=new Doc();d.wanted=[0x65b0];d.registerFont=async function(data,font,stamp){assert.equal(hash(data),entry.sha256);this.covered.add(0x65b0);return {sha256:entry.sha256,index:0};};return d;};
  assert.ok((await service.ensure(make(),0)).complete);assert.equal(service.statistics().memoryBytes,large.length);assert.ok((await service.ensure(make(),0)).complete);assert.equal(calls,1);assert.equal(service.statistics().reservedBytes,0);service.dispose();checks.push('32 MiB memory-hit accounting without needless eviction/reload');
}
{
  const events=[],storage={open:async()=>{throw Error('storage unavailable');},delete:async()=>{throw Error('storage unavailable');}};
  const service=new FontResourceService(manifest(),{cacheStorage:storage,load:async e=>sources[e.id],onEvent:e=>events.push(e)});assert.ok((await service.ensure(new Doc(),0)).complete);assert.ok(events.some(e=>e.type==='cache-error'));await reject(service.clearCache(),'resource_limit');assert.equal(service.statistics().memoryBytes,0);service.dispose();checks.push('cache-storage failures remain observable, nonfatal to rendering, and structured on clear');
}
{
  const storage=new Storage(),cache=new Cache(),hold=deferred();cache.holdPut=hold;storage.next=cache;
  const old=new FontResourceService(manifest(),{cacheStorage:storage,load:async e=>sources[e.id]});const oldWork=old.ensure(new Doc(),0),oldOutcome=reject(oldWork,'cancelled');await until(()=>cache.putStarted);old.dispose();
  let callbacks=0;const fresh=new FontResourceService(manifest(),{cacheStorage:storage,load:async e=>{callbacks++;return sources[e.id];}}),freshWork=fresh.ensure(new Doc(),0);await tick();assert.equal(callbacks,0);hold.resolve();await oldOutcome;assert.ok((await freshWork).complete);fresh.dispose();
  const offline=new FontResourceService(manifest(),{cacheStorage:storage,network:false,load:async()=>{throw Error('network forbidden');}});assert.ok((await offline.ensure(new Doc(),0)).complete);await offline.clearCache();offline.dispose();checks.push('shared-namespace cache ordering prevents disposed service cleanup deleting a fresh service write');
}
{
  assert.equal(fontLanguageRank('JA-jp','ja-JP'),0);assert.equal(fontLanguageRank('zh-Hant-HK','zh-Hant'),1);assert.equal(fontLanguageRank('zh-Hant','zh-Hans'),4);assert.equal(fontLanguageRank('bad hint','ja'),0);
  const m=manifest();m.resources[0].font.languages=['zh-Hans'];m.resources[1].font.languages=['ja-JP'];const loaded=[];
  const service=new FontResourceService(m,{persistent:false,load:async e=>{loaded.push(e.id);return sources[e.id];}}),doc=new Doc(),original=doc.fontRequirements.bind(doc);
  doc.fontRequirements=(...args)=>{const report=original(...args);report.fonts.forEach(f=>f.language='ja-JP');return report;};assert.ok((await service.ensure(doc,0)).complete);assert.deepEqual(loaded,['Symbol','CJK']);service.dispose();checks.push('language-aware candidate loading matches exact/prefix/script conflict native ranking');
}
{
  const alternate=symbol.slice(0);const changed=new Uint8Array(alternate.length+4);changed.set(alternate);
  const jp=makeEntry('Japanese',symbol,0xd7),zh=makeEntry('Traditional',changed,0xd7);jp.font.aliases=[];zh.font.aliases=[];jp.font.languages=['ja-JP'];zh.font.languages=['zh-Hant'];
  const chosen=[],doc=new Doc();doc.fontRequirements=()=>({stamp:doc.stamp(),page:0,renderOptions:{},complete:chosen.length>0,codepointsRedacted:false,fonts:['ja-JP','zh-Hant-HK'].map(language=>{const best=chosen.slice().sort((a,b)=>fontLanguageRank(language,a.font.languages[0])-fontLanguageRank(language,b.font.languages[0]))[0];return {fontId:language,name:'MissingLocale',language,weight:400,italic:false,serif:false,codepoints:[0xd7],missingCount:best?0:1,missingCodepoints:best?[]:[0xd7],missingUnicode:0,needsShaping:0,selected:best?[{sha256:best.sha256,index:0,count:1,preference:[1,fontLanguageRank(language,best.font.languages[0]),0,0,0]}]:[]};})});
  doc.registerFont=async(bytes,font)=>{const identity={sha256:hash(bytes),index:0};chosen.push({...identity,font});return identity;};
  const loaded=[],service=new FontResourceService({version:1,resources:[jp,zh]},{persistent:false,load:async e=>{loaded.push(e.id);return e.id==='Japanese'?symbol:changed;}});
  assert.ok((await service.ensure(doc,0)).complete);assert.deepEqual(loaded,['Japanese','Traditional']);assert.ok((await service.ensure(doc,0)).complete);assert.equal(loaded.length,2);service.dispose();checks.push('covered glyphs still load a better locale face without requiring a matching family alias');
}
console.log(JSON.stringify({status:'passed',checks}));
