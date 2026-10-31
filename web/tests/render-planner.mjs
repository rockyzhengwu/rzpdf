import assert from 'node:assert/strict';
import {planTiles,renderTiles,renderPreview} from '../dist/render-planner.js';

const caps={maxRegionDpi:1152,maxRegionSide:1024,maxRenderPixels:4000000,maxRenderBytes:16000000};
const info={width:600,height:840};
assert.equal(planTiles(info,caps,{dpi:576}).width,4800);
assert.equal(planTiles(info,caps,{dpi:576}).tileSide,512);
assert.throws(()=>planTiles(info,caps,{dpi:1153}));
assert.throws(()=>planTiles(info,caps,{dpi:72,tileSide:Infinity}));
assert.throws(()=>planTiles({width:100000,height:600},caps,{dpi:72}));
let revision=0,calls=0;
const document={
  pageInfo:()=>info, capabilities:()=>caps,
  state:()=>({stamp:{engine:'engine',document:1,revision:String(revision)}}),
  renderRegion:(_page,region)=>{
    calls++;
    const pixels=new Uint8ClampedArray(region.width*region.height*4);
    // A position-dependent oracle checks row strides and tile placement.
    for(let y=0;y<region.height;y++)for(let x=0;x<region.width;x++){
      const p=(y*region.width+x)*4;
      pixels.set([(region.x+x)%256,(region.y+y)%256,123,255],p);
    }
    return {...region,pixels};
  },
};
const preview=await renderPreview(document,0,{maxWidth:600,maxHeight:840});
assert.equal(preview.dpi,72);
assert.equal(calls,4);
for(const [x,y] of [[0,0],[511,511],[512,512],[599,839]])assert.deepEqual([...preview.pixels.subarray((y*600+x)*4,(y*600+x)*4+4)],[x%256,y%256,123,255]);
const controller=new AbortController();
const iterator=renderTiles(document,0,{dpi:72,signal:controller.signal});
await iterator.next();controller.abort();
await assert.rejects(iterator.next(),e=>e.name==='AbortError');
const changed=renderTiles(document,0,{dpi:72});
await changed.next();revision++;
await assert.rejects(changed.next(),/changed/);
const resource=Object.assign(new Error('image source bytes'),{code:'resource_limit',resource:{reason:'image_source_bytes'}});
calls=0;
await assert.rejects(renderPreview({...document,renderRegion(){calls++;throw resource;}},0,{maxWidth:600,maxHeight:840}),e=>e===resource);
assert.equal(calls,1,'source errors must not trigger tile retry loops');
await assert.rejects(renderPreview(document,0,{maxWidth:1,maxHeight:1}),/minimum/);
console.log('render planner: bounded assembly, tile coordinates, revisions, cancellation and no ineffective retries passed');

await assert.rejects(renderPreview(document,0,{maxWidth:.5,maxHeight:.5}),/pixel/);
await assert.rejects(renderPreview({...document,pageInfo:()=>({width:4000,height:4000})},0,{maxWidth:4000,maxHeight:4000}),e=>e.code==='resource_limit'&&e.resource.reason==='render_pixels'&&e.resource.limit===4000000&&e.resource.actual===16000000);

const {PdfApiError}=await import('../dist/errors.js');
const rasterLimit=new PdfApiError('resource_limit','group surface limit','render',0,undefined,undefined,{reason:'render_live_bytes',stage:'render_group',limit:100,actual:200});
let retries=0;
const adaptive=await renderPreview({...document,renderRegion(...args){retries++;if(args[1].width>200||args[1].height>200)throw rasterLimit;return document.renderRegion(...args);}},0,{maxWidth:600,maxHeight:840});
assert.deepEqual(adaptive.pixels,preview.pixels,'raster retry placement must preserve the whole overview');
assert.ok(retries>4);
retries=0;
await assert.rejects(renderPreview({...document,renderRegion(){retries++;throw rasterLimit;}},0,{maxWidth:600,maxHeight:840}),e=>e===rasterLimit);
assert.equal(retries,3,'adaptive raster retries have at most two bisection levels');
