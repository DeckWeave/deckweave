import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { DatabaseSync } from 'node:sqlite';
import path from 'node:path';
import { ROOT, setup, pageSpec, stripSpec } from '../personal-workspace.mjs';

test('personal setup stages valid Core commands and preserves unrelated profiles, devices and preferences', async () => {
  const directory = await mkdtemp(path.join(ROOT, '.local/workspace-test-'));
  try {
    const database = path.join(directory, 'fixture.sqlite3');
    const init = spawnSync(path.join(ROOT, 'target/debug/deckweave-config.exe'), [database], {
      input: JSON.stringify({ type: 'add_device', expected_revision: 0 }) + '\n',
      encoding: 'utf8',
    });
    assert.equal(init.status, 0, init.stderr);
    const db = new DatabaseSync(database);
    const before = JSON.parse(db.prepare('select json from configuration').get().json);
    before.devices[0].provider_id = 'deckweave.n4';
    before.devices[0].hardware_key = 'n4pro-' + 'a'.repeat(64);
    before.devices[0].profile.pages[0].bindings['key-6'] = {
      id: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
      kind: 'plugin',
      label: 'Weather',
      plan: null,
      plugin: {
        package: 'dev.deckweave.weather',
        action: 'dev.deckweave.weather.current',
        settings: { name: 'Екатеринбург', latitude: 56.85733, longitude: 60.61529, language: 'ru' },
        controller: 'Keypad',
      },
    };
    before.devices[1].keyboard_aliases = [];
    before.preferences.disabled_plugins = ['com.elgato.weather'];
    before.preferences.customFutureField = { preserve: true };
    before.devices[0].customFutureDevice = 'keep';
    db.prepare('update configuration set json=?').run(JSON.stringify(before));
    db.close();
    const result = await setup({ database, directory: path.join(directory, 'setup') });
    assert.equal(result.apply, false);
    const stage = new DatabaseSync(result.stagedDatabase, { readOnly: true });
    const after = JSON.parse(stage.prepare('select json from configuration').get().json);
    stage.close();
    const d = after.devices[0];
    assert.equal(d.profile.name, 'Рабочий стол');
    assert.equal(d.profile.pages.length, 5);
    assert.deepEqual(
      d.profile.pages.map((p) => p.name),
      pageSpec.map((p) => p[0]),
    );
    assert.equal(d.profile.active_page_id, d.profile.pages[0].id);
    assert.equal(d.customFutureDevice, 'keep');
    assert.deepEqual(after.preferences.customFutureField, { preserve: true });
    assert.ok(after.preferences.disabled_plugins.includes('com.elgato.weather'));
    const bindingIds = new Set();
    for (const p of d.profile.pages) {
      assert.deepEqual(
        p.panel.regions.map((r) => r.rect),
        stripSpec.map((r) => r[2]),
      );
      for (const b of Object.values(p.bindings)) {
        assert.ok(!bindingIds.has(b.id));
        bindingIds.add(b.id);
      }
      assert.equal(p.bindings['dial-4'].plan.type, 'page_cycle');
      assert.equal(p.bindings['dial-1'].plugin.action, 'dev.deckweave.audio.output');
      for (const b of Object.values(p.bindings).filter(
        (b) => b.plugin?.package === 'dev.deckweave.focus',
      ))
        assert.equal(b.plugin.settings.workspace, 'personal');
    }
    const source = new DatabaseSync(database, { readOnly: true });
    assert.deepEqual(
      JSON.parse(source.prepare('select json from configuration').get().json),
      before,
      'dry run must leave source untouched',
    );
    source.close();
    // Simulate publication in this disposable store, then exercise ownership rules.
    const published = new DatabaseSync(database);
    published
      .prepare('update configuration set revision=?,json=?')
      .run(after.revision, JSON.stringify(after));
    published.close();
    await writeFile(path.join(directory, 'setup/installed.json'), JSON.stringify(result));
    assert.equal(
      (await setup({ database, directory: path.join(directory, 'setup') })).unchanged,
      true,
    );
    const customized = new DatabaseSync(database);
    after.devices[0].profile.pages[0].bindings['key-1'].label = 'Owner customization';
    customized.prepare('update configuration set json=?').run(JSON.stringify(after));
    customized.close();
    await assert.rejects(
      setup({ database, directory: path.join(directory, 'setup') }),
      /customized/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
