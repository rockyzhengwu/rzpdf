import type {RenderOptions,MutationRequest,SavedPdf} from "./interaction.js";
import {createEngine,PdfApiError,type PdfEngine,type PdfDocument,type OpenOptions,type RgbaPage} from './client.js';
import {WORKER_PROTOCOL,validateArguments,type WorkerRequest,type WorkerResponse,type OpenedDocument} from './protocol.js';
const scope=globalThis as unknown as {onmessage:((event:MessageEvent<unknown>)=>void)|null;postMessage:(message:WorkerResponse,transfer?:Transferable[])=>void};
let session:string|undefined,engine:PdfEngine|undefined,nextDocument=0,lastTask=0;
const documents=new Map<number,PdfDocument>();
function send(request:WorkerRequest,payload:Omit<WorkerResponse,'protocol'|'session'|'id'>,transfer:Transferable[]=[]):void {
  scope.postMessage({protocol:WORKER_PROTOCOL,session:request.session,id:request.id,...payload} as WorkerResponse,transfer);
}
function document(request:WorkerRequest):PdfDocument {
  const doc=documents.get(request.document!);
  if(!doc)throw new PdfApiError('invalid_handle','Invalid Worker document','document');
  return doc;
}
async function execute(request:WorkerRequest):Promise<void> {
  if(!request||typeof request.session!=='string'||!Number.isSafeInteger(request.id))return;
  try {
    if(!request || request.protocol!==WORKER_PROTOCOL || typeof request.session!=='string' || !request.session || !Number.isSafeInteger(request.id) || request.id<0 || !Array.isArray(request.args))
      throw new PdfApiError('invalid_argument','Invalid Worker request','protocol');
    try { validateArguments(request.operation,request.args); } catch { throw new PdfApiError('invalid_argument','Invalid Worker arguments',request.operation); }
    if(request.operation==='initialize') {
      if(session || request.id!==0)throw new PdfApiError('invalid_handle','Worker already initialized','initialize');
      session=request.session;engine=await createEngine();send(request,{kind:'ready'});return;
    }
    if(request.session!==session || !engine)throw new PdfApiError('invalid_handle','Stale Worker session','session');
    if(request.id<=lastTask)throw new PdfApiError('invalid_handle','Stale Worker task ID','protocol');
    lastTask=request.id;
    send(request,{kind:'started'});
    let result:unknown;
    switch(request.operation) {
      case 'open': {
        if(nextDocument>=0xffffffff)throw new PdfApiError('resource_limit','Document ID exhausted','open');
        const doc=engine.open(request.args[0] as Uint8Array,request.args[1] as OpenOptions);
        let count:number;
        try {count=doc.pageCount;} catch(error) {doc.close();throw error;}
        const id=++nextDocument;documents.set(id,doc);
        result={document:id,pageCount:count} satisfies OpenedDocument;break;
      }
      case 'registerDocumentFont':result=document(request).registerFont(request.args[0] as Uint8Array,request.args[1] as import('./fonts.js').FontRegistration,request.args[2] as import('./fonts.js').FontStamp);break;
      case 'fontRequirements':result=document(request).fontRequirements(request.args[0] as number,request.args[1] as RenderOptions);break;
      case 'registerFont':result=engine.registerFont(request.args[0] as Uint8Array,request.args[1] as import('./fonts.js').FontRegistration);break;
      case 'clearFonts':engine.clearFonts();result=null;break;
      case 'font':engine.setSubstituteFont(request.args[0] as Uint8Array|undefined,request.args[1] as number);result=null;break;
      case 'statistics':result={...engine.statistics(),documents:documents.size};break;
      case 'close':document(request).close();documents.delete(request.document!);result=null;break;
      case 'pageInfo':result=document(request).pageInfo(request.args[0] as number);break;
      case 'text':result=document(request).extractText(request.args[0] as number);break;
      case 'renderRegion':result=document(request).renderRegion(request.args[0] as number,request.args[3] as import('./raster.js').RasterRegion,request.args[1] as number,request.args[2] as RenderOptions);break;
      case 'render':result=document(request).render(request.args[0] as number,request.args[1] as number,request.args[2] as RenderOptions);break;
      case 'search':result=document(request).searchText(request.args[0] as string,request.args[1] as {caseSensitive?:boolean;maxResults?:number});break;
      case 'state':result=document(request).state();break;
      case 'annotations':result=document(request).annotations(request.args[0] as number);break;
      case 'formFields':result=document(request).formFields();break;
      case 'content':result=document(request).content(request.args[0] as number);break;
      case 'apply':result=document(request).apply(request.args[0] as MutationRequest,request.args[1] as Uint8Array|undefined);break;
      case 'save':result=document(request).save(request.args[0] as number);break;
      case 'outlines':result=document(request).outlines();break;
      case 'links':result=document(request).links(request.args[0] as number);break;
      case 'capabilities':result=document(request).capabilities();break;
      case 'diagnostics':result=document(request).diagnostics();break;
      default:throw new PdfApiError('unsupported','Unsupported Worker operation','protocol');
    }
    const transfer=(request.operation==='render'||request.operation==='renderRegion')?[(result as RgbaPage).pixels.buffer]:request.operation==='save'?[(result as SavedPdf).bytes.buffer]:[];
    send(request,{kind:'result',value:result} as Omit<WorkerResponse,'protocol'|'session'|'id'>,transfer);
  }catch(error) {
    const known=error instanceof PdfApiError;
    const fatal=!known || error.code==='internal';
    // Never serialize the request, argument list or password.
    send(request,{kind:'error',error:{code:known?error.code:'internal',message:known?error.message:'Worker execution failed',operation:known?error.operation:'worker',page:known?error.page:undefined,fatal}} as Omit<WorkerResponse,'protocol'|'session'|'id'>);
  }
}
let chain=Promise.resolve();
scope.onmessage=event=>{chain=chain.then(()=>execute(event.data as WorkerRequest));};
