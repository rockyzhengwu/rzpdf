import type {ContentSnapshot,InteractionState,MutationRequest} from './interaction.js';
/** Build controls from one revision; owner serializes edits and refreshes all views. */
export function contentControls(snapshot:ContentSnapshot|null,state:InteractionState,page:number,apply:(request:MutationRequest,asset?:Uint8Array|Promise<Uint8Array>)=>Promise<void>,error:(e:unknown)=>void,selection:{text:number;image:number},unavailableReason?:string):HTMLElement {
  const root=document.createElement('section');root.setAttribute('aria-label','页面内容编辑');
  const title=(text:string)=>{const h=document.createElement('h2');h.textContent=text;root.append(h);};
  const row=()=>{const div=document.createElement('div');div.className='editor-row';root.append(div);return div;};
  const button=(label:string,disabled:boolean,action:()=>void)=>{const b=document.createElement('button');b.textContent=label;b.disabled=disabled;b.addEventListener('click',action);return b;};
  title('页面');
  const pages=row();pages.append(button('顺时针旋转页面',!state.permissions.modify,()=>{void apply({operation:'rotatePage',stamp:state.stamp,page,degrees:90});}));
  const prefix=document.createElement('input');prefix.placeholder='复制字段名称前缀';prefix.setAttribute('aria-label','复制名称前缀');prefix.value='copy_'+Date.now().toString(36);const copyOptions=document.createElement('details'),copySummary=document.createElement('summary');copySummary.textContent='复制页面选项';copyOptions.append(copySummary,prefix);pages.append(copyOptions);
  pages.append(button('复制当前页',!state.permissions.modify||!state.permissions.assemble,()=>{void apply({operation:'copyPage',stamp:state.stamp,page,insertion:page+1,namePrefix:prefix.value});}));
  pages.append(button('删除当前页',state.pageCount<=1||!state.permissions.modify||!state.permissions.assemble,()=>{void apply({operation:'deletePage',stamp:state.stamp,page});}));
  for(const [label,delta] of [['页面前移',-1],['页面后移',1]] as const)pages.append(button(label,!state.permissions.modify||!state.permissions.assemble||page+delta<0||page+delta>=state.pageCount,()=>{const order=Array.from({length:state.pageCount},(_,i)=>i);[order[page],order[page+delta]]=[order[page+delta],order[page]];void apply({operation:'reorderPages',stamp:state.stamp,order});}));
  if(!snapshot){const note=document.createElement('p');note.textContent=unavailableReason??'此文档内容编辑暂不可用。';root.append(note);return root;}
  title('文字内容');
  const groups:{start:number;end:number;text:string;editable:boolean}[]=[];
  for(const character of snapshot.characters){const last=groups.at(-1);if(last&&snapshot.characters[last.start].objectIndex===character.objectIndex){last.end=character.index+1;last.text+=character.unicode??'�';last.editable&&=character.editable;}else groups.push({start:character.index,end:character.index+1,text:character.unicode??'�',editable:character.editable});}
  const textRow=row(),select=document.createElement('select');select.setAttribute('aria-label','编辑文字段');
  for(const [index,group] of groups.entries()){const option=document.createElement('option');option.value=String(index);option.textContent=group.text.slice(0,80);select.append(option);}
  const replacement=document.createElement('textarea');replacement.setAttribute('aria-label','替换文字');replacement.maxLength=4096;
  const start=document.createElement('input'),end=document.createElement('input');start.type=end.type='number';start.min=end.min='1';start.setAttribute('aria-label','起始字符');end.setAttribute('aria-label','结束字符');
  const preserve=document.createElement('input');preserve.type='checkbox';preserve.checked=true;preserve.setAttribute('aria-label','保留后续文字位置');
  const preserveLabel=document.createElement('label');preserveLabel.append(preserve,document.createTextNode('保留后续文字位置'));
  const span=document.createElement('input');span.type='checkbox';span.setAttribute('aria-label','跨文字段范围');const spanLabel=document.createElement('label');spanLabel.append(span,document.createTextNode('跨文字段（本页字符项编号）'));
  const details=document.createElement('details'),summary=document.createElement('summary');summary.textContent='新增字形的原始字体';details.append(summary);
  const fontFile=document.createElement('input');fontFile.type='file';fontFile.accept='.ttf';fontFile.setAttribute('aria-label','写入字体源文件');
  const psName=document.createElement('input');psName.setAttribute('aria-label','写入字体 PostScript 名');psName.placeholder='字体 PostScript 名';
  const capHeight=document.createElement('input'),stemV=document.createElement('input');capHeight.type=stemV.type='number';capHeight.step=stemV.step='any';capHeight.value='733';stemV.value='80';capHeight.setAttribute('aria-label','字体大写高度');stemV.setAttribute('aria-label','字体主干宽度');
  const serif=document.createElement('input'),omit=document.createElement('input');serif.type=omit.type='checkbox';serif.setAttribute('aria-label','写入字体有衬线');omit.setAttribute('aria-label','允许省略字体布局及辅助表');
  const serifLabel=document.createElement('label'),omitLabel=document.createElement('label');serifLabel.append(serif,document.createTextNode('有衬线'));omitLabel.append(omit,document.createTextNode('允许省略布局及辅助表'));
  const fontNote=document.createElement('small');fontNote.textContent='提供有授权、与原嵌入字体匹配的静态 TrueType 源（最多 32 MiB）。替代显示字体不能直接写回。描述信息以该字体为准。';details.append(fontFile,psName,capHeight,stemV,serifLabel,omitLabel,fontNote);
  const refresh=()=>{const group=groups[Number(select.value)];replacement.value=group?.text??'';start.value=String(span.checked&&group?group.start+1:1);end.value=String(group?(span.checked?group.end:group.end-group.start):1);replacement.disabled=!group?.editable||!state.permissions.modify;psName.value=(group?snapshot.characters[group.start].fontName??'':'').replace(/^[A-Z]{6}\+/,'');};
  select.value=String(Math.min(selection.text,Math.max(0,groups.length-1)));refresh();select.addEventListener('change',()=>{selection.text=Number(select.value);refresh();});span.addEventListener('change',refresh);
  const submit=button('替换选中文字',!groups.length||!state.permissions.modify,()=>{
    const group=groups[Number(select.value)],first=Number(start.value)-1,last=Number(end.value),base=span.checked?0:group?.start??0,limit=span.checked?snapshot.characters.length:group?group.end-group.start:0;
    if(!group||!Number.isInteger(first)||!Number.isInteger(last)||first<0||last<=first||last>limit||snapshot.characters.slice(base+first,base+last).some(c=>!c.editable)){error(Error('请选择有效字符范围'));return;}
    const request:MutationRequest={operation:span.checked?'replaceTextSpan':'replaceText',stamp:snapshot.stamp,page,start:base+first,end:base+last,text:replacement.value,preserveAdvance:preserve.checked};
    const file=fontFile.files?.[0];
    if(!file){void apply(request);return;}
    if(!file.size||file.size>32*1024*1024){error(Error('字体源文件最多 32 MiB'));return;}
    request.font={postscriptName:psName.value,capHeight:Number(capHeight.value),stemV:Number(stemV.value),serif:serif.checked,omitLayoutAndAuxiliary:omit.checked};
    void apply(request,file.arrayBuffer().then(data=>new Uint8Array(data)));
  });
  textRow.append(select,spanLabel,start,end,replacement,preserveLabel,details,submit);
  title('图片内容');
  const images=row(),imageSelect=document.createElement('select');imageSelect.setAttribute('aria-label','编辑图片');
  for(const image of snapshot.images){const option=document.createElement('option');option.value=String(image.index);option.textContent=`图片 ${image.index+1}${image.nested?'（嵌套）':''}`;option.disabled=!image.editable;imageSelect.append(option);}
  imageSelect.value=String(Math.min(selection.image,Math.max(0,snapshot.images.length-1)));imageSelect.addEventListener('change',()=>{selection.image=Number(imageSelect.value);});
  const dx=document.createElement('input'),dy=document.createElement('input');dx.type=dy.type='number';dx.value=dy.value='0';dx.setAttribute('aria-label','图片水平移动');dy.setAttribute('aria-label','图片垂直移动');
  const disabled=!snapshot.images.some(i=>i.editable)||!state.permissions.modify;
  images.append(imageSelect,dx,dy,button('移动图片',disabled,()=>{const x=Number(dx.value),y=Number(dy.value);if(!Number.isFinite(x)||!Number.isFinite(y)){error(Error('请输入有效移动距离'));return;}void apply({operation:'transformImage',stamp:snapshot.stamp,page,index:Number(imageSelect.value),matrix:[1,0,0,1,x,y]});}));
  images.append(button('删除图片绘制',disabled,()=>{void apply({operation:'removeImage',stamp:snapshot.stamp,page,index:Number(imageSelect.value)});}));
  const file=document.createElement('input');file.type='file';file.accept='image/png,image/jpeg';file.setAttribute('aria-label','替换图片文件');file.disabled=disabled;
  const replace=button('替换图片',disabled,()=>{
    const selected=file.files?.[0];if(!selected||!selected.size||selected.size>8*1024*1024){error(Error('请选择最多 8 MiB 的 PNG/JPEG'));return;}
    const request:MutationRequest={operation:'replaceImage',stamp:snapshot.stamp,page,index:Number(imageSelect.value),format:'png'};
    const asset=selected.arrayBuffer().then(data=>{
      const bytes=new Uint8Array(data),png=bytes[0]===137&&bytes[1]===80&&bytes[2]===78&&bytes[3]===71,jpeg=bytes[0]===255&&bytes[1]===216;
      if(!png&&!jpeg)throw Error('请选择 PNG/JPEG 图片');request.format=png?'png':'jpeg';return bytes;
    });void apply(request,asset);
  });images.append(file,replace);
  const note=document.createElement('small');note.textContent='文字使用原字体编码，不自动重排。删除绘制内容不等于安全脱敏。图片移动距离使用页面显示单位。';root.append(note);
  for(const control of Array.from(root.querySelectorAll<HTMLInputElement|HTMLSelectElement|HTMLTextAreaElement>('input[aria-label],select[aria-label],textarea[aria-label]'))){
    if(control.closest('label'))continue;
    const label=document.createElement('label');label.className='field';const caption=document.createElement('span');caption.textContent=control.getAttribute('aria-label');control.before(label);label.append(caption,control);
  }
  return root;
}
