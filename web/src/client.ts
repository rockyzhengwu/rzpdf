import {normalizeRasterRegion,MAX_REGION_DPI,type RasterRegion} from "./raster.js";
export type {RasterRegion} from "./raster.js";
import {normalizeFontRegistration,normalizeFontStamp,type FontRegistration,type FontIdentity,type FontStamp,type FontRequirements} from "./fonts.js";
export type {FontRegistration,FontIdentity,FontStamp,FontRequirements} from "./fonts.js";
import {BUFFER_BUDGET,normalizeRenderOptions,snapshotMutation,mutationAssetLimit,type DocumentStamp,type InteractionState,type AnnotationSnapshot,type FormSnapshot,type MutationRequest,type MutationResult,type SavedPdf,type RenderOptions,type ContentSnapshot} from "./interaction.js";
export type * from "./interaction.js";
import { searchPage, validateQuery, type TextMatch } from "./text.js";
/** WA-04/05 adapter. Every returned byte array owns its memory. */
interface Module {
  HEAPU8: Uint8Array;
  _rz_abi_version(): number;
  _rz_alloc(n: number): number;
  _rz_buffer_ptr(h: number): number;
  _rz_buffer_len(h: number): number;
  _rz_open(h: number): number;
  _rz_open_with_options(h: number, options: number): number;
  _rz_query(h: number, request: number): number;
  _rz_error_code(): number;
  _rz_dispose(): number;
  _rz_handle_count(): number;
  _rz_retained_bytes(): number;
  _rz_set_substitute_font(h: number, index: number): number;
  _rz_register_font(h:number,options:number):number;
  _rz_clear_fonts():number;
  _rz_page_count(h: number): number;
  _rz_render(h: number, page: number, dpi: number): number;
  _rz_render_page(h:number,page:number,dpi:number,flags:number,zoom:number):number;
  _rz_render_region(h:number,page:number,dpi:number,flags:number,zoom:number,x:number,y:number,width:number,height:number):number;
  _rz_apply(h:number,request:number):number;
  _rz_apply_asset(h:number,request:number,asset:number):number;
  _rz_save(h:number,maxBytes:number):number;
  _rz_image_width(h: number): number;
  _rz_image_height(h: number): number;
  _rz_release(h: number): number;
  _rz_error_ptr(): number;
  _rz_error_len(): number;
  _rz_error_details_ptr?(): number;
  _rz_error_details_len?(): number;
}
export interface OpenOptions { mode?: "strict" | "compatible"; password?: string; resourceProfile?: "default" | "conservative"; }
export interface ImageDecodeDetails {encoding:string;width:number;height:number;targetWidth:number|null;targetHeight:number|null;targetDecodeSupported:boolean;unsupportedReason:string|null;jpegProcess:string|null;maskKind:string|null;maskWidth:number|null;maskHeight:number|null;maskEncoding:string|null;maskBitsPerComponent:number|null;mattePresent:boolean;}
export interface ResourceDetails {reason:string;stage:string;limit?:number;actual?:number;image?:ImageDecodeDetails;}
export interface RenderCacheStatistics {sourceInputBytes:number;rawStreamResidentBytes:number;rawStreamPinnedBytes:number;rawStreamTransientBytes:number;rawStreamPeakTransientBytes:number;rawStreamPeakResidentBytes:number;rawStreamLimitBytes:number;rawStreamEntries:number;rawStreamHits:number;rawStreamMisses:number;rawStreamEvictions:number;pageHits:number;pageMisses:number;pageEntries:number;pageEstimatedBytes:number;fontHits:number;fontMisses:number;fontEntries:number;fontEstimatedBytes:number;byteAccounting:string;linearMemoryMayRemainGrown:boolean;callerBuffersReleased:boolean;skiaCacheScope:string;skiaFontBytes:number;skiaFontLimit:number;skiaResourceBytes:number;skiaResourceLimit:number;
  renderSessionEstimatedBytes:number;renderSessionByteLimit:number;
  renderSessionFontManagerEntries:number;renderSessionFontManagerHits:number;renderSessionFontEntries:number;renderSessionImageEntries:number;renderSessionGeometryEntries:number;
  renderSessionFontHits:number;renderSessionImageHits:number;renderSessionGeometryHits:number;
  renderSessionMisses:number;renderSessionEvictions:number;
}
export type Matrix = [number, number, number, number, number, number];
export interface PageInfo {
  width: number; height: number; rotation: number; userUnit: number;
  visibleBox: [number, number, number, number]; userToDisplay: Matrix;
  coordinateSpace: "display-points-top-left";
}
export interface TextCharacter {
  unicode: string | null; charcode: number; codeLength: number;
  position: [number, number]; advance: [number, number]; emToDisplay: Matrix | null;
  fontSize: number; fontName: string | null; writingMode: "Horizontal" | "Vertical" | null;
  objectIndex: number; operationIndex: number | null; nested: boolean;
}
export interface ContentDiagnostic { kind: string; operator: string; byteOffset: number | null; message: string; source: unknown; }
export interface TextPage { order: "paint"; coordinateSpace: "display-points-top-left"; characters: TextCharacter[]; diagnostics: ContentDiagnostic[]; }
export interface PageTarget { kind: "page"; page: number; view: {kind: string; left?:number|null; top?:number|null; zoom?:number|null; rect?:number[]}; }
export type LinkTarget = PageTarget | { kind: "uri"; uri: string };
export interface PageLink { rect: [number,number,number,number]; coordinateSpace: "pdf-user"; target: LinkTarget; }
export interface OutlineItem { id: [number,number]; parent: [number,number] | null; title: string; open: boolean; target: PageTarget; }
export type ErrorCode = "format" | "password_required" | "password_invalid" | "permission" | "unsupported" | "font" | "resource_limit" | "invalid_handle" | "invalid_argument" | "internal" | "cancelled";
const errorCodes: ErrorCode[] = ["internal","format","password_required","password_invalid","permission","unsupported","font","resource_limit","invalid_handle","invalid_argument","internal","cancelled"];
export class PdfApiError extends Error {
  override name = "PdfApiError";
  constructor(readonly code: ErrorCode, message: string, readonly operation: string, readonly page?: number, readonly taskId?: number, readonly session?: string, readonly resource?: ResourceDetails) { super(message); }
}
export interface SearchMatch extends TextMatch { page: number; }
export interface SearchReport { matches: SearchMatch[]; truncated: boolean; pagesScanned: number; missingUnicode: number; diagnostics: number; }
export interface RgbaPage { width: number; height: number; pixels: Uint8ClampedArray<ArrayBuffer>; }
export interface RgbaRegion extends RgbaPage {x:number;y:number;}
export class PdfEngine {
  readonly identity = Array.from(crypto.getRandomValues(new Uint8Array(16)),b=>b.toString(16).padStart(2,"0")).join("");
  private alive = true;
  private disposed = false;
  constructor(readonly module: Module) {
    if (module._rz_abi_version() !== 2) throw new PdfApiError("unsupported", "Unsupported ABI", "initialize");
  }
  invoke<T>(fn: () => T): T {
    if (this.disposed) throw new PdfApiError("invalid_handle", "Engine is disposed", "session");
    if (!this.alive) throw new PdfApiError("internal", "Wasm session failed; recreate the engine", "session");
    try { return fn(); } catch (e) {
      // A trap/abort invalidates every handle in this module. No catch_unwind promise.
      this.alive = false; throw new PdfApiError("internal", "Wasm execution failed; recreate the engine", "session");
    }
  }
  statistics(): {handles:number;retainedBytes:number} {
    return this.invoke(()=>({handles:this.module._rz_handle_count(),retainedBytes:this.module._rz_retained_bytes()}));
  }
  dispose(): void {
    if(this.disposed)return;
    try { if(this.alive)this.invoke(()=>this.module._rz_dispose()); }
    finally { this.disposed=true;this.alive=false; }
  }
  error(operation = "api", page?: number): PdfApiError {
    const code = errorCodes[this.module._rz_error_code()] ?? "internal";
    const n = this.module._rz_error_len();
    const p = this.module._rz_error_ptr();
    const message=new TextDecoder().decode(this.module.HEAPU8.slice(p,p+n));
    const length=this.module._rz_error_details_len?.()??0,ptr=this.module._rz_error_details_ptr?.()??0;
    const resource=length?JSON.parse(new TextDecoder().decode(this.module.HEAPU8.slice(ptr,ptr+length))) as ResourceDetails:undefined;
    return new PdfApiError(code, message || "PDF operation failed", operation, page,undefined,undefined,resource);
  }
  capabilities():Record<string,unknown> {return this.query(0,'capabilities');}
  release(handle: number): void {
    if (this.alive) this.invoke(() => this.module._rz_release(handle));
  }
  registerFont(bytes:Uint8Array,options:FontRegistration,scope?:FontStamp):FontIdentity {
    let metadata:Required<FontRegistration>;
    try {metadata=normalizeFontRegistration(options);}catch{throw new PdfApiError('invalid_argument','Invalid font metadata','registerFont');}
    if(!(bytes instanceof Uint8Array)||!bytes.length||bytes.length>32*1024*1024)throw new PdfApiError('resource_limit','Font must contain 1 byte to 32 MiB','registerFont');
    if(scope){try{scope=normalizeFontStamp(scope);}catch{throw new PdfApiError('invalid_argument','Invalid font source stamp','registerFont');}if(scope.engine!==this.identity)throw new PdfApiError('invalid_handle','Font request belongs to another engine','registerFont');}
    const payload=scope?{...metadata,document:scope.document,revision:scope.revision,...(scope.fontEpoch===undefined?{}:{fontEpoch:scope.fontEpoch})}:metadata;
    return this.withInput(new TextEncoder().encode(JSON.stringify(payload)),request=>this.withInput(bytes,input=>{
      const result=this.invoke(()=>this.module._rz_register_font(input,request));
      if(!result)throw this.error('registerFont');
      try{return JSON.parse(new TextDecoder().decode(this.readBuffer(result))) as FontIdentity;}finally{this.release(result);}
    }));
  }
  clearFonts():void {if(!this.invoke(()=>this.module._rz_clear_fonts()))throw this.error('clearFonts');}
  setSubstituteFont(bytes?: Uint8Array, index = 0): void {
    if (!Number.isInteger(index) || index < 0 || index > 0xffffffff) throw new PdfApiError("invalid_argument", "Invalid font collection index", "registerFont");
    if (!bytes) { this.invoke(() => this.module._rz_set_substitute_font(0, 0)); return; }
    if (!bytes.length || bytes.length > 32 * 1024 * 1024) throw new PdfApiError("resource_limit", "Font must contain 1 byte to 32 MiB", "registerFont");
    const input = this.invoke(() => this.module._rz_alloc(bytes.length));
    if (!input) throw this.error();
    try {
      const ptr = this.invoke(() => this.module._rz_buffer_ptr(input));
      if (!ptr) throw this.error();
      this.module.HEAPU8.set(bytes, ptr);
      if (!this.invoke(() => this.module._rz_set_substitute_font(input, index))) throw this.error();
    } finally { this.release(input); }
  }
  withInput<T>(bytes: Uint8Array, action: (input: number) => T): T {
    const input = this.invoke(() => this.module._rz_alloc(bytes.length));
    if (!input) throw this.error("allocate");
    try {
      const ptr = this.invoke(() => this.module._rz_buffer_ptr(input));
      if (!ptr) throw this.error("input");
      this.module.HEAPU8.set(bytes, ptr);
      return action(input);
    } finally { this.release(input); }
  }
  readBuffer(handle: number, maximum=16*1024*1024): Uint8Array<ArrayBuffer> {
    const size = this.invoke(() => this.module._rz_buffer_len(handle));
    const ptr = this.invoke(() => this.module._rz_buffer_ptr(handle));
    if (!Number.isInteger(maximum) || maximum<1 || maximum>BUFFER_BUDGET)
      throw new PdfApiError("invalid_argument","Invalid output buffer limit","readBuffer");
    if (size>maximum)throw new PdfApiError("resource_limit","Output buffer exceeds the requested limit","readBuffer",undefined,undefined,undefined,{reason:"buffer_bytes",stage:"sdk_read",limit:maximum,actual:size});
    if (!ptr || ptr + size > this.module.HEAPU8.length) throw new PdfApiError("internal","Invalid Wasm output buffer","readBuffer");
    return this.module.HEAPU8.slice(ptr, ptr + size);
  }
  query<T>(document: number, operation: string, page?: number,extra:Record<string,unknown>={}): T {
    const bytes = new TextEncoder().encode(JSON.stringify({...extra,operation, page}));
    return this.withInput(bytes, input => {
      const output = this.invoke(() => this.module._rz_query(document, input));
      if (!output) throw this.error(operation, page);
      try { return JSON.parse(new TextDecoder("utf-8", {fatal:true}).decode(this.readBuffer(output))) as T; }
      finally { this.release(output); }
    });
  }
  apply(document:number,request:MutationRequest,asset?:Uint8Array):MutationResult {
    let command:MutationRequest;
    try {command=snapshotMutation(request);}catch{throw new PdfApiError("invalid_argument","Invalid or oversized mutation request","apply");}
    if(command.stamp.engine!==this.identity||command.operation==='importPages'&&command.source.engine!==this.identity)throw new PdfApiError("invalid_handle","Selection belongs to another module/session","apply");
    const assetLimit=mutationAssetLimit(command);
    if((assetLimit>0)!==(asset!==undefined)||asset!==undefined&&(!(asset instanceof Uint8Array)||!asset.length||asset.length>assetLimit))throw new PdfApiError('invalid_argument','Invalid or oversized mutation asset','apply');
    const bytes=new TextEncoder().encode(JSON.stringify(command));
    try {return this.withInput(bytes,input=>{
      const execute=(assetHandle:number)=>{const result=this.invoke(()=>this.module._rz_apply_asset(document,input,assetHandle));if(!result)throw this.error("apply",'page' in command?command.page:undefined);return result;};
      const result=asset?this.withInput(asset,execute):execute(0);
      try {const value=JSON.parse(new TextDecoder().decode(this.readBuffer(result))) as MutationResult;return {...value,stamp:Object.freeze({...value.stamp,engine:this.identity})};}
      finally {this.release(result);}
    });}finally{bytes.fill(0);}
  }
  open(bytes: Uint8Array, options: OpenOptions = {}): PdfDocument {
    if(options.resourceProfile!==undefined&&!['default','conservative'].includes(options.resourceProfile))throw new PdfApiError('invalid_argument','Invalid resource profile','open');
    if (!Number.isSafeInteger(bytes.length) || bytes.length < 1 || bytes.length > BUFFER_BUDGET)
      throw new PdfApiError("resource_limit", "PDF input is empty or exceeds the shared buffer budget", "open");
    if (options.mode !== undefined && options.mode !== "strict" && options.mode !== "compatible") throw new PdfApiError("invalid_argument", "Invalid parse mode", "open");
    if (options.password !== undefined && (typeof options.password !== "string" || new TextEncoder().encode(options.password).length > 1024)) throw new PdfApiError("invalid_argument", "Invalid password length", "open");
    const settings = new TextEncoder().encode(JSON.stringify(options));
    try {
      return this.withInput(bytes, input => this.withInput(settings, opts => {
        const handle = this.invoke(() => this.module._rz_open_with_options(input, opts));
        if (!handle) throw this.error("open");
        return new PdfDocument(this, handle);
      }));
    } finally { settings.fill(0); }
  }
}
export class PdfDocument {
  private closed = false;
  constructor(private engine: PdfEngine, private handle: number) {}
  private check(): void { if (this.closed) throw new PdfApiError("invalid_handle", "Document is closed", "document"); }
  get pageCount(): number {
    this.check();
    const count = this.engine.invoke(() => this.engine.module._rz_page_count(this.handle));
    if (!count && this.engine.module._rz_error_code()) throw this.engine.error("pageCount");
    return count;
  }
  private checkPage(page: number): void {
    this.check();
    if (!Number.isInteger(page) || page < 0 || page >= this.pageCount) throw new PdfApiError("invalid_argument", "Page index out of range", "page", page);
  }
  pageInfo(page: number): PageInfo { this.checkPage(page); return this.engine.query(this.handle, "pageInfo", page); }
  fontRequirements(page:number,options:RenderOptions={}):FontRequirements {
    this.checkPage(page);let renderOptions:Required<RenderOptions>;
    try{renderOptions=normalizeRenderOptions(options);}catch{throw new PdfApiError('invalid_argument','Invalid render options','fontRequirements');}
    const report=this.engine.query<FontRequirements>(this.handle,'fontRequirements',page,{renderOptions});
    return {...report,stamp:Object.freeze({...report.stamp,engine:this.engine.identity})};
  }
  registerFont(bytes:Uint8Array,options:FontRegistration,stamp:FontStamp):FontIdentity {
    this.check();let scope:FontStamp;try{scope=normalizeFontStamp(stamp);}catch{throw new PdfApiError('invalid_argument','Invalid font request stamp','registerFont');}
    if(scope.document!==this.handle||scope.engine!==this.engine.identity)throw new PdfApiError('invalid_handle','Font request belongs to another document','registerFont');
    return this.engine.registerFont(bytes,options,scope);
  }
  extractText(page: number): TextPage { this.checkPage(page); return this.engine.query(this.handle, "text", page); }
  searchText(query: string, options: {caseSensitive?:boolean; maxResults?:number} = {}): SearchReport {
    this.check();
    try { validateQuery(query); } catch { throw new PdfApiError("invalid_argument", "Search requires 1–1024 characters without NUL", "searchText"); }
    if(options.caseSensitive !== undefined && typeof options.caseSensitive !== "boolean")throw new PdfApiError("invalid_argument", "Invalid case sensitivity", "searchText");
    const maximum=options.maxResults ?? 1000;
    if(!Number.isInteger(maximum) || maximum<1 || maximum>10000)throw new PdfApiError("invalid_argument","Search limit must be 1–10000","searchText");
    const report:SearchReport={matches:[],truncated:false,pagesScanned:0,missingUnicode:0,diagnostics:0};
    for(let page=0;page<this.pageCount;page++) {
      const text=this.extractText(page);
      report.pagesScanned++;report.missingUnicode+=text.characters.filter(c=>c.unicode===null).length;
      report.diagnostics+=text.diagnostics.length;
      const matches=searchPage(text,query,options.caseSensitive,maximum-report.matches.length+1);
      for(const match of matches) {
        if(report.matches.length===maximum){report.truncated=true;return report;}
        report.matches.push({page,...match});
      }
    }
    return report;
  }
  outlines(): OutlineItem[] { this.check(); return this.engine.query(this.handle, "outlines"); }
  links(page: number): PageLink[] { this.checkPage(page); return this.engine.query(this.handle, "links", page); }
  capabilities(): Record<string, unknown> { this.check(); return this.engine.query(this.handle, "capabilities"); }
  cacheStatistics():RenderCacheStatistics {this.check();return this.engine.query(this.handle,'cacheStatistics');}
  clearSourceStreamCache():RenderCacheStatistics {this.check();return this.engine.query(this.handle,'clearSourceStreamCache');}
  clearRenderCaches():RenderCacheStatistics {this.check();return this.engine.query(this.handle,'clearRenderCaches');}
  diagnostics(): {reason:string;offset:number;object:number[]|null}[] { this.check(); return this.engine.query(this.handle, "diagnostics"); }
  private interaction<T extends {stamp:DocumentStamp}>(operation:string,page?:number):T {
    this.check();const value=this.engine.query<T>(this.handle,operation,page);
    return {...value,stamp:Object.freeze({...value.stamp,engine:this.engine.identity})};
  }
  state():InteractionState {return this.interaction("state");}
  annotations(page:number):AnnotationSnapshot {this.checkPage(page);return this.interaction("annotations",page);}
  formFields():FormSnapshot {return this.interaction("formFields");}
  content(page:number):ContentSnapshot {this.checkPage(page);return this.interaction("content",page);}
  apply(request:MutationRequest,asset?:Uint8Array):MutationResult {this.check();return this.engine.apply(this.handle,request,asset);}
  save(maxBytes=BUFFER_BUDGET):SavedPdf {
    this.check();if(!Number.isInteger(maxBytes)||maxBytes<1||maxBytes>BUFFER_BUDGET)throw new PdfApiError("resource_limit","Save limit exceeds the shared buffer budget","save");
    const stamp=this.state().stamp,output=this.engine.invoke(()=>this.engine.module._rz_save(this.handle,maxBytes));
    if(!output)throw this.engine.error("save");
    try {return {bytes:this.engine.readBuffer(output,maxBytes),stamp};}finally{this.engine.release(output);}
  }
  render(page = 0, dpi = 72,options:RenderOptions={}): RgbaPage {
    this.check();
    if (!Number.isInteger(page) || page < 0 || page >= this.pageCount) throw new PdfApiError("invalid_argument", "Page index out of range", "render", page);
    if (!Number.isFinite(dpi) || dpi < 1 || dpi > 288) throw new PdfApiError("invalid_argument", "DPI must be between 1 and 288", "render", page);
    let settings:Required<RenderOptions>;try{settings=normalizeRenderOptions(options);}catch{throw new PdfApiError("invalid_argument","Invalid render options","render",page);}
    const flags=(settings.annotations?(settings.includeWidgets?3:1):0)|(settings.viewRotation/90<<2)|(settings.imageDecode==='target'?16:0);
    const m = this.engine.module;
    const image = this.engine.invoke(() => m._rz_render_page(this.handle, page, dpi,flags,settings.viewZoom));
    return this.readRgba(image,"render",page);
  }
  renderRegion(page:number,region:RasterRegion,dpi=72,options:RenderOptions={}):RgbaRegion {
    this.checkPage(page);
    if(!Number.isFinite(dpi)||dpi<1||dpi>MAX_REGION_DPI)throw new PdfApiError("invalid_argument","DPI exceeds region limit","renderRegion",page);
    let area:RasterRegion,settings:Required<RenderOptions>;
    try{area=normalizeRasterRegion(region);settings=normalizeRenderOptions(options);}catch{throw new PdfApiError("invalid_argument","Invalid render region or options","renderRegion",page);}
    const flags=(settings.annotations?(settings.includeWidgets?3:1):0)|(settings.viewRotation/90<<2)|(settings.imageDecode==='target'?16:0),m=this.engine.module;
    const image=this.engine.invoke(()=>m._rz_render_region(this.handle,page,dpi,flags,settings.viewZoom,area.x,area.y,area.width,area.height));
    const result=this.readRgba(image,"renderRegion",page);
    if(result.width!==area.width||result.height!==area.height)throw new PdfApiError("internal","Unexpected region dimensions","renderRegion",page);
    return {...result,x:area.x,y:area.y};
  }
  private readRgba(image:number,operation:string,page:number):RgbaPage {
    if(!image)throw this.engine.error(operation,page);
    const m=this.engine.module;
    try {
      const width = this.engine.invoke(() => m._rz_image_width(image));
      const height = this.engine.invoke(() => m._rz_image_height(image));
      const size = this.engine.invoke(() => m._rz_buffer_len(image));
      const ptr = this.engine.invoke(() => m._rz_buffer_ptr(image));
      if (!width || !height || !ptr || size !== width * height * 4) throw new PdfApiError("internal", "Invalid RGBA result", operation, page);
      // Always reacquire HEAPU8 after calls that may grow memory. No borrowed view escapes.
      return { width, height, pixels: new Uint8ClampedArray(m.HEAPU8.slice(ptr, ptr + size)) };
    } finally { this.engine.release(image); }
  }
  close(): void {
    if (!this.closed) { this.engine.release(this.handle); this.closed = true; }
  }
}
export async function createEngine(): Promise<PdfEngine> {
  const url = new URL("./rzpdf_web.mjs", import.meta.url);
  const { default: factory } = await import(url.href);
  return new PdfEngine(await factory());
}
