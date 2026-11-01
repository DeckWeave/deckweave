// SPDX-License-Identifier: GPL-3.0-or-later
import test from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { PassThrough, Writable } from 'node:stream';
import { settings, render, touchVolume } from './audio.mjs';
import { AudioClient } from './client.mjs';
import { AudioQueue } from './queue.mjs';

function fixture(onRequest) {
  const child = new EventEmitter();
  child.stdout = new PassThrough();
  child.stdin = new Writable({
    write(chunk, _, callback) {
      onRequest(JSON.parse(chunk.toString()), child);
      callback();
    },
  });
  child.kill = () => {
    child.killed = true;
  };
  let starts = 0;
  const client = new AudioClient({
    locate: async () => 'fixture.exe',
    spawnProcess: () => {
      starts++;
      return child;
    },
    timeout: 40,
  });
  return { client, child, starts: () => starts };
}

test('settings and touch positions are bounded; SVG labels are escaped', () => {
  assert.equal(settings({ step: 99 }).step, 10);
  assert.equal(settings({ step: -9 }).step, 1);
  assert.equal(touchVolume({ x: 1.2 }), 100);
  assert.equal(touchVolume({ x: 0.333 }), 33);
  assert.equal(touchVolume({ x: Number.NaN }), null);
  const svg = render({ volume: 45, muted: false }, settings({ label: '<script>&' }), [800, 128]);
  assert.ok(svg.includes('&lt;script&gt;&amp;'));
  assert.ok(svg.includes('45%'));
  assert.ok(render(null, settings(), [112, 112], 'failure').includes('Offline'));
});

test('one pooled helper handles concurrent read-only requests and closes cleanly', async () => {
  const { client, child, starts } = fixture((request, process) => {
    assert.equal(request.command, 'status');
    process.stdout.write(
      JSON.stringify({
        id: request.id,
        ok: true,
        state: { volume: 44, muted: true, endpoint: 'fixture' },
      }) + '\n',
    );
  });
  const results = await Promise.all([client.request('status'), client.request('status')]);
  assert.equal(starts(), 1);
  assert.equal(results[1].volume, 44);
  assert.equal(results[0].muted, true);
  client.close();
  assert.equal(child.killed, true);
  await assert.rejects(client.request('status'), /closed/);
});

test('silent or oversized helper response is terminated instead of hanging', async () => {
  const silent = fixture(() => {});
  await assert.rejects(silent.client.request('status'), /timed out/);
  assert.equal(silent.child.killed, true);
  silent.client.close();
  const oversized = fixture((_, child) => child.stdout.write('x'.repeat(17000)));
  await assert.rejects(oversized.client.request('status'), /exceeds limit/);
  assert.equal(oversized.child.killed, true);
  oversized.client.close();
});

test('protocol error and child exit reject pending requests', async () => {
  const invalid = fixture((_, child) => child.stdout.write('invalid\n'));
  await assert.rejects(invalid.client.request('status'), /Invalid audio response/);
  invalid.client.close();
  const exited = fixture((_, child) => child.emit('exit', 1));
  await assert.rejects(exited.client.request('status'), /stopped/);
  exited.client.close();
});

test('context disappearance before flush cancels only its own queued input', async () => {
  const executed = [];
  const queue = new AudioQueue(async (...operation) => executed.push(operation), { delay: 1000 });
  const gone = { active: true },
    visible = { active: true };
  queue.enqueue(gone, 'adjust_volume', { delta: 2 });
  queue.enqueue(gone, 'set_volume', { value: 90 });
  queue.enqueue(visible, 'toggle_mute');
  gone.active = false;
  queue.cancel(gone);
  await queue.flush();
  assert.deepEqual(
    executed.map(([command]) => command),
    ['toggle_mute'],
  );
  queue.close();
});

test('mixed absolute, relative and mute inputs preserve order and saturation', async () => {
  let volume = 50;
  const executed = [],
    owner = { active: true },
    dial = { active: true };
  const queue = new AudioQueue(
    async (command, values) => {
      executed.push([command, values]);
      if (command === 'set_volume') volume = values.value;
      if (command === 'adjust_volume') volume = Math.max(0, Math.min(100, volume + values.delta));
    },
    { delay: 1000 },
  );
  queue.enqueue(owner, 'set_volume', { value: 70 });
  queue.enqueue(owner, 'set_volume', { value: 80 });
  queue.enqueue(dial, 'adjust_volume', { delta: 50 });
  queue.enqueue(dial, 'adjust_volume', { delta: -20 });
  queue.enqueue(dial, 'toggle_mute');
  await queue.flush();
  assert.equal(volume, 80);
  assert.deepEqual(
    executed.map(([command]) => command),
    ['set_volume', 'adjust_volume', 'adjust_volume', 'toggle_mute'],
  );
  assert.equal(executed[0][1].value, 80);
  queue.close();
});

test('context disappears while helper is starting: no mutation is written', async () => {
  let release;
  const located = new Promise((resolve) => {
    release = resolve;
  });
  const { client, child } = fixture(() =>
    assert.fail('Disposed input must not reach helper stdin'),
  );
  client.locate = () => located;
  const abort = new AbortController();
  const pending = client.request('toggle_mute', {}, { signal: abort.signal });
  abort.abort();
  release('fixture.exe');
  await assert.rejects(pending, { name: 'AbortError' });
  client.close();
  assert.equal(child.killed, true);
});
