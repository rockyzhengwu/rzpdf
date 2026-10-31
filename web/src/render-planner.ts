import {PdfApiError} from './errors.js';
import {rasterSize,type RasterRegion} from './raster.js';
import type {PageInfo,RgbaRegion,RgbaPage} from './client.js';
import type {InteractionState,RenderOptions} from './interaction.js';

/** Shared by the synchronous SDK and Worker; no new RPC operations. */
export interface TileDocument {
  pageInfo(page:number):PageInfo|Promise<PageInfo>;
  capabilities():Record<string,unknown>|Promise<Record<string,unknown>>;
  state():InteractionState|Promise<InteractionState>;
  renderRegion(page:number,region:RasterRegion,dpi:number,options?:RenderOptions):RgbaRegion|Promise<RgbaRegion>;
}
export interface TilePlan {width:number;height:number;dpi:number;tileSide:number;tiles:number;}
export interface TileOptions {dpi:number;tileSide?:number;signal?:AbortSignal;renderOptions?:RenderOptions;}
function positive(value:unknown,name:string):number {
  if(typeof value!=='number'||!Number.isFinite(value)||value<=0)throw new RangeError(`Invalid ${name}`);
  return value;
}
/** Headroom covers several simultaneous surfaces. Complex pages can still
 * return a typed resource error; this planner never weakens engine limits. */
export function planTiles(info:Pick<PageInfo,'width'|'height'>,caps:Record<string,unknown>,options:TileOptions):TilePlan {
  const requestedDpi=positive(options.dpi,'DPI');
  if(requestedDpi>positive(caps.maxRegionDpi,'maxRegionDpi'))throw new RangeError('DPI exceeds region capability');
  const dpi=Math.fround(requestedDpi);
  const {width,height}=rasterSize(info.width,info.height,dpi);
  const requested=options.tileSide??512;
  if(!Number.isSafeInteger(requested)||requested<1)throw new RangeError('Invalid tile side');
  const tileSide=Math.floor(Math.min(requested,positive(caps.maxRegionSide,'maxRegionSide'),
    Math.sqrt(positive(caps.maxRenderPixels,'maxRenderPixels')),
    Math.sqrt(positive(caps.maxRenderBytes,'maxRenderBytes')/24)));
  if(tileSide<1)throw new RangeError('Insufficient raster budget');
  return {width,height,dpi,tileSide,tiles:Math.ceil(width/tileSide)*Math.ceil(height/tileSide)};
}
function cancelled(signal?:AbortSignal):void {signal?.throwIfAborted();}
function sameRevision(before:InteractionState,after:InteractionState):void {
  if(before.stamp.engine!==after.stamp.engine||before.stamp.document!==after.stamp.document||before.stamp.revision!==after.stamp.revision)
    throw new Error('Document changed during tiled rendering');
}
/** Consumers own each yielded tile. Retaining tiles increases application
 * memory. Abort between tiles is cooperative and keeps the engine usable. */
export async function* renderTiles(doc:TileDocument,page:number,options:TileOptions):AsyncGenerator<RgbaRegion> {
  cancelled(options.signal);
  const initial=await doc.state();
  const plan=planTiles(await doc.pageInfo(page),await doc.capabilities(),options);
  async function* draw(region:RasterRegion,retries:number):AsyncGenerator<RgbaRegion> {
    cancelled(options.signal);
    sameRevision(initial,await doc.state());
    let tile:RgbaRegion;
    try{tile=await doc.renderRegion(page,region,plan.dpi,options.renderOptions);}
    catch(error){
      // Only a known raster-allocation boundary can benefit from smaller
      // regions. Source decode/work/format errors propagate without retries.
      const smaller=error instanceof PdfApiError&&error.code==='resource_limit'
        &&error.resource?.reason==='render_live_bytes'
        &&['render_group','group_color_conversion','group_uncompositing','group_soft_mask'].includes(error.resource.stage);
      if(!smaller||retries===0||region.width<2||region.height<2)throw error;
      const halfWidth=Math.ceil(region.width/2),halfHeight=Math.ceil(region.height/2);
      for(let y=0;y<region.height;y+=halfHeight)for(let x=0;x<region.width;x+=halfWidth)
        yield* draw({x:region.x+x,y:region.y+y,width:Math.min(halfWidth,region.width-x),height:Math.min(halfHeight,region.height-y)},retries-1);
      return;
    }
    sameRevision(initial,await doc.state());
    cancelled(options.signal);
    yield tile;
  }
  for(let y=0;y<plan.height;y+=plan.tileSide)for(let x=0;x<plan.width;x+=plan.tileSide)
    yield* draw({x,y,width:Math.min(plan.tileSide,plan.width-x),height:Math.min(plan.tileSide,plan.height-y)},2);
}
export interface PreviewOptions {maxWidth:number;maxHeight:number;dpi?:number;signal?:AbortSignal;renderOptions?:RenderOptions;}
/** Explicit bounded overview. Returns the chosen DPI; ordinary render() is
 * unchanged. Source-image errors propagate without ineffective tile retries. */
export async function renderPreview(doc:TileDocument,page:number,options:PreviewOptions):Promise<RgbaPage&{dpi:number}> {
  cancelled(options.signal);
  const initial=await doc.state();
  const info=await doc.pageInfo(page),caps=await doc.capabilities();
  const w=Math.floor(positive(options.maxWidth,'maxWidth')),h=Math.floor(positive(options.maxHeight,'maxHeight'));
  if(w<1||h<1)throw new RangeError('Preview bounds must contain at least one pixel');
  let dpi=Math.fround(Math.min(options.dpi===undefined?72:positive(options.dpi,'DPI'),
    positive(caps.maxRegionDpi,'maxRegionDpi'),72*w/info.width,72*h/info.height));
  // f32 rounding may round a fitted DPI upwards, adding an unwanted pixel.
  // Step down in the native representation until both requested bounds fit.
  const float=new Float32Array(1),bits=new Uint32Array(float.buffer);
  while(Math.ceil(Math.fround(info.width)*dpi/72)>Math.floor(w)||Math.ceil(Math.fround(info.height)*dpi/72)>Math.floor(h)){
    float[0]=dpi;bits[0]--;dpi=float[0];
  }
  if(dpi<1)throw new PdfApiError('unsupported','Preview requires less than minimum supported DPI; use a larger preview bound','renderPreview',page);
  const size=rasterSize(info.width,info.height,dpi);
  for(const [reason,actual,limit] of [
    ['render_pixels',size.width*size.height,positive(caps.maxRenderPixels,'maxRenderPixels')],
    ['render_bytes',size.width*size.height*4,positive(caps.maxRenderBytes,'maxRenderBytes')],
  ] as const)if(actual>limit)throw new PdfApiError('resource_limit','Preview exceeds output budget; consume renderTiles instead','renderPreview',page,undefined,undefined,{reason,stage:'render_plan',limit,actual});
  sameRevision(initial,await doc.state());
  const pixels=new Uint8ClampedArray(size.width*size.height*4);
  for await(const tile of renderTiles(doc,page,{dpi,signal:options.signal,renderOptions:{imageDecode:"target",...options.renderOptions}})){
    sameRevision(initial,await doc.state());
    for(let row=0;row<tile.height;row++)pixels.set(tile.pixels.subarray(row*tile.width*4,(row+1)*tile.width*4),((tile.y+row)*size.width+tile.x)*4);
  }
  return {...size,pixels,dpi};
}
