import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTypeScript } from './load-ts-source.mjs';
const { placed, ordered } = await loadTypeScript(
  new URL('../../apps/desktop/src/library.ts', import.meta.url),
);

test('dragging a plugin group preserves hidden groups and action order outside its sibling set', () => {
  const old = ['action:weather', 'plugin:obs', 'plugin:hidden', 'plugin:clock', 'action:clock'];
  const siblings = ['plugin:clock', 'plugin:obs', 'plugin:hidden', 'plugin:new'];
  const result = placed(old, siblings, 'plugin:clock', 'plugin:obs');
  assert.deepEqual(
    ordered(siblings, (x) => x, result),
    ['plugin:clock', 'plugin:obs', 'plugin:hidden', 'plugin:new'],
  );
  assert.deepEqual(
    result.filter((id) => id.startsWith('action:')),
    ['action:weather', 'action:clock'],
  );
  assert.deepEqual(old, [
    'action:weather',
    'plugin:obs',
    'plugin:hidden',
    'plugin:clock',
    'action:clock',
  ]);
});

test('inserting after a sibling works in either direction and never duplicates an ID', () => {
  const ids = ['a', 'b', 'c', 'd'];
  const forward = placed([], ids, 'a', 'c', true);
  assert.deepEqual(
    ordered(ids, (x) => x, forward),
    ['b', 'c', 'a', 'd'],
  );
  const backward = placed(forward, ids, 'd', 'b', false);
  assert.deepEqual(
    ordered(ids, (x) => x, backward),
    ['d', 'b', 'c', 'a'],
  );
  assert.equal(new Set(backward).size, backward.length);
});

test('a cancelled/self drop or a drop into another plugin cannot reorder actions', () => {
  const old = ['plugin:a', 'action:a', 'action:b', 'plugin:b'];
  assert.equal(placed(old, ['action:a', 'action:b'], 'action:a', 'action:a'), old);
  assert.equal(placed(old, ['action:a', 'action:b'], 'action:a', 'plugin:b'), old);
  assert.equal(placed(old, ['action:a', 'action:b'], 'missing', 'action:b'), old);
});
