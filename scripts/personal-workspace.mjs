// SPDX-License-Identifier: GPL-3.0-or-later
// Offline personal setup: stage all Core commands, then publish one guarded SQLite update.
import { DatabaseSync, backup } from 'node:sqlite';
import { spawn, execFileSync } from 'node:child_process';
import { createInterface } from 'node:readline';
import { readFile, writeFile, mkdir, access } from 'node:fs/promises';
import { createHash, randomUUID } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = fileURLToPath(new URL('../', import.meta.url));
const canonical = (value) =>
  Array.isArray(value)
    ? value.map(canonical)
    : value && typeof value === 'object'
      ? Object.fromEntries(
          Object.keys(value)
            .sort()
            .map((k) => [k, canonical(value[k])]),
        )
      : value;
const digest = (value) =>
  createHash('sha256')
    .update(JSON.stringify(canonical(value)))
    .digest('hex');
const profileDigest = (profile) => digest({ ...profile, active_page_id: '' });
const profiles = (d) => [d.profile, ...d.profiles];
const getProfile = (d, id) => profiles(d).find((p) => p.id === id);
const read = (db) => {
  const r = db.prepare('SELECT revision,json FROM configuration WHERE id=1').get();
  const c = JSON.parse(r.json);
  if (c.format_version !== 3 || c.revision !== r.revision)
    throw Error('Unsupported configuration or inconsistent revision');
  return c;
};
function assertStopped() {
  const output = execFileSync(
    'powershell.exe',
    [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      '$ErrorActionPreference="Stop"; @(Get-Process | Where-Object ProcessName -eq "deckweave-desktop").Count',
    ],
    { encoding: 'utf8', timeout: 15000 },
  ).trim();
  if (output !== '0')
    throw Error(
      'Close DeckWeave before publishing the personal workspace; no configuration was changed',
    );
}
async function driver(file) {
  const binary = path.join(ROOT, 'target/debug/deckweave-config.exe');
  await access(binary);
  const child = spawn(binary, [file], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
  let stderr = '',
    closed = false;
  child.stderr.on('data', (b) => {
    stderr = (stderr + b).slice(-8000);
  });
  const lines = createInterface({ input: child.stdout });
  const queue = [],
    waiting = [];
  function accept(value) {
    if (waiting.length) waiting.shift().resolve(value);
    else queue.push(value);
  }
  lines.on('line', (line) => {
    try {
      accept(JSON.parse(line));
    } catch {
      child.kill();
    }
  });
  const failure = (error) => {
    closed = true;
    while (waiting.length) waiting.shift().reject(error);
  };
  child.on('error', failure);
  child.on('exit', (code) => failure(Error(`Offline Core exited (${code}): ${stderr}`)));
  function next() {
    if (queue.length) return Promise.resolve(queue.shift());
    if (closed) return Promise.reject(Error(`Offline Core closed: ${stderr}`));
    return new Promise((resolve, reject) => waiting.push({ resolve, reject }));
  }
  let first = await next();
  if (!first.ok) throw Error(first.error);
  let snapshot = first.snapshot;
  return {
    get configuration() {
      return snapshot.configuration;
    },
    async send(command) {
      child.stdin.write(
        JSON.stringify({ ...command, expected_revision: snapshot.configuration.revision }) + '\n',
      );
      const response = await next();
      if (!response.ok) throw Error(response.error);
      snapshot = response.snapshot;
      return snapshot.configuration;
    },
    async close() {
      child.stdin.end();
      if (!closed) await new Promise((resolve) => child.once('exit', resolve));
    },
  };
}

export const pageSpec = [
  [
    'Главная',
    [
      ['VS Code', 'launch.code'],
      ['Codex', 'launch.codex'],
      ['Браузер', 'launch.browser'],
      ['Telegram', 'launch.telegram'],
      ['Телемост', 'launch.telemost'],
      ['Zotero', 'launch.zotero'],
      ['Отчёты', 'page:1'],
      ['Codex · команды', 'page:2'],
      ['Браузер · команды', 'page:3'],
      ['Нагрузка', 'page:4'],
    ],
  ],
  [
    'Отчёты',
    [
      ['Превью', 'report.preview'],
      ['PDF', 'report.pdf'],
      ['Структура', 'report.outline'],
      ['Формула', 'report.formula'],
      ['Ссылка', 'report.reference'],
      ['Рисунок', 'report.image'],
      ['Таблица', 'report.table'],
      ['Жирный', 'report.bold'],
      ['Курсив', 'report.italic'],
      ['Главная', 'page:0'],
    ],
  ],
  [
    'Codex',
    [
      ['Codex', 'launch.codex'],
      ['Команды', 'codex.commandMenu'],
      ['Найти', 'codex.find'],
      ['Предыдущий чат', 'codex.previousChat'],
      ['Следующий чат', 'codex.nextChat'],
      ['Терминал', 'codex.terminal'],
      ['VS Code · Codex', 'vscode.codexSidebar'],
      ['VS Code · новый чат', 'vscode.codexNewChat'],
      ['Добавить выделение', 'vscode.codexAddSelection'],
      ['Главная', 'page:0'],
    ],
  ],
  [
    'Браузер',
    [
      ['Назад', 'browser.back'],
      ['Вперёд', 'browser.forward'],
      ['Обновить', 'browser.reload'],
      ['Новая вкладка', 'browser.newTab'],
      ['Предыдущая вкладка', 'browser.previousTab'],
      ['Следующая вкладка', 'browser.nextTab'],
      ['Адрес / поиск', 'browser.address'],
      ['Найти на странице', 'browser.find'],
      ['Масштаб 100%', 'browser.zoomReset'],
      ['Главная', 'page:0'],
    ],
  ],
  [
    'Нагрузка',
    [
      ['CPU', 'metric:cpu'],
      ['RAM', 'metric:memory'],
      ['GPU', 'metric:gpu'],
      ['VRAM', 'metric:gpu-memory'],
      ['Помодоро', 'focus:toggle'],
      ['Пауза', 'focus:pause'],
      ['Сброс', 'focus:reset'],
      ['Следующий этап', 'focus:skip'],
      ['Время работы', 'metric:uptime'],
      ['Главная', 'page:0'],
    ],
  ],
];
export const focusSettings = {
  workspace: 'personal',
  workMinutes: 25,
  shortMinutes: 5,
  longMinutes: 15,
  rounds: 4,
  sound: 'complete',
  volume: 35,
  language: 'ru',
  holdReset: false,
};
export const stripSpec = [
  ['weather', 'Погода', [0, 0, 0.375, 1]],
  ['clock', 'Дата и время', [0.375, 0, 0.275, 1]],
  ['focus', 'Помодоро', [0.65, 0, 0.35, 1]],
];
function nestedPlugin(operation, label) {
  return {
    id: randomUUID().replaceAll('-', ''),
    kind: 'plugin',
    label,
    plugin: {
      package: 'dev.deckweave.workspace',
      action: 'dev.deckweave.workspace.command',
      controller: 'Keypad',
      settings: { operation, label, language: 'ru' },
    },
    plan: null,
  };
}
export async function stageWorkspace(core, deviceId, weather) {
  const send = (command) => core.send({ device_id: deviceId, ...command });
  const device = () => core.configuration.devices.find((d) => d.id === deviceId);
  await send({ type: 'add_profile', name: 'Рабочий стол' });
  const profileId = device().profile.id;
  for (let i = 1; i < pageSpec.length; i++) await send({ type: 'add_page' });
  const pageIds = device().profile.pages.map((p) => p.id);
  const plugin = (page_id, control_id, packageId, action, settings, label, controller = 'Keypad') =>
    send({
      type: 'configure_plugin',
      page_id,
      control_id,
      label,
      plugin: { package: packageId, action, settings, controller },
    });
  for (let index = 0; index < pageSpec.length; index++) {
    const [name, keys] = pageSpec[index],
      page_id = pageIds[index];
    await send({ type: 'rename_page', page_id, name });
    for (let k = 0; k < keys.length; k++) {
      const [label, operation] = keys[k],
        control_id = `key-${k + 1}`;
      if (operation.startsWith('page:'))
        await send({
          type: 'configure_program',
          page_id,
          control_id,
          label,
          plan: { type: 'page', page_id: pageIds[Number(operation.slice(5))] },
        });
      else if (operation.startsWith('metric:'))
        await plugin(
          page_id,
          control_id,
          'dev.deckweave.monitor',
          `dev.deckweave.monitor.${operation.slice(7)}`,
          { label, language: 'ru' },
          label,
        );
      else if (operation.startsWith('focus:'))
        await plugin(
          page_id,
          control_id,
          'dev.deckweave.focus',
          operation === 'focus:toggle'
            ? 'dev.deckweave.focus.pomodoro'
            : 'dev.deckweave.focus.control',
          { ...focusSettings, operation: operation.slice(6), label },
          label,
        );
      else
        await plugin(
          page_id,
          control_id,
          'dev.deckweave.workspace',
          'dev.deckweave.workspace.command',
          { operation, label, language: 'ru' },
          label,
        );
    }
    const regions = stripSpec.map(([kind, name, rect]) => ({
      id: randomUUID().replaceAll('-', ''),
      name,
      rect,
      dials: [],
      kind,
    }));
    await send({
      type: 'set_panel_layout',
      page_id,
      layout: { regions: regions.map(({ kind, ...r }) => r), swipe_pages: true },
    });
    for (const r of regions) {
      if (r.kind === 'weather')
        await plugin(
          page_id,
          `panel-${r.id}`,
          'dev.deckweave.weather',
          'dev.deckweave.weather.current',
          weather,
          'Погода',
          'Panel',
        );
      if (r.kind === 'clock')
        await plugin(
          page_id,
          `panel-${r.id}`,
          'dev.deckweave.clock',
          'dev.deckweave.clock.digital',
          { language: 'ru', timezone: 'Asia/Yekaterinburg' },
          'Дата и время',
          'Panel',
        );
      if (r.kind === 'focus')
        await plugin(
          page_id,
          `panel-${r.id}`,
          'dev.deckweave.focus',
          'dev.deckweave.focus.pomodoro',
          focusSettings,
          'Помодоро',
          'Panel',
        );
    }
    await plugin(
      page_id,
      'dial-1',
      'dev.deckweave.audio',
      'dev.deckweave.audio.output',
      { language: 'ru', step: 2 },
      'Громкость',
      'Encoder',
    );
    await plugin(
      page_id,
      'dial-2',
      'dev.deckweave.focus',
      'dev.deckweave.focus.pomodoro',
      focusSettings,
      'Помодоро',
      'Encoder',
    );
    if (index === 3)
      await send({
        type: 'configure_program',
        page_id,
        control_id: 'dial-3',
        label: 'Масштаб',
        plan: {
          type: 'dial_bindings',
          left: nestedPlugin('browser.zoomOut', 'Уменьшить'),
          right: nestedPlugin('browser.zoomIn', 'Увеличить'),
          press: nestedPlugin('browser.zoomReset', '100%'),
          tap: null,
        },
      });
    await send({
      type: 'configure_program',
      page_id,
      control_id: 'dial-4',
      label: 'Страницы',
      plan: { type: 'page_cycle', direction: 'dial' },
    });
  }
  await send({ type: 'select_page', page_id: pageIds[0] });
  await send({ type: 'device_preferences', default_profile: profileId });
  return { profileId, pageIds };
}

export function assertPreserved(before, after, deviceId, ownedId) {
  const old = before.devices.find((d) => d.id === deviceId),
    current = after.devices.find((d) => d.id === deviceId);
  for (const d of before.devices.filter((d) => d.id !== deviceId)) {
    if (digest(d) !== digest(after.devices.find((x) => x.id === d.id)))
      throw Error(`Unrelated device changed: ${d.id}`);
  }
  for (const p of profiles(old))
    if (digest(p) !== digest(getProfile(current, p.id)))
      throw Error(`Existing profile changed: ${p.id}`);
  const normalize = (d) => ({ ...d, profile: null, profiles: [], default_profile: null });
  if (digest(normalize(old)) !== digest(normalize(current)))
    throw Error('Hardware, pinned bindings or unrelated device settings changed');
  if (digest(before.preferences) !== digest(after.preferences))
    throw Error('Application preferences changed');
  if (profiles(current).length !== profiles(old).length + 1 || !getProfile(current, ownedId))
    throw Error('Unexpected added profile set');
}

export async function setup({
  database,
  apply = false,
  directory = path.join(ROOT, '.local/personal-workspace'),
} = {}) {
  database ??= path.join(process.env.APPDATA, 'app.deckweave.studio/deckweave.sqlite3');
  if (apply) assertStopped();
  await mkdir(directory, { recursive: true });
  const source = new DatabaseSync(database, { readOnly: true });
  try {
    const before = read(source);
    const device = before.devices.find((d) => d.provider_id === 'deckweave.n4');
    if (!device) throw Error('Mirabox is not configured');
    if (Object.keys(device.pinned).length)
      throw Error(
        'Existing global pinned actions need an explicit layout decision; no changes made',
      );
    const manifestFile = path.join(directory, 'installed.json');
    let installed;
    try {
      installed = JSON.parse(await readFile(manifestFile, 'utf8'));
    } catch (e) {
      if (e.code !== 'ENOENT') throw e;
    }
    if (installed) {
      const p = getProfile(device, installed.profileId);
      if (!p)
        throw Error(
          'Previously installed profile was removed; archive the setup manifest before creating a new one',
        );
      if (profileDigest(p) !== installed.profileDigest)
        throw Error('Workspace was customized; keeping your changes. No automatic replacement');
      source.close();
      return { unchanged: true, profileId: p.id, database };
    }
    if (profiles(device).some((p) => p.name === 'Рабочий стол'))
      throw Error('A profile with this name exists without setup ownership; no replacement');
    const weather = profiles(device)
      .flatMap((p) => p.pages.flatMap((pg) => Object.values(pg.bindings)))
      .find(
        (b) =>
          b.plugin?.package === 'dev.deckweave.weather' &&
          Number.isFinite(b.plugin.settings.latitude),
      )?.plugin.settings;
    if (!weather) throw Error('Choose a weather city on Mirabox before setup');
    const run = path.join(directory, new Date().toISOString().replaceAll(':', '-'));
    await mkdir(run);
    const snapshot = path.join(run, 'configuration-before.sqlite3');
    await backup(source, snapshot);
    const stage = path.join(run, 'configuration-staged.sqlite3');
    await backup(source, stage);
    source.close();
    for (const name of ['host-services.sqlite3']) {
      const file = path.join(path.dirname(database), name);
      try {
        await access(file);
        const db = new DatabaseSync(file, { readOnly: true });
        try {
          await backup(db, path.join(run, name));
        } finally {
          db.close();
        }
      } catch (e) {
        if (e.code !== 'ENOENT') throw e;
      }
    }
    const core = await driver(stage);
    let ids;
    try {
      ids = await stageWorkspace(core, device.id, weather);
    } finally {
      await core.close();
    }
    const staged = new DatabaseSync(stage, { readOnly: true });
    const after = read(staged);
    staged.close();
    assertPreserved(before, after, device.id, ids.profileId);
    const report = {
      schema: 1,
      setup: 'personal-workspace',
      database,
      deviceId: device.id,
      ...ids,
      previousActiveProfile: device.profile.id,
      previousDefault: device.default_profile,
      preRevision: before.revision,
      stagedRevision: after.revision,
      snapshot,
      serviceSnapshot: path.join(run, 'host-services.sqlite3'),
      profileDigest: profileDigest(
        getProfile(
          after.devices.find((d) => d.id === device.id),
          ids.profileId,
        ),
      ),
      profileName: 'Рабочий стол',
      pages: pageSpec.map((p) => p[0]),
      apply,
    };
    await writeFile(path.join(run, 'report.json'), JSON.stringify(report, null, 2));
    if (!apply) return { ...report, stagedDatabase: stage };
    assertStopped();
    const live = new DatabaseSync(database);
    try {
      live.exec('BEGIN IMMEDIATE');
      const current = read(live);
      if (digest(current) !== digest(before))
        throw Error('Configuration changed during staging; no update published');
      // Staging may have many revisions. Publishing is a single logical mutation.
      after.revision = before.revision + 1;
      const changed = live
        .prepare('UPDATE configuration SET revision=?,json=? WHERE id=1 AND revision=?')
        .run(after.revision, JSON.stringify(after), before.revision);
      if (changed.changes !== 1) throw Error('Stale configuration revision');
      live.exec('COMMIT');
    } catch (e) {
      try {
        live.exec('ROLLBACK');
      } catch {}
      throw e;
    } finally {
      live.close();
    }
    report.postRevision = after.revision;
    await writeFile(manifestFile, JSON.stringify(report, null, 2));
    await writeFile(path.join(run, 'report.json'), JSON.stringify(report, null, 2));
    return report;
  } finally {
    if (source.isOpen) source.close();
  }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2),
    at = args.indexOf('--database');
  console.log(
    JSON.stringify(
      await setup({
        apply: args.includes('--apply'),
        database: at >= 0 ? args[at + 1] : undefined,
      }),
      null,
      2,
    ),
  );
}
