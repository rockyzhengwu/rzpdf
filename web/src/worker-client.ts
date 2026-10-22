import {normalizeRasterRegion,type RasterRegion} from "./raster.js";
import {normalizeFontRegistration,normalizeFontStamp,type FontRegistration,type FontIdentity,type FontStamp,type FontRequirements} from "./fonts.js";
import {normalizeRenderOptions,snapshotMutation,type RenderOptions,type InteractionState,type AnnotationSnapshot,type FormSnapshot,type MutationRequest,type MutationResult,type SavedPdf,type ContentSnapshot} from "./interaction.js";
import {PdfApiError,type OpenOptions,type PageInfo,type TextPage,type OutlineItem,type PageLink,type RgbaPage,type SearchReport,type ErrorCode} from './client.js';
import {WORKER_PROTOCOL,validateArguments,type WorkerOperation,type WorkerRequest,type WorkerResponse,type OpenedDocument} from './protocol.js';
export interface TaskOptions { signal?:AbortSignal; }
export interface WorkerEngineOptions { workerUrl?:URL; taskTimeoutMs?:number; }
interface Job {
  id:number;session:string;operation:WorkerOperation;document?:number;args:unknown[];
  bytes:number;transfer:Transferable[];resolve:(value:unknown)=>void;reject:(error:PdfApiError)=>void;
  signal?:AbortSignal;abort?:()=>void;timer?:ReturnType<typeof setTimeout>;
}
const documentKey=Symbol('Worker document');
function sessionIdentity():string {
  if(typeof crypto==='undefined'||typeof crypto.getRandomValues!=='function')throw new PdfApiError('unsupported','Secure randomness is unavailable','initialize');
  const bytes=new Uint8Array(16);crypto.getRandomValues(bytes);
  return Array.from(bytes,b=>b.toString(16).padStart(2,'0')).join('');
}

const MAX_TASKS=32,MAX_BYTES=64*1024*1024;
function validateTask(task:TaskOptions):void {
  if(!task||typeof task!=='object'||Object.keys(task).some(k=>k!=='signal'))throw new PdfApiError('invalid_argument','Invalid task options','task');
  const signal=task.signal;
  if(signal!==undefined&&(!signal||typeof signal.aborted!=='boolean'||typeof signal.addEventListener!=='function'||typeof signal.removeEventListener!=='function'))throw new PdfApiError('invalid_argument','Invalid abort signal','task');
}

