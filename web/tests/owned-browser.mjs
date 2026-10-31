// Keep the browser process group owned until teardown. A graceful Chrome exit
// can leave its on-device-model utility holding Playwright's inherited pipes.
// Kill the owned group before the launcher observes the main process exiting.
export async function launchOwnedBrowser(chromium,options){
  const server=await chromium.launchServer({...options,handleSIGINT:false,handleSIGTERM:false,handleSIGHUP:false});
  let browser,closing;
  const handlers=new Map();
  const close=()=>closing??=(async()=>{
    try{await server.kill();}
    finally{
      // Detached crash collectors/updaters can inherit these pipes outside the
      // owned browser process group. Close only our launcher's stream ends;
      // never search for or terminate unrelated system processes.
      for(const stream of new Set(server.process().stdio??[]))stream?.destroy?.();
      await browser?.close().catch(()=>{});
      for(const [signal,handler] of handlers)process.removeListener(signal,handler);
    }
  })();
  for(const [signal,code] of [['SIGINT',130],['SIGTERM',143],['SIGHUP',129]]){
    const handler=()=>{void close().finally(()=>process.exit(code));};
    handlers.set(signal,handler);process.once(signal,handler);
  }
  try{browser=await chromium.connect(server.wsEndpoint());return {browser,close};}
  catch(error){await close();throw error;}
}
