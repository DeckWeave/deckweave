import test from 'node:test';
import assert from 'node:assert/strict';
import { render } from './clock.mjs';
test('digital clock uses explicit timezone and legible date on native-sized canvases', () => {
  for (const canvas of [
    [112, 112],
    [220, 128],
    [800, 128],
  ]) {
    const svg = render(
      new Date('2026-11-03T12:34:56Z'),
      { language: 'ru', timezone: 'Asia/Yekaterinburg' },
      canvas,
    );
    assert.ok(svg.includes('17:34'));
    assert.ok(svg.includes('нояб'));
    assert.ok(svg.includes('вторник'));
    assert.ok(!svg.includes('NaN'));
    assert.ok(!svg.includes('12:34'));
  }
});
test('invalid timezone has explicit unavailable image instead of crashing producer', () => {
  assert.match(render(new Date(), { timezone: 'invalid' }), /Проверьте настройки/);
});
