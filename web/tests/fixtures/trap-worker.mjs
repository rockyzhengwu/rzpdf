// Test-only controlled real Wasm fault through the production Worker dispatcher.
import {PdfEngine} from '/dist/client.js';
import '/dist/pdf-worker.js';
const binary=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,7,8,1,4,116,114,97,112,0,0,10,5,1,3,0,0,11]);
const trap=new WebAssembly.Instance(new WebAssembly.Module(binary)).exports.trap;
const invoke=PdfEngine.prototype.invoke;
let eligible=false,armed=false;
PdfEngine.prototype.invoke=function(fn){
  if(armed){armed=false;return invoke.call(this,()=>trap());}
  return invoke.call(this,fn);
};
const original=self.onmessage;
self.onmessage=event=>{
  if(event.data?.operation==='open')eligible=event.data.args[1]?.mode==='compatible';
  if(event.data?.operation==='render'&&eligible){eligible=false;armed=true;}
  original.call(self,event);
};
