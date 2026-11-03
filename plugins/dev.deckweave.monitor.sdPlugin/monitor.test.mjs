// SPDX-License-Identifier: GPL-3.0-or-later
import test from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { PassThrough, Writable } from 'node:stream';
import { GpuClient, validSample } from './client.mjs';
import { SharedSampler } from './sampler.mjs';
import { metricView, render } from './monitor.mjs';

const sample = (now = 1000) => ({
  status: 'ready',
  sampled_at_ms: now,
  selected_adapter_id: '00000000:00000001',
  reason: null,
  adapters: [
    {
      id: '00000000:00000001',
      index: 0,
      name: 'Fixture GPU',
      dedicated_total_bytes: 8 * 2 ** 30,
      dedicated_used_bytes: 2 * 2 ** 30,
      busiest_engine_percent: 0,
      gpu_status: 'ready',
      memory_status: 'ready',
      gpu_reason: null,
      memory_reason: null,
    },
  ],
});
const flush = () => new Promise((resolve) => setImmediate(resolve));
function fixture(onRequest, timeout = 40) {
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
  const client = new GpuClient({
    locate: async () => 'fixture.exe',
    timeout,
    spawnProcess: (_, args, options) => {
      assert.deepEqual(args, []);
      assert.equal(options.shell, false);
      assert.equal(options.windowsHide, true);
      starts++;
      return child;
    },
  });
  return { client, child, starts: () => starts };
}

test('GPU protocol validates adapter values and keeps valid zero', () => {
  assert.equal(validSample(sample()), true);
  const invalid = sample();
  invalid.adapters[0].busiest_engine_percent = NaN;
  assert.equal(validSample(invalid), false);
  const absent = sample();
  absent.adapters[0].dedicated_used_bytes = null;
  assert.equal(validSample(absent), false);
  assert.equal(validSample({ ...sample(), adapters: Array(17).fill(sample().adapters[0]) }), false);
});

test('concurrent consumers coalesce into one persistent request; close kills helper', async () => {
  let requests = 0;
  const f = fixture((request, child) => {
    requests++;
    assert.equal(request.command, 'gpu_sample');
    setImmediate(() =>
      child.stdout.write(JSON.stringify({ id: request.id, ok: true, state: sample() }) + '\n'),
    );
  });
  const [a, b] = await Promise.all([f.client.sample(), f.client.sample()]);
  assert.equal(requests, 1);
  assert.equal(f.starts(), 1);
  assert.equal(a, b);
  await f.client.sample();
  assert.equal(requests, 2);
  assert.equal(f.starts(), 1);
  f.client.close();
  assert.equal(f.child.killed, true);
  await assert.rejects(f.client.sample(), /closed/);
});

test('timeout, oversized and malformed responses terminate the helper', async () => {
  for (const [reply, expected] of [
    [() => {}, /timed out/],
    [(_, child) => child.stdout.write('x'.repeat(65_537)), /exceeds limit/],
    [(_, child) => child.stdout.write('not-json\n'), /Invalid GPU response/],
  ]) {
    const f = fixture(reply);
    await assert.rejects(f.client.sample(), expected);
    assert.equal(f.child.killed, true);
    f.client.close();
  }
});

test('close while locating the helper prevents a late spawn', async () => {
  let locate,
    starts = 0;
  const client = new GpuClient({
    locate: () =>
      new Promise((resolve) => {
        locate = resolve;
      }),
    spawnProcess: () => {
      starts++;
      throw new Error('must not spawn');
    },
  });
  const request = client.sample();
  client.close();
  locate('fixture.exe');
  await assert.rejects(request, /closed/);
  assert.equal(starts, 0);
});

