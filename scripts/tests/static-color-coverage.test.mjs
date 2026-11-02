import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTypeScript } from './load-ts-source.mjs';
const { observedStaticColors, unsupportedStaticColors } = await loadTypeScript(
  new URL('../../apps/desktop/src/static-color-coverage.ts', import.meta.url),
);
const keys = Array.from({ length: 84 }, (_, i) => `key-${i}`);
test('84 layout controls do not imply 84 writable static color addresses', () => {
  const coverage = observedStaticColors({ 'key-0': { r: 1, g: 2, b: 3 } }, keys);
  assert.deepEqual(Object.keys(coverage), ['key-0']);
  assert.equal(unsupportedStaticColors({ 'key-0': { r: 2, g: 3, b: 4 } }, coverage), false);
  assert.equal(
    unsupportedStaticColors(
      { 'key-0': { r: 2, g: 3, b: 4 }, 'key-83': { r: 1, g: 2, b: 3 } },
      coverage,
    ),
    true,
  );
});
test('absent and malformed readback never expands writable coverage', () => {
  for (const value of [undefined, null, [], {}, 'saved desired colors'])
    assert.deepEqual(observedStaticColors(value, keys), {});
  const value = Object.create({ 'key-0': { r: 1, g: 2, b: 3 } });
  value['key-1'] = { r: 256, g: 0, b: 0 };
  value['key-2'] = { r: 0, g: 0.5, b: 0 };
  value['key-3'] = { r: 0, g: 0 };
  value['outside-layout'] = { r: 1, g: 2, b: 3 };
  assert.deepEqual(observedStaticColors(value, keys), {});
});
test('known zero-valued colors remain supported and saved requests cannot add absent addresses', () => {
  const coverage = observedStaticColors({ 'key-0': { r: 0, g: 0, b: 0 } }, keys);
  assert.deepEqual(coverage, { 'key-0': { r: 0, g: 0, b: 0 } });
  const saved = { 'key-0': { r: 255, g: 255, b: 255 }, 'key-4': { r: 100, g: 100, b: 100 } };
  assert.deepEqual(observedStaticColors(saved, Object.keys(coverage)), {
    'key-0': { r: 255, g: 255, b: 255 },
  });
  assert.equal(unsupportedStaticColors(undefined, coverage), false);
});
