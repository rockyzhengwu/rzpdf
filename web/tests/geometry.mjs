import assert from 'node:assert/strict';
import {widgetPlacement,displayToUser} from '../dist/geometry.js';
const near=(a,b)=>assert.ok(Math.abs(a-b)<1e-8,`${a} != ${b}`);
for(const [rotation,matrix] of [[0,[1,0,0,-1,-10,220]],[90,[0,1,1,0,-20,-10]],[180,[-1,0,0,1,210,-20]],[270,[0,-1,-1,0,220,210]]]) {
  for(const scale of [.5,1,2,4])for(const userUnit of [1,2]) {
    const info={rotation,userToDisplay:matrix.map(n=>n*userUnit)};
    for(const flags of [0,8,16,24]) {
      const p=widgetPlacement(info,[20,30,60,50],scale,flags);
      const [x,y]=displayToUser(info,p.left/scale,p.top/scale);near(x,20);near(y,50);
      near(Math.hypot(p.matrix[0],p.matrix[1]),userUnit*((flags&8)?1:scale));
      near(Math.hypot(p.matrix[2],p.matrix[3]),userUnit*((flags&8)?1:scale));
      if(flags&16){near(p.matrix[1],0);near(p.matrix[2],0);assert.ok(p.matrix[0]>0&&p.matrix[3]>0);}
    }
  }
}
assert.throws(()=>displayToUser({userToDisplay:[0,0,0,0,0,0]},1,1));
assert.throws(()=>widgetPlacement({userToDisplay:[1,0,0,-1,0,100],rotation:0},[2,2,1,1],1));
console.log('Widget rotation/CropBox/UserUnit/zoom/NoZoom/NoRotate geometry passed');
const {viewTransform,inverseViewPoint}=await import('../dist/geometry.js');
for(const rotation of [0,90,180,270]) {
  const t=viewTransform(320,240,rotation),[a,b,c,d,e,f]=t.matrix;
  assert.deepEqual([t.width,t.height],rotation%180?[240,320]:[320,240]);
  for(const [x,y] of [[0,0],[320,240],[17.25,84.5]]) {
    const [px,py]=inverseViewPoint(320,240,rotation,a*x+c*y+e,b*x+d*y+f);near(px,x);near(py,y);
  }
  const p=widgetPlacement({rotation:0,userToDisplay:[1,0,0,-1,0,240]},[20,30,60,50],2,16,rotation);
  near(a*p.matrix[0]+c*p.matrix[1],2);near(b*p.matrix[0]+d*p.matrix[1],0);
  near(a*p.matrix[2]+c*p.matrix[3],0);near(b*p.matrix[2]+d*p.matrix[3],2);
}
assert.throws(()=>viewTransform(320,240,45));
