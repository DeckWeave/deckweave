import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { settings, render, operations } from './operations.mjs';
import { NativeClient, WorkspaceClient } from './client.mjs';
import { start } from './plugin.mjs';
import { setImmediate as settle } from 'node:timers/promises';

class PluginSocket extends EventTarget {
  static last;
  readyState = 0;
  sent = [];
  constructor() {
    super();
    PluginSocket.last = this;
  }
  send(raw) {
    this.sent.push(JSON.parse(raw));
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
for (const controller of ['Keypad', 'Encoder', 'Panel'])
  test(`real plugin keeps ${controller} artwork and cause on failure, then recovers after success`, async () => {
    const calls = [];
    let failed = true;
    const p = start(
      {
        arguments: {
          port: 1234,
          uuid: 'dev.deckweave.workspace',
          registerEvent: 'registerPlugin',
          info: { application: { language: 'ru' } },
        },
        WebSocket: PluginSocket,
      },
      {
        async request(operation) {
          calls.push(operation);
          if (failed)
            throw Error(
              'Windows did not focus the selected application; focus it manually and retry',
            );
        },
        close() {},
      },
    );
    const s = PluginSocket.last;
    s.open();
    try {
      const appear = (context) =>
        s.message({
          event: 'willAppear',
          context,
          action: 'dev.deckweave.workspace.command',
          payload: {
            controller,
            canvas:
              controller === 'Panel'
                ? [280, 128]
                : controller === 'Encoder'
                  ? [200, 100]
                  : [112, 112],
            settings: { operation: 'launch.zotero', label: 'Zotero', language: 'ru' },
          },
        });
      appear('a');
      appear('b');
      assert.equal(calls.length, 0, 'appearance must never launch an app');
      s.message({
        event: controller === 'Keypad' ? 'keyUp' : controller === 'Encoder' ? 'dialUp' : 'panelUp',
        context: 'a',
        payload: {},
      });
      await settle();
      assert.equal(calls.length, 1);
      const alert = s.sent.find((m) => m.event === 'showAlert');
      assert.match(alert.payload.message, /Windows did not focus/);
      assert.match(p.contexts.get('a').state.error, /Windows did not focus/);
      const images = s.sent.filter(
        (m) =>
          m.context === 'a' && m.event === (controller === 'Encoder' ? 'setFeedback' : 'setImage'),
      );
      const uri =
        controller === 'Encoder'
          ? images.at(-1).payload['full-canvas']
          : images.at(-1).payload.image;
      const svg = Buffer.from(uri.split(',')[1], 'base64').toString();
      assert.match(svg, /Нет окна/);
      assert.match(svg, /#ffb4a6/);
      assert.equal(
        p.contexts.get('b').state.error,
        undefined,
        'another button keeps its appearance',
      );
      s.message({ event: 'propertyInspectorDidAppear', context: 'a' });
      assert.match(
        s.sent.filter((m) => m.event === 'sendToPropertyInspector').at(-1).payload.error,
        /Windows did not focus/,
      );
      failed = false;
      s.message({
        event: controller === 'Keypad' ? 'keyUp' : controller === 'Encoder' ? 'dialUp' : 'panelUp',
        context: 'a',
        payload: {},
      });
      await settle();
      assert.equal(calls.length, 2);
      assert.equal(p.contexts.get('a').state.error, '');
      assert.ok(s.sent.some((m) => m.event === 'showOk'));
      assert.ok(
        !s.sent.some((m) => m.event === 'setSettings'),
        'execution must never remove or rewrite assignments',
      );
    } finally {
      p.dispose();
    }
  });

test('a pending real plugin action cannot duplicate execution or redraw after disappearance', async () => {
  let resolve,
    calls = 0;
  const p = start(
    {
      arguments: {
        port: 1234,
        uuid: 'dev.deckweave.workspace',
        registerEvent: 'registerPlugin',
        info: {},
      },
      WebSocket: PluginSocket,
    },
    {
      request() {
        calls++;
        return new Promise((r) => {
          resolve = r;
        });
      },
      close() {},
    },
  );
  const s = PluginSocket.last;
  s.open();
  s.message({
    event: 'willAppear',
    context: 'a',
    payload: { settings: { operation: 'launch.code' } },
  });
  s.message({ event: 'keyUp', context: 'a' });
  s.message({ event: 'keyUp', context: 'a' });
  assert.equal(calls, 1);
  s.message({ event: 'willDisappear', context: 'a' });
  const count = s.sent.length;
  resolve();
  await settle();
  assert.equal(s.sent.length, count);
  p.dispose();
});
test('editor routing reaches exactly one focused companion and does not retry execution', async () => {
  const calls = [];
  const client = new WorkspaceClient({
    native: {
      async request(op) {
        calls.push(op);
      },
    },
    settle: async () => {},
    discover: async () => [{ pipe: 'a' }, { pipe: 'b' }],
    async call(endpoint, op) {
      calls.push(`${endpoint.pipe}:${op}`);
      if (op === 'status') return { focused: endpoint.pipe === 'b' };
      throw new Error('command failed');
    },
  });
  await assert.rejects(client.request('report.bold'), /command failed/);
  assert.deepEqual(calls, ['launch.code', 'a:status', 'b:status', 'b:report.bold']);
});
test('ambiguous focus and disposed contexts cannot execute editor commands', async () => {
  for (const focused of [false, true]) {
    const calls = [];
    const client = new WorkspaceClient({
      native: { async request() {} },
      settle: async () => {},
      discover: async () => [{}, {}],
      async call(_, op) {
        calls.push(op);
        return { focused };
      },
    });
    await assert.rejects(client.request('report.bold'));
    assert.deepEqual(calls, ['status', 'status']);
  }
  const abort = new AbortController();
  const client = new WorkspaceClient({
    native: {
      async request() {
        abort.abort();
      },
    },
    settle: async () => {},
    discover: async () => {
      throw new Error('must not discover');
    },
  });
  await assert.rejects(client.request('report.bold', { signal: abort.signal }), {
    name: 'AbortError',
  });
});
test('unknown settings fail closed and renderer escapes labels at key and strip dimensions', async () => {
  assert.equal(settings({ operation: 'evil' }).operation, 'evil');
  const client = new WorkspaceClient();
  await assert.rejects(client.request('evil'), /supported/);
  client.close();
  for (const canvas of [
    [112, 112],
    [180, 100],
    [480, 100],
  ]) {
    const svg = render(settings({ label: '<script>' }), canvas);
    assert.ok(!svg.includes('<script>'));
    assert.ok(svg.includes('&lt;script&gt;'));
    assert.ok(svg.includes(`width="${canvas[0]}" height="${canvas[1]}"`));
  }
  assert.ok(Object.keys(operations).includes('vscode.codexAddSelection'));
});
function childFixture() {
  const child = new EventEmitter();
  child.stdout = new EventEmitter();
  child.stdout.setEncoding = () => {};
  child.stdin = new EventEmitter();
  child.stdin.destroy = () => {};
  child.kill = () => {};
  return child;
}
test('native stdio rejects mismatched responses and times out without retry', async () => {
  for (const behavior of ['wrong', 'oversize', 'timeout']) {
    const child = childFixture();
    let writes = 0;
    child.stdin.write = (line) => {
      writes++;
      const { id } = JSON.parse(line);
      queueMicrotask(() => {
        if (behavior === 'wrong')
          child.stdout.emit('data', JSON.stringify({ id: id + 1, ok: true }) + '\n');
        if (behavior === 'oversize') child.stdout.emit('data', 'x'.repeat(4097));
      });
    };
    const client = new NativeClient({
      locate: async () => 'fixture',
      spawnProcess: () => child,
      timeout: 20,
    });
    await assert.rejects(client.request('launch.code'));
    assert.equal(writes, 1);
    client.close();
  }
});
