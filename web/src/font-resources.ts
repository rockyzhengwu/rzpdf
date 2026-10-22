import {PdfApiError,type PdfDocument} from './client.js';
import type {WorkerPdfDocument} from './worker-client.js';
import {normalizeFontRegistration,normalizeFontStamp,fontLanguageRank,validFontLanguage,type FontRegistration,type FontRequirements,type FontStamp,type FontRequirement} from './fonts.js';
import {normalizeRenderOptions,type RenderOptions} from './interaction.js';

export interface FontResource {
  id:string;version:string;format:'truetype'|'opentype'|'collection'|'type1'|'cff';
  sha256:string;sizeBytes:number;font:FontRegistration;
  license:{spdx:string;notice?:string;url?:string};url?:string;ranges?:[number,number][];
}
export interface FontManifest {version:1;resources:FontResource[];}
export interface FontResourceEvent {type:'request'|'cache-hit'|'loaded'|'registered'|'failed'|'cancelled'|'cache-error'|'unresolved';requestId:number;resourceId?:string;stamp?:FontStamp;source?:'memory'|'persistent'|'network'|'callback';code?:string;}
export interface FontResourceOptions {
  root?:string;load?:(resource:Readonly<FontResource>,signal:AbortSignal)=>Promise<Uint8Array>;
  network?:boolean;persistent?:boolean;cacheName?:string;cacheStorage?:CacheStorage;
  memoryBytes?:number;persistentBytes?:number;timeoutMs?:number;maxLoads?:number;
  onEvent?:(event:Readonly<FontResourceEvent>)=>void;
}
type Entry=FontResource & {font:Required<FontRegistration>;resolvedUrl?:string;canonical:string;};
type Loaded={data:Uint8Array;source:'memory'|'persistent'|'network'|'callback'};
type Job={key:string;entry:Entry;controller:AbortController;promise:Promise<Loaded>;resolve:(v:Loaded)=>void;reject:(e:unknown)=>void;refs:number;cost:number;done:boolean;value?:Loaded;memory?:Uint8Array;};
type Lease=Loaded & {release:()=>void};
const MAX_BYTES=64*1024*1024;
const cacheTails=new Map<string,Promise<unknown>>();
const fail=(code:ConstructorParameters<typeof PdfApiError>[0],message:string)=>new PdfApiError(code,message,'fontResources');
function aborted(signal:AbortSignal):never {throw signal.reason instanceof PdfApiError?signal.reason:fail('cancelled','Font resource request cancelled');}
function alive(signal:AbortSignal):void{if(signal.aborted)aborted(signal);}
function abortable<T>(promise:Promise<T>,signal:AbortSignal):Promise<T>{
  if(signal.aborted){void promise.catch(()=>{});try{aborted(signal);}catch(e){return Promise.reject(e);}}
  return new Promise((resolve,reject)=>{let settled=false;const finish=(action:()=>void)=>{if(settled)return;settled=true;signal.removeEventListener('abort',stop);action();};const stop=()=>finish(()=>{try{aborted(signal);}catch(e){reject(e);}});signal.addEventListener('abort',stop,{once:true});promise.then(value=>finish(()=>resolve(value)),error=>finish(()=>reject(error)));});
}
function text(value:unknown,max:number):value is string{return typeof value==='string'&&value.length>0&&value.length<=max&&new TextEncoder().encode(value).length<=max&&!/[\u0000-\u001f\u007f-\u009f]/u.test(value);}
function keys(value:unknown,allowed:string[]):boolean{return !!value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).every(k=>allowed.includes(k));}
function integer(value:number,min:number,max:number):boolean{return Number.isSafeInteger(value)&&value>=min&&value<=max;}
function decodedPath(url:URL):string{let path:string;try{path=decodeURIComponent(url.pathname);}catch{throw fail('invalid_argument','Invalid resource path');}if(path.includes('%')||path.includes('\\')||/[\u0000-\u001f\u007f]/u.test(path)||path.split('/').some(p=>p==='.'||p==='..'))throw fail('invalid_argument','Invalid resource path');return path;}
function resourceRoot(value?:string):URL|undefined{
  if(value===undefined)return undefined;let url:URL;try{url=new URL(value);}catch{throw fail('invalid_argument','Resource root must be an absolute HTTP(S) URL');}
  if(!['http:','https:'].includes(url.protocol)||url.username||url.password||url.search||url.hash||!url.pathname.endsWith('/'))throw fail('invalid_argument','Invalid resource root');decodedPath(url);return url;
}
function normalizeManifest(manifest:FontManifest,root?:URL,callback=false):Entry[]{
  if(!keys(manifest,['version','resources'])||manifest.version!==1||!Array.isArray(manifest.resources)||manifest.resources.length>128)throw fail('invalid_argument','Invalid font manifest');
  const ids=new Set<string>(),faces=new Set<string>();
  return manifest.resources.map(value=>{
    if(!keys(value,['id','version','format','sha256','sizeBytes','font','license','url','ranges'])||!text(value.id,64)||!text(value.version,64)||!['truetype','opentype','collection','type1','cff'].includes(value.format)||typeof value.sha256!=='string'||!/^[0-9a-f]{64}$/.test(value.sha256)||!integer(value.sizeBytes,1,32*1024*1024))throw fail('invalid_argument','Invalid font resource');
    if(!keys(value.license,['spdx','notice','url'])||!text(value.license.spdx,128)||value.license.notice!==undefined&&!text(value.license.notice,8192)||value.license.url!==undefined&&!text(value.license.url,2048))throw fail('invalid_argument','Font resource requires license metadata');
    if(value.license.url!==undefined){let license:URL;try{license=new URL(value.license.url);}catch{throw fail('invalid_argument','Invalid license URL');}if(!['http:','https:'].includes(license.protocol)||license.username||license.password)throw fail('invalid_argument','Invalid license URL');}
    let font:Required<FontRegistration>;try{font=normalizeFontRegistration(value.font);}catch{throw fail('invalid_argument','Invalid resource font metadata');}
    const face=value.sha256+':'+font.index;if(ids.has(value.id)||faces.has(face))throw fail('invalid_argument','Duplicate font resource or face identity');ids.add(value.id);faces.add(face);
    if(value.ranges!==undefined&&(!Array.isArray(value.ranges)||value.ranges.length>256||value.ranges.some(r=>!Array.isArray(r)||r.length!==2||!integer(r[0],0,0x10ffff)||!integer(r[1],r[0],0x10ffff))))throw fail('invalid_argument','Invalid font coverage hints');
    let resolvedUrl:string|undefined;
    if(value.url!==undefined){if(!text(value.url,2048)||value.url.includes('\\'))throw fail('invalid_argument','Invalid font resource URL');if(root){let url:URL;try{url=new URL(value.url,root);}catch{throw fail('invalid_argument','Invalid font resource URL');}if(url.origin!==root.origin||url.username||url.password||url.hash||!decodedPath(url).startsWith(decodedPath(root)))throw fail('invalid_argument','Font resource outside configured root');resolvedUrl=url.href;}}
    if(!callback&&!resolvedUrl)throw fail('invalid_argument','Font resource requires a configured URL or loader');
    const license={...value.license};const ranges=value.ranges?.map(r=>[r[0],r[1]] as [number,number]);
    const normalized={id:value.id,version:value.version,format:value.format,sha256:value.sha256,sizeBytes:value.sizeBytes,font,license,...(value.url===undefined?{}:{url:value.url}),...(ranges===undefined?{}:{ranges})};
    const canonical=JSON.stringify({schema:1,...normalized});
    Object.freeze(font.aliases);Object.freeze(font.languages);Object.freeze(font);Object.freeze(license);if(ranges){ranges.forEach(Object.freeze);Object.freeze(ranges);}
    return Object.freeze({...normalized,resolvedUrl,canonical});
  });
}
async function sha256(bytes:Uint8Array):Promise<string>{if(!globalThis.crypto?.subtle)throw fail('unsupported','Secure hashing is unavailable');return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes as Uint8Array<ArrayBuffer>)),b=>b.toString(16).padStart(2,'0')).join('');}
function validFormat(data:Uint8Array,format:FontResource['format']):boolean{
  const tag=String.fromCharCode(...data.subarray(0,4)),tt=data.length>=4&&(data[0]===0&&data[1]===1&&data[2]===0&&data[3]===0||tag==='true');
  return format==='truetype'?tt:format==='opentype'?tt||tag==='OTTO':format==='collection'?tag==='ttcf':format==='type1'?data.length>=2&&(data[0]===0x80&&data[1]===1||data[0]===37&&data[1]===33):data.length>=4&&(data[0]===1||data[0]===2)&&data[2]>=4;
}
async function verify(data:Uint8Array,entry:Entry):Promise<void>{if(data.length!==entry.sizeBytes)throw fail('font','Font resource length mismatch');if(await sha256(data)!==entry.sha256)throw fail('font','Font resource hash mismatch');if(!validFormat(data,entry.format))throw fail('font','Font resource format mismatch');}
function normalizedName(name:string):string{return name.replace(/^[A-Z]{6}\+/u,'').replace(/[^\p{L}\p{N}]/gu,'').toLowerCase();}
function candidateScore(entry:Entry,requirements:FontRequirement[]):number[]|undefined{
  let best:number[]|undefined;
  for(const need of requirements){
    const name=normalizedName(need.name),named=[entry.font.family,...entry.font.aliases].some(n=>normalizedName(n)===name);
    const language=!need.language||!validFontLanguage(need.language)?0:entry.font.languages.length?Math.min(...entry.font.languages.map(l=>fontLanguageRank(need.language,l))):3;
    const score=[named?0:1,language,entry.font.italic===need.italic?0:1,entry.font.serif===need.serif?0:1,Math.abs(entry.font.weight-need.weight)];
    const hard=need.missingCount>0;
    const improve=!hard&&!!need.language&&language<=3&&need.selected.some(s=>compare(score,s.preference??[0,s.languageRank??3,0,0,0])<0)&&!need.selected.some(s=>'sha256' in s&&s.sha256===entry.sha256&&s.index===entry.font.index);
    if(!hard&&!improve)continue;const points=hard?need.missingCodepoints:(need.codepoints??[]);
    if(points.length&&entry.ranges&&!points.some(c=>entry.ranges!.some(([a,b])=>c>=a&&c<=b)))continue;

    if(!best||compare(score,best)<0)best=score;
  }return best;
}
function compare(a:number[],b:number[]):number{for(let i=0;i<a.length;i++){if(a[i]!==b[i])return a[i]-b[i];}return 0;}

