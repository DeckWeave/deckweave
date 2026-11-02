import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTypeScript } from './load-ts-source.mjs';
const { emptyDepthFrame, keyDepth } = await loadTypeScript(
  new URL('../../apps/desktop/src/depth-observation.ts', import.meta.url),
);
const frame = (travelUm, ageMs = 0) => ({
  enabled: true,
  state: 'observing',
  sequence: 1,
  samples: [{ controlId: 'key-0', travelUm, ageMs }],
});
test('only fresh physical samples create ring values; missing data is not a released key', () => {
  assert.equal(keyDepth(emptyDepthFrame(), 'key-0', 1000, 1000, 4000), null);
  assert.equal(keyDepth(frame(2000), 'key-1', 1000, 1000, 4000), null);
  assert.deepEqual(keyDepth(frame(0), 'key-0', 1000, 1000, 4000), { um: 0, fraction: 0 });
  assert.deepEqual(keyDepth(frame(2000), 'key-0', 1000, 1000, 4000), { um: 2000, fraction: 0.5 });
  assert.equal(keyDepth(frame(2000, 599), 'key-0', 1000, 1001, 4000), null);
  assert.equal(keyDepth(frame(2000), 'key-0', 1000, 1600, 4000), null);
  assert.equal(keyDepth({ ...frame(2000), state: 'error' }, 'key-0', 1000, 1000, 4000), null);
});
test('ring clamps nominal range while retaining measured micrometres and rejects malformed values', () => {
  assert.deepEqual(keyDepth(frame(5000), 'key-0', 1000, 1000, 4000), { um: 5000, fraction: 1 });
  for (const value of [-1, 10001, NaN, Infinity, 0.5])
    assert.equal(keyDepth(frame(value), 'key-0', 1000, 1000, 4000), null);
  for (const max of [0, -1, NaN, Infinity])
    assert.equal(keyDepth(frame(1000), 'key-0', 1000, 1000, max), null);
});
