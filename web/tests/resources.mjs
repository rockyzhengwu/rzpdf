// Deterministic SDK/protocol tests. These stubs do not claim Wasm runtime QA.
import assert from 'node:assert/strict';
import {PdfEngine,PdfDocument} from '../dist/client.js';
import {rasterSize} from '../dist/raster.js';
import {validateArguments} from '../dist/protocol.js';
import {WorkerPdfEngine} from '../dist/worker-client.js';

for(const dpi of [576,1152]) {
  validateArguments('renderRegion',[0,dpi,{}, {x:0,y:0,width:64,height:64}]);
  assert.deepEqual(rasterSize(100,200,dpi),{width:100*dpi/72,height:200*dpi/72});
  assert.throws(()=>validateArguments('render',[0,dpi,{}]));
}
for(const dpi of [NaN,Infinity,0,1153]) {
  assert.throws(()=>validateArguments('renderRegion',[0,dpi,{}, {x:0,y:0,width:1,height:1}]));
  assert.throws(()=>rasterSize(100,200,dpi));
}
assert.throws(()=>rasterSize(10000,10000,1152));
assert.deepEqual(rasterSize(612.283,790.866,72),{width:613,height:791});
assert.deepEqual(rasterSize(600,792,96),{width:800,height:1056});
assert.deepEqual(rasterSize(0.1,0.2,72),{width:1,height:1});
validateArguments('open',[new Uint8Array([1]),{resourceProfile:'conservative'}]);
assert.throws(()=>validateArguments('open',[new Uint8Array([1]),{resourceProfile:'unbounded'}]));

const resource={reason:'image_pixels',stage:'image_decode',limit:4000000,actual:8750000};
const message=new TextEncoder().encode('Image decoding exceeds pixel budget');
const details=new TextEncoder().encode(JSON.stringify(resource));
const heap=new Uint8Array(4096);heap.set(message,32);heap.set(details,1024);
let nativeDpi;
const engine=new PdfEngine({HEAPU8:heap,_rz_abi_version:()=>2,_rz_page_count:()=>1,
  _rz_error_code:()=>7,_rz_error_ptr:()=>32,_rz_error_len:()=>message.length,
  _rz_error_details_ptr:()=>1024,_rz_error_details_len:()=>details.length,
  _rz_render_region:(_doc,_page,dpi)=>{nativeDpi=dpi;return 0;},_rz_dispose:()=>1});
const doc=new PdfDocument(engine,1);
assert.throws(()=>doc.renderRegion(0,{x:0,y:0,width:64,height:64},576),e=>e.code==='resource_limit'&&JSON.stringify(e.resource)===JSON.stringify(resource));
assert.equal(nativeDpi,576);
assert.throws(()=>doc.render(0,576),e=>e.code==='invalid_argument');
engine.dispose();

class FakeWorker {
  static instance;
  constructor(){FakeWorker.instance=this;}
  postMessage(request){queueMicrotask(()=>{
    const send=value=>this.onmessage({data:{protocol:2,session:request.session,id:request.id,...value}});
    if(request.operation==='initialize'){send({kind:'ready'});return;}
    if(request.operation==='renderRegion'){send({kind:'error',error:{code:'resource_limit',message:'Image decoding limit',operation:'renderRegion',resource,fatal:false}});return;}
    const value=request.operation==='open'?{document:1,pageCount:1}:
      request.operation==='capabilities'?{maxFullPageDpi:288,maxRegionDpi:1152}:
      request.operation==='cacheStatistics'?{pageEntries:1,pageHits:2}:request.operation==='clearRenderCaches'?{pageEntries:0}:null;
    send({kind:'result',value});
  });}
  terminate(){}
}
globalThis.Worker=FakeWorker;
const worker=new WorkerPdfEngine();
assert.equal((await worker.capabilities()).maxRegionDpi,1152);
const remote=await worker.open(new Uint8Array([1]),{resourceProfile:'conservative'});
assert.equal((await remote.cacheStatistics()).pageHits,2);
assert.equal((await remote.clearRenderCaches()).pageEntries,0);
await assert.rejects(remote.renderRegion(0,{x:0,y:0,width:1,height:1},576),e=>e.code==='resource_limit'&&e.resource.reason==='image_pixels'&&e.resource.actual===8750000);
assert.equal((await remote.capabilities()).maxFullPageDpi,288); // Nonfatal, retryable session.
await remote.close();worker.dispose();
console.log('Resource SDK/protocol: high-DPI modes, profiles, typed failures, engine queries and cache operations passed');
