import assert from 'node:assert/strict';
import {ViewportRaster,rasterWindow,TILE_CACHE_BYTES,TILE_CACHE_ENTRIES,VIEWPORT_BYTES} from '../dist/viewport-raster.js';
globalThis.ImageData=class {constructor(data,width,height){this.data=data;this.width=width;this.height=height;}};
class Canvas {
  w=1;h=1;peakPixels=1;style={};dataset={};attributes={};paint=[];
  get width(){return this.w;}set width(value){this.w=value;this.peakPixels=Math.max(this.peakPixels,this.w*this.h);}
  get height(){return this.h;}set height(value){this.h=value;this.peakPixels=Math.max(this.peakPixels,this.w*this.h);}
  setAttribute(k,v){this.attributes[k]=v;}
  getContext(){return {fillRect:()=>{},putImageData:(image,x,y)=>this.paint.push({color:image.data[0],x,y,width:image.width,height:image.height})};}
}
const tick=()=>new Promise(resolve=>setTimeout(resolve,0));
const tile=(r,color=17)=>({...r,pixels:new Uint8ClampedArray(r.width*r.height*4).fill(color)});
assert.deepEqual(rasterWindow(100,200,{x:0,y:0,width:100,height:200}),{x:0,y:0,width:100,height:200});
assert.equal(rasterWindow(6000,4000,{x:7000,y:0,width:1,height:1}),null);
assert.throws(()=>rasterWindow(32769,1,null));
assert.throws(()=>rasterWindow(6000,4000,{x:0,y:0,width:6000,height:4000}),e=>e.code==='resource_limit');
const clipped=rasterWindow(4000,4000,{x:512,y:512,width:2560,height:2560});
assert.deepEqual(clipped,{x:512,y:512,width:2560,height:2560}); // overscan alone exceeds the budget
assert.ok(clipped.width*clipped.height*4<=VIEWPORT_BYTES);
{
  const canvas=new Canvas(),errors=[];let visible={x:0,y:0,width:100,height:100},active=0,peak=0;
  const scheduler=new ViewportRaster(canvas,()=>visible,e=>errors.push(e));
  const requests=[];
  const render=r=>new Promise(resolve=>{active++;peak=Math.max(peak,active);requests.push({r,done:(color=17)=>{active--;resolve(tile(r,color));}});});
  const source={key:'document-a-revision-1-font-1',width:6000,height:4000,cssWidth:6000,cssHeight:4000,valid:()=>true,render};
  const sourceReady=scheduler.setSource(source),first=scheduler.refresh();assert.equal(requests.length,1);assert.equal(requests[0].r.x,0);
  visible={x:5500,y:3500,width:100,height:100};const newest=scheduler.refresh();
  assert.equal(await first,false);requests[0].done(11);await tick();
  assert.equal(canvas.paint.length,0,'stale viewport result painted');assert.ok(requests[1].r.x>=5120,'obsolete pending work was dispatched');
  while(canvas.attributes['aria-busy']==='true'){requests.at(-1).done();await tick();}
  assert.equal(await newest,true);assert.equal(await sourceReady,true);assert.equal(peak,1);assert.equal(errors.length,0);
  const before=requests.length;assert.equal(await scheduler.refresh(),true);assert.equal(requests.length,before);
  // Return to an old window: retained old-view tile can be reused, but cannot repaint a newer viewport.
  visible={x:0,y:0,width:100,height:100};const returned=scheduler.refresh();
  while(canvas.attributes['aria-busy']==='true'){requests.at(-1).done();await tick();}
  assert.equal(await returned,true);assert.ok(scheduler.statistics().cacheHits>0);
  visible={x:3000,y:2000,width:100,height:100};const replaced=scheduler.refresh();const old=requests.at(-1);
  const latest=scheduler.setSource({...source,key:'document-b',width:100,height:100,cssWidth:100,cssHeight:100});
  visible={x:0,y:0,width:100,height:100};const latestVisible=scheduler.refresh();old.done(99);await tick();
  while(canvas.attributes['aria-busy']==='true'){requests.at(-1).done(23);await tick();}
  assert.equal(await replaced,false);await latest;assert.equal(await latestVisible,true);
  assert.notEqual(canvas.paint.at(-1).color,99,'late source result painted');assert.equal(peak,1);
  scheduler.stop(true);assert.equal(scheduler.statistics().cacheBytes,0);assert.equal(scheduler.statistics().cacheEntries,0);assert.equal(canvas.width,1);
}
{
  const canvas=new Canvas();let visible={x:0,y:0,width:50,height:50};const scheduler=new ViewportRaster(canvas,()=>visible,e=>{throw e;});
  const source={key:'large',width:6000,height:4000,cssWidth:6000,cssHeight:4000,valid:()=>true,render:async r=>tile(r)};
  await scheduler.setSource(source);
  for(let y=0;y<4000;y+=900)for(let x=0;x<6000;x+=900){visible={x,y,width:50,height:50};assert.equal(await scheduler.refresh(),true);assert.ok(scheduler.statistics().cacheBytes<=TILE_CACHE_BYTES);assert.ok(scheduler.statistics().surfaceBytes<=VIEWPORT_BYTES);}
  assert.ok(scheduler.statistics().renderedTiles>16,'eviction pressure was not exercised');
  const before=scheduler.statistics().renderedTiles;await scheduler.setSource({...source,key:'new-font-epoch'});assert.ok(scheduler.statistics().renderedTiles>before,'new source reused stale font pixels');scheduler.stop(true);
}
{
  const canvas=new Canvas(),errors=[],visible={x:0,y:0,width:100,height:100};const scheduler=new ViewportRaster(canvas,()=>visible,e=>errors.push(e));
  const source={key:'retry',width:100,height:100,cssWidth:100,cssHeight:100,valid:()=>true,render:async()=>{throw Error('native allocation failed');}};
  assert.equal(await scheduler.setSource(source),false);assert.equal(canvas.width,1);assert.equal(errors.length,1);
  assert.equal(await scheduler.setSource({...source,render:async r=>tile(r)}),true);
  assert.equal(await scheduler.setSource({...source,key:'bad',render:async r=>({...tile(r),width:1})}),false);assert.equal(errors.length,2);
  scheduler.stop(true);
}
{
  const canvas=new Canvas();let visible={x:0,y:0,width:200,height:15000};
  const scheduler=new ViewportRaster(canvas,()=>visible,e=>{throw e;});
  await scheduler.setSource({key:'resize-pressure',width:16000,height:16000,cssWidth:16000,cssHeight:16000,valid:()=>true,render:async r=>tile(r)});
  visible={x:0,y:0,width:15000,height:200};assert.equal(await scheduler.refresh(),true);
  assert.ok(canvas.peakPixels*4<=VIEWPORT_BYTES,'intermediate canvas resize exceeded the display budget');scheduler.stop(true);
}
{
  const canvas=new Canvas(),scheduler=new ViewportRaster(canvas,()=>({x:0,y:0,width:1,height:1}),e=>{throw e;});
  for(let page=0;page<150;page++)await scheduler.setSource({key:'tiny-page-'+page,width:1,height:1,cssWidth:1,cssHeight:1,valid:()=>true,render:async r=>tile(r)});
  assert.equal(scheduler.statistics().cacheEntries,TILE_CACHE_ENTRIES);assert.equal(scheduler.statistics().cacheBytes,TILE_CACHE_ENTRIES*4);scheduler.stop(true);
}
console.log('Viewport raster: bounded window/cache, serial dispatch, latest view/source, stale suppression, eviction and retry passed');
