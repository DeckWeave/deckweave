// SPDX-License-Identifier: GPL-3.0-or-later
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { setImmediate as tick } from 'node:timers/promises';
import { start } from './plugin.mjs';
import { config, render, workspace } from './focus.mjs';

class Socket extends EventTarget {
  static last;
  readyState = 0;
  sent = [];
  job;
  constructor() {
    super();
    Socket.last = this;
  }
  message(value) {
    this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(value) }));
  }
  open() {
    this.readyState = 1;
    this.dispatchEvent(new Event('open'));
  }
  close() {
    this.readyState = 3;
    this.dispatchEvent(new Event('close'));
  }
  send(raw) {
    const message = JSON.parse(raw);
    this.sent.push(message);
    if (message.event !== 'deckweaveService') return;
    queueMicrotask(() => {
      let value;
      if (message.operation === 'focus_get') {
        this.job ??= {
          version: 1,
          owner: 'dev.deckweave.focus',
          workspace: message.payload.workspace,
          revision: 1,
          run_id: 'test-run',
          phase: 'work',
          status: 'idle',
          round: 1,
          transition: 0,
          remaining_ms: 1500000,
          live_remaining_ms: 1500000,
          due_ms: null,
          config: message.payload.config,
        };
        value = this.job;
      } else if (message.operation === 'focus_control') {
        if (message.payload.revision !== this.job.revision) {
          this.message({
            event: 'deckweaveServiceResult',
            requestId: message.requestId,
            ok: false,
            error: { code: 'conflict', message: 'Pomodoro changed' },
          });
          return;
        }
        value = { ...this.job, revision: this.job.revision + 1 };
        if (message.payload.command === 'toggle')
          value.status = value.status === 'running' ? 'paused' : 'running';
        if (message.payload.command === 'configure') value.config = message.payload.config;
        this.job = value;
      } else throw new Error('Unexpected service operation');
      this.message({
        event: 'deckweaveServiceResult',
        requestId: message.requestId,
        ok: true,
        value,
      });
    });
  }
}
const args = {
  port: 1234,
  uuid: 'dev.deckweave.focus',
  registerEvent: 'registerPlugin',
  info: { deckweave: { services: { focus: 1 } } },
};
function appear(socket, id, settings = {}) {
  socket.message({
    event: 'willAppear',
    context: id,
    action: 'dev.deckweave.focus.pomodoro',
    payload: { controller: 'Panel', canvas: [240, 100], settings },
  });
}
async function settle() {
  for (let n = 0; n < 5; n++) await tick();
}
test('real SDK negotiates focus alone, copies share one host job and use guarded unique controls', async () => {
  const errors = [],
    plugin = start({ arguments: args, WebSocket: Socket, onError: (e) => errors.push(e) }),
    socket = Socket.last;
  socket.open();
  assert.equal(socket.sent[1].event, 'deckweaveServiceRegister');
  appear(socket, 'home', { workspace: 'personal' });
  appear(socket, 'reports', { workspace: 'personal' });
  await settle();
  assert.equal(socket.sent.filter((m) => m.operation === 'focus_get').length, 1);
  socket.message({ event: 'panelDown', context: 'home' });
  socket.message({ event: 'panelUp', context: 'home' });
  await settle();
  let controls = socket.sent.filter((m) => m.operation === 'focus_control');
  assert.equal(controls.length, 1);
  assert.equal(controls[0].payload.revision, 1);
  assert.equal(socket.job.status, 'running');
  const images = socket.sent.filter((m) => m.event === 'setImage');
  assert.ok(images.some((m) => m.context === 'reports'));
  socket.message({ event: 'panelUp', context: 'reports' });
  await settle();
  controls = socket.sent.filter((m) => m.operation === 'focus_control');
  assert.equal(controls[1].payload.revision, 2);
  assert.notEqual(controls[0].payload.commandId, controls[1].payload.commandId);
  assert.equal(socket.job.status, 'paused');
  socket.message({ event: 'willDisappear', context: 'home' });
  socket.message({ event: 'willDisappear', context: 'reports' });
  const before = socket.sent.length;
  socket.message({
    event: 'deckweaveFocusDidChange',
    payload: {
      jobs: [
        {
          ...socket.job,
          status: 'ready',
          phase: 'short_break',
          revision: 4,
          live_remaining_ms: 300000,
        },
      ],
    },
  });
  await settle();
  assert.equal(socket.sent.length, before);
  assert.equal(plugin.contexts.size, 0);
  assert.deepEqual(errors, []);
  plugin.dispose();
});
test('inspector config uses authoritative shared settings and never opens an overlay', async () => {
  const plugin = start({ arguments: args, WebSocket: Socket }),
    socket = Socket.last;
  socket.open();
  appear(socket, 'one');
  await settle();
  socket.message({
    event: 'deckweaveFocusDidChange',
    payload: { jobs: [{ ...socket.job, config: { ...socket.job.config, short_seconds: 420 } }] },
  });
  socket.message({
    event: 'sendToPlugin',
    context: 'one',
    payload: { command: 'configure', settings: { workMinutes: 30 } },
  });
  await settle();
  const request = socket.sent.find((m) => m.operation === 'focus_control');
  assert.equal(request.payload.command, 'configure');
  assert.equal(request.payload.config.work_seconds, 1800);
  assert.equal(request.payload.config.short_seconds, 420);
  assert.ok(socket.sent.some((m) => m.event === 'setSettings' && m.payload.workMinutes === 30));
  assert.ok(
    !socket.sent.some(
      (m) => m.operation === 'open' || m.operation === 'sound' || m.event === 'setInterval',
    ),
  );
  plugin.dispose();
});
test('unsupported host reports unavailable without falling back to a plugin clock', async () => {
  const plugin = start({ arguments: { ...args, info: {} }, WebSocket: Socket }),
    socket = Socket.last;
  socket.open();
  appear(socket, 'one');
  await settle();
  assert.ok(socket.sent.some((m) => m.event === 'showAlert'));
  assert.ok(!socket.sent.some((m) => m.event === 'deckweaveService'));
  plugin.dispose();
});
test('settings bounds and actual-size renders cover phases and error states', () => {
  assert.equal(config({}).work_seconds, 1500);
  assert.equal(workspace({}), 'personal');
  assert.throws(() => workspace({ workspace: '../personal/' }));
  assert.throws(() => config({ rounds: 17 }));
  assert.throws(() => config({ workMinutes: 0 }));
  const job = {
    phase: 'long_break',
    status: 'ready',
    round: 4,
    live_remaining_ms: 900000,
    config: config({}),
  };
  for (const size of [
    [72, 72],
    [112, 112],
    [200, 100],
    [240, 100],
    [300, 100],
  ]) {
    const svg = render(job, { language: 'ru' }, size);
    assert.match(svg, /15:00/);
    assert.match(svg, /Отдых/);
    assert.match(svg, /Далее 4\/4/);
    assert.ok(!svg.includes('NaN'));
    assert.match(svg, new RegExp(`width="${size[0]}" height="${size[1]}"`));
  }
  assert.match(render(null, {}, [112, 112], 'Disconnected'), /Unavailable/);
});
