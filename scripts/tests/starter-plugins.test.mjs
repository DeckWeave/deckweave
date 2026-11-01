import test from 'node:test';
import assert from 'node:assert/strict';
import {
  cpuSample,
  utilization,
  uptime,
  render as monitor,
} from '../../plugins/dev.deckweave.monitor.sdPlugin/monitor.mjs';
import {
  duration,
  restore,
  valueAt,
  toggle,
  reset,
  serialize,
  format,
  render,
} from '../../plugins/dev.deckweave.timers.sdPlugin/timer.mjs';
import { Handoffs } from '../../plugins/dev.deckweave.timers.sdPlugin/handoff.mjs';
test('timer handoff bridges stale reappearance but respects changed settings and bounded lifetime', () => {
  const h = new Handoffs(),
    runtime = { status: 'running', value: 59000, startedAt: 1000 };
  h.put('a', runtime, [undefined], 60, 0);
  assert.equal(h.take('a', { seconds: 60 }, 1), runtime);
  h.put('a', runtime, [undefined], 60, 0);
  assert.equal(h.take('a', { seconds: 90 }, 1), null);
  h.put('a', runtime, [undefined], 60, 0);
  assert.equal(h.take('a', { seconds: 60 }, 10000), null);
  for (let i = 0; i < 300; i++) h.put(String(i), runtime, [undefined], 60, 0);
  assert.equal(h.entries.size, 256);
  assert.equal(h.take('0', { seconds: 60 }, 1), null);
});
test('CPU is a delta across all cores, guards resets and zero intervals', () => {
  const a = cpuSample([{ times: { idle: 100, user: 50 } }, { times: { idle: 100, user: 50 } }]);
  const b = cpuSample([{ times: { idle: 120, user: 80 } }, { times: { idle: 120, user: 80 } }]);
  assert.equal(utilization(a, b), 60);
  assert.equal(utilization(a, a), null);
  assert.equal(utilization(b, a), null);
  assert.equal(uptime(90061), '1d 1h');
});
test('countdown uses monotonic active time and persists navigation anchor', () => {
  let s = restore(null, true, 60000, 1000, 20);
  s = toggle(s, 20);
  assert.equal(valueAt(s, 1020), 59000);
  const saved = serialize(s, 2000, 1020);
  const returned = restore(saved, true, 60000, 12000, 5);
  assert.equal(valueAt(returned, 5), 49000);
  const paused = toggle(returned, 1005);
  assert.equal(valueAt(paused, 999999), 48000);
  assert.equal(valueAt(reset(paused, 0), 500), 60000);
});
test('expired hidden timer returns done; stopwatch and independent timers retain semantics', () => {
  const done = restore({ status: 'running', value: 1000, startedAt: 1000 }, true, 60000, 5000, 10);
  assert.equal(done.status, 'done');
  assert.equal(valueAt(toggle(done, 20), 20), 60000);
  const watch = restore({ status: 'running', value: 1000, startedAt: 1000 }, false, 60000, 5000, 0);
  assert.equal(valueAt(watch, 2000), 7000);
  assert.equal(valueAt(done, 2000), 0);
  assert.equal(format(999, true), '0:01');
  assert.equal(format(999, false), '0:00');
  assert.equal(format(3600000), '1:00:00');
  assert.equal(duration(Infinity), 1500000);
});
test('widgets remain finite and escape user text on key, encoder and wide panel canvases', () => {
  for (const canvas of [
    [112, 112],
    [200, 100],
    [800, 128],
  ]) {
    const svg = monitor(
      'cpu',
      { cpu: null, memory: 50, usedGiB: 4, totalGiB: 8, uptime: 42 },
      { label: '<script>&' },
      canvas,
    );
    assert.ok(svg.includes('&lt;script&gt;&amp;'));
    assert.ok(!svg.includes('NaN'));
    const timer = render(restore(null, true, 60000, 0, 0), { label: '<timer>' }, canvas, 0);
    assert.ok(timer.includes('&lt;timer&gt;'));
    assert.ok(!timer.includes('NaN'));
  }
});
