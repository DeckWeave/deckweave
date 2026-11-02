import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTypeScript } from './load-ts-source.mjs';
const {
  onboardAssignment,
  keyboardOverlay,
  keyboardOwner,
  keyboardPreviewCurrent,
  keyboardPreviewForContext,
} = await loadTypeScript(
  new URL('../../apps/desktop/src/keyboard-assignments.ts', import.meta.url),
);
const session = (main, fn, labels = {}) => ({
  state: 'connected',
  observed: { keyMappings: { T: { main, fn } }, onboardKeyLabels: { T: labels } },
});
const raw = (...record) => ({ page: record[0], parameters: record.slice(1) });

test('dropping onto an unselected key preserves its new proposal when the selection effect flushes', () => {
  const before = ['io84', 'work', 'page1', 'fn', 'T', 'actions'];
  const selected = [...before];
  selected[4] = 'Q';
  // applyLibrary selects Q and creates this proposal in the same event. The
  // subsequent context effect must not erase it merely because selection changed.
  const proposal = { context: selected.join('\0'), controlId: 'Q', binding: { label: 'Launch' } };
  assert.equal(keyboardPreviewForContext(proposal, selected.join('\0')), proposal);
  assert.equal(keyboardPreviewForContext(proposal, selected.join('\0')), proposal);
  assert.equal(
    keyboardPreviewForContext({ ...proposal, context: before.join('\0') }, selected.join('\0')),
    null,
  );
  for (const [index, value] of [
    [0, 'other-device'],
    [1, 'streaming'],
    [2, 'page2'],
    [3, 'main'],
    [4, 'W'],
    [5, 'lighting'],
  ]) {
    const changed = [...selected];
    changed[index] = value;
    assert.equal(keyboardPreviewForContext(proposal, changed.join('\0')), null);
  }
  assert.equal(keyboardPreviewForContext(null, selected.join('\0')), null);
});

test('real unresolved ownership takes precedence over later legacy references for the same physical source', () => {
  const owned = {
    control_id: 'T',
    layer: 'fn',
    provenance: 'deckweave',
    phase: 'uncertain',
    alias: 13,
  };
  const legacy = {
    control_id: 'T',
    layer: 'fn',
    provenance: 'legacy',
    phase: 'legacy_unverified',
    alias: 14,
  };
  assert.equal(keyboardOwner([owned, legacy], 'T', 'fn'), owned);
  assert.equal(keyboardOwner([legacy, owned], 'T', 'fn'), owned);
  assert.equal(keyboardOwner([owned, legacy], 'T', 'main'), undefined);
  assert.equal(keyboardOwner([legacy], 'T', 'fn'), legacy);
  assert.equal(keyboardOwner(undefined, 'T', 'fn'), undefined);
});

test('a replacement preview cannot silently follow a changed mapping or reconnected keyboard', () => {
  const native = onboardAssignment(
    session(raw(2, 0, 23, 0), raw(2, 0, 23, 0)),
    'keyMappings',
    'T',
    'fn',
    'en',
  );
  const changed = onboardAssignment(
    session(raw(2, 0, 23, 0), raw(2, 0, 24, 0)),
    'keyMappings',
    'T',
    'fn',
    'en',
  );
  assert.equal(keyboardPreviewCurrent(native, 4, native, 4), true);
  assert.equal(keyboardPreviewCurrent(native, 4, changed, 4), false);
  assert.equal(keyboardPreviewCurrent(native, 4, native, 5), false);
  assert.equal(keyboardPreviewCurrent(native, null, native, undefined), false);
  assert.equal(
    keyboardPreviewCurrent(native, 4, onboardAssignment(null, 'keyMappings', 'T', 'fn', 'en'), 4),
    false,
  );
});

test('Main and Fn come from separate readback records, even with no DeckWeave binding', () => {
  const state = session(raw(2, 0, 23, 0), raw(2, 0, 104, 0));
  assert.equal(onboardAssignment(state, 'keyMappings', 'T', 'main', 'en').label, 'T');
  assert.equal(onboardAssignment(state, 'keyMappings', 'T', 'fn', 'en').label, 'F13');
  assert.equal(
    keyboardOverlay(onboardAssignment(state, 'keyMappings', 'T', 'fn', 'en'), undefined, null)
      .label,
    'F13',
  );
});

test('adapter labels reveal prefilled device functions, without making them writable', () => {
  const state = session(raw(2, 0, 23, 0), raw(13, 0, 0, 13), {
    fn: {
      kind: 'device-function',
      label: { en: 'Key brightness +', ru: 'Яркость клавиш +' },
      known: true,
    },
  });
  const assignment = onboardAssignment(state, 'keyMappings', 'T', 'fn', 'ru');
  assert.equal(assignment.label, 'Яркость клавиш +');
  assert.equal(assignment.kind, 'device-function');
  assert.equal(assignment.known, true);
  assert.equal(assignment.replaceable, false);
  assert.deepEqual(assignment.record, [13, 0, 0, 13]);
});

test('older adapters do not invent a device function from a key position or usage field', () => {
  const state = session(raw(2, 0, 23, 0), { ...raw(13, 0, 0, 13), usage: 104 });
  const assignment = onboardAssignment(state, 'keyMappings', 'T', 'fn', 'en');
  assert.equal(assignment.label, 'Unknown assignment');
  assert.equal(assignment.replaceable, false);
  assert.equal(assignment.usage, null);
  assert.deepEqual(assignment.record, [13, 0, 0, 13]);
});

test('unknown, modified and malformed records stay read only and never become empty', () => {
  for (const value of [
    raw(99, 0, 0, 0),
    raw(2, 1, 104, 0),
    raw(2, 0, 104, 1),
    raw(2, 0, 124, 0),
    raw(0, 0, 0, 0),
    { page: 2, parameters: [0, 104] },
    raw(2, 0, -1, 0),
  ]) {
    const assignment = onboardAssignment(session(value, value), 'keyMappings', 'T', 'fn', 'en');
    assert.equal(assignment.replaceable, false);
    assert.ok(assignment.label.length);
  }
  const missing = onboardAssignment(null, 'keyMappings', 'T', 'fn', 'en');
  assert.equal(missing.label, 'Assignment not read');
  assert.equal(missing.observed, false);
});

test('saved Fn Launch preserves host intent while explicitly exposing hardware mismatch', () => {
  const host = {
    id: 'user-launch',
    kind: 'program',
    label: 'Open Notepad',
    plugin: null,
    plan: { type: 'launch', program: 'notepad.exe', args: [] },
  };
  const native = onboardAssignment(
    session(raw(2, 0, 23, 0), raw(2, 0, 23, 0)),
    'keyMappings',
    'T',
    'fn',
    'en',
  );
  assert.deepEqual(keyboardOverlay(native, host, null), {
    label: 'Open Notepad',
    host: true,
    mapping: 'unconfigured',
  });
  assert.deepEqual(keyboardOverlay(native, host, 104), {
    label: 'Open Notepad',
    host: true,
    mapping: 'mismatch',
  });
  const verified = onboardAssignment(
    session(raw(2, 0, 23, 0), raw(2, 0, 104, 0)),
    'keyMappings',
    'T',
    'fn',
    'en',
  );
  assert.equal(keyboardOverlay(verified, host, 104).mapping, 'verified');
  // The projection cannot claim an input route from readback; that is separate status.
  assert.equal('ready' in keyboardOverlay(verified, host, 104), false);
  assert.equal(host.plan.program, 'notepad.exe');
});
