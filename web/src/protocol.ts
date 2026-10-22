import {normalizeRasterRegion} from "./raster.js";
import {normalizeFontRegistration,normalizeFontStamp,type FontRegistration,type FontStamp} from "./fonts.js";
import {normalizeRenderOptions,snapshotMutation,mutationAssetLimit} from "./interaction.js";
import type { ErrorCode, OpenOptions } from './client.js';
export const WORKER_PROTOCOL=2;
export type WorkerOperation='open'|'font'|'registerFont'|'registerDocumentFont'|'fontRequirements'|'clearFonts'|'pageInfo'|'text'|'render'|'renderRegion'|'search'|'outlines'|'links'|'capabilities'|'diagnostics'|'close'|'statistics'|'state'|'annotations'|'formFields'|'apply'|'save'|'content';
export interface WorkerRequest { protocol:number;session:string;id:number;operation:WorkerOperation|'initialize';document?:number;args:unknown[]; }
export interface WorkerFailure { code:ErrorCode;message:string;operation:string;page?:number;fatal:boolean; }
export type WorkerResponse =
  | {protocol:number;session:string;id:number;kind:'ready'}
  | {protocol:number;session:string;id:number;kind:'started'}
  | {protocol:number;session:string;id:number;kind:'result';value:unknown}
  | {protocol:number;session:string;id:number;kind:'error';error:WorkerFailure};
export interface OpenedDocument { document:number;pageCount:number; }
export type {OpenOptions};

/** Validate before structuredClone so malformed calls cannot enqueue unbounded metadata. */
export function validateArguments(operation:WorkerRequest['operation'],args:unknown[]):void {
  const counts:Record<string,number>={initialize:0,open:2,font:2,registerFont:2,registerDocumentFont:3,fontRequirements:2,clearFonts:0,pageInfo:1,text:1,render:3,renderRegion:4,search:2,outlines:0,links:1,capabilities:0,diagnostics:0,close:0,statistics:0,state:0,annotations:1,formFields:0,apply:2,save:1,content:1};
  const invalid=()=>{throw new RangeError('Invalid Worker arguments');};
  if(!Array.isArray(args)||counts[operation]!==args.length)invalid();
  const integer=(n:unknown)=>typeof n==='number'&&Number.isSafeInteger(n)&&n>=0&&n<=0xffffffff;
  const record=(v:unknown):v is Record<string,unknown>=>!!v&&typeof v==='object'&&!Array.isArray(v);
  if(['pageInfo','text','render','renderRegion','links','annotations','content','fontRequirements'].includes(operation)&&!integer(args[0]))invalid();
  if((operation==='render'||operation==='renderRegion')&&(typeof args[1]!=='number'||!Number.isFinite(args[1])||args[1]<1||args[1]>288))invalid();
  if(operation==='render'||operation==='renderRegion')normalizeRenderOptions(args[2] as import('./interaction.js').RenderOptions);
  if(operation==='renderRegion')normalizeRasterRegion(args[3] as import('./raster.js').RasterRegion);
  if(operation==='apply') {
    const command=snapshotMutation(args[0] as import('./interaction.js').MutationRequest);
    const maximum=mutationAssetLimit(command);
    if((maximum>0)!==(args[1]!==undefined)||args[1]!==undefined&&(!(args[1] instanceof Uint8Array)||!args[1].length||args[1].length>maximum))invalid();
  }
  if(operation==='save'&&(!integer(args[0])||(args[0] as number)<1||(args[0] as number)>8*1024*1024))invalid();
  if(operation==='open') {
    if(!(args[0] instanceof Uint8Array)||!args[0].length||args[0].length>8*1024*1024)invalid();
    const options=args[1];
    if(!record(options)||Object.keys(options).some(k=>!['mode','password'].includes(k)))invalid();
    const opts=options as Record<string,unknown>;
    if(opts.mode!==undefined&&opts.mode!=='strict'&&opts.mode!=='compatible')invalid();
    if(opts.password!==undefined&&(typeof opts.password!=='string'||new TextEncoder().encode(opts.password).length>1024))invalid();
  }
  if(operation==='fontRequirements')normalizeRenderOptions(args[1] as import('./interaction.js').RenderOptions);
  if(operation==='registerDocumentFont'){if(!(args[0] instanceof Uint8Array)||!args[0].length||args[0].length>32*1024*1024)invalid();normalizeFontRegistration(args[1] as FontRegistration);normalizeFontStamp(args[2] as FontStamp);}
  if(operation==='registerFont'){if(!(args[0] instanceof Uint8Array)||!args[0].length||args[0].length>32*1024*1024)invalid();normalizeFontRegistration(args[1] as FontRegistration);}
  if(operation==='font'&&((args[0]!==undefined&&(!(args[0] instanceof Uint8Array)||!args[0].length||args[0].length>32*1024*1024))||!integer(args[1])))invalid();
  if(operation==='search') {
    const options=args[1];
    if(typeof args[0]!=='string'||!args[0].length||args[0].length>1024||args[0].includes('\0')||!record(options)||Object.keys(options).some(k=>!['caseSensitive','maxResults'].includes(k)))invalid();
    const opts=options as Record<string,unknown>;
    if(opts.caseSensitive!==undefined&&typeof opts.caseSensitive!=='boolean')invalid();
    if(opts.maxResults!==undefined&&(!integer(opts.maxResults)||(opts.maxResults as number)<1||(opts.maxResults as number)>10000))invalid();
  }
}
