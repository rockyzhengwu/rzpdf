// Dedicated-Worker QA for the signed i32 / unsigned source-ID ABI boundary.
import {createEngine} from '../../dist/client.js';
self.onmessage=async({data:file})=>{
  let engine,response;
  try{
    engine=await createEngine();
    const ids=[];
    for(const seed of [0x7fffffff,0xfffffffe]){
      engine.nextBlob=seed; // Test-only counter seeding; not a public SDK API.
      const doc=engine.openBlob(file,{mode:'strict'});
      const image=doc.render(0,72);
      if(!image.pixels.every((value,index)=>value===[255,0,0,255][index%4]))throw Error('Source-ID boundary changed pixels');
      doc.close();
      const stats=engine.statistics();
      if(stats.handles!==0||stats.externalSources!==0)throw Error('Unsigned source owner was not released');
      ids.push(seed+1);
    }
    let exhausted;
    try{engine.openBlob(file);}catch(error){exhausted=error;}
    if(exhausted?.code!=='resource_limit')throw Error('Source-ID exhaustion must be explicit');
    response={unsignedSourceIds:ids,release:true,exhaustion:'resource_limit'};
  }catch(error){response={error:error.message};}
  finally{engine?.dispose();}
  self.postMessage(response);
};
