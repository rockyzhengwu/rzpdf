import {FontControls} from './font-controls.js';
import {EditorPanel} from './editor.js';
import {PdfApiError,type PageInfo,type PageTarget,type TextPage,type OpenOptions} from './client.js';
import {createWorkerEngine,type WorkerPdfDocument} from './worker-client.js';
import {ViewportRaster} from './viewport-raster.js';
import {rasterSize,type RasterRegion} from './raster.js';
import {viewTransform,inverseViewPoint} from './geometry.js';
import {characterQuad} from './text.js';
const $=<T extends Element>(id:string)=>document.querySelector<T>('#'+id)!;
const status=$<HTMLOutputElement>('status'),canvas=$<HTMLCanvasElement>('canvas');
const input=$<HTMLInputElement>('pdf'),page=$<HTMLInputElement>('page'),button=$<HTMLButtonElement>('render');
const fontInput=$<HTMLInputElement>('font'),password=$<HTMLInputElement>('password'),unlock=$<HTMLButtonElement>('unlock');
const mode=$<HTMLSelectElement>('mode'),zoom=$<HTMLInputElement>('zoom'),layer=$<HTMLDivElement>('text-layer');
const rotation=$<HTMLSelectElement>('view-rotation'),content=$<HTMLDivElement>('page-content');
const fit=$<HTMLSelectElement>('fit'),viewport=$<HTMLDivElement>('viewport');
const links=$<HTMLDivElement>('links'),outline=$<HTMLDivElement>('outline'),results=$<HTMLDivElement>('results');
const marks=$<SVGSVGElement>('highlights'),view=$<HTMLDivElement>('page-view');
const cancel=$<HTMLButtonElement>('cancel'),reopen=$<HTMLButtonElement>('reopen'),find=$<HTMLButtonElement>('find');
let doc:WorkerPdfDocument|undefined,generation=0,fontGeneration=0,drawing=0,searching=0,opening=0,pending:Uint8Array|undefined;
let currentInfo:PageInfo|undefined;
let rasterSource:{width:number;height:number;cssWidth:number;cssHeight:number;rotation:number}|undefined;
const raster=new ViewportRaster(canvas,()=>{
  const source=rasterSource;if(!source)return null;
  const port=viewport.getBoundingClientRect(),box=view.getBoundingClientRect();
  const left=Math.max(0,port.left+viewport.clientLeft-box.left),top=Math.max(0,port.top+viewport.clientTop-box.top);
  const right=Math.min(box.width,port.left+viewport.clientLeft+viewport.clientWidth-box.left),bottom=Math.min(box.height,port.top+viewport.clientTop+viewport.clientHeight-box.top);
  if(right<=left||bottom<=top)return null;
  const points=[[left,top],[right,top],[right,bottom],[left,bottom]].map(([x,y])=>inverseViewPoint(source.cssWidth,source.cssHeight,source.rotation,x,y));
  const xs=points.map(p=>p[0]*source.width/source.cssWidth),ys=points.map(p=>p[1]*source.height/source.cssHeight);
  const x=Math.max(0,Math.floor(Math.min(...xs))),y=Math.max(0,Math.floor(Math.min(...ys)));
  return {x,y,width:Math.min(source.width,Math.ceil(Math.max(...xs)))-x,height:Math.min(source.height,Math.ceil(Math.max(...ys)))-y} as RasterRegion;
},showError);
let rasterFrame:number|undefined;
function refreshRaster():void {if(rasterFrame===undefined)rasterFrame=requestAnimationFrame(()=>{rasterFrame=undefined;void raster.refresh();});}
function stopRaster(clearCache=false):void {if(rasterFrame!==undefined)cancelAnimationFrame(rasterFrame);rasterFrame=undefined;rasterSource=undefined;raster.stop(clearCache);}