test('warmup, valid zero, missing adapter, unavailable VRAM and stale differ', () => {
  const data = { gpu: sample() };
  assert.equal(metricView('gpu', { gpu: null }, {}, 1000).status, 'warming_up');
  assert.equal(metricView('gpu', data, {}, 1000).value, '0%');
  assert.equal(metricView('gpu-memory', data, {}, 1000).value, '2.0');
  assert.equal(metricView('gpu-memory', data, {}, 1000).detail, '/ 8.0 GiB');
  assert.equal(metricView('gpu', data, {}, 8000).status, 'stale');
  assert.equal(
    metricView('gpu', data, { adapterId: '00000000:00000002' }, 1000).status,
    'unavailable',
  );
  data.gpu.adapters[0].gpu_status = 'warming_up';
  data.gpu.adapters[0].busiest_engine_percent = null;
  assert.equal(metricView('gpu', data, {}, 1000).value, '…');
  data.gpu.adapters[0].memory_status = 'unavailable';
  data.gpu.adapters[0].dedicated_total_bytes = 0;
  assert.equal(metricView('gpu-memory', data, {}, 1000).value, '—');
});

test('native key and strip SVG retain readable type, finite values and escaped labels', () => {
  for (const canvas of [
    [112, 112],
    [200, 128],
    [800, 128],
  ]) {
    const svg = render('gpu', { gpu: sample() }, { label: '<GPU>&' }, canvas, 1000);
    assert.ok(svg.includes('&lt;GPU&gt;&amp;'));
    assert.ok(svg.includes('>0%</text>'));
    assert.ok(!svg.includes('NaN'));
    assert.ok(!svg.includes('Infinity'));
    const fonts = [...svg.matchAll(/font-size="([\d.]+)"/g)].map((m) => Number(m[1]));
    assert.ok(fonts.every((size) => size >= 13));
  }
});

test('shared two-second sampler starts GPU only on demand and stops the last consumer', async () => {
  let cpu = 0,
    creates = 0,
    closes = 0,
    reads = 0,
    publications = 0,
    cancelled = 0,
    tick;
  const sampler = new SharedSampler(() => publications++, {
    system: {
      cpus: () => [{ times: { idle: (cpu += 10), user: cpu } }],
      totalmem: () => 8 * 2 ** 30,
      freemem: () => 4 * 2 ** 30,
      uptime: () => 100,
    },
    createGpuClient: () => {
      creates++;
      return {
        sample: async () => {
          reads++;
          return sample();
        },
        close: () => closes++,
      };
    },
    every: (callback, milliseconds) => {
      assert.equal(milliseconds, 2000);
      tick = callback;
      return 1;
    },
    cancel: () => cancelled++,
  });
  sampler.setDemand('cpu', false);
  assert.equal(creates, 0);
  assert.equal(sampler.sample.cpu, null);
  sampler.setDemand('gpu1', true);
  sampler.setDemand('gpu2', true);
  await flush();
  assert.equal(creates, 1);
  assert.equal(reads, 1);
  tick();
  await flush();
  assert.equal(reads, 2);
  assert.equal(sampler.sample.memory, 50);
  sampler.setDemand('gpu1', null);
  assert.equal(closes, 0);
  sampler.setDemand('gpu2', null);
  assert.equal(closes, 1);
  assert.equal(cancelled, 0);
  sampler.setDemand('cpu', null);
  assert.equal(cancelled, 1);
  const before = publications;
  tick();
  assert.equal(publications, before);
  sampler.close();
});

test('slow GPU source never queues ticks and a late reply cannot republish after close', async () => {
  let resolve,
    reads = 0,
    publications = 0,
    tick;
  const sampler = new SharedSampler(() => publications++, {
    system: { cpus: () => [], totalmem: () => 0, freemem: () => 0, uptime: () => 0 },
    createGpuClient: () => ({
      sample: () => {
        reads++;
        return new Promise((r) => {
          resolve = r;
        });
      },
      close: () => {},
    }),
    every: (callback) => {
      tick = callback;
      return 1;
    },
    cancel: () => {},
  });
  sampler.setDemand('gpu', true);
  tick();
  tick();
  assert.equal(reads, 1);
  sampler.close();
  const before = publications;
  resolve(sample());
  await flush();
  assert.equal(publications, before);
  assert.equal(sampler.sample.gpu, null);
});
