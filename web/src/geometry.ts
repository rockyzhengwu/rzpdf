import type {Matrix,PageInfo} from './client.js';
export type Rect=[number,number,number,number];
/** Map a top-left CSS control's local axes to PDF user space and then display. */
export function widgetPlacement(info:PageInfo,rect:Rect,scale:number,flags=0,viewRotation=0):{left:number;top:number;width:number;height:number;matrix:Matrix} {
  const [x0,y0,x1,y1]=rect,[a,b,c,d,e,f]=info.userToDisplay;
  if(![...rect,...info.userToDisplay,scale].every(Number.isFinite)||scale<=0||x1<=x0||y1<=y0)throw new RangeError('Invalid widget geometry');
  const angle=(flags&16)?-(info.rotation+viewRotation)*Math.PI/180:0,cos=Math.cos(angle),sin=Math.sin(angle),factor=(flags&8)?1:scale;
  return {left:(a*x0+c*y1+e)*scale,top:(b*x0+d*y1+f)*scale,width:x1-x0,height:y1-y0,
    matrix:[(cos*a-sin*b)*factor,(sin*a+cos*b)*factor,(-cos*c+sin*d)*factor,(-sin*c-cos*d)*factor,0,0]};
}
export function displayToUser(info:PageInfo,x:number,y:number):[number,number] {
  const [a,b,c,d,e,f]=info.userToDisplay,det=a*d-b*c;
  if(![a,b,c,d,e,f,x,y,det].every(Number.isFinite)||Math.abs(det)<1e-12)throw new RangeError('Invalid page transform');
  return [(d*(x-e)-c*(y-f))/det,(-b*(x-e)+a*(y-f))/det];
}

/** Extra clockwise view rotation in CSS pixels, after PDF display geometry. */
export function viewTransform(width:number,height:number,rotation:number):{width:number;height:number;matrix:Matrix} {
  if(![width,height].every(n=>Number.isFinite(n)&&n>0))throw new RangeError('Invalid view dimensions');
  switch(rotation){
    case 0:return {width,height,matrix:[1,0,0,1,0,0]};
    case 90:return {width:height,height:width,matrix:[0,1,-1,0,height,0]};
    case 180:return {width,height,matrix:[-1,0,0,-1,width,height]};
    case 270:return {width:height,height:width,matrix:[0,-1,1,0,0,width]};
    default:throw new RangeError('View rotation must be a quarter turn');
  }
}
export function inverseViewPoint(width:number,height:number,rotation:number,x:number,y:number):[number,number] {
  const [a,b,c,d,e,f]=viewTransform(width,height,rotation).matrix;
  return [a*(x-e)+b*(y-f),c*(x-e)+d*(y-f)];
}