/** Configured assets only. Native document-scoped registration is the final liveness gate. */
export class FontResourceService {
  private entries:Entry[];private options:FontResourceOptions;private disposed=false;private generation=0;private next=0;
  private memory=new Map<string,Uint8Array>();private memorySize=0;private reserved=0;
  private jobs=new Map<string,Job>();private queue:Job[]=[];private active=0;
  private requests=new Set<AbortController>();
  private memoryLimit:number;private persistentLimit:number;private timeout:number;private maxLoads:number;private cacheName:string;
  constructor(manifest:FontManifest,options:FontResourceOptions={}){
    if(!keys(options,['root','load','network','persistent','cacheName','cacheStorage','memoryBytes','persistentBytes','timeoutMs','maxLoads','onEvent'])||options.load!==undefined&&typeof options.load!=='function'||options.onEvent!==undefined&&typeof options.onEvent!=='function'||options.network!==undefined&&typeof options.network!=='boolean'||options.persistent!==undefined&&typeof options.persistent!=='boolean')throw fail('invalid_argument','Invalid font resource options');
    if(options.cacheStorage!==undefined&&(!options.cacheStorage||typeof options.cacheStorage.open!=='function'||typeof options.cacheStorage.delete!=='function'))throw fail('invalid_argument','Invalid font cache storage');
    this.options={...options};this.memoryLimit=options.memoryBytes??MAX_BYTES;this.persistentLimit=options.persistentBytes??MAX_BYTES;this.timeout=options.timeoutMs??15000;this.maxLoads=options.maxLoads??16;this.cacheName=options.cacheName??'rzpdf-fonts-v1-default';
    if(!integer(this.memoryLimit,0,MAX_BYTES)||!integer(this.persistentLimit,0,MAX_BYTES)||!integer(this.timeout,100,60000)||!integer(this.maxLoads,1,128)||!/^rzpdf-fonts-v1-[a-z0-9-]{1,64}$/.test(this.cacheName))throw fail('invalid_argument','Invalid font resource limits/cache namespace');
    this.entries=normalizeManifest(manifest,resourceRoot(options.root),!!options.load);
  }
  statistics(){return {memoryBytes:this.memorySize,memoryEntries:this.memory.size,inflight:this.jobs.size,reservedBytes:this.reserved,activeLoads:this.active,activeRequests:this.requests.size,generation:this.generation};}
  private emit(event:FontResourceEvent):void{try{this.options.onEvent?.(Object.freeze({...event,stamp:event.stamp?Object.freeze({...event.stamp}):undefined}));}catch{/* Observers cannot change registration/cleanup semantics. */}}
  private check():void{if(this.disposed)throw fail('invalid_handle','Font resource service is disposed');}
  private evict(bytes:number):void{while(this.memory.size&&this.memorySize+this.reserved+bytes>MAX_BYTES){const key=this.memory.keys().next().value!;this.memorySize-=this.memory.get(key)!.length;this.memory.delete(key);}if(this.reserved+this.memorySize+bytes>MAX_BYTES)throw fail('resource_limit','Font resource byte budget exceeded');}
  private remember(key:string,data:Uint8Array):void{if(data.length>this.memoryLimit)return;const old=this.memory.get(key);if(old){this.memorySize-=old.length;this.memory.delete(key);}while(this.memory.size&&this.memorySize+data.length>this.memoryLimit){const k=this.memory.keys().next().value!;this.memorySize-=this.memory.get(k)!.length;this.memory.delete(k);}this.evict(data.length);this.memory.set(key,data);this.memorySize+=data.length;}
  private cacheTask<T>(fn:()=>Promise<T>):Promise<T>{
    const previous=cacheTails.get(this.cacheName)??Promise.resolve();
    const next=previous.then(async()=>{const locks=globalThis.navigator?.locks;if(!locks)return fn();let entered=false;try{return await locks.request('rzpdf-font-cache:'+this.cacheName,{mode:'exclusive'},()=>{entered=true;return fn();});}catch(error){if(entered)throw error;return fn();}});
    const settled=next.catch(()=>{});cacheTails.set(this.cacheName,settled);void settled.finally(()=>{if(cacheTails.get(this.cacheName)===settled)cacheTails.delete(this.cacheName);});return next;
  }
  private async cache():Promise<Cache|undefined>{if(this.options.persistent===false)return undefined;const storage=this.options.cacheStorage??globalThis.caches;if(!storage)return undefined;return storage.open(this.cacheName);}
  private cacheUrl(key:string):string{return 'https://rzpdf.invalid/font-cache/v1/'+key;}
  private async readResponse(response:Response,entry:Entry,signal:AbortSignal,stored=false):Promise<Uint8Array>{
    const length=response.headers.get('content-length');if(stored&&length!==null&&Number(length)!==entry.sizeBytes)throw fail('font','Font resource declared length mismatch');
    if(!response.body)throw fail('font','Font resource has no body');const reader=response.body.getReader();const data=new Uint8Array(entry.sizeBytes);let size=0,reads=0;
    const cancel=()=>{void reader.cancel().catch(()=>{});};signal.addEventListener('abort',cancel,{once:true});
    try{while(true){alive(signal);if(++reads>100000)throw fail('resource_limit','Font resource stream operation limit');if(reads%256===0)await abortable(new Promise<void>(resolve=>setTimeout(resolve,0)),signal);const part=await abortable(reader.read(),signal);if(part.done)break;if(size+part.value.length>entry.sizeBytes)throw fail('resource_limit','Font resource exceeds declared byte limit');data.set(part.value,size);size+=part.value.length;}if(size!==entry.sizeBytes)throw fail('font','Font resource length mismatch');return data;}
    finally{signal.removeEventListener('abort',cancel);void reader.cancel().catch(()=>{});try{reader.releaseLock();}catch{/* A cancelled pending read releases with its stream. */}}
  }
  private async read(job:Job):Promise<Loaded>{
    const {entry,key,controller}=job,signal=controller.signal;alive(signal);
    const memory=job.memory;if(memory){try{await verify(memory,entry);alive(signal);if(this.memory.get(key)===memory){this.memory.delete(key);this.memory.set(key,memory);}return {data:memory,source:'memory'};}catch(error){if(this.memory.get(key)===memory){this.memory.delete(key);this.memorySize-=memory.length;}job.memory=undefined;if(signal.aborted)throw error;this.emit({type:'cache-error',requestId:0,resourceId:entry.id,code:'font'});}}
    const extra=entry.sizeBytes*2-job.cost;if(extra>0){this.evict(extra);this.reserved+=extra;job.cost+=extra;}
    const persistent=await this.cacheTask(async()=>{alive(signal);try{const cache=await this.cache();alive(signal);if(!cache){if(this.options.persistent!==false)this.emit({type:'cache-error',requestId:0,resourceId:entry.id,code:'unsupported'});return undefined;}const response=await cache.match(this.cacheUrl(key));alive(signal);if(!response)return undefined;
      try{if(!response.ok||response.redirected)throw fail('font','Invalid cached font response');const data=await this.readResponse(response,entry,signal,true);await verify(data,entry);alive(signal);return data;}catch(error){if(signal.aborted)throw error;await cache.delete(this.cacheUrl(key));this.emit({type:'cache-error',requestId:0,resourceId:entry.id,code:'font'});return undefined;}
    }catch(error){if(signal.aborted)throw error;this.emit({type:'cache-error',requestId:0,resourceId:entry.id,code:'resource_limit'});return undefined;}});if(persistent)return {data:persistent,source:'persistent'};
    alive(signal);if(this.options.network===false)throw fail('font','Font resource unavailable offline');
    let data:Uint8Array;
    if(this.options.load){const supplied=await abortable(Promise.resolve().then(()=>this.options.load!(entry,signal)),signal);alive(signal);if(!(supplied instanceof Uint8Array)||supplied.length!==entry.sizeBytes)throw fail('font','Font callback returned invalid bytes');data=supplied.slice();}
    else{let response:Response;try{response=await fetch(entry.resolvedUrl!,{signal,redirect:'error',credentials:'omit',cache:'no-store'});}catch(error){if(signal.aborted)aborted(signal);throw fail('font','Font resource network request failed');}if(!response.ok||response.redirected)throw fail('font','Font resource response refused');data=await this.readResponse(response,entry,signal);}
    await verify(data,entry);alive(signal);return {data,source:this.options.load?'callback':'network'};
  }
  private pump():void{while(this.active<2&&this.queue.length){const job=this.queue.shift()!;if(job.controller.signal.aborted){job.reject(fail('cancelled','Font resource request cancelled'));continue;}this.active++;
      const timer=setTimeout(()=>job.controller.abort(fail('resource_limit','Font resource load timed out')),this.timeout);
      void abortable(this.read(job),job.controller.signal).then(value=>{alive(job.controller.signal);job.value=value;job.done=true;this.reserved-=job.cost-value.data.length;job.cost=value.data.length;job.resolve(value);},error=>{job.done=true;job.reject(error);}).catch(error=>job.reject(error)).finally(()=>{clearTimeout(timer);this.active--;this.pump();});
    }}
  private async acquire(entry:Entry,key:string,signal:AbortSignal):Promise<Lease>{
    this.check();alive(signal);let job=this.jobs.get(key);if(job?.controller.signal.aborted){if(this.jobs.get(key)===job)this.jobs.delete(key);job=undefined;}
    if(!job){if(this.jobs.size>=128)throw fail('resource_limit','Font resource queue is full');const memory=this.memory.get(key),cost=entry.sizeBytes*(memory?1:2);this.evict(cost);let resolve!:(v:Loaded)=>void,reject!:(e:unknown)=>void;const promise=new Promise<Loaded>((a,b)=>{resolve=a;reject=b;});void promise.catch(()=>{});
      job={key,entry,controller:new AbortController(),promise,resolve,reject,refs:0,cost,done:false,memory};this.reserved+=job.cost;this.jobs.set(key,job);this.queue.push(job);
    }
    const current=job;current.refs++;let released=false;
    const release=()=>{if(released)return;released=true;current.refs--;if(current.refs===0){if(this.jobs.get(key)===current)this.jobs.delete(key);this.reserved-=current.cost;current.cost=0;if(!current.done){current.controller.abort();const index=this.queue.indexOf(current);if(index>=0){this.queue.splice(index,1);current.reject(fail('cancelled','Font resource request cancelled'));}}if(current.value)current.value.data=new Uint8Array();current.memory=undefined;}};
    try{this.pump();const value=await abortable(current.promise,signal);alive(signal);const lease:Lease={...value,release:()=>{release();lease.data=new Uint8Array();lease.release=()=>{};}};return lease;}catch(error){release();throw error;}
  }
  private async commit(key:string,entry:Entry,data:Uint8Array,signal:AbortSignal,requestId:number):Promise<void>{
    alive(signal);
    if(entry.sizeBytes<=this.persistentLimit)await this.cacheTask(async()=>{alive(signal);let held=0;try{
      const cache=await this.cache();alive(signal);if(!cache)return;const keys=await cache.keys();alive(signal);let total=0;const existing:{request:Request;size:number}[]=[];
      for(const request of keys){if(request.url===this.cacheUrl(key))continue;const response=await cache.match(request);alive(signal);const size=Number(response?.headers.get('x-rzpdf-font-size'));if(!integer(size,1,32*1024*1024)||!request.url.startsWith('https://rzpdf.invalid/font-cache/v1/')){await cache.delete(request);continue;}existing.push({request,size});total+=size;}
      while(existing.length&&(total+data.length>this.persistentLimit||existing.length>=128)){const old=existing.shift()!;await cache.delete(old.request);total-=old.size;}
      alive(signal);this.evict(data.length);this.reserved+=data.length;held=data.length;
      await cache.put(this.cacheUrl(key),new Response(data as Uint8Array<ArrayBuffer>,{headers:{'content-length':String(data.length),'x-rzpdf-font-size':String(data.length),'content-type':'application/octet-stream'}}));
      if(signal.aborted){await cache.delete(this.cacheUrl(key));aborted(signal);}
    }catch(error){if(signal.aborted)throw error;this.emit({type:'cache-error',requestId,resourceId:entry.id,code:'resource_limit'});}finally{this.reserved-=held;}});
    alive(signal);try{this.remember(key,data);}catch{this.emit({type:'cache-error',requestId,resourceId:entry.id,code:'resource_limit'});}
  }
  async ensure(document:PdfDocument|WorkerPdfDocument,page:number,options:RenderOptions={},task:{signal?:AbortSignal}={}):Promise<FontRequirements>{
    this.check();if(this.requests.size>=16)throw fail('resource_limit','Too many font resource requests');if(this.next>=Number.MAX_SAFE_INTEGER)throw fail('resource_limit','Font request IDs exhausted');
    let renderOptions:Required<RenderOptions>;try{renderOptions=normalizeRenderOptions(options);}catch{throw fail('invalid_argument','Invalid font preflight options');}
    if(!keys(task,['signal'])||task.signal!==undefined&&(!task.signal||typeof task.signal.aborted!=='boolean'||typeof task.signal.addEventListener!=='function'||typeof task.signal.removeEventListener!=='function'))throw fail('invalid_argument','Invalid font resource task');
    const controller=new AbortController(),signal=controller.signal,parent=task.signal,stop=()=>controller.abort(parent?.reason);if(parent?.aborted)stop();else parent?.addEventListener('abort',stop,{once:true});
    const requestId=++this.next,generation=this.generation;this.requests.add(controller);let stamp:FontStamp|undefined;
    try{alive(signal);let report=await abortable(Promise.resolve(document.fontRequirements(page,renderOptions,{signal})),signal);stamp=normalizeFontStamp(report.stamp);if(stamp.fontEpoch===undefined)throw fail('unsupported','Font preflight lacks a cache epoch');
      const attempted=new Set<string>();
      while(attempted.size<this.maxLoads){alive(signal);if(this.generation!==generation)throw fail('cancelled','Font resource configuration changed');
        const candidates=this.entries.filter(e=>!attempted.has(e.id)).map(entry=>({entry,score:candidateScore(entry,report.fonts)})).filter(v=>v.score).sort((a,b)=>compare(a.score!,b.score!));
        if(!candidates.length)break;const entry=candidates[0].entry;attempted.add(entry.id);this.emit({type:'request',requestId,resourceId:entry.id,stamp});
        const key=await sha256(new TextEncoder().encode(entry.canonical));alive(signal);let lease:Lease|undefined;
        try{lease=await this.acquire(entry,key,signal);alive(signal);this.emit({type:lease.source==='memory'||lease.source==='persistent'?'cache-hit':'loaded',requestId,resourceId:entry.id,stamp,source:lease.source});
          alive(signal);const identity=await abortable(Promise.resolve(document.registerFont(lease.data,entry.font,stamp,{signal})),signal);alive(signal);if(identity.sha256!==entry.sha256||identity.index!==entry.font.index)throw fail('font','Registered font identity differs from manifest');
          this.emit({type:'registered',requestId,resourceId:entry.id,stamp,source:lease.source});await this.commit(key,entry,lease.data,signal,requestId);
        }finally{lease?.release();}
        report=await abortable(Promise.resolve(document.fontRequirements(page,renderOptions,{signal})),signal);
        const next=normalizeFontStamp(report.stamp);if(next.engine!==stamp.engine||next.document!==stamp.document||next.revision!==stamp.revision||next.fontEpoch!==stamp.fontEpoch)throw fail('invalid_handle','Font preflight changed while loading');
      }
      alive(signal);if(!report.complete)this.emit({type:'unresolved',requestId,stamp});return report;
    }catch(error){const known=error instanceof PdfApiError?error:fail('font','Font resource loading failed');this.emit({type:known.code==='cancelled'?'cancelled':'failed',requestId,stamp,code:known.code});throw known;}
    finally{parent?.removeEventListener('abort',stop);this.requests.delete(controller);}
  }
  private invalidate(clearMemory=true):void{this.generation++;for(const request of [...this.requests])request.abort(fail('cancelled','Font resource service invalidated'));for(const job of [...this.jobs.values()])job.controller.abort();if(clearMemory){this.memory.clear();this.memorySize=0;}}
  cancel():void{this.check();this.invalidate(false);}
  async clearCache():Promise<void>{this.check();this.invalidate();try{await this.cacheTask(async()=>{const storage=this.options.cacheStorage??globalThis.caches;if(storage)await storage.delete(this.cacheName);});}catch{this.emit({type:'cache-error',requestId:0,code:'resource_limit'});throw fail('resource_limit','Persistent font cache could not be cleared');}}
  dispose():void{if(this.disposed)return;this.disposed=true;this.invalidate();}
}
