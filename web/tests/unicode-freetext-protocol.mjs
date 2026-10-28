// Request/asset contract only. Real Wasm is exercised by unicode-freetext-browser.mjs.
import assert from 'node:assert/strict';
import {validateArguments} from '../dist/protocol.js';
import {mutationAssetLimit,snapshotMutation} from '../dist/interaction.js';
const stamp={engine:'test',document:1,revision:'0'};
const annotation={kind:'FreeText',rect:[20,20,220,80],contents:'你好',fontSize:16};
const font={postscriptName:'NotoSansSC-Regular',capHeight:733,stemV:80,serif:false,omitLayoutAndAuxiliary:true};
const create={operation:'addUnicodeFreeText',stamp,page:0,annotation,font};
const update={operation:'updateUnicodeFreeText',stamp,page:0,object:[5,0],annotation};
assert.equal(mutationAssetLimit(create),32*1024*1024);
assert.equal(mutationAssetLimit(update),0);
assert.equal(mutationAssetLimit({...update,font}),32*1024*1024);
const copy=snapshotMutation(create);annotation.contents='Changed';assert.equal(copy.annotation.contents,'你好');
validateArguments('apply',[copy,new Uint8Array([1])]);
validateArguments('apply',[update,undefined]);
validateArguments('apply',[{...update,font},new Uint8Array([1])]);
for(const args of [[copy,undefined],[copy,new Uint8Array()],[copy,new Uint8Array(32*1024*1024+1)],[update,new Uint8Array([1])]])assert.throws(()=>validateArguments('apply',args));
assert.equal(mutationAssetLimit({operation:'addAnnotation',stamp,page:0,annotation}),0);
console.log('Unicode FreeText protocol: request snapshots, immutable metadata and required/optional bounded asset rules passed');
