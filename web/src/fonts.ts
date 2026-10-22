/** Explicit font selection metadata; display registration does not authorize PDF embedding. */
export interface FontRegistration {
  family:string;aliases?:string[];weight:number;italic:boolean;serif:boolean;languages?:string[];index?:number;
}
export interface FontIdentity {sha256:string;index:number;}
export function normalizeFontRegistration(value:FontRegistration):Required<FontRegistration> {
  if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(k=>!['family','aliases','weight','italic','serif','languages','index'].includes(k)))throw new RangeError('Invalid font metadata');
  const text=(s:unknown,max:number)=>typeof s==='string'&&s.length>0&&s.length<=max&&new TextEncoder().encode(s).length<=max&&!/[\u0000-\u001f\u007f-\u009f]/u.test(s);
  const list=(v:unknown,max:number):string[]=>{if(v===undefined)return [];if(!Array.isArray(v)||v.length>16||v.some(s=>!text(s,max)))throw new RangeError('Invalid font list');return v.slice() as string[];};
  const index=value.index??0;
  if(!text(value.family,256)||!Number.isInteger(value.weight)||value.weight<1||value.weight>1000||typeof value.italic!=='boolean'||typeof value.serif!=='boolean'||!Number.isInteger(index)||index<0||index>0xffffffff)throw new RangeError('Invalid font metadata');
  return {family:value.family,aliases:list(value.aliases,256),weight:value.weight,italic:value.italic,serif:value.serif,languages:list(value.languages,64),index};
}

export interface FontStamp {engine:string;document:number;revision:string;fontEpoch?:string;}
export function normalizeFontStamp(stamp:FontStamp):FontStamp {
  if(!stamp||typeof stamp!=='object'||typeof stamp.engine!=='string'||!stamp.engine.length||stamp.engine.length>128||!Number.isInteger(stamp.document)||stamp.document<1||stamp.document>0xffffffff||typeof stamp.revision!=='string'||!/^\d{1,20}$/.test(stamp.revision))throw new RangeError('Invalid font source stamp');
  if(stamp.fontEpoch!==undefined&&(typeof stamp.fontEpoch!=='string'||!/^\d{1,20}$/.test(stamp.fontEpoch)))throw new RangeError('Invalid font epoch');
  return {engine:stamp.engine,document:stamp.document,revision:stamp.revision,...(stamp.fontEpoch===undefined?{}:{fontEpoch:stamp.fontEpoch})};
}
export interface FontRequirement {
  fontId:string;name:string;language?:string|null;weight:number;italic:boolean;serif:boolean;
  codepoints?:number[];missingCount:number;missingCodepoints:number[];missingUnicode:number;needsShaping:number;
  selected:({sha256:string;index:number;count:number;languageRank?:number;preference?:number[]}|{legacy:true;count:number;languageRank?:number;preference?:number[]})[];
}
export interface FontRequirements {
  stamp:FontStamp;page:number;renderOptions:{annotations:boolean;includeWidgets:boolean;viewZoom:number;viewRotation:number};
  codepointsRedacted:boolean;complete:boolean;fonts:FontRequirement[];
}

export function validFontLanguage(value:string):boolean {
  return !!value&&value.length<=64&&value.split('-').every((p,i)=>p.length>0&&p.length<=8&&(i===0?/^[A-Za-z]+$/:/^[A-Za-z0-9]+$/).test(p));
}
/** Keep locale ranking aligned with the native resolver. */
export function fontLanguageRank(request:string|null|undefined,candidate:string):number {
  if(!request||!validFontLanguage(request))return 0;if(!validFontLanguage(candidate))return 3;
  const a=request.toLowerCase(),b=candidate.toLowerCase();if(a===b)return 0;const aa=a.split('-'),bb=b.split('-');if(aa[0]!==bb[0])return 5;
  const script=(parts:string[])=>parts.slice(1).find(p=>/^[a-z]{4}$/.test(p));const x=script(aa),y=script(bb);if(x&&y&&x!==y)return 4;
  return a.startsWith(b+'-')||b.startsWith(a+'-')?1:2;
}
