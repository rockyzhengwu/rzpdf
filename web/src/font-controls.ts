import {FontResourceService,type FontManifest} from './font-resources.js';
import type {WorkerPdfDocument} from './worker-client.js';
import type {FontRequirements} from './fonts.js';
import type {RenderOptions} from './interaction.js';

interface Callbacks {idle:()=>Promise<void>;clearFonts:()=>Promise<void>;invalidate:()=>void;changed:()=>Promise<unknown>;busyChanged:()=>void;}
/** Reader-owned configuration. File/engine lifecycle stays with the reader. */
export class FontControls {
  private rasterRevision=0;
  get rasterEpoch():number{return this.rasterRevision;}
  private service:FontResourceService|undefined;private serial=0;private loading=0;private applying=false;private disposed=false;private cacheWarning=false;
  private file=document.createElement('input');private root=document.createElement('input');private offline=document.createElement('input');private persistent=document.createElement('input');
  private apply=document.createElement('button');private clear=document.createElement('button');private disable=document.createElement('button');private status=document.createElement('output');private licenses=document.createElement('ul');
  constructor(element:HTMLElement,private callbacks:Callbacks){
    const details=document.createElement('details'),summary=document.createElement('summary');summary.textContent='按需字体资源';details.append(summary);
    const intro=document.createElement('p');intro.textContent='从指定地址按清单加载缺少的字体，可缓存供离线使用。';details.append(intro);
    const label=(text:string,input:HTMLInputElement)=>{const p=document.createElement('p'),l=document.createElement('label');l.append(text,input);p.append(l);details.append(p);};
    this.file.type='file';this.file.accept='.json,application/json';label('字体资源清单（JSON，最多 1 MiB）',this.file);
    this.root.type='url';this.root.placeholder='https://example.com/fonts/';this.root.size=48;label('字体资源根地址',this.root);
    this.offline.type='checkbox';label('仅使用离线缓存',this.offline);this.persistent.type='checkbox';this.persistent.checked=true;label('保存离线缓存',this.persistent);
    this.apply.textContent='应用字体资源';this.clear.textContent='清除下载缓存';this.disable.textContent='停用按需字体';
    const buttons=document.createElement('p');buttons.append(this.apply,this.clear,this.disable);details.append(buttons);
    this.status.setAttribute('role','status');this.status.setAttribute('aria-label','字体资源状态');this.status.textContent='尚未配置按需字体。';details.append(this.status,this.licenses);element.replaceChildren(details);
    for(const field of [this.file,this.root,this.offline,this.persistent])field.addEventListener('change',()=>{this.serial++;this.status.textContent='配置已更改，请应用。';});
    this.apply.addEventListener('click',()=>{void this.configure();});this.clear.addEventListener('click',()=>{void this.clearCache();});this.disable.addEventListener('click',()=>{void this.stop();});this.update();
  }
  get busy():boolean{return this.loading>0||this.applying;}
  private update():void{for(const field of [this.file,this.root,this.offline,this.persistent])field.disabled=this.applying;this.apply.disabled=this.applying;this.clear.disabled=this.disable.disabled=this.applying||!this.service;this.callbacks.busyChanged();}
  private error(error:unknown):void{this.status.textContent=error instanceof Error?error.message:String(error);}
  private async configure():Promise<void>{
    if(this.disposed)return;const serial=++this.serial;this.applying=true;this.update();let candidate:FontResourceService|undefined;
    try{
      const file=this.file.files?.[0];if(!file)throw Error('请选择字体资源清单。');if(file.size>1024*1024)throw Error('字体资源清单超过 1 MiB。');
      const root=this.root.value.trim(),offline=this.offline.checked,persistent=this.persistent.checked;
      const content=await file.text();if(this.disposed||serial!==this.serial)return;let manifest:FontManifest;try{manifest=JSON.parse(content) as FontManifest;}catch{throw Error('字体资源清单不是有效的 JSON。');}
      const configured=new FontResourceService(manifest,{root,network:!offline,persistent,onEvent:event=>{if(this.service===configured){if(event.type==='cache-error')this.cacheWarning=true;if(event.type==='registered')this.rasterRevision++;}}});candidate=configured;
      await this.callbacks.idle();if(this.disposed||serial!==this.serial)return;
      this.callbacks.invalidate();const previous=this.service;this.service=undefined;previous?.dispose();await this.callbacks.clearFonts();if(this.disposed||serial!==this.serial)return;
      this.service=candidate;candidate=undefined;this.cacheWarning=false;this.licenses.replaceChildren();
      for(const resource of manifest.resources){const row=document.createElement('li');row.textContent=`${resource.font.family} · ${resource.license.spdx}`;if(resource.license.notice)row.title=resource.license.notice;this.licenses.append(row);}
      this.status.textContent=offline?'已配置字体资源：仅使用离线缓存。':'已配置按需字体资源。';this.applying=false;this.update();await this.callbacks.changed();
    }catch(error){if(!this.disposed&&serial===this.serial)this.error(error);}
    finally{candidate?.dispose();if(serial===this.serial)this.applying=false;this.update();}
  }
  async ensure(document:WorkerPdfDocument,page:number,options:RenderOptions,current:()=>boolean):Promise<FontRequirements|undefined>{
    const service=this.service;if(!service)return;this.loading++;this.update();if(current())this.status.textContent='正在准备页面字体…';
    try{const report=await service.ensure(document,page,options);if(this.service!==service||!current())return;
      if(!report.complete){const blocked=report.fonts.some(f=>f.missingUnicode||f.needsShaping);throw Error(blocked?'页面字体映射或文字排版不受支持。':'配置的字体仍无法覆盖页面所需字符。');}
      this.status.textContent=this.cacheWarning?'页面字体已就绪；部分离线缓存不可用。':'页面字体已就绪。';return report;
    }catch(error){if(this.service===service&&current())this.error(error);throw error;}
    finally{this.loading--;this.update();}
  }
  cancel():void{if(this.disposed)return;if(this.applying){this.serial++;this.applying=false;this.status.textContent='字体配置操作已取消。';this.update();}this.service?.cancel();}
  private async clearCache():Promise<void>{const service=this.service;if(!service||this.disposed)return;const serial=++this.serial;this.applying=true;this.update();
    try{await this.callbacks.idle();if(this.disposed||serial!==this.serial||this.service!==service)return;this.callbacks.invalidate();await service.clearCache();if(this.disposed||serial!==this.serial)return;this.cacheWarning=false;this.status.textContent='已清除下载缓存。';this.applying=false;this.update();await this.callbacks.changed();}
    catch(error){if(!this.disposed&&serial===this.serial)this.error(error);}finally{if(serial===this.serial)this.applying=false;this.update();}}
  private async stop():Promise<void>{if(this.disposed)return;const serial=++this.serial;this.applying=true;this.update();try{await this.callbacks.idle();if(this.disposed||serial!==this.serial)return;this.callbacks.invalidate();const service=this.service;this.service=undefined;service?.dispose();await this.callbacks.clearFonts();this.licenses.replaceChildren();this.status.textContent='按需字体已停用。';this.applying=false;this.update();await this.callbacks.changed();}catch(error){if(!this.disposed&&serial===this.serial)this.error(error);}finally{if(serial===this.serial)this.applying=false;this.update();}}
  resume():void{if(!this.disposed)return;this.disposed=false;this.serial++;this.applying=false;this.licenses.replaceChildren();this.status.textContent=this.file.files?.length?'资源服务已恢复，请重新应用字体配置。':'尚未配置按需字体。';this.update();}
  dispose():void{if(this.disposed)return;this.disposed=true;this.serial++;const service=this.service;this.service=undefined;service?.dispose();}
}
