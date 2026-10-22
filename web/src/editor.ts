import {contentControls} from './content-editor.js';
import {PdfApiError} from './client.js';
import {widgetPlacement,displayToUser,inverseViewPoint} from './geometry.js';
import type {PageInfo,OpenOptions} from './client.js';
import type {WorkerPdfDocument} from './worker-client.js';
import type {AnnotationSnapshot,FormSnapshot,InteractionState,MutationRequest,FormValue,AnnotationInput} from './interaction.js';
/** A panel owns one document revision. Refresh after every edit, including rollback. */
export class EditorPanel {
  private serial=0;
  private contentSelection={key:'',text:0,image:0};
  constructor(private root:HTMLElement,private overlay:HTMLElement,private changed:(doc:WorkerPdfDocument)=>Promise<unknown>,private error:(e:unknown)=>void,private openSource:(bytes:Uint8Array,options:OpenOptions)=>Promise<WorkerPdfDocument>){}
  clear():void {this.serial++;this.root.replaceChildren();this.overlay.replaceChildren();this.overlay.style.pointerEvents='none';this.overlay.style.touchAction='auto';this.overlay.dataset.drawing='false';this.overlay.onpointerdown=this.overlay.onpointermove=this.overlay.onpointerup=this.overlay.onpointercancel=null;}
  async show(doc:WorkerPdfDocument,page:number,info:PageInfo,scale:number,viewRotation=0):Promise<void> {
    const serial=++this.serial;
    let contentError:string|undefined;
    const [state,fields,annotations,content]=await Promise.all([doc.state(),doc.formFields(),doc.annotations(page),doc.content(page).catch(error=>{if(error instanceof PdfApiError&&['permission','resource_limit','unsupported','font','format'].includes(error.code)){contentError=error.message;return null;}throw error;})]);
    if(serial!==this.serial)return;
    this.root.replaceChildren();this.overlay.replaceChildren();
    const title=(text:string)=>{const h=document.createElement('h2');h.textContent=text;this.root.append(h);};
    let editing=false;
    const apply=async(command:MutationRequest,asset?:Uint8Array|Promise<Uint8Array>)=>{
      if(serial!==this.serial||editing)return;editing=true;
      for(const container of [this.root,this.overlay])container.querySelectorAll<HTMLButtonElement|HTMLInputElement|HTMLSelectElement|HTMLTextAreaElement>('button,input,select,textarea').forEach(e=>e.disabled=true);
      let failure:unknown,attempted=false;
      try {const data=asset===undefined||asset instanceof Uint8Array?asset:await asset;if(serial!==this.serial)return;attempted=true;await doc.apply(command,{asset:data});}catch(e){failure=e;}
      finally {const showFailure=serial===this.serial;if(attempted||showFailure){const refreshed=await this.changed(doc);if(failure&&showFailure&&refreshed===true)this.error(failure);}}
    };
    const key=`${state.stamp.engine}:${state.stamp.document}:${page}`;
    if(this.contentSelection.key!==key)this.contentSelection={key,text:0,image:0};
    this.root.append(contentControls(content,state,page,apply,error=>{if(serial===this.serial)this.error(error);},this.contentSelection,contentError));
    this.importControls(doc,state,serial,page,apply);
    title('表单');
    for(const field of fields.records) {
      if(!field.widgets.some(w=>w.page===page))continue;
      const row=document.createElement('label');row.className='editor-row';row.append(document.createTextNode(field.name));
      const detail=field.details;
      if(!detail||detail.type.kind==='pushButton') {
        const note=document.createElement('small');note.textContent=field.unsupportedReason??'按钮动作不执行';row.append(note);this.root.append(row);continue;
      }
      let control:HTMLInputElement|HTMLTextAreaElement|HTMLSelectElement;
      let value:()=>FormValue;
      if(detail.type.kind==='text') {
        const text=detail.type.multiline?document.createElement('textarea'):document.createElement('input');
        text.value=detail.value.kind==='text'?detail.value.text:'';control=text;value=()=>({kind:'text',text:text.value});
      }else if(detail.type.kind==='choice'&&detail.type.editable) {
        const text=document.createElement('input'),list=document.createElement('datalist');list.id='choices-'+field.object.join('-');text.setAttribute('list',list.id);
        for(const item of detail.options){const option=document.createElement('option');option.value=item.export;option.label=item.display;list.append(option);}
        text.value=detail.value.kind==='choice'?detail.value.values[0]??'':detail.value.kind==='choiceIndices'?detail.options[detail.value.indices[0]]?.export??'':'';
        row.append(list);control=text;value=()=>({kind:'choice',values:[text.value]});
      }else if(detail.type.kind==='checkbox') {
        const checkbox=document.createElement('input');checkbox.type='checkbox';checkbox.checked=detail.value.kind==='checkbox'&&detail.value.checked;control=checkbox;value=()=>({kind:'checkbox',checked:checkbox.checked});
      }else {
        const select=document.createElement('select');control=select;
        if(detail.type.kind==='radio') {
          const off=document.createElement('option');off.value='';off.textContent='未选择';off.disabled=!!detail.type.noToggleToOff;select.append(off);
          for(const state of new Set(detail.buttonStates)){const option=document.createElement('option');option.value=state;option.textContent=state;select.append(option);}
          select.value=detail.value.kind==='radio'||detail.value.kind==='buttonState'?detail.value.state??'':'';
          value=()=>({kind:'radio',state:select.value||null});
        }else {
          select.multiple=!!detail.type.multiple;
          for(const [index,item] of detail.options.entries()) {
            const option=document.createElement('option');option.value=String(index);option.textContent=item.display;
            option.selected=detail.value.kind==='choice'?detail.value.values.includes(item.export):detail.value.kind==='choiceIndices'&&detail.value.indices.includes(index);select.append(option);
          }
          value=()=>({kind:'choiceIndices',indices:Array.from(select.selectedOptions,o=>Number(o.value))});
        }
      }
      control.setAttribute('aria-label',field.name);control.disabled=!state.permissions.fillForms||!!(field.flags&1);
      control.addEventListener('change',()=>{void apply({operation:'setFormValue',stamp:fields.stamp,object:field.object,value:value()});});
      row.append(control);this.root.append(row);
      for(const widget of field.widgets) {
        if(widget.page!==page||!widget.visible||widget.rect[2]<=widget.rect[0]||widget.rect[3]<=widget.rect[1])continue;
        let input:HTMLInputElement|HTMLTextAreaElement|HTMLSelectElement;
        if(detail.type.kind==='radio') {
          if(!widget.onState)continue;
          const radio=document.createElement('input');radio.type='radio';radio.name='widget-'+(detail.type.inUnison?widget.object:field.object).join('-');
          radio.checked=(detail.value.kind==='radio'||detail.value.kind==='buttonState')&&detail.value.state===widget.onState;
          radio.addEventListener('change',()=>{if(radio.checked){control.value=widget.onState!;control.dispatchEvent(new Event('change'));}});input=radio;
        }else {
          input=control.cloneNode(true) as typeof control;input.value=control.value;
          if(input instanceof HTMLSelectElement&&control instanceof HTMLSelectElement)Array.from(input.options).forEach((option,i)=>option.selected=control.options[i].selected);
          if(input instanceof HTMLInputElement&&control instanceof HTMLInputElement)input.checked=control.checked;
          input.addEventListener('change',()=>{
            control.value=input.value;
            if(input instanceof HTMLSelectElement&&control instanceof HTMLSelectElement){const source=input;Array.from(control.options).forEach((option,i)=>option.selected=source.options[i].selected);}
            if(input instanceof HTMLInputElement&&control instanceof HTMLInputElement)control.checked=input.checked;
            control.dispatchEvent(new Event('change'));
          });
        }
        input.disabled=control.disabled||!!(widget.flags&64);input.setAttribute('aria-label',`页面表单 ${field.name}${detail.type.kind==='radio'?' '+widget.onState:''}`);
        input.dataset.widget=widget.object.join(':');const placement=widgetPlacement(info,widget.rect,scale,widget.flags,viewRotation);
        input.style.cssText=`left:${placement.left}px;top:${placement.top}px;width:${placement.width}px;height:${placement.height}px;transform:matrix(${placement.matrix.join(',')})`;
        this.overlay.append(input);
      }
    }
    title('批注');this.tools(annotations,state,page,info,scale,viewRotation,apply);this.annotations(annotations,state,page,apply);
    const save=document.createElement('button');save.id='save-pdf';save.textContent='保存并下载 PDF';save.disabled=!state.permissions.save;
    save.addEventListener('click',async()=>{
      if(serial!==this.serial)return;save.disabled=true;
      try {
        const result=await doc.save();if(serial!==this.serial)return;
        const url=URL.createObjectURL(new Blob([result.bytes],{type:'application/pdf'}));
        const link=document.createElement('a');link.href=url;link.download='edited.pdf';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
      }catch(e){if(serial===this.serial)this.error(e);}finally {if(serial===this.serial)save.disabled=!state.permissions.save;}
    });this.root.append(save);
  }
  private importControls(doc:WorkerPdfDocument,state:InteractionState,serial:number,page:number,apply:(request:MutationRequest)=>Promise<void>):void {
    const row=document.createElement('section');row.className='editor-row';row.setAttribute('aria-label','导入页面');
    const heading=document.createElement('h2');heading.textContent='导入页面';row.append(heading);
    const file=document.createElement('input');file.type='file';file.accept='application/pdf';file.setAttribute('aria-label','导入 PDF 文件');
    const password=document.createElement('input');password.type='password';password.autocomplete='off';password.placeholder='来源密码';password.setAttribute('aria-label','导入文件密码');
    const pages=document.createElement('input');pages.placeholder='页码，如 1,2；留空导入全部';pages.setAttribute('aria-label','导入页码');
    const insertion=document.createElement('input');insertion.type='number';insertion.min='1';insertion.max=String(state.pageCount+1);insertion.value=String(page+2);insertion.setAttribute('aria-label','导入到第几页之前');
    const prefix=document.createElement('input');prefix.placeholder='名称前缀';prefix.value='import_'+Date.now().toString(36);prefix.setAttribute('aria-label','导入名称前缀');
    const navigation=document.createElement('input');navigation.type='checkbox';navigation.setAttribute('aria-label','导入来源书签');const label=document.createElement('label');label.append(navigation,document.createTextNode('同时导入书签（需包含全部目标页）'));
    const button=document.createElement('button');button.textContent='导入选定页面';button.disabled=!state.permissions.modify||!state.permissions.assemble;
    button.addEventListener('click',async()=>{
      const selected=file.files?.[0],pass=password.value;password.value='';
      if(!selected||!selected.size||selected.size>8*1024*1024){this.error(Error('请选择最多 8 MiB 的 PDF'));return;}
      const target=Number(insertion.value)-1;if(!Number.isInteger(target)||target<0||target>state.pageCount){this.error(Error('请输入有效插入位置'));return;}
      const selectedPages=pages.value.trim(),namePrefix=prefix.value,includeNavigation=navigation.checked;
      button.disabled=true;let source:WorkerPdfDocument|undefined;
      try {
        const bytes=new Uint8Array(await selected.arrayBuffer());if(serial!==this.serial)return;
        source=await this.openSource(bytes,pass?{password:pass}:{});if(serial!==this.serial)return;
        const indices=selectedPages?selectedPages.split(',').map(s=>Number(s.trim())-1):Array.from({length:source.pageCount},(_,i)=>i);
        if(!indices.length||indices.length>10000||indices.some(i=>!Number.isInteger(i)||i<0||i>=source!.pageCount)||new Set(indices).size!==indices.length)throw Error('请输入不重复的有效来源页码（最多 10000 页）');
        const snapshot=await source.state();if(serial!==this.serial)return;
        await apply({operation:'importPages',stamp:state.stamp,source:snapshot.stamp,pages:indices,insertion:target,namePrefix,includeNavigation});
      }catch(error){if(serial===this.serial)this.error(error);}
      finally {try{await source?.close();}catch(error){if(serial===this.serial)this.error(error);}if(serial===this.serial)button.disabled=!state.permissions.modify||!state.permissions.assemble;}
    });
    row.append(file,password,pages,insertion,prefix,label,button);this.root.append(row);
  }
  private tools(snapshot:AnnotationSnapshot,state:InteractionState,page:number,info:PageInfo,scale:number,viewRotation:number,apply:(command:MutationRequest)=>Promise<void>):void {
    const row=document.createElement('div');row.className='editor-row';
    const tool=document.createElement('select');tool.setAttribute('aria-label','页面批注工具');tool.disabled=!state.permissions.annotate;
    for(const name of ['选择/填写','Text','Square','Circle','FreeText','Highlight','Line','Ink']){const option=document.createElement('option');option.textContent=name;option.value=name==='选择/填写'?'':name;tool.append(option);}
    const content=document.createElement('textarea');content.setAttribute('aria-label','绘制批注内容');content.placeholder='批注内容';row.append(tool,content);this.root.append(row);
    let pointer:number|undefined,points:[number,number][]=[],preview:SVGSVGElement|undefined;
    const finish=()=>{pointer=undefined;points=[];preview?.remove();preview=undefined;};
    tool.addEventListener('change',()=>{finish();this.overlay.dataset.drawing=String(!!tool.value);this.overlay.style.pointerEvents=tool.value?'auto':'none';this.overlay.style.touchAction=tool.value?'none':'auto';});
    const position=(event:PointerEvent):[number,number]=>{const box=this.overlay.getBoundingClientRect();const [x,y]=inverseViewPoint(info.width*scale,info.height*scale,viewRotation,event.clientX-box.left,event.clientY-box.top);return [Math.max(0,Math.min(info.width,x/scale)),Math.max(0,Math.min(info.height,y/scale))];};
    const paint=()=>{
      if(!preview)return;
      const path=document.createElementNS('http://www.w3.org/2000/svg','polyline');
      const first=points[0],last=points[points.length-1];
      const outline=tool.value==='Ink'?points:tool.value==='Line'?[first,last]:[first,[last[0],first[1]],last,[first[0],last[1]],first];
      path.setAttribute('points',outline.map(p=>`${p[0]*scale},${p[1]*scale}`).join(' '));path.setAttribute('fill','none');path.setAttribute('stroke','#387fe8');path.setAttribute('stroke-width','2');preview.replaceChildren(path);
    };
    this.overlay.onpointerdown=event=>{
      if(!tool.value||tool.disabled||event.button!==0||pointer!==undefined)return;
      event.preventDefault();pointer=event.pointerId;points=[position(event)];this.overlay.setPointerCapture(pointer);
      preview=document.createElementNS('http://www.w3.org/2000/svg','svg');preview.style.cssText='position:absolute;inset:0;width:100%;height:100%;pointer-events:none';this.overlay.append(preview);paint();
    };
    this.overlay.onpointermove=event=>{
      if(pointer!==event.pointerId)return;const next=position(event);
      if(tool.value==='Ink'){const last=points[points.length-1];if(Math.hypot(next[0]-last[0],next[1]-last[1])*scale<1)return;if(points.length>=4096){finish();this.error(Error('笔迹超过 4096 点'));return;}points.push(next);}
      else points=[points[0],next];paint();
    };
    this.overlay.onpointercancel=()=>finish();
    this.overlay.onpointerup=event=>{
      if(pointer!==event.pointerId)return;event.preventDefault();
      const kind=tool.value as AnnotationInput['kind'],sample=kind==='Ink'?[...points,position(event)]:[points[0],position(event)];
      const user=sample.map(([x,y])=>displayToUser(info,x,y)),xs=user.map(p=>p[0]),ys=user.map(p=>p[1]);
      let rect:[number,number,number,number]=[Math.min(...xs),Math.min(...ys),Math.max(...xs),Math.max(...ys)];
      if(kind==='Text'&&Math.hypot(sample.at(-1)![0]-sample[0][0],sample.at(-1)![1]-sample[0][1])*scale<3)rect=[rect[0],rect[1]-24,rect[0]+24,rect[1]];
      if(rect[2]-rect[0]<.1)rect[2]=rect[0]+.1;if(rect[3]-rect[1]<.1)rect[3]=rect[1]+.1;
      const annotation:AnnotationInput={kind,rect,contents:content.value};
      if(kind==='Highlight'){annotation.quads=[[rect[0],rect[3],rect[2],rect[3],rect[0],rect[1],rect[2],rect[1]]];annotation.color=[1,1,0];}
      if(kind==='Line')annotation.points=[...user[0],...user[user.length-1]];
      if(kind==='Ink')annotation.strokes=[user];
      finish();tool.disabled=true;this.overlay.style.pointerEvents='none';void apply({operation:'addAnnotation',stamp:snapshot.stamp,page,annotation});
    };
  }
  private annotations(snapshot:AnnotationSnapshot,state:InteractionState,page:number,apply:(command:MutationRequest)=>Promise<void>):void {
    for(const record of snapshot.records) {
      if(record.subtype==='Widget'||record.subtype==='Link')continue;
      const row=document.createElement('div');row.className='editor-row';
      const label=document.createElement('span');label.textContent=record.subtype;row.append(label);
      if(record.annotation&&record.object) {
        const annotation=record.annotation,object=record.object;
        const text=document.createElement('textarea');text.value=annotation.contents??'';text.setAttribute('aria-label','批注内容');
        const locked=!!((annotation.flags??0)&(64|128|512));text.disabled=!state.permissions.annotate||locked;
        text.addEventListener('change',()=>{void apply({operation:'updateAnnotation',stamp:snapshot.stamp,page,object,annotation:{...annotation,contents:text.value}});});
        const remove=document.createElement('button');remove.textContent='删除批注';remove.disabled=text.disabled;
        remove.addEventListener('click',()=>{void apply({operation:'removeAnnotation',stamp:snapshot.stamp,page,object});});row.append(text,remove);
      }else {const note=document.createElement('small');note.textContent=record.unsupportedReason??'只读批注';row.append(note);}
      this.root.append(row);
    }
    const form=document.createElement('form');form.className='editor-row';
    const kind=document.createElement('select');kind.setAttribute('aria-label','新增批注类型');
    for(const name of ['Text','Square','Circle','FreeText'] as const){const option=document.createElement('option');option.value=name;option.textContent=name;kind.append(option);}
    const rect=document.createElement('input');rect.value='20,20,120,60';rect.setAttribute('aria-label','批注矩形 x0,y0,x1,y1');
    const content=document.createElement('textarea');content.placeholder='批注内容';content.setAttribute('aria-label','新增批注内容');
    const add=document.createElement('button');add.textContent='新增批注';add.disabled=!state.permissions.annotate;
    form.append(kind,rect,content,add);form.addEventListener('submit',event=>{
      event.preventDefault();const numbers=rect.value.split(',').map(Number);
      if(numbers.length!==4||!numbers.every(Number.isFinite)||numbers[2]<=numbers[0]||numbers[3]<=numbers[1]){this.error(Error('请输入有效 PDF 坐标矩形'));return;}
      void apply({operation:'addAnnotation',stamp:snapshot.stamp,page,annotation:{kind:kind.value as 'Text'|'Square'|'Circle'|'FreeText',rect:numbers as [number,number,number,number],contents:content.value}});
    });this.root.append(form);
  }
}