let fontResources:FontControls|undefined;
let refreshEditor:(doc:WorkerPdfDocument)=>Promise<unknown>=async()=>false;
let openImportSource:(bytes:Uint8Array,options:OpenOptions)=>Promise<WorkerPdfDocument>=async()=>{throw Error('引擎尚未就绪');};
const editor=new EditorPanel($<HTMLElement>('editor'),$<HTMLElement>('widgets'),target=>refreshEditor(target),showError,(bytes,options)=>openImportSource(bytes,options));
function showError(error:unknown) {status.textContent=error instanceof Error?error.message:String(error);}
function externalUri(uri:string):string|undefined {
  try {const url=new URL(uri);return ['http:','https:','mailto:'].includes(url.protocol)?url.href:undefined;}catch{return undefined;}
}
function clearView():void {
  stopRaster(true);editor.clear();
  canvas.width=canvas.height=1;view.style.width=view.style.height='1px';content.style.width=content.style.height='1px';content.style.transform='none';
  layer.replaceChildren();links.replaceChildren();marks.replaceChildren();outline.replaceChildren();results.replaceChildren();currentInfo=undefined;
}
try {
  let engine=await createWorkerEngine();
  openImportSource=(bytes,options)=>engine.open(bytes,options);
  status.textContent='请选择本地 PDF（最多 8 MiB）';input.disabled=false;fontInput.disabled=false;
  function bindEngine():void {
  const target=engine;
  target.addEventListener('queuechange',()=>{if(target===engine)cancel.disabled=!(engine.busy||fontResources?.busy);});
  target.addEventListener('sessionend',()=>{
    if(target!==engine)return;
    fontResources?.cancel();
    generation++;drawing++;searching++;opening++;fontGeneration++;
    doc=undefined;pending=undefined;password.value='';unlock.hidden=true;button.disabled=find.disabled=true;
    reopen.disabled=!input.files?.length;clearView();
    status.textContent='处理会话已结束，请重新打开文件；加密文件需要重新输入密码。';
  });
  }
  fontResources=new FontControls($<HTMLElement>('font-resources'),{
    idle:async()=>{const target=engine;if(!target.busy)return;await new Promise<void>(resolve=>{const changed=()=>{if(target!==engine||!target.busy){target.removeEventListener('queuechange',changed);resolve();}};target.addEventListener('queuechange',changed);changed();});},
    clearFonts:()=>engine.clearFonts(),invalidate:()=>{drawing++;stopRaster(true);editor.clear();},changed:async()=>{if(doc)return draw();},busyChanged:()=>{cancel.disabled=!(engine.busy||fontResources?.busy);}
  });
  bindEngine();
  async function navigate(target:PageTarget):Promise<void> {
    if(!doc)return;
    page.value=String(target.page+1);
    if(target.view.kind==='XYZ'&&target.view.zoom&&target.view.zoom>0){fit.value='manual';zoom.value=String(Math.min(400,Math.max(10,target.view.zoom*100)));}
    if(!await draw())return;
    if(currentInfo&&(target.view.left!=null||target.view.top!=null)) {
      const [a,b,c,d,e,f]=currentInfo.userToDisplay;
      const x=target.view.left??currentInfo.visibleBox[0],y=target.view.top??currentInfo.visibleBox[3];
      const scale=Number(zoom.value)/100,box=view.getBoundingClientRect(),port=viewport.getBoundingClientRect();
      const [ra,rb,rc,rd,re,rf]=viewTransform(currentInfo.width*scale,currentInfo.height*scale,Number(rotation.value)).matrix;
      const dx=(a*x+c*y+e)*scale,dy=(b*x+d*y+f)*scale;
      viewport.scrollTo({left:viewport.scrollLeft+box.left-port.left+ra*dx+rc*dy+re,top:viewport.scrollTop+box.top-port.top+rb*dx+rd*dy+rf});
    }
  }
  async function draw():Promise<boolean> {
    if(!doc)return false;
    stopRaster();editor.clear();currentInfo=undefined;layer.replaceChildren();links.replaceChildren();marks.replaceChildren();
    const target=doc,stamp=generation,drawId=++drawing,index=Number(page.value)-1,viewRotation=Number(rotation.value);
    let scale=Number(zoom.value)/100;
    const current=()=>doc===target&&stamp===generation&&drawId===drawing;
    try {
      const info=await target.pageInfo(index);if(!current())return false;
      if(fit.value!=='manual'){
        const css=getComputedStyle(viewport),availableWidth=viewport.clientWidth-parseFloat(css.paddingLeft)-parseFloat(css.paddingRight);
        const availableHeight=parseFloat(css.maxHeight)-parseFloat(css.paddingTop)-parseFloat(css.paddingBottom);
        const rotated=viewTransform(info.width,info.height,viewRotation);
        const ratio=fit.value==='page'?Math.min(availableWidth/rotated.width,availableHeight/rotated.height):availableWidth/rotated.width;
        scale=Math.min(4,Math.max(.1,ratio));zoom.value=String(scale*100);
      }
      if(!Number.isFinite(scale)||scale<.1||scale>4)throw Error('缩放范围为 10%–400%');
      status.textContent=`正在显示第 ${index+1} 页…`;
      const fontReport=await fontResources?.ensure(target,index,{viewZoom:scale,viewRotation},current);if(!current())return false;
      const state=await target.state();if(!current())return false;
      const dpi=Math.min(288,72*scale*window.devicePixelRatio),size=rasterSize(info.width,info.height,dpi);
      const width=info.width*scale,height=info.height*scale,layout=viewTransform(width,height,viewRotation);
      content.style.width=width+'px';content.style.height=height+'px';
      content.style.transform=`matrix(${layout.matrix.join(',')})`;view.style.width=layout.width+'px';view.style.height=layout.height+'px';
      view.dataset.rotation=String(viewRotation);
      rasterSource={...size,cssWidth:width,cssHeight:height,rotation:viewRotation};
      const key=JSON.stringify([state.stamp.engine,state.stamp.document,state.stamp.revision,fontGeneration,fontResources?.rasterEpoch,fontReport?.stamp.fontEpoch,index,dpi,scale,viewRotation]);
      const rendered=raster.setSource({key,...size,cssWidth:width,cssHeight:height,valid:current,render:area=>target.renderRegion(index,area,dpi,{viewZoom:scale,viewRotation})});
      const textResult=target.extractText(index).then(text=>({text,copyDenied:false})).catch(error=>{
        if(error instanceof PdfApiError&&error.code==='permission')return {text:{order:'paint',coordinateSpace:'display-points-top-left',characters:[],diagnostics:[]} as TextPage,copyDenied:true};
        throw error;
      });
      const [{text,copyDenied},pageLinks,ready]=await Promise.all([textResult,target.links(index),rendered]);
      if(!ready)return false;
      if(!current())return false;
      currentInfo=info;page.value=String(index+1);
      layer.replaceChildren();links.replaceChildren();marks.replaceChildren();marks.setAttribute('viewBox',`0 0 ${info.width} ${info.height}`);
      find.disabled=copyDenied;
      const measure=document.createElement('canvas').getContext('2d')!;
      for(const char of text.characters) {
        if(!char.emToDisplay)continue;
        const span=document.createElement('span'),value=char.unicode??'\uFFFD';span.textContent=value;
        if(char.unicode===null){span.dataset.unmapped='true';span.setAttribute('aria-label','缺少 Unicode 映射');}
        const [a,b,c,d]=char.emToDisplay,[x,y]=char.position,[ax,ay]=char.advance;
        const length=Math.hypot(ax,ay),[nx,ny]=char.writingMode==='Vertical'?[a,b]:[c,d],height=Math.hypot(nx,ny)*scale;
        if(!height)continue;
        measure.font=`${height}px sans-serif`;const natural=measure.measureText(value).width||1;
        const ux=length?ax/length:1,uy=length?ay/length:0;
        span.style.left=`${(x+nx*.8)*scale}px`;span.style.top=`${(y+ny*.8)*scale}px`;span.style.fontSize=`${height}px`;
        span.style.transform=`matrix(${ux*length/natural*scale},${uy*length/natural*scale},${-nx*scale/height},${-ny*scale/height},0,0)`;layer.append(span);
      }
      for(const link of pageLinks) {
        const [x0,y0,x1,y1]=link.rect,[a,b,c,d,e,f]=info.userToDisplay;
        const points=[[x0,y0],[x1,y0],[x1,y1],[x0,y1]].map(([x,y])=>[a*x+c*y+e,b*x+d*y+f]);
        const xs=points.map(p=>p[0]),ys=points.map(p=>p[1]),left=Math.min(...xs),top=Math.min(...ys);
        const anchor=document.createElement('a');anchor.style.cssText=`left:${left*scale}px;top:${top*scale}px;width:${(Math.max(...xs)-left)*scale}px;height:${(Math.max(...ys)-top)*scale}px`;
        if(link.target.kind==='page') {
          const target=link.target;anchor.href='#';anchor.setAttribute('aria-label',`转到第 ${target.page+1} 页`);
          anchor.addEventListener('click',event=>{event.preventDefault();void navigate(target);});
        }else {
          const uri=externalUri(link.target.uri);if(!uri)continue;
          anchor.href=uri;anchor.target='_blank';anchor.rel='noopener noreferrer';anchor.title=uri;anchor.setAttribute('aria-label',uri);
        }
        links.append(anchor);
      }
      await editor.show(target,index,info,scale,viewRotation);
      if(!current())return false;
      const missing=text.characters.filter(c=>c.unicode===null).length;
      status.textContent=`第 ${index+1} / ${target.pageCount} 页${copyDenied?'；此文档不允许复制或搜索文字':''}${missing?`；${missing} 个字符缺少文字映射`:''}${text.diagnostics.length?`；${text.diagnostics.length} 条兼容诊断`:''}`;
      return true;
    }catch(error){if(current()){stopRaster();showError(error);}return false;}
  }
  refreshEditor=async(edited)=>{
    const target=doc,stamp=generation;if(!target||target!==edited)return false;
    drawing++;searching++;stopRaster(true);editor.clear();results.replaceChildren();marks.replaceChildren();outline.replaceChildren();links.replaceChildren();layer.replaceChildren();
    window.getSelection()?.removeAllRanges();status.textContent='正在更新页面…';
    try {
      page.max=String(target.pageCount);page.value=String(Math.min(target.pageCount,Math.max(1,Number(page.value)||1)));
      await loadOutline(target,stamp);if(target===doc&&stamp===generation)return await draw();
    }catch(error){if(target===doc&&stamp===generation)showError(error);}
    return false;
  };
  async function loadOutline(target:WorkerPdfDocument,stamp:number):Promise<void> {
    const items=await target.outlines();if(doc!==target||stamp!==generation)return;
    outline.replaceChildren();const depths=new Map<string,number>();
    for(const item of items) {
      const depth=item.parent?(depths.get(item.parent.join(':'))??0)+1:0;depths.set(item.id.join(':'),depth);
      const entry=document.createElement('button');entry.textContent=item.title;entry.style.marginLeft=`${Math.min(depth,64)*12}px`;
      entry.addEventListener('click',()=>{void navigate(item.target);});outline.append(entry);
    }
  }
  async function openPending():Promise<void> {
    if(!pending)return;
    const bytes=pending,stamp=generation,openId=++opening;unlock.disabled=true;
    const current=()=>stamp===generation&&openId===opening;
    try {
      const pass=password.value;password.value='';
      const candidate=await engine.open(bytes,{mode:mode.value==='compatible'?'compatible':'strict',...(pass?{password:pass}:{})});
      if(!current()){await candidate.close();return;}
      doc=candidate;pending=undefined;unlock.hidden=true;reopen.disabled=false;
      page.value='1';page.max=String(candidate.pageCount);button.disabled=false;find.disabled=false;
      await loadOutline(candidate,stamp);if(current())await draw();
    }catch(error){
      if(!current())return;
      if(error instanceof PdfApiError&&['password_required','password_invalid'].includes(error.code)){unlock.hidden=false;password.focus();}
      else pending=undefined;
      showError(error);
    }finally {if(current())unlock.disabled=false;}
  }
  async function loadFile():Promise<void> {
    fontResources?.cancel();
    const previous=doc;if(engine.busy)engine.cancel();
    const current=++generation;drawing++;searching++;opening++;doc=undefined;pending=undefined;
    button.disabled=find.disabled=true;unlock.hidden=true;unlock.disabled=false;clearView();status.textContent='正在打开文件…';
    try {
      await previous?.close();const file=input.files?.[0];if(!file)return;
      if(file.size>8*1024*1024)throw Error('文件超过 8 MiB');
      const bytes=new Uint8Array(await file.arrayBuffer());if(current!==generation)return;
      const font=fontInput.files?.[0];
      if(font&&font.size>32*1024*1024)throw Error('字体文件超过 32 MiB');
      const fontBytes=font?new Uint8Array(await font.arrayBuffer()):undefined;if(current!==generation)return;
      await engine.clearFonts();if(current!==generation)return;
      await engine.setSubstituteFont(fontBytes);if(current!==generation)return;
      pending=bytes;await openPending();
    }catch(error){if(current===generation)showError(error);}
  }
  input.addEventListener('change',()=>{void loadFile();});reopen.addEventListener('click',()=>{void loadFile();});
  cancel.addEventListener('click',()=>{fontResources?.cancel();if(engine.busy)engine.cancel();});
  unlock.addEventListener('click',()=>{void openPending();});password.addEventListener('keydown',event=>{if(event.key==='Enter'&&pending)void openPending();});
  button.addEventListener('click',()=>{void draw();});
  zoom.addEventListener('input',()=>{fit.value='manual';});
  fit.addEventListener('change',()=>{if(doc)void draw();});
  rotation.addEventListener('change',()=>{if(doc)void draw();});
  let fitTimer:ReturnType<typeof setTimeout>|undefined;
  const resizeFit=()=>{
    if(fitTimer!==undefined)clearTimeout(fitTimer);
    if(doc&&fit.value!=='manual')fitTimer=setTimeout(()=>{fitTimer=undefined;if(doc&&fit.value!=='manual')void draw();},100);
  };
  // Observe width only: fitting changes content height and must not schedule itself.
  let viewportWidth=viewport.clientWidth;
  const fitObserver=new ResizeObserver(()=>{refreshRaster();const width=viewport.clientWidth;if(width!==viewportWidth){viewportWidth=width;resizeFit();}});
  fitObserver.observe(viewport);
  viewport.addEventListener('scroll',refreshRaster,{passive:true});
  let pixelRatio=window.devicePixelRatio;
  const resized=()=>{if(pixelRatio!==window.devicePixelRatio){pixelRatio=window.devicePixelRatio;if(doc)void draw();}else{resizeFit();refreshRaster();}};
  window.addEventListener('resize',resized);
  let density:MediaQueryList|undefined;
  const densityChanged=()=>{resized();watchDensity();};
  function watchDensity():void {density?.removeEventListener('change',densityChanged);density=matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);density.addEventListener('change',densityChanged,{once:true});}
  watchDensity();
  window.addEventListener('pagehide',()=>{stopRaster(true);density?.removeEventListener('change',densityChanged);});
  window.addEventListener('pagehide',()=>{fitObserver.disconnect();if(fitTimer!==undefined)clearTimeout(fitTimer);});
  $<HTMLFormElement>('search').addEventListener('submit',async event=>{
    event.preventDefault();if(!doc)return;
    const target=doc,stamp=generation,searchId=++searching,current=()=>doc===target&&generation===stamp&&searching===searchId;
    try {
      status.textContent='正在搜索…';const report=await target.searchText($<HTMLInputElement>('query').value);if(!current())return;
      results.replaceChildren();
      for(const match of report.matches) {
        const entry=document.createElement('button');entry.textContent=`第 ${match.page+1} 页`;
        entry.addEventListener('click',async()=>{
          if(!current())return;
          page.value=String(match.page+1);if(!await draw()||!current())return;
          const drawId=drawing;
          try {
            const text=await target.extractText(match.page);if(!current()||drawId!==drawing)return;
            for(const char of text.characters.slice(match.start,match.end)) {
              const quad=characterQuad(char);if(!quad)continue;
              const polygon=document.createElementNS('http://www.w3.org/2000/svg','polygon');polygon.setAttribute('points',[0,2,4,6].map(i=>`${quad[i]},${quad[i+1]}`).join(' '));marks.append(polygon);
            }
          }catch(error){if(current())showError(error);}
        });results.append(entry);
      }
      status.textContent=`找到 ${report.matches.length} 处${report.truncated?'（达到结果上限）':''}${report.missingUnicode?'；部分字符缺少映射，搜索结果可能不完整':''}`;
    }catch(error){if(current())showError(error);}
  });
  fontInput.addEventListener('change',async()=>{
    const current=++fontGeneration;drawing++;stopRaster(true);
    try {
      const file=fontInput.files?.[0];if(file&&file.size>32*1024*1024)throw Error('字体文件超过 32 MiB');
      const bytes=file?new Uint8Array(await file.arrayBuffer()):undefined;if(current!==fontGeneration)return;
      await engine.setSubstituteFont(bytes);if(current===fontGeneration&&doc)await draw();
    }catch(error){if(current===fontGeneration)showError(error);}
  });
  window.addEventListener('pageshow',async event=>{
    if(!event.persisted)return;
    const stamp=++generation;input.disabled=fontInput.disabled=true;
    try {
      const candidate=await createWorkerEngine();
      if(stamp!==generation){candidate.dispose();return;}
      engine=candidate;bindEngine();fitObserver.observe(viewport);watchDensity();fontResources?.resume();input.disabled=fontInput.disabled=false;reopen.disabled=!input.files?.length;
      status.textContent='处理会话已恢复，请重新打开文件。';
    }catch(error){if(stamp===generation)showError(error);}
  });
  window.addEventListener('pagehide',()=>{fontResources?.dispose();generation++;drawing++;searching++;opening++;fontGeneration++;pending=undefined;password.value='';engine.dispose();});
}catch(error){status.textContent=`加载失败：${error}`;}
