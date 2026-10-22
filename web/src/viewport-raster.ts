import {PdfApiError,type RgbaRegion} from './client.js';
import type {RasterRegion} from './raster.js';

export const TILE_SIDE=512;
export const TILE_CACHE_BYTES=16*1024*1024;
export const TILE_CACHE_ENTRIES=128;
export const VIEWPORT_BYTES=32*1024*1024;
export interface RasterSource {
  key:string;width:number;height:number;cssWidth:number;cssHeight:number;
  valid:()=>boolean;
  render:(region:RasterRegion)=>Promise<RgbaRegion>;
}
export interface RasterStatistics {cacheBytes:number;cacheEntries:number;renderedTiles:number;cacheHits:number;pendingTiles:number;surfaceBytes:number;}
function resource(message:string):PdfApiError{return new PdfApiError('resource_limit',message,'viewport');}
function same(a:RasterRegion,b:RasterRegion):boolean{return a.x===b.x&&a.y===b.y&&a.width===b.width&&a.height===b.height;}
/** Round outwards on the fixed raster grid. Overscan never takes priority over the visible pixels. */
export function rasterWindow(width:number,height:number,visible:RasterRegion|null):RasterRegion|null {
  if(!Number.isSafeInteger(width)||!Number.isSafeInteger(height)||width<1||height<1||width>32768||height>32768)throw resource('Page raster dimensions exceed viewport limits');
  if(!visible)return null;
  if(![visible.x,visible.y,visible.width,visible.height].every(Number.isFinite)||visible.width<=0||visible.height<=0)throw new RangeError('Invalid visible raster rectangle');
  const left=Math.max(0,Math.min(width,visible.x)),top=Math.max(0,Math.min(height,visible.y));
  const right=Math.max(0,Math.min(width,visible.x+visible.width)),bottom=Math.max(0,Math.min(height,visible.y+visible.height));
  if(right<=left||bottom<=top)return null;
  const window=(margin:number):RasterRegion=>{
    const x=Math.max(0,(Math.floor(left/TILE_SIDE)-margin)*TILE_SIDE),y=Math.max(0,(Math.floor(top/TILE_SIDE)-margin)*TILE_SIDE);
    return {x,y,width:Math.min(width,(Math.ceil(right/TILE_SIDE)+margin)*TILE_SIDE)-x,height:Math.min(height,(Math.ceil(bottom/TILE_SIDE)+margin)*TILE_SIDE)-y};
  };
  let area=window(1);if(area.width*area.height*4>VIEWPORT_BYTES)area=window(0);
  if(area.width*area.height*4>VIEWPORT_BYTES)throw resource('Visible raster exceeds the 32 MiB canvas limit');
  return area;
}
function prioritize(tiles:RasterRegion[],box:RasterRegion):void {
  const cx=box.x+box.width/2,cy=box.y+box.height/2;
  const distance=(t:RasterRegion)=>Math.hypot(t.x+t.width/2-cx,t.y+t.height/2-cy);
  const onscreen=(t:RasterRegion)=>t.x<box.x+box.width&&t.y<box.y+box.height&&t.x+t.width>box.x&&t.y+t.height>box.y;
  tiles.sort((a,b)=>Number(onscreen(b))-Number(onscreen(a))||distance(a)-distance(b));
}
interface Plan {source:RasterSource;epoch:number;area:RasterRegion;tiles:RasterRegion[];done:Set<string>;settled:boolean;promise:Promise<boolean>;finish:(value:boolean)=>void;}
/** Owns transferred tile bytes and one viewport canvas; never stores a logical full-page bitmap. */
export class ViewportRaster {
  private source:RasterSource|undefined;
  private plan:Plan|undefined;
  private epoch=0;
  private running=false;
  private cache=new Map<string,RgbaRegion>();
  private bytes=0;
  private rendered=0;
  private hits=0;
  private failed=false;
  constructor(private canvas:HTMLCanvasElement,private visible:()=>RasterRegion|null,private error:(error:unknown)=>void){}
  statistics():RasterStatistics{return {cacheBytes:this.bytes,cacheEntries:this.cache.size,renderedTiles:this.rendered,cacheHits:this.hits,pendingTiles:this.plan&&!this.plan.settled?this.plan.tiles.length-this.plan.done.size:0,surfaceBytes:this.canvas.width*this.canvas.height*4};}
  private report():void {
    const stats=this.statistics();
    for(const [key,value] of Object.entries(stats))this.canvas.dataset[key]=String(value);
    this.canvas.setAttribute('aria-busy',String(!!stats.pendingTiles));
  }
  private resetCanvas():void {
    this.canvas.width=this.canvas.height=1;this.canvas.style.width=this.canvas.style.height='1px';this.canvas.style.left=this.canvas.style.top='0px';
    delete this.canvas.dataset.rasterX;delete this.canvas.dataset.rasterY;
  }
  /** Drop pending work immediately; an already dispatched native task is allowed to finish. */
  stop(clearCache=false):void {
    this.epoch++;this.source=undefined;this.plan?.finish(false);this.plan=undefined;this.failed=false;
    if(clearCache){this.cache.clear();this.bytes=0;}
    this.resetCanvas();this.report();
  }
  setSource(source:RasterSource):Promise<boolean> {
    this.stop();
    if(!source.key||source.key.length>2048||![source.cssWidth,source.cssHeight].every(n=>Number.isFinite(n)&&n>0))throw new RangeError('Invalid raster source');
    rasterWindow(source.width,source.height,null);
    const owned={...source};this.source=owned;void this.refresh();return this.waitForLatest(owned);
  }
  private async waitForLatest(source:RasterSource):Promise<boolean> {
    while(this.source===source&&source.valid()){
      const plan=this.plan;if(!plan)return !this.failed;
      const ready=await plan.promise;
      if(this.source!==source||!source.valid())return false;
      if(this.plan===plan)return ready;
    }
    return false;
  }
  refresh():Promise<boolean> {
    const source=this.source;if(!source||!source.valid())return Promise.resolve(false);
    try {
      const visible=this.visible(),area=rasterWindow(source.width,source.height,visible);
      if(!area){this.plan?.finish(false);this.plan=undefined;this.resetCanvas();this.report();return Promise.resolve(true);}
      if(!this.failed&&this.plan&&this.plan.source===source&&same(this.plan.area,area)){if(!this.plan.settled)prioritize(this.plan.tiles,visible!);return this.plan.promise;}
      this.failed=false;this.plan?.finish(false);
      let finish!:(value:boolean)=>void;const promise=new Promise<boolean>(resolve=>{finish=resolve;});
      const tiles:RasterRegion[]=[];
      for(let y=area.y;y<area.y+area.height;y+=TILE_SIDE)for(let x=area.x;x<area.x+area.width;x+=TILE_SIDE)tiles.push({x,y,width:Math.min(TILE_SIDE,source.width-x),height:Math.min(TILE_SIDE,source.height-y)});
      prioritize(tiles,visible!);
      this.plan={source,epoch:this.epoch,area,tiles,done:new Set(),settled:false,promise,finish};
      // Reset both axes before resizing so old-height × new-width cannot exceed the budget.
      this.canvas.width=this.canvas.height=1;this.canvas.width=area.width;this.canvas.height=area.height;
      this.canvas.style.left=area.x/source.width*source.cssWidth+'px';this.canvas.style.top=area.y/source.height*source.cssHeight+'px';
      this.canvas.style.width=area.width/source.width*source.cssWidth+'px';this.canvas.style.height=area.height/source.height*source.cssHeight+'px';
      this.canvas.dataset.rasterX=String(area.x);this.canvas.dataset.rasterY=String(area.y);
      const context=this.canvas.getContext('2d');if(!context)throw resource('Viewport canvas is unavailable');
      context.fillStyle='white';context.fillRect(0,0,area.width,area.height);
      // Paint retained tiles before new arrivals can evict them from the LRU.
      for(const tile of tiles){
        const key=this.key(source,tile),image=this.cache.get(key);if(!image)continue;
        this.hits++;this.cache.delete(key);this.cache.set(key,image);
        context.putImageData(new ImageData(image.pixels,image.width,image.height),tile.x-area.x,tile.y-area.y);this.plan.done.add(key);
      }
      this.report();void this.drain();return promise;
    }catch(error){this.fail(error);return Promise.resolve(false);}
  }
  private key(source:RasterSource,tile:RasterRegion):string{return JSON.stringify([source.key,tile.x,tile.y,tile.width,tile.height]);}
  private retain(key:string,tile:RgbaRegion):void {
    const size=tile.pixels.byteLength;if(size>TILE_CACHE_BYTES)return;
    const previous=this.cache.get(key);if(previous){this.bytes-=previous.pixels.byteLength;this.cache.delete(key);}
    while(this.bytes+size>TILE_CACHE_BYTES||this.cache.size>=TILE_CACHE_ENTRIES){const first=this.cache.entries().next().value!;this.cache.delete(first[0]);this.bytes-=first[1].pixels.byteLength;}
    this.cache.set(key,tile);this.bytes+=size;
  }
  private fail(error:unknown):void {
    this.failed=true;this.plan?.finish(false);if(this.plan)this.plan.settled=true;
    this.resetCanvas();this.report();this.error(error);
  }
  private async drain():Promise<void> {
    if(this.running)return;this.running=true;
    try {
      for(;;){
        const plan=this.plan,source=this.source;
        if(!plan||plan.settled||!source)return;
        if(!source.valid()){this.stop(true);return;}
        const next=plan.tiles.find(tile=>!plan.done.has(this.key(source,tile)));
        if(!next){plan.settled=true;plan.finish(true);this.report();return;}
        const key=this.key(source,next);let image=this.cache.get(key);
        try {
          if(image){this.hits++;this.cache.delete(key);this.cache.set(key,image);}
          else {
            image=await source.render({...next});this.rendered++;
            // Source changes invalidate ownership; scroll changes may retain an otherwise valid tile.
            if(this.epoch!==plan.epoch||this.source!==source||!source.valid())continue;
            if(image.x!==next.x||image.y!==next.y||image.width!==next.width||image.height!==next.height||!(image.pixels instanceof Uint8ClampedArray)||image.pixels.byteLength!==next.width*next.height*4)throw new PdfApiError('internal','Invalid region output','viewport');
            this.retain(key,image);
          }
          if(this.plan!==plan||this.source!==source||!source.valid())continue;
          const context=this.canvas.getContext('2d');if(!context)throw resource('Viewport canvas is unavailable');
          context.putImageData(new ImageData(image.pixels,image.width,image.height),next.x-plan.area.x,next.y-plan.area.y);
          plan.done.add(key);this.report();
        }catch(error){if(this.plan===plan&&this.source===source&&source.valid())this.fail(error);}
      }
    }finally{this.running=false;}
  }
}
