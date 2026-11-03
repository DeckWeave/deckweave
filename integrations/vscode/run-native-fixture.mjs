// Launch a separate VS Code test process with isolated data and only two local extension copies.
import { cp, mkdir, readdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
const integration = dirname(fileURLToPath(import.meta.url));
const root = resolve(integration, '../..');
const fixture = join(root, '.local', `workspace-vscode-fixture-${Date.now()}`);
const sourceExtensions = join(process.env.USERPROFILE, '.vscode', 'extensions');
const code = join(process.env.LOCALAPPDATA, 'Programs', 'Microsoft VS Code', 'Code.exe');
const names = await readdir(sourceExtensions);
await mkdir(join(fixture, 'extensions'), { recursive: true });
await mkdir(join(fixture, 'user-data', 'User'), { recursive: true });
await mkdir(join(fixture, 'document'), { recursive: true });
await mkdir(join(fixture, 'portable'), { recursive: true });
for (const prefix of ['local.typst-typewriter-', 'myriad-dreamin.tinymist-']) {
  const name = names
    .filter((item) => item.startsWith(prefix))
    .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }))
    .at(-1);
  if (!name) throw new Error(`Installed extension missing: ${prefix}`);
  await cp(join(sourceExtensions, name), join(fixture, 'extensions', name), { recursive: true });
}
await writeFile(
  join(fixture, 'user-data', 'User', 'settings.json'),
  JSON.stringify({
    'telemetry.telemetryLevel': 'off',
    'update.mode': 'none',
    'workbench.startupEditor': 'none',
    'window.restoreWindows': 'none',
    'extensions.autoUpdate': false,
    'extensions.autoCheckUpdates': false,
    'tinymist.preview.browsing': false,
  }),
);
const document = join(fixture, 'document', 'main.typ');
await writeFile(document, '= Disposable integration fixture\n\nA fixture for hardware commands.\n');
const result = join(fixture, 'result.json');
const child = spawn(
  code,
  [
    join(fixture, 'document'),
    '--new-window',
    '--disable-gpu',
    '--in-process-gpu',
    '--disable-workspace-trust',
    '--skip-welcome',
    '--skip-release-notes',
    `--user-data-dir=${join(fixture, 'user-data')}`,
    `--extensions-dir=${join(fixture, 'extensions')}`,
    `--extensionDevelopmentPath=${join(integration, 'fixtures', 'host')}`,
    `--extensionTestsPath=${join(integration, 'native-fixture.cjs')}`,
  ],
  {
    shell: false,
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: {
      ...process.env,
      VSCODE_PORTABLE: join(fixture, 'portable'),
      DECKWEAVE_FIXTURE_RESULT: result,
      DECKWEAVE_FIXTURE_DOCUMENT: document,
      DECKWEAVE_FIXTURE_HELPER:
        process.env.DECKWEAVE_FIXTURE_HELPER ||
        join(root, 'plugins/dev.deckweave.workspace.sdPlugin/bin/deckweave-workspace.exe'),
    },
  },
);
child.stdout.pipe(process.stdout);
child.stderr.pipe(process.stderr);
const exit = await new Promise((resolveExit, reject) => {
  child.on('error', reject);
  child.on('exit', resolveExit);
});
console.log(`Fixture evidence: ${result}`);
process.exitCode = exit || 0;
