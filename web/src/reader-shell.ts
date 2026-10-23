/** Presentation and reader navigation. PDF work remains owned by demo's Worker lifecycle. */
const paths:Record<string,string>={
 document:'M14 2H5a1 1 0 0 0-1 1v18a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1V8z M14 2v6h6 M8 12h8 M8 16h6',
 folder:'M3 6h6l2 2h10v12H3z M3 6V4h6l2 2',download:'M12 3v12 M7 10l5 5 5-5 M4 16v5h16v-5',
 sidebar:'M3 4h18v16H3z M9 4v16 M5.5 8h1 M5.5 11h1',left:'M15 5l-7 7 7 7',right:'M9 5l7 7-7 7',
 minus:'M5 12h14',plus:'M5 12h14 M12 5v14',rotate:'M20 10a8 8 0 1 0-1 8 M20 4v6h-6',refresh:'M20 10a8 8 0 1 0-1 8 M20 4v6h-6',
 expand:'M8 3H3v5 M16 3h5v5 M3 16v5h5 M21 16v5h-5',edit:'M15 4l5 5 M4 16L16 4a2 2 0 0 1 4 4L8 20H4z',
 pages:'M7 3h13v15H7z M4 7v14h12',bookmark:'M6 3h12v18l-6-4-6 4z',search:'M21 21l-5-5 M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0',
 shield:'M12 2l8 3v6c0 5-4 9-8 11-4-2-8-6-8-11V5z M8 11l3 3 5-6',check:'M5 12l4 4L19 6',close:'M6 6l12 12 M6 18L18 6',
 settings:'M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1z M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0',moon:'M20 15A9 9 0 0 1 9 3a9 9 0 1 0 11 12'
};
const el=<T extends HTMLElement>(id:string)=>document.getElementById(id) as T;
export class ReaderShell {
 private count=0;private current=1;private file:File|undefined;private dragDepth=0;
 constructor(private render:()=>void){
  document.querySelectorAll<HTMLElement>('[data-icon]').forEach(node=>{const path=paths[node.dataset.icon!];if(path)node.innerHTML=`<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="${path}"/></svg>`;});
  document.querySelectorAll<HTMLElement>('label[for=pdf]').forEach(label=>{label.tabIndex=0;label.setAttribute('role','button');label.addEventListener('keydown',event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();el<HTMLInputElement>('pdf').click();}});});
  this.sidebar(!matchMedia('(max-width:700px)').matches);
  el('sidebar-toggle').addEventListener('click',()=>this.sidebar(el('app').dataset.sidebar!=='true'));
  document.querySelectorAll<HTMLElement>('[data-tab]').forEach(button=>button.addEventListener('click',()=>this.tab(button.dataset.tab!)));
  document.querySelectorAll<HTMLElement>('[data-panel]').forEach(button=>button.addEventListener('click',()=>this.panel(button.dataset.panel as 'edit'|'settings')));
  document.querySelector('.sidebar-tabs')!.addEventListener('keydown',event=>{const key=(event as KeyboardEvent).key;if(key!=='ArrowLeft'&&key!=='ArrowRight')return;event.preventDefault();const names=['pages','outline','search'],selected=document.querySelector<HTMLElement>('[data-tab][aria-selected=true]')!.dataset.tab!,next=names[(names.indexOf(selected)+(key==='ArrowRight'?1:2))%3];this.tab(next);el('tab-'+next).focus();});
  el('close-inspector').addEventListener('click',()=>this.panel());
  el('previous-page').addEventListener('click',()=>this.navigate(this.current-1));el('next-page').addEventListener('click',()=>this.navigate(this.current+1));
  el('page').addEventListener('change',()=>this.navigate(Number(el<HTMLInputElement>('page').value)));
  el('page').addEventListener('keydown',event=>{if(event.key==='Enter')this.navigate(Number(el<HTMLInputElement>('page').value));});
  el('zoom-in').addEventListener('click',()=>this.zoom(25));el('zoom-out').addEventListener('click',()=>this.zoom(-25));
  el('zoom').addEventListener('change',()=>{el<HTMLSelectElement>('fit').value='manual';this.render();});
  el('rotate').addEventListener('click',()=>{const select=el<HTMLSelectElement>('view-rotation');select.value=String((Number(select.value)+90)%360);select.dispatchEvent(new Event('change'));});
  el('fullscreen').addEventListener('click',()=>{void (document.fullscreenElement?document.exitFullscreen():el('app').requestFullscreen()).catch(()=>{el('status').textContent='当前浏览器无法进入全屏。';});});
  document.addEventListener('fullscreenchange',()=>el('fullscreen').setAttribute('aria-label',document.fullscreenElement?'退出全屏':'全屏阅读'));
  el('download').addEventListener('click',()=>el<HTMLButtonElement>('save-pdf')?.click());
  new MutationObserver(()=>{const save=el<HTMLButtonElement>('save-pdf');el<HTMLButtonElement>('download').disabled=!save||save.disabled;}).observe(el('editor'),{childList:true,subtree:true,attributes:true,attributeFilter:['disabled']});
  let theme='light';try{theme=localStorage.getItem('rzpdf-reader-theme')??'light';}catch{/* Storage is optional. */}
  this.theme(theme==='dark');document.querySelectorAll<HTMLElement>('[data-theme-toggle]').forEach(button=>button.addEventListener('click',()=>{const dark=document.documentElement.dataset.theme!=='dark';this.theme(dark);try{localStorage.setItem('rzpdf-reader-theme',dark?'dark':'light');}catch{/* Storage is optional. */}}));
  document.addEventListener('keydown',event=>{
   const modifier=event.ctrlKey||event.metaKey,key=event.key.toLowerCase();
   if(modifier&&key==='o'){event.preventDefault();el<HTMLInputElement>('pdf').click();return;}
   if(modifier&&key==='s'){event.preventDefault();el<HTMLButtonElement>('download').click();return;}
   if(modifier&&key==='f'){event.preventDefault();this.tab('search');el('query').focus();return;}
   if(event.key==='Escape'){this.panel();if(matchMedia('(max-width:700px)').matches)this.sidebar(false);return;}
   if(event.target instanceof Element&&event.target.closest('input,select,textarea,button,a,[contenteditable=true]'))return;
   if(event.altKey||modifier||event.shiftKey)return;
   if(event.key==='ArrowLeft'||event.key==='PageUp'){event.preventDefault();this.navigate(this.current-1);}
   if(event.key==='ArrowRight'||event.key==='PageDown'){event.preventDefault();this.navigate(this.current+1);}
   if(event.key==='Home'){event.preventDefault();this.navigate(1);}
   if(event.key==='End'){event.preventDefault();this.navigate(this.count);}
  });
  const isFile=(event:DragEvent)=>Array.from(event.dataTransfer?.types??[]).includes('Files');
  document.addEventListener('dragenter',event=>{if(isFile(event)){event.preventDefault();this.dragDepth++;el('drop-overlay').hidden=false;}});
  document.addEventListener('dragover',event=>{if(isFile(event)){event.preventDefault();if(event.dataTransfer)event.dataTransfer.dropEffect='copy';}});
  document.addEventListener('dragleave',()=>{if(--this.dragDepth<=0){this.dragDepth=0;el('drop-overlay').hidden=true;}});
  document.addEventListener('drop',event=>{if(!isFile(event))return;event.preventDefault();this.dragDepth=0;el('drop-overlay').hidden=true;const file=event.dataTransfer?.files[0],input=el<HTMLInputElement>('pdf');if(!file||input.disabled)return;if(!/\.pdf$/i.test(file.name)&&file.type!=='application/pdf'){this.error('请选择 PDF 格式的文件。');return;}const transfer=new DataTransfer();transfer.items.add(file);input.files=transfer.files;input.dispatchEvent(new Event('change'));});
 }
 private theme(dark:boolean):void{document.documentElement.dataset.theme=dark?'dark':'light';document.querySelectorAll('[data-theme-toggle]').forEach(button=>button.setAttribute('aria-pressed',String(dark)));}
 private sidebar(open:boolean):void{el('app').dataset.sidebar=String(open);el('sidebar').hidden=!open;const button=el('sidebar-toggle');button.setAttribute('aria-expanded',String(open));button.setAttribute('aria-label',open?'收起侧边栏':'展开侧边栏');}
 tab(name:string):void{this.sidebar(true);document.querySelectorAll<HTMLElement>('[data-tab]').forEach(button=>{const active=button.dataset.tab===name;button.setAttribute('aria-selected',String(active));button.tabIndex=active?0:-1;el('panel-'+button.dataset.tab).hidden=!active;});}
 panel(name?:'edit'|'settings'):void{el('inspector').hidden=!name;el('edit-panel').hidden=name!=='edit';el('settings-panel').hidden=name!=='settings';el('inspector-title').textContent=name==='settings'?'阅读设置':'编辑文档';}
 error(message:string):void{el('app').dataset.error='true';el('status').textContent=message;}
 reset():void{this.count=0;this.current=1;el('welcome').hidden=false;el('viewport').hidden=true;el('page-count').textContent='—';el('pages-label').textContent='0 页';el('reading-progress').textContent='准备就绪';el('page-list').replaceChildren();el<HTMLButtonElement>('previous-page').disabled=el<HTMLButtonElement>('next-page').disabled=true;el('app').dataset.error='false';el('document-name').textContent='你的下一页，从这里开始';el('document-meta').textContent='本地阅读 · 安心专注';document.title='rzpdf · 阅读空间';}
 opened(count:number):void{this.count=count;this.file=el<HTMLInputElement>('pdf').files?.[0];el('welcome').hidden=true;el('viewport').hidden=false;el('document-name').textContent=this.file?.name??'PDF 文档';const bytes=this.file?.size??0;el('document-meta').textContent=`${count} 页 · ${bytes<1024*1024?Math.max(1,Math.round(bytes/1024))+' KiB':(bytes/1024/1024).toFixed(1)+' MiB'} · 本地文档`;document.title=`${this.file?.name??'PDF'} · rzpdf`;el('page-count').textContent=String(count);el('pages-label').textContent=`${count} 页`;this.page(1);if(matchMedia('(max-width:700px)').matches)this.sidebar(false);}
 page(number:number):void{this.current=number;el('app').dataset.error='false';el<HTMLButtonElement>('previous-page').disabled=number<=1;el<HTMLButtonElement>('next-page').disabled=number>=this.count;el('reading-progress').textContent=`第 ${number} 页，共 ${this.count} 页`;
  // Keep navigation bounded for very large documents. This is a page index, not a thumbnail render cache.
  const start=Math.max(1,number-40),end=Math.min(this.count,start+79),list=el('page-list');list.replaceChildren();
  for(let index=start;index<=end;index++){const button=document.createElement('button');button.className='page-card';button.type='button';button.setAttribute('aria-label',`转到第 ${index} 页`);if(index===number)button.setAttribute('aria-current','page');const preview=document.createElement('span');preview.className='page-preview';preview.setAttribute('aria-hidden','true');const num=document.createElement('b');num.textContent=String(index).padStart(2,'0');preview.append(num,document.createElement('i'),document.createElement('i'),document.createElement('i'));const label=document.createElement('span');label.textContent=`第 ${index} 页`;button.append(preview,label);button.addEventListener('click',()=>this.navigate(index));list.append(button);}
 }
 private navigate(number:number):void{if(!this.count)return;const value=Math.min(this.count,Math.max(1,Math.trunc(number)||1));el<HTMLInputElement>('page').value=String(value);el('viewport').scrollTop=0;this.render();if(matchMedia('(max-width:700px)').matches)this.sidebar(false);}
 private zoom(delta:number):void{const input=el<HTMLInputElement>('zoom');input.value=String(Math.min(400,Math.max(10,Math.round(Number(input.value))+delta)));el<HTMLSelectElement>('fit').value='manual';this.render();}
}
