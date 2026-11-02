import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTypeScript } from './load-ts-source.mjs';
const {ProviderEditQueue,mergeSettings}=await loadTypeScript(new URL('../../apps/desktop/src/provider-edit-queue.ts',import.meta.url));
const tick=()=>new Promise(resolve=>setImmediate(resolve));
const gate=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return{promise,resolve,reject};};

test('fast completed color gestures keep the newest successor while the first verifies',async()=>{
 const writes=[],states=[], first=gate(),second=gate();
 const queue=new ProviderEditQueue(async patch=>{writes.push(patch);await(writes.length===1?first:second).promise;},s=>states.push(s));
 queue.enqueue({r:255,g:0,b:0});
 queue.enqueue({r:0,g:255,b:0});
 queue.enqueue({r:0,g:0,b:255});
 assert.deepEqual(writes,[{r:255,g:0,b:0}]);
 assert.deepEqual(states.at(-1).desired,{r:0,g:0,b:255});
 first.resolve();await tick();
 assert.deepEqual(writes,[{r:255,g:0,b:0},{r:0,g:0,b:255}]);
 second.resolve();await tick();
 assert.equal(states.at(-1).pending,false);
 assert.deepEqual(states.at(-1).desired,{});
});
test('uncertain write discards queued changes and cannot replay after refresh',async()=>{
 const first=gate(),writes=[],states=[];
 const queue=new ProviderEditQueue(async p=>{writes.push(p);await first.promise;},s=>states.push(s));
 queue.enqueue({r:1,g:2,b:3});queue.enqueue({r:4,g:5,b:6});
 first.reject(new Error('readback mismatch'));await tick();
 assert.equal(writes.length,1);
 assert.match(states.at(-1).error,/readback mismatch/);
 assert.equal(queue.enqueue({r:7,g:8,b:9}),false);
});
test('closing an editor cancels its successor and suppresses stale completion',async()=>{
 const first=gate(),writes=[],states=[];
 const queue=new ProviderEditQueue(async(p,cancelled)=>{writes.push(p);await first.promise;assert.equal(cancelled(),true);},s=>states.push(s));
 queue.enqueue({brightness:1});queue.enqueue({brightness:5});
 queue.dispose();const count=states.length;first.resolve();await tick();
 assert.equal(writes.length,1);assert.equal(states.length,count);
});
test('per-key edits coalesce without losing another selected key or zero-valued channels',()=>{
 assert.deepEqual(mergeSettings({colors:{a:{r:1,g:2,b:3}}},{colors:{b:{r:0,g:0,b:0},a:{r:0,g:8,b:9}}}),{colors:{a:{r:0,g:8,b:9},b:{r:0,g:0,b:0}}});
});