/** One serialized Worker session. Active cancellation invalidates every document. */
export class WorkerPdfEngine extends EventTarget {
  private worker:Worker|undefined;
  private session=sessionIdentity();
  private ready:Promise<void>|undefined;
  private readyResolve:(()=>void)|undefined;
  private readyReject:((error:PdfApiError)=>void)|undefined;
  private readyTimer:ReturnType<typeof setTimeout>|undefined;
  private initialized=false;
  private disposed=false;
  private next=0;
  private queue:Job[]=[];
  private active:Job|undefined;
  private bytes=0;
  private documents=new Set<number>();
  private timeout:number;
  constructor(private options:WorkerEngineOptions={}) {
    super();
    if(!options||typeof options!=='object'||Object.keys(options).some(k=>!['workerUrl','taskTimeoutMs'].includes(k))||(options.workerUrl!==undefined&&!(options.workerUrl instanceof URL)))throw new PdfApiError('invalid_argument','Invalid Worker engine options','initialize');
    this.options={...options,workerUrl:options.workerUrl?new URL(options.workerUrl.href):undefined};
    this.timeout=options.taskTimeoutMs??60_000;
    if(!Number.isFinite(this.timeout)||this.timeout<100||this.timeout>120_000)throw new PdfApiError('invalid_argument','Task timeout must be 100–120000 ms','initialize');
  }
  get sessionId():string {return this.session;}
  get busy():boolean {return !!this.active||this.queue.length>0||!!this.worker&&!this.initialized;}
  private changed():void {this.dispatchEvent(new Event("queuechange"));}
  private valid():void {if(this.disposed)throw new PdfApiError('invalid_handle','Worker engine is disposed','session');if(typeof Worker==='undefined')throw new PdfApiError('unsupported','Web Workers are unavailable','initialize');}
  initialize():Promise<void> {
    this.valid();if(this.ready)return this.ready;
    this.session=sessionIdentity();const session=this.session;
    this.ready=new Promise<void>((resolve,reject)=>{this.readyResolve=resolve;this.readyReject=reject;});
    // Queued requests have their own rejection paths; bootstrap may also be awaited.
    void this.ready.catch(()=>{});
    let worker:Worker;
    try {worker=new Worker(this.options.workerUrl??new URL('./pdf-worker.js',import.meta.url),{type:'module',name:'rzpdf'});}
    catch {const error=new PdfApiError('internal','Worker could not start','initialize');const ready=this.ready;this.reset(error);return ready;}
    this.worker=worker;
    worker.onmessage=event=>{
      if(this.worker!==worker||this.session!==session)return;
      const response=event.data as WorkerResponse;
      if(!response||response.protocol!==WORKER_PROTOCOL||response.session!==session)return;
      if(response.kind==='ready'&&response.id===0) {
        if(this.initialized)return;
        this.initialized=true;clearTimeout(this.readyTimer);this.readyResolve?.();this.readyResolve=undefined;this.readyReject=undefined;this.pump();return;
      }
      if(response.kind==='error'&&response.id===0) {this.reset(new PdfApiError(response.error.code,response.error.message,response.error.operation,undefined,0,session));return;}
      const job=this.active;
      if(!job||response.id!==job.id)return;
      if(response.kind==='started') {this.dispatchEvent(new CustomEvent('taskstart',{detail:{id:job.id,session,operation:job.operation}}));return;}
      if(response.kind==='error') {
        const error=new PdfApiError(response.error.code,response.error.message,response.error.operation,response.error.page,job.id,session);
        if(response.error.fatal){this.reset(error);return;}
        this.finish(job);job.reject(error);this.pump();return;
      }
      if(response.kind==='result') {this.finish(job);job.resolve(response.value);this.pump();}
    };
    worker.onerror=event=>{event.preventDefault();if(this.worker===worker)this.reset(new PdfApiError('internal','Worker execution failed; reopen the document','worker',undefined,this.active?.id,session));};
    worker.onmessageerror=()=>{if(this.worker===worker)this.reset(new PdfApiError('internal','Worker response could not be decoded','worker',undefined,this.active?.id,session));};
    this.readyTimer=setTimeout(()=>{if(this.worker===worker)this.reset(new PdfApiError('resource_limit','Worker initialization timed out','initialize',undefined,0,session));},120_000);
    worker.postMessage({protocol:WORKER_PROTOCOL,session,id:0,operation:'initialize',args:[]} satisfies WorkerRequest);
    return this.ready;
  }
  private error(job:Job,code:ErrorCode,message:string):PdfApiError {
    const page=['render','renderRegion','text','pageInfo','links'].includes(job.operation)?job.args[0] as number:undefined;
    return new PdfApiError(code,message,job.operation,page,job.id,job.session);
  }
  private detach(job:Job):void {clearTimeout(job.timer);if(job.abort)job.signal?.removeEventListener('abort',job.abort);this.bytes-=job.bytes;}
  private finish(job:Job):void {this.detach(job);if(this.active===job)this.active=undefined;this.changed();}
  private reset(error:PdfApiError):void {
    const old=this.session;
    clearTimeout(this.readyTimer);const worker=this.worker;this.worker=undefined;
    if(worker){worker.onmessage=worker.onerror=worker.onmessageerror=null;worker.terminate();}
    this.initialized=false;
    this.readyReject?.(error);this.readyResolve=undefined;this.readyReject=undefined;this.ready=undefined;
    const jobs=[...(this.active?[this.active]:[]),...this.queue];this.active=undefined;this.queue=[];
    for(const job of jobs){this.detach(job);job.reject(this.error(job,error.code,error.message));}
    this.bytes=0;this.documents.clear();this.session=sessionIdentity();
    this.dispatchEvent(new CustomEvent('sessionend',{detail:{session:old,code:error.code,message:error.message}}));
    this.changed();
  }
  private pump():void {
    if(!this.initialized||!this.worker||this.active||!this.queue.length)return;
    const job=this.queue.shift()!;this.active=job;
    job.timer=setTimeout(()=>{if(this.active===job)this.reset(this.error(job,'resource_limit','Worker task timed out; reopen the document'));},this.timeout);
    try {this.worker.postMessage({protocol:WORKER_PROTOCOL,session:job.session,id:job.id,operation:job.operation,document:job.document,args:job.args} satisfies WorkerRequest,job.transfer);}
    catch {this.finish(job);job.reject(this.error(job,'invalid_argument','Worker request could not be transferred'));this.pump();}
  }
  private enqueue<T>(operation:WorkerOperation,args:unknown[],document?:number,task:TaskOptions={},bytes=0,transfer:Transferable[]=[]):Promise<T> {
    try {
      this.valid();validateTask(task);
      args=args.slice();
      if(['open','search'].includes(operation)&&args[1]&&typeof args[1]==='object'&&!Array.isArray(args[1]))args[1]={...args[1]};
      if(operation==='apply')args[0]=snapshotMutation(args[0] as MutationRequest);
      if(operation==='render'||operation==='renderRegion')args[2]=normalizeRenderOptions(args[2] as RenderOptions);
      if(operation==='renderRegion')args[3]=normalizeRasterRegion(args[3] as RasterRegion);
      validateArguments(operation,args);
    }catch(error){return Promise.reject(error instanceof PdfApiError?error:new PdfApiError('invalid_argument','Invalid Worker arguments',operation));}
    if(task.signal?.aborted)return Promise.reject(new PdfApiError('cancelled','Task was cancelled before dispatch',operation));
    if(this.queue.length+Number(!!this.active)>=(operation==='close'?MAX_TASKS+8:MAX_TASKS)||this.bytes+bytes>MAX_BYTES)return Promise.reject(new PdfApiError('resource_limit','Worker task queue exceeds its budget',operation));
    const initialization=this.initialize();void initialization.catch(()=>{});
    if(!this.worker)return Promise.reject(new PdfApiError('internal','Worker is unavailable',operation));
    if(this.next>=Number.MAX_SAFE_INTEGER)return Promise.reject(new PdfApiError('resource_limit','Task IDs exhausted; recreate the engine',operation));
    return new Promise<T>((resolve,reject)=>{
      const job:Job={id:++this.next,session:this.session,operation,document,args,bytes,transfer,resolve:value=>resolve(value as T),reject,signal:task.signal};
      job.abort=()=>{
        if(this.active===job){this.reset(this.error(job,'cancelled','Active task cancelled; reopen documents in a new session'));return;}
        const index=this.queue.indexOf(job);if(index<0)return;
        this.queue.splice(index,1);this.detach(job);this.changed();job.reject(this.error(job,'cancelled','Queued task cancelled'));
      };
      this.bytes+=bytes;this.queue.push(job);task.signal?.addEventListener('abort',job.abort,{once:true});this.changed();
      if(task.signal?.aborted)job.abort();else this.pump();
    });
  }
  private admit(bytes:number,operation:string,task:TaskOptions):void {
    this.valid();
    if(task.signal?.aborted)throw new PdfApiError('cancelled','Task cancelled before dispatch',operation);
    if(this.queue.length+Number(!!this.active)>=MAX_TASKS||this.bytes+bytes>MAX_BYTES)throw new PdfApiError('resource_limit','Worker task queue exceeds its budget',operation);
  }
  async open(bytes:Uint8Array,options:OpenOptions={},task:TaskOptions={}):Promise<WorkerPdfDocument> {
    this.valid();validateTask(task);
    if(!(bytes instanceof Uint8Array)||!bytes.length||bytes.length>8*1024*1024)throw new PdfApiError('resource_limit','PDF input must contain 1 byte to 8 MiB','open');
    if(task.signal?.aborted)throw new PdfApiError('cancelled','Open was cancelled','open');
    try {validateArguments('open',[bytes,options]);}catch{throw new PdfApiError('invalid_argument','Invalid open options','open');}
    this.admit(bytes.length,"open",task);
    const owned=bytes.slice();
    const opening=this.enqueue<OpenedDocument>('open',[owned,options],undefined,task,owned.length,[owned.buffer]);
    const session=this.session,result=await opening;this.checkSession(session);
    this.documents.add(result.document);
    return new WorkerPdfDocument(this,session,result.document,result.pageCount,documentKey);
  }
  async registerFont(bytes:Uint8Array,options:FontRegistration,task:TaskOptions={}):Promise<FontIdentity> {
    this.valid();validateTask(task);
    let metadata:Required<FontRegistration>;
    try {metadata=normalizeFontRegistration(options);}catch{throw new PdfApiError('invalid_argument','Invalid font metadata','registerFont');}
    if(!(bytes instanceof Uint8Array)||!bytes.length||bytes.length>32*1024*1024)throw new PdfApiError('resource_limit','Font must contain 1 byte to 32 MiB','registerFont');
    this.admit(bytes.length,'registerFont',task);
    const owned=bytes.slice();
    return this.enqueue('registerFont',[owned,metadata],undefined,task,owned.length,[owned.buffer]);
  }
  async clearFonts(task:TaskOptions={}):Promise<void> {return this.enqueue('clearFonts',[],undefined,task);}
  async setSubstituteFont(bytes?:Uint8Array,index=0,task:TaskOptions={}):Promise<void> {
    this.valid();validateTask(task);
    if(bytes&&(!(bytes instanceof Uint8Array)||!bytes.length||bytes.length>32*1024*1024))throw new PdfApiError('resource_limit','Font must contain 1 byte to 32 MiB','registerFont');
    try {validateArguments('font',[bytes,index]);}catch{throw new PdfApiError('invalid_argument','Invalid font arguments','registerFont');}
    this.admit(bytes?.length??0,'registerFont',task);
    const owned=bytes?.slice();
    await this.enqueue('font',[owned,index],undefined,task,owned?.length??0,owned?[owned.buffer]:[]);
  }
  statistics(task:TaskOptions={}):Promise<{handles:number;retainedBytes:number;documents:number}> {return this.enqueue('statistics',[],undefined,task);}
  checkSession(session:string):void {
    this.valid();if(session!==this.session||!this.worker)throw new PdfApiError('invalid_handle','Document belongs to an expired Worker session','session',undefined,undefined,session);
  }
  checkDocument(session:string,id:number):void {
    this.checkSession(session);if(!this.documents.has(id))throw new PdfApiError('invalid_handle','Document is closed or belongs to another engine','document');
  }
  callDocument<T>(session:string,id:number,operation:WorkerOperation,args:unknown[],task:TaskOptions={}):Promise<T> {
    try {this.checkDocument(session,id);if(!['pageInfo','text','render','renderRegion','search','outlines','links','capabilities','diagnostics','state','annotations','formFields','apply','save','content','fontRequirements','registerDocumentFont'].includes(operation))throw new PdfApiError('invalid_argument','Not a document operation',operation);}catch(error){return Promise.reject(error);}
    if(operation==='registerDocumentFont') {
      try {
        validateArguments(operation,args);this.admit((args[0] as Uint8Array).length,'registerFont',task);
        const metadata=normalizeFontRegistration(args[1] as FontRegistration),stamp=normalizeFontStamp(args[2] as FontStamp),owned=(args[0] as Uint8Array).slice();
        return this.enqueue(operation,[owned,metadata,stamp],id,task,owned.length,[owned.buffer]);
      }catch(error){return Promise.reject(error instanceof PdfApiError?error:new PdfApiError('invalid_argument','Invalid document font input','registerFont'));}
    }
    if(operation==='fontRequirements') {
      try{args=[args[0],normalizeRenderOptions(args[1] as RenderOptions)];}catch{return Promise.reject(new PdfApiError('invalid_argument','Invalid render options','fontRequirements'));}
    }
    if(operation==='apply'&&args[1]!==undefined) {
      try {validateArguments(operation,args);this.admit((args[1] as Uint8Array).length,'apply',task);}catch(error){return Promise.reject(error instanceof PdfApiError?error:new PdfApiError('invalid_argument','Invalid replacement input','apply'));}
      const owned=(args[1] as Uint8Array).slice();return this.enqueue(operation,[args[0],owned],id,task,owned.length,[owned.buffer]);
    }
    return this.enqueue(operation,args,id,task);
  }
  async closeDocument(session:string,id:number):Promise<void> {
    if(this.disposed||session!==this.session||!this.worker||!this.documents.delete(id))return;
    const remaining:Job[]=[];
    for(const job of this.queue) {
      if(job.document===id){this.detach(job);job.reject(this.error(job,'invalid_handle','Document was closed'));}
      else remaining.push(job);
    }
    this.queue=remaining;
    if(this.active?.document===id){this.reset(this.error(this.active,'cancelled','Document closed during an active task; reopen other documents'));return;}
    await this.enqueue('close',[],id);
  }
  /** Hard cancel all queued/active work and invalidate every current document. */
  cancel():void {this.valid();if(!this.worker&&!this.active&&!this.queue.length)return;this.reset(new PdfApiError('cancelled','Worker session cancelled; reopen the document','cancel'));}
  dispose():void {if(this.disposed)return;this.disposed=true;this.reset(new PdfApiError('cancelled','Worker engine disposed','dispose'));}
}
export class WorkerPdfDocument {
  private closed=false;
  private closing:Promise<void>|undefined;
  constructor(private engine:WorkerPdfEngine,private session:string,private id:number,private count:number,key:typeof documentKey) {
    if(key!==documentKey)throw new PdfApiError('invalid_handle','Documents must be opened by their engine','document');
  }
  private check():void {if(this.closed)throw new PdfApiError('invalid_handle','Document is closed','document');this.engine.checkDocument(this.session,this.id);}
  get pageCount():number {this.check();return this.count;}
  private call<T>(operation:WorkerOperation,args:unknown[]=[],task:TaskOptions={}):Promise<T> {
    try {this.check();}catch(error){return Promise.reject(error);}
    return this.engine.callDocument(this.session,this.id,operation,args,task);
  }
  pageInfo(page:number,task:TaskOptions={}):Promise<PageInfo>{return this.call('pageInfo',[page],task);}
  fontRequirements(page:number,options:RenderOptions={},task:TaskOptions={}):Promise<FontRequirements>{return this.call('fontRequirements',[page,options],task);}
  registerFont(bytes:Uint8Array,options:FontRegistration,stamp:FontStamp,task:TaskOptions={}):Promise<FontIdentity>{return this.call('registerDocumentFont',[bytes,options,stamp],task);}
  extractText(page:number,task:TaskOptions={}):Promise<TextPage>{return this.call('text',[page],task);}
  renderRegion(page:number,region:RasterRegion,dpi=72,options:TaskOptions&RenderOptions={}):Promise<import('./client.js').RgbaRegion>{
    try{const {signal,...render}=options;return this.call('renderRegion',[page,dpi,normalizeRenderOptions(render),normalizeRasterRegion(region)],{signal});}
    catch{return Promise.reject(new PdfApiError('invalid_argument','Invalid render region or options','renderRegion',page));}
  }
  render(page=0,dpi=72,options:TaskOptions&RenderOptions={}):Promise<RgbaPage>{
    try {
      if(!options||typeof options!=="object")throw new RangeError();
      const {signal,...render}=options;return this.call('render',[page,dpi,normalizeRenderOptions(render)],{signal});
    }catch{return Promise.reject(new PdfApiError('invalid_argument','Invalid render options','render',page));}
  }
  state(task:TaskOptions={}):Promise<InteractionState>{return this.call('state',[],task);}
  annotations(page:number,task:TaskOptions={}):Promise<AnnotationSnapshot>{return this.call('annotations',[page],task);}
  formFields(task:TaskOptions={}):Promise<FormSnapshot>{return this.call('formFields',[],task);}
  content(page:number,task:TaskOptions={}):Promise<ContentSnapshot>{return this.call('content',[page],task);}
  async apply(request:MutationRequest,task:TaskOptions&{asset?:Uint8Array}={}):Promise<MutationResult>{
    if(!task||typeof task!=='object')throw new PdfApiError('invalid_argument','Invalid mutation options','apply');
    const {asset,...options}=task;const result=await this.call<MutationResult>('apply',[request,asset],options);this.count=result.pageCount;return result;
  }
  save(options:{maxBytes?:number}={},task:TaskOptions={}):Promise<SavedPdf>{
    if(!options||typeof options!=="object"||Object.keys(options).some(k=>k!=="maxBytes"))return Promise.reject(new PdfApiError('invalid_argument','Invalid save options','save'));
    return this.call('save',[options.maxBytes??8*1024*1024],task);
  }
  searchText(query:string,options:{caseSensitive?:boolean;maxResults?:number}={},task:TaskOptions={}):Promise<SearchReport>{return this.call('search',[query,options],task);}
  outlines(task:TaskOptions={}):Promise<OutlineItem[]>{return this.call('outlines',[],task);}
  links(page:number,task:TaskOptions={}):Promise<PageLink[]>{return this.call('links',[page],task);}
  capabilities(task:TaskOptions={}):Promise<Record<string,unknown>>{return this.call('capabilities',[],task);}
  diagnostics(task:TaskOptions={}):Promise<{reason:string;offset:number;object:number[]|null}[]>{return this.call('diagnostics',[],task);}
  close():Promise<void>{if(this.closing)return this.closing;this.closed=true;this.closing=this.engine.closeDocument(this.session,this.id);return this.closing;}
}
export async function createWorkerEngine(options:WorkerEngineOptions={}):Promise<WorkerPdfEngine> {
  const engine=new WorkerPdfEngine(options);try {await engine.initialize();return engine;}catch(error){engine.dispose();throw error;}
}
