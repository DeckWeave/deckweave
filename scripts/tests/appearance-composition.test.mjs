import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';
import { loadTypeScript } from './load-ts-source.mjs';
const { themeColors, luminance } = await loadTypeScript(
  new URL('../../apps/desktop/src/theme.ts', import.meta.url),
);
const { acceptsNested, reordered } = await loadTypeScript(
  new URL('../../apps/desktop/src/action-drop.ts', import.meta.url),
);
const { freeRegion, splitRegion, validRegion } = await loadTypeScript(
  new URL('../../apps/desktop/src/panel-layout.ts', import.meta.url),
);
test('splitting a full panel preserves the first region and produces adjacent valid bounds', () => {
  const old = { id: 'one', name: 'Full', rect: [0, 0, 1, 1], dials: ['dial-1'] };
  assert.equal(freeRegion([old]), null);
  const [left, right] = splitRegion(old);
  assert.deepEqual(old.rect, [0, 0, 1, 1]);
  const next = [
    { ...old, rect: left },
    { id: 'two', rect: right },
  ];
  assert.equal(validRegion(left, next, 'one'), true);
  assert.equal(validRegion(right, next, 'two'), true);
  assert.equal(validRegion([0.4, 0, 0.5, 1], next, 'two'), false);
  assert.equal(validRegion([0, 0, 2, 1], [], 'one'), false);
  assert.equal(validRegion([0, 0, NaN, 1], [], 'one'), false);
  assert.equal(splitRegion({ ...old, rect: [0, 0, 13 / 800, 1] }), null);
  assert.ok(freeRegion([{ ...old, rect: [0, 0, 0.5, 1] }]));
});
const ratio = (a, b) => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
test('arbitrary accents keep text and focus readable in both themes', () => {
  for (const theme of ['light', 'dark'])
    for (const color of [
      '#000000',
      '#ffffff',
      '#e8b466',
      '#ff0000',
      '#00ff00',
      '#0000ff',
      '#777777',
      '#808080',
      '#123abc',
    ]) {
      const c = themeColors(theme, color);
      assert.ok(
        ratio(luminance(c.accent), luminance(c.ink)) >= 4.5,
        `${theme} ${color}: selected text`,
      );
      assert.ok(
        ratio(luminance(c.focus), luminance(theme === 'light' ? '#ffffff' : '#252525')) >= 4.5,
        `${theme} ${color}: focus`,
      );
    }
  assert.equal(themeColors('dark', 'url(x)').accent, '#e8b466');
});
test('nested drops enforce controller, sequence and depth restrictions before replacing settings', () => {
  const keypad = { controller: 'Keypad', depth: 1, inSequence: false };
  assert.equal(acceptsNested({ type: 'plugin', controllers: ['Encoder'] }, keypad), false);
  assert.equal(
    acceptsNested(
      { type: 'plugin', controllers: ['Encoder'] },
      { ...keypad, controller: 'Encoder' },
    ),
    true,
  );
  for (const plan of ['key_logic', 'dial_bindings', 'dial_stack', 'action_wheel'])
    assert.equal(acceptsNested({ type: 'plan', plan }, keypad), false);
  assert.equal(acceptsNested({ type: 'folder' }, keypad), false);
  assert.equal(acceptsNested({ type: 'plan', plan: 'sequence' }, keypad), true);
  for (const plan of ['sequence', 'switch'])
    assert.equal(acceptsNested({ type: 'plan', plan }, { ...keypad, inSequence: true }), false);
  assert.equal(
    acceptsNested({ type: 'plan', plan: 'launch' }, { ...keypad, inSequence: true }),
    true,
  );
  assert.equal(acceptsNested({ type: 'plan', plan: 'switch' }, { ...keypad, depth: 6 }), false);
});
test('step reordering retains action settings and delays, rejecting foreign and cancelled positions', () => {
  const steps = [
    { delay_ms: 10, action: { id: 'a', settings: { path: 'keep' } } },
    { delay_ms: 500, action: { id: 'b' } },
    { delay_ms: 0, action: { id: 'c' } },
  ];
  const moved = reordered(steps, 0, 2);
  assert.deepEqual(
    moved.map((s) => s.action.id),
    ['b', 'c', 'a'],
  );
  assert.equal(moved[2], steps[0]);
  assert.equal(reordered(steps, 0, -1), steps);
  assert.equal(reordered(steps, 0, 3), steps);
  assert.equal(reordered(steps, 1, 1), steps);
});

test('standalone inspectors opt in after registration and ignore appearance messages in Elgato', async () => {
  const source = await readFile(
    new URL('../../assets/branding/inspector-theme.js', import.meta.url),
    'utf8',
  );
  const properties = new Map();
  const root = { dataset: {}, style: { setProperty: (key, value) => properties.set(key, value) } };
  const timers = [];
  const sandbox = {
    document: { documentElement: root },
    setTimeout: (callback) => timers.push(callback),
  };
  vm.runInNewContext(source, sandbox);
  const appearance = sandbox.DeckWeaveAppearance;
  const listeners = [];
  const sent = [];
  const socket = {
    readyState: 1,
    addEventListener: (event, callback) => listeners.push(callback),
    send: (message) => sent.push(JSON.parse(message)),
  };
  appearance.apply(undefined);
  appearance.subscribe(socket, 'context');
  assert.equal(listeners.length, 0);
  appearance.apply({ theme: 'light', accentColor: '#8fb9ff' });
  appearance.subscribe(socket, 'context');
  assert.equal(root.dataset.theme, 'light');
  assert.equal(properties.get('--accent'), '#8fb9ff');
  listeners[0]();
  assert.equal(sent.length, 0);
  socket.send(JSON.stringify({ event: 'registerPropertyInspector', uuid: 'context' }));
  timers[0]();
  assert.deepEqual(
    sent.map((m) => m.event),
    ['registerPropertyInspector', 'deckweaveSubscribeAppearance'],
  );
  appearance.receive({ event: 'didReceiveSettings', payload: { theme: 'dark' } });
  assert.equal(root.dataset.theme, 'light');
  appearance.receive({
    event: 'deckweaveDidChangeAppearance',
    payload: { theme: 'dark', accentColor: 'url(x)' },
  });
  assert.equal(root.dataset.theme, 'dark');
  assert.equal(properties.get('--accent'), '#8fb9ff');
  for (const name of ['audio', 'diagnostics', 'monitor', 'obs', 'timers', 'weather']) {
    const root = new URL(`../../plugins/dev.deckweave.${name}.sdPlugin/`, import.meta.url);
    assert.equal(await readFile(new URL('inspector-theme.js', root), 'utf8'), source);
    const html = await readFile(new URL('settings.html', root), 'utf8');
    assert.ok(
      html.indexOf('src="inspector-theme.js"') < html.indexOf('DeckWeaveAppearance.apply'),
      name,
    );
    assert.ok(html.includes('DeckWeaveAppearance.subscribe(socket, uuid)'), name);
  }
});
