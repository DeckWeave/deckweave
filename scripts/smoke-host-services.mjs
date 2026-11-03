// Opt-in native integration + coarse debug-build process-tree resource sample.
// No user database or hardware access; the debug driver rejects other directories.
import { cp, mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { spawn, execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import os from 'node:os';
const root = fileURLToPath(new URL('../', import.meta.url));
await mkdir(path.join(root, '.local'), { recursive: true });
const data = await mkdtemp(path.join(root, '.local', 'host-services-smoke-'));
// Real Workspace UI/SDK/host handlers; only OS launch is replaced. Never open a
// personal app or document merely to exercise an error/success transition.
const workspaceBundle = path.join(data, 'plugins/dev.deckweave.workspace.sdPlugin');
await cp(path.join(root, 'plugins/dev.deckweave.workspace.sdPlugin'), workspaceBundle, {
  recursive: true,
  filter: (p) => !p.split(path.sep).includes('dist'),
});
const workspaceManifest = JSON.parse(
  await readFile(path.join(workspaceBundle, 'manifest.json'), 'utf8'),
);
workspaceManifest.CodePath = 'smoke-entry.mjs';
await writeFile(path.join(workspaceBundle, 'manifest.json'), JSON.stringify(workspaceManifest));
await writeFile(
  path.join(workspaceBundle, 'smoke-entry.mjs'),
  `import {start} from './plugin.mjs';\nlet calls=0;\nstart(undefined,{async request(){calls++;if(calls===1)throw Error('Deliberate target window unavailable');},close(){}});\n`,
);
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
    new Promise((_, reject) => {
      setTimeout(() => reject(Error('Smoke exit timed out')), 60000).unref();
    }),
  ]);
  const report = JSON.parse(await readFile(path.join(data, 'result.json'), 'utf8'));
  // Refuse an old executable's successful report: this driver requires the actual
  // authenticated Focus SDK/hidden-profile path as well as the previous cases.
  const requiredAssertions = [
    'authenticated_monitor_panel',
    'monitor_survived_profile_change',
    'closed_note_settings_updated_without_reopening',
    'closed_timer_finished',
    'silent_windows_playback_finished',
    'authenticated_focus_sdk_appearance',
    'focus_started_via_core_input',
    'focus_owner_retained_while_hidden',
    'focus_completed_one_phase_while_hidden',
    'focus_silent_windows_playback_finished',
    'focus_profile_return_preserved_job',
    'focus_manual_next_phase_preserved',
    'focus_no_desktop_overlay',
    'workspace_error_artwork_visible',
    'workspace_success_restored_artwork',
    'workspace_assignments_preserved',
  ];
  const validationErrors = requiredAssertions
    .filter((name) => report[name] !== true)
    .map((name) => `Missing or failed native assertion: ${name}`);
  const focus = report.focus_evidence;
  if (
    !focus ||
    focus.owner !== 'dev.deckweave.focus' ||
    focus.workspace !== 'services-smoke-focus' ||
    typeof focus.run_id !== 'string' ||
    focus.phase !== 'short_break' ||
    focus.status !== 'ready' ||
    focus.round !== 1 ||
    focus.transition !== 1 ||
    focus.remaining_ms !== 1000 ||
    focus.config?.work_seconds !== 1 ||
    focus.config?.short_seconds !== 1 ||
    focus.config?.long_seconds !== 1 ||
    focus.config?.rounds !== 2 ||
    focus.config?.sound !== 'notice' ||
    focus.config?.volume !== 0 ||
    focus.matching_jobs !== 1 ||
    focus.sound_event_id !== `focus-${focus.run_id}-1` ||
    focus.sound_queued_count !== 1 ||
    focus.sound_finished_count !== 1 ||
    focus.open_overlays !== 0 ||
    focus.added_panels !== 0
  )
    validationErrors.push(
      'Native Focus evidence did not preserve one completed manual-start phase',
    );
  const result = {
    build: 'debug; production web assets; coarse process-tree sample, not a release budget',
    logicalProcessors: os.availableParallelism(),
    ...report,
    validationErrors,
    samples,
  };
  await writeFile(path.join(data, 'resources.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ data, code, ...report, validationErrors }));
  if (code !== 0 || report.error || validationErrors.length) process.exitCode = 1;
} finally {
  if (child.exitCode === null) child.kill();
}
