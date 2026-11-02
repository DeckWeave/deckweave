import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTypeScript } from './load-ts-source.mjs';
const { conditionMatches, colorVisible, colorHex, colorChanges, colorForeground, patchMatches, hexHsv, hsvHex } = await loadTypeScript(new URL('../../apps/desktop/src/lighting-ui.ts', import.meta.url));
const rgb = {id:'primary',r:'r',g:'g',b:'b',visibleWhen:{settingId:'effect',values:[1,3]}};
const settings = ['r','g','b'].map(id => ({id,type:'number',visibleWhen:{settingId:'colorMode',values:[0]}}));

test('conditional color controls require both observed effect and channel capabilities', () => {
  assert.equal(conditionMatches({settingId:'effect',values:[1]}, {}), false);
  assert.equal(colorVisible(rgb,settings,{effect:3,colorMode:0}), true);
  assert.equal(colorVisible(rgb,settings,{effect:6,colorMode:0}), false);
  assert.equal(colorVisible(rgb,settings,{effect:3,colorMode:1}), false);
  assert.equal(colorVisible(rgb,settings,{effect:3}), false);
});

test('readback confirms only the requested subset, including real false and zero', () => {
  const patch = { colors:{a:{r:0,g:25,b:42}}, enabled:false };
  assert.equal(patchMatches(patch,{colors:{a:{r:0,g:25,b:42},b:{r:10,g:20,b:30}},enabled:false}),true);
  assert.equal(patchMatches(patch,{colors:{a:{r:0,g:25,b:43}},enabled:false}),false);
  assert.equal(patchMatches({brightness:0},{}),false);
});

test('HSV picker roundtrips colors, including white, black and hue wraparound', () => {
  for (const hex of ['#ffffff','#000000','#ff0000','#00ff00','#0000ff','#35a8f0','#ed72c4','#010203']) {
    const {h,s,v}=hexHsv(hex);
    assert.equal(hsvHex(h,s,v),hex);
  }
  assert.equal(hsvHex(360,100,100),'#ff0000');
  assert.equal(hsvHex(-60,100,100),'#ff00ff');
  assert.equal(hsvHex(60,150,150),'#ffff00');
});
test('color editing sends one atomic RGB patch and never supplies an unknown channel', () => {
  assert.deepEqual(colorChanges(rgb,'#35A8f0'), {r:53,g:168,b:240});
  assert.equal(colorHex(rgb,{r:53,g:168,b:240}), '#35a8f0');
  for(const value of ['#fff','#12345678','rgba(0,0,0)','invalid']) assert.equal(colorChanges(rgb,value),null);
  for(const observed of [{r:0,g:0},{r:0,g:0,b:256},{r:0,g:0,b:.5}]) assert.equal(colorHex(rgb,observed),null);
  assert.equal(colorForeground('#ffffff'),'#11141b');
  assert.equal(colorForeground('#12152c'),'#ffffff');
});
