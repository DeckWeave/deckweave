'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const net = require('node:net');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const { operations, validateRequest, createDispatcher } = require('./core.cjs');
const { startServer } = require('./server.cjs');
function fixture() {
  const calls = [];
  const selection = {
    isEmpty: false,
    isEqual(other) {
      return other === this;
    },
  };
  const editor = {
    document: { languageId: 'typst', uri: { scheme: 'file' }, version: 1 },
    selection,
  };
  const api = {
    window: { state: { focused: true }, activeTextEditor: editor },
    workspace: { isTrusted: true },
    extensions: {
      getExtension() {
        return { isActive: true };
      },
    },
    commands: {
      async getCommands() {
        return Object.values(operations).map((item) => item[0]);
      },
      async executeCommand(...args) {
        calls.push(args);
      },
    },
  };
  return { api, editor, calls, dispatch: createDispatcher(api) };
}
test('fixed commands invoke installed extension actions without caller arguments', async () => {
  const f = fixture();
  await f.dispatch('report.bold');
  await f.dispatch('report.formula');
  await f.dispatch('vscode.codexAddSelection');
  assert.deepEqual(f.calls, [
    ['typstPanel.bold'],
    ['typstPanel.insertFormula'],
    ['chatgpt.addToThread'],
  ]);
  await assert.rejects(f.dispatch('chatgpt.implementTodo'), /Unsupported/);
  assert.ok(
    !Object.values(operations).some((value) =>
      /implementTodo|send|approve|newCodexPanel/.test(value[0]),
    ),
  );
});
test('focus, Typst language, workspace trust and selection fail closed', async () => {
  const f = fixture();
  f.api.window.state.focused = false;
  await assert.rejects(f.dispatch('report.bold'), { code: 'not_focused' });
  f.api.window.state.focused = true;
  f.editor.document.languageId = 'markdown';
  await assert.rejects(f.dispatch('report.bold'), { code: 'editor' });
  f.editor.document.languageId = 'typst';
  f.api.workspace.isTrusted = false;
  await assert.rejects(f.dispatch('report.bold'), { code: 'untrusted' });
  f.api.workspace.isTrusted = true;
  f.editor.selection.isEmpty = true;
  await assert.rejects(f.dispatch('vscode.codexAddSelection'), { code: 'selection' });
  assert.equal(f.calls.length, 0);
});
test('extension activation cannot redirect an old press to a changed document or focus', async () => {
  for (const change of [
    (f) => {
      f.editor.document.version++;
    },
    (f) => {
      f.api.window.state.focused = false;
    },
    (f) => {
      f.api.window.activeTextEditor = { ...f.editor };
    },
  ]) {
    const f = fixture();
    f.api.extensions.getExtension = () => ({
      isActive: false,
      async activate() {
        change(f);
      },
    });
    await assert.rejects(f.dispatch('report.bold'));
    assert.equal(f.calls.length, 0);
  }
});
test('missing commands, abort and overlapping commands never cause a fallback', async () => {
  const f = fixture();
  f.api.commands.getCommands = async () => [];
  await assert.rejects(f.dispatch('report.bold'), { code: 'command' });
  const abort = new AbortController();
  abort.abort();
  await assert.rejects(f.dispatch('report.bold', { signal: abort.signal }), { code: 'expired' });
  let finish;
  f.api.commands.getCommands = () =>
    new Promise((resolve) => {
      finish = resolve;
    });
  const pending = f.dispatch('report.bold');
  await assert.rejects(f.dispatch('report.italic'), { code: 'busy' });
  finish(['typstPanel.bold']);
  await pending;
  assert.equal(f.calls.length, 1);
});
test('authentication and strict request shape reject arbitrary commands and arguments', () => {
  const token = 'a'.repeat(64),
    base = { id: 1, token, operation: 'report.bold' };
  validateRequest(base, token);
  for (const value of [
    { ...base, token: 'b'.repeat(64) },
    { ...base, arguments: [] },
    { ...base, operation: 'workbench.action.terminal.sendSequence' },
    { ...base, token: 'é'.repeat(64) },
    { ...base, id: -1 },
  ]) {
    assert.throws(() => validateRequest(value, token));
  }
});
function exchange(pipe, message) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(pipe);
    let output = '';
    socket.setTimeout(3500, () => {
      socket.destroy();
      reject(new Error('fixture timeout'));
    });
    socket.on('error', reject);
    socket.on('data', (data) => {
      output += data;
    });
    socket.on('close', () => resolve(output));
    socket.on('connect', () => socket.write(message));
  });
}
test('actual local pipe bounds frames, authenticates, rejects replay and returns no editor text', async (t) => {
  const nonce = crypto.randomBytes(8).toString('hex');
  const pipe =
    process.platform === 'win32'
      ? `\\\\.\\pipe\\deckweave-fixture-${nonce}`
      : path.join(os.tmpdir(), `deckweave-${nonce}.sock`);
  const token = crypto.randomBytes(32).toString('hex');
  const f = fixture();
  const service = await startServer({ pipe, token, dispatch: f.dispatch });
  t.after(() => service.close());
  const request = (id, op, auth = token) =>
    JSON.stringify({ id, token: auth, operation: op }) + '\n';
  assert.equal(
    JSON.parse(await exchange(pipe, request(1, 'report.bold', 'b'.repeat(64)))).code,
    'auth',
  );
  const result = JSON.parse(await exchange(pipe, request(2, 'report.bold')));
  assert.deepEqual(result, { id: 2, ok: true, executed: 'report.bold' });
  assert.equal(JSON.parse(await exchange(pipe, request(2, 'report.bold'))).code, 'replay');
  assert.equal(await exchange(pipe, 'x'.repeat(1025)), '');
  assert.equal(f.calls.length, 1);
});
