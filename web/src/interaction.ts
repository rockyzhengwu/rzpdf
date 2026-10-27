/** Shared transfer/output budget; PDF input has no separate 8 MiB cap. */
export const BUFFER_BUDGET=64*1024*1024;
export type ObjectId=[number,number];
export interface DocumentStamp { readonly engine:string;readonly document:number;readonly revision:string; }
export interface InteractionState { stamp:DocumentStamp;commitRevision:string;pageCount:number;permissions:{annotate:boolean;fillForms:boolean;modify:boolean;assemble:boolean;copy:boolean;save:boolean;securityRevision:number|null}; }
export interface AnnotationInput {
  kind:'Square'|'Circle'|'Text'|'Popup'|'FreeText'|'Line'|'Polygon'|'PolyLine'|'Highlight'|'Underline'|'Squiggly'|'StrikeOut'|'Ink'|'Stamp'|'FileAttachment';
  rect:[number,number,number,number];color?:[number,number,number];interiorColor?:[number,number,number]|null;contents?:string;flags?:number;opacity?:number;strokeWidth?:number;
  icon?:string;open?:boolean;fontSize?:number;alignment?:'left'|'center'|'right';points?:number[];vertices?:number[][];quads?:number[][];strokes?:number[][][];name?:string;file?:ObjectId;
}
export interface AnnotationRecord {object:ObjectId|null;subtype:string;annotation:AnnotationInput|null;unsupportedReason:string|null;popup:ObjectId|null;popupParent:ObjectId|null;replyTo:ObjectId|null;}
export interface AnnotationSnapshot {stamp:DocumentStamp;page:number;records:AnnotationRecord[];}
export type FormValue={kind:'text';text:string}|{kind:'checkbox';checked:boolean}|{kind:'radio'|'buttonState';state:string|null}|{kind:'choice';values:string[]}|{kind:'choiceIndices';indices:number[]}|{kind:'pushButton'};
export interface FormFieldRecord {object:ObjectId;name:string;fieldType:string|null;flags:number;widgets:{object:ObjectId;page:number;rect:[number,number,number,number];onState:string|null;flags:number;visible:boolean}[];details:{type:{kind:'text'|'checkbox'|'radio'|'choice'|'pushButton';multiline?:boolean;noToggleToOff?:boolean;inUnison?:boolean;combo?:boolean;editable?:boolean;multiple?:boolean};value:FormValue;options:{export:string;display:string}[];buttonStates:string[];buttonExports:string[]|null}|null;unsupportedReason:string|null;}
export interface FormSnapshot {stamp:DocumentStamp;records:FormFieldRecord[];}
export interface ContentSnapshot {stamp:DocumentStamp;page:number;coordinateSpace:'display-points-top-left';characters:{index:number;unicode:string|null;objectIndex:number;position:[number,number];advance:[number,number];emToDisplay:[number,number,number,number,number,number]|null;fontName:string|null;editable:boolean;nested:boolean}[];images:{index:number;matrix:[number,number,number,number,number,number];editable:boolean;nested:boolean}[];}
export interface FontWriteOptions {postscriptName:string;capHeight:number;stemV:number;serif:boolean;omitLayoutAndAuxiliary?:boolean;}
export type ContentMutation =
 | {operation:'replaceText'|'replaceTextSpan';stamp:DocumentStamp;page:number;start:number;end:number;text:string;preserveAdvance?:boolean;font?:FontWriteOptions}
 | {operation:'removeImage';stamp:DocumentStamp;page:number;index:number}
 | {operation:'transformImage';stamp:DocumentStamp;page:number;index:number;matrix:[number,number,number,number,number,number]}
 | {operation:'replaceImage';stamp:DocumentStamp;page:number;index:number;format:'png'|'jpeg'}
 | {operation:'rotatePage';stamp:DocumentStamp;page:number;degrees:number}
 | {operation:'copyPage';stamp:DocumentStamp;page:number;insertion:number;namePrefix?:string}
 | {operation:'deletePage';stamp:DocumentStamp;page:number}
 | {operation:'reorderPages';stamp:DocumentStamp;order:number[]}
 | {operation:'importPages';stamp:DocumentStamp;source:DocumentStamp;pages:number[];insertion:number;namePrefix?:string;includeNavigation?:boolean};
export type MutationRequest = ContentMutation
 | {operation:'addAnnotation';stamp:DocumentStamp;page:number;annotation:AnnotationInput}
 | {operation:'updateAnnotation';stamp:DocumentStamp;page:number;object:ObjectId;annotation:AnnotationInput}
 | {operation:'removeAnnotation';stamp:DocumentStamp;page:number;object:ObjectId;cascade?:boolean}
 | {operation:'setFormValue';stamp:DocumentStamp;object:ObjectId;value:FormValue};
export interface MutationResult {stamp:DocumentStamp;pageCount:number;created:ObjectId|null;}
export interface SavedPdf {bytes:Uint8Array<ArrayBuffer>;stamp:DocumentStamp;}
export interface RenderOptions {imageDecode?:'budgeted'|'target';annotations?:boolean;includeWidgets?:boolean;viewZoom?:number;viewRotation?:number;}
export function normalizeRenderOptions(options:RenderOptions):Required<RenderOptions> {
  if(!options||typeof options!=='object'||Object.keys(options).some(k=>!['annotations','includeWidgets','viewZoom','viewRotation','imageDecode'].includes(k)))throw new RangeError('Invalid render options');
  const out={imageDecode:options.imageDecode??'budgeted',annotations:options.annotations??true,includeWidgets:options.includeWidgets??true,viewZoom:options.viewZoom??1,viewRotation:options.viewRotation??0};
  if(!['budgeted','target'].includes(out.imageDecode)||![0,90,180,270].includes(out.viewRotation)||typeof out.annotations!=='boolean'||typeof out.includeWidgets!=='boolean'||typeof out.viewZoom!=='number'||!Number.isFinite(out.viewZoom)||out.viewZoom<.1||out.viewZoom>4)throw new RangeError('Invalid render options');
  return out;
}
/** Own a bounded JSON-only command before enqueueing it in another realm. */
export function snapshotMutation(request:MutationRequest):MutationRequest {
  let text:string;
  try {text=JSON.stringify(request);}catch{throw new RangeError('Invalid mutation data');}
  if(typeof text!=='string'||new TextEncoder().encode(text).length>64*1024)throw new RangeError('Mutation request exceeds 64 KiB');
  const copy=JSON.parse(text);
  if(!copy||typeof copy!=='object'||!['addAnnotation','updateAnnotation','removeAnnotation','setFormValue','replaceText','replaceTextSpan','removeImage','transformImage','replaceImage','rotatePage','copyPage','deletePage','reorderPages','importPages'].includes(copy.operation)||!copy.stamp||typeof copy.stamp.engine!=='string'||copy.stamp.engine.length>128||!Number.isInteger(copy.stamp.document)||typeof copy.stamp.revision!=='string'||!/^\d{1,20}$/.test(copy.stamp.revision))throw new RangeError('Invalid mutation identity');
  if(copy.operation==='importPages'&&(!copy.source||typeof copy.source.engine!=='string'||!Number.isInteger(copy.source.document)||typeof copy.source.revision!=='string'||!/^\d{1,20}$/.test(copy.source.revision)))throw new RangeError('Invalid import source identity');
  return copy;
}

export function mutationAssetLimit(request:MutationRequest):number {
  if(request.operation==='replaceImage')return 8*1024*1024;
  if((request.operation==='replaceText'||request.operation==='replaceTextSpan')&&request.font!==undefined)return 32*1024*1024;
  return 0;
}
