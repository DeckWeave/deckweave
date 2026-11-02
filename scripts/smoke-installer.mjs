// SPDX-License-Identifier: GPL-3.0-or-later
// Opt-in native integration. Close other DeckWeave instances before running.
import { cp, mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync, spawn } from 'node:child_process';
const root = fileURLToPath(new URL('../', import.meta.url));
await mkdir(path.join(root, '.local'), { recursive: true });
const run = await mkdtemp(path.join(root, '.local', 'installer-smoke-'));
const fixtures = path.join(run, 'fixtures'),
  data = path.join(run, 'data');
await mkdir(fixtures);
await mkdir(data);
const id = 'dev.deckweave.installsmoke';
for (const version of [1, 2]) {
  const bundle = path.join(fixtures, `v${version}`, `${id}.sdPlugin`);
  const source = path.join(root, 'plugins/dev.deckweave.timers.sdPlugin');
  for (const name of ['tools', 'lib'])
    await cp(path.join(source, name), path.join(bundle, name), { recursive: true });
  await writeFile(
    path.join(bundle, 'manifest.json'),
    JSON.stringify({
      UUID: id,
      Name: 'Installer regression fixture',
      Version: `${version}.0.0`,
      CodePath: 'plugin.mjs',
      Actions: [{ UUID: `${id}.action`, Name: 'Fixture', Controllers: ['Keypad'] }],
    }),
  );
  await writeFile(
    path.join(bundle, 'plugin.mjs'),
    `import {createPlugin} from './lib/sdk.mjs';\nconst p=createPlugin();\np.on('willAppear',ctx=>ctx.setTitle('v${version} '+ctx.settings.prefix));\n`,
  );
  await writeFile(path.join(bundle, 'LICENSE'), 'SPDX-License-Identifier: GPL-3.0-or-later\n');
  execFileSync(process.execPath, [path.join(bundle, 'tools/package.mjs'), 'pack'], {
    cwd: root,
    windowsHide: true,
    stdio: 'inherit',
  });
  const archive = (await readdir(path.join(bundle, 'dist'))).find((name) => name.endsWith('.zip'));
  await cp(path.join(bundle, 'dist', archive), path.join(fixtures, `v${version}.zip`));
}
const providerArchive = process.env.DECKWEAVE_SMOKE_DEVICE_ARCHIVE;
if (providerArchive) await cp(path.resolve(providerArchive), path.join(fixtures, 'provider.zip'));
const binary = path.join(
  root,
  'target/debug',
  process.platform === 'win32' ? 'deckweave-desktop.exe' : 'deckweave-desktop',
);
const child = spawn(binary, [], {
  cwd: root,
  windowsHide: true,
  stdio: 'ignore',
  env: {
    ...process.env,
    DECKWEAVE_DATA_DIR: data,
    DECKWEAVE_VIRTUAL_ONLY: '1',
    DECKWEAVE_SMOKE_INSTALLER: fixtures,
  },
});
await new Promise((resolve, reject) => {
  const deadline = setTimeout(() => {
    child.kill();
    reject(new Error('Installer smoke timed out'));
  }, 45000);
  child.once('error', (error) => {
    clearTimeout(deadline);
    reject(error);
  });
  child.once('close', (code) => {
    clearTimeout(deadline);
    code === 0 ? resolve() : reject(new Error(`Smoke exited ${code}`));
  });
});
let report;
try {
  report = JSON.parse(await readFile(path.join(data, 'installer-smoke.json'), 'utf8'));
} catch {
  throw new Error(
    `Missing result. Close other DeckWeave instances, build debug and retry. Data: ${data}`,
  );
}
console.log(JSON.stringify(report, null, 2));
if (
  report.error ||
  !report.install ||
  !report.live_update ||
  !report.settings_retained ||
  !report.disable_enable ||
  !report.uninstall_restore ||
  !report.restart_gate ||
  !report.device_remove_restore ||
  (providerArchive && !report.provider_lifecycle)
)
  process.exitCode = 1;
console.log(`Isolated data: ${data}`);
