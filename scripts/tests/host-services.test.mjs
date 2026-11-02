import { test } from 'node:test';
import assert from 'node:assert/strict';
import { start as desktop } from '../../plugins/dev.deckweave.desktop.sdPlugin/plugin.mjs';
import { start as monitor } from '../../plugins/dev.deckweave.monitor.sdPlugin/plugin.mjs';
class Socket extends EventTarget {
  static last;
  readyState = 0;
  sent = [];
  constructor() {
    super();
    Socket.last = this;
  }
  send(text) {
    const m = JSON.parse(text);
    this.sent.push(m);
    if (m.operation === 'open' && !this.panel)
      this.panel = { id: 'panel', revision: 1, open: true, spec: structuredClone(m.payload.spec) };
    if (m.operation === 'configure')
      this.panel = { ...this.panel, revision: this.panel.revision + 1, spec: m.payload.spec };
    if (m.event === 'deckweaveService')
      queueMicrotask(() =>
        this.message({
          event: 'deckweaveServiceResult',
          requestId: m.requestId,
          ok: true,
          value: ['open', 'get', 'configure'].includes(m.operation)
            ? this.panel
            : { status: 'updated' },
        }),
      );
  }
  open() {
    this.readyState = 1;
    this.dispatchEvent(new Event('open'));
  }
  message(value) {
    this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(value) }));
  }
  close() {
    this.readyState = 3;
    this.dispatchEvent(new Event('close'));
  }
}
const options = (services = { sound: 1, panels: 1, timers: 1 }) => ({
  arguments: {
    port: 12345,
    uuid: 'dev.test',
    registerEvent: 'registerPlugin',
    info: { application: { language: 'en' }, deckweave: { services } },
  },
  WebSocket: Socket,
  onError: (e) => {
    throw e;
  },
});
const settle = () => new Promise((r) => setImmediate(r));
test('desktop settings update only edited fields and never reopen a saved closed panel', async () => {
  const p = desktop(options()),
    s = Socket.last;
  s.open();
  s.message({
    event: 'willAppear',
    context: 'a',
    action: 'dev.deckweave.desktop.note',
    payload: { settings: { text: '<script> is plain text' } },
  });
  s.message({ event: 'keyUp', context: 'a' });
  await settle();
  const requests = () => s.sent.filter((m) => m.event === 'deckweaveService');
  assert.deepEqual(
    requests().map((m) => m.operation),
    ['open'],
  );
  assert.equal(requests()[0].payload.spec.content.text, '<script> is plain text');
  s.panel.open = false;
  s.panel.spec.placement.foreground = '#abcdef';
  s.message({
    event: 'didReceiveSettings',
    context: 'a',
    payload: { settings: { text: 'Edited' } },
  });
  await settle();
  assert.deepEqual(
    requests().map((m) => m.operation),
    ['open', 'get', 'configure'],
  );
  assert.equal(s.panel.open, false);
  assert.equal(s.panel.spec.content.text, 'Edited');
  assert.equal(s.panel.spec.placement.foreground, '#abcdef');
  p.dispose();
});
test('timer is host-owned; sound is opt-in and unsupported hosts report an alert', async () => {
  const p = desktop(options()),
    s = Socket.last;
  s.open();
  s.message({
    event: 'willAppear',
    context: 'a',
    action: 'dev.deckweave.desktop.timer',
    payload: { settings: { seconds: 5, sound: 'notice', volume: 21 } },
  });
  s.message({ event: 'keyUp', context: 'a' });
  await settle();
  const requests = s.sent.filter((m) => m.event === 'deckweaveService');
  assert.deepEqual(
    requests.map((m) => m.operation),
    ['open', 'timer'],
  );
  assert.equal(requests[0].payload.spec.content.volume, 21);
  s.message({ event: 'willDisappear', context: 'a' });
  await settle();
  assert.equal(s.sent.filter((m) => m.event === 'deckweaveService').length, 2);
  p.dispose();
  const q = desktop(options({})),
    socket = Socket.last;
  socket.open();
  socket.message({ event: 'willAppear', context: 'b', action: 'dev.deckweave.desktop.sound' });
  socket.message({ event: 'keyUp', context: 'b' });
  await settle();
  assert.ok(socket.sent.some((m) => m.event === 'showAlert'));
  assert.ok(!socket.sent.some((m) => m.event === 'deckweaveService'));
  q.dispose();
});
test('real monitor keeps the shared sampler for background demand after visible context disappears', async () => {
  const p = monitor(options()),
    s = Socket.last;
  s.open();
  s.message({
    event: 'willAppear',
    context: 'a',
    action: 'dev.deckweave.monitor.cpu',
    payload: { settings: { desktopPanel: true } },
  });
  s.message({ event: 'keyUp', context: 'a' });
  await settle();
  assert.ok(s.sent.some((m) => m.operation === 'open'));
  s.message({
    event: 'deckweaveServiceDidAppear',
    context: 'service',
    payload: {
      panel: { id: 'panel', revision: 7, spec: { content: { type: 'metric', detail: 'cpu' } } },
    },
  });
  s.message({ event: 'willDisappear', context: 'a' });
  await settle();
  const count = s.sent.filter((m) => m.operation === 'update').length;
  await new Promise((r) => setTimeout(r, 2100));
  assert.ok(s.sent.filter((m) => m.operation === 'update').length > count);
  assert.equal(s.sent.filter((m) => m.operation === 'update').at(-1).payload.revision, 7);
  s.message({ event: 'deckweaveServiceDidDisappear', context: 'service' });
  p.dispose();
});
