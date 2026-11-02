// Opt-in native integration + coarse debug-build process-tree resource sample.
// No user database or hardware access; the debug driver rejects other directories.
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { spawn, execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import os from 'node:os';
const root = fileURLToPath(new URL('../', import.meta.url));
await mkdir(path.join(root, '.local'), { recursive: true });
const data = await mkdtemp(path.join(root, '.local', 'host-services-smoke-'));
const child = spawn(path.join(root, 'target/debug/deckweave-desktop.exe'), [], {
  cwd: root,
  windowsHide: true,
  stdio: 'ignore',
  env: {
    ...process.env,
    DECKWEAVE_DATA_DIR: data,
    DECKWEAVE_VIRTUAL_ONLY: '1',
    DECKWEAVE_SMOKE_SERVICES: '1',
  },
});
const exit = new Promise((resolve, reject) => {
  child.once('error', reject);
  child.once('close', (code) => resolve(code));
});
const pause = (ms) => new Promise((r) => setTimeout(r, ms));
const metrics = () =>
  JSON.parse(
    execFileSync(
      'powershell.exe',
      [
        '-NoProfile',
        '-File',
        path.join(root, 'scripts/process-metrics.ps1'),
        '-RootPid',
        String(child.pid),
      ],
      { windowsHide: true, encoding: 'utf8', timeout: 15000 },
    ),
  );
const total = (rows, field) => rows.reduce((n, r) => n + (r[field] ?? 0), 0);
const samples = [];
try {
  for (const stage of ['0', '1', '4', '8', 'closed']) {
    const start = Date.now();
    while (true) {
      try {
        if (JSON.parse(await readFile(path.join(data, 'stage.json'), 'utf8')).stage === stage)
          break;
      } catch {}
      if (Date.now() - start > 30000) throw Error('Waiting for stage ' + stage);
      await pause(100);
    }
    if (process.env.DECKWEAVE_SMOKE_FAST) {
      await pause(1500);
      await writeFile(path.join(data, 'continue-' + stage), 'continue');
      continue;
    }
    await pause(2500);
    const a = metrics(),
      since = performance.now();
    await pause(3000);
    const b = metrics(),
      elapsed = (performance.now() - since) / 1000;
    const cpuOneCorePercent = (100 * (total(b, 'cpuSeconds') - total(a, 'cpuSeconds'))) / elapsed;
    samples.push({
      stage,
      processes: b.length,
      privateMiB: total(b, 'privateBytes') / 2 ** 20,
      workingSetMiB: total(b, 'workingSetBytes') / 2 ** 20,
      cpuOneCorePercent,
      cpuMachinePercent: cpuOneCorePercent / os.availableParallelism(),
      details: b,
    });
    console.log(JSON.stringify(samples.at(-1)));
    await writeFile(path.join(data, 'continue-' + stage), 'continue');
  }
  const code = await Promise.race([
    exit,
    pause(20000).then(() => {
      throw Error('Smoke exit timed out');
    }),
  ]);
  const report = JSON.parse(await readFile(path.join(data, 'result.json'), 'utf8'));
  const result = {
    build: 'debug; production web assets; coarse process-tree sample, not a release budget',
    logicalProcessors: os.availableParallelism(),
    ...report,
    samples,
  };
  await writeFile(path.join(data, 'resources.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ data, code, ...report }));
  if (code !== 0 || report.error) process.exitCode = 1;
} finally {
  if (child.exitCode === null) child.kill();
}
