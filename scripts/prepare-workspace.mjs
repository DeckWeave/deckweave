// Build native helpers and copy only generated outputs into standalone bundles.
import { spawnSync } from 'node:child_process';
import { copyFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { ROOT } from './personal-workspace.mjs';
if (process.platform !== 'win32')
  throw Error('Personal workspace application/GPU helpers currently require Windows');
if (!process.argv.includes('--skip-build')) {
  for (const args of [
    ['build', '--release', '-p', 'deckweave-platform', '--bin', 'deckweave-system', '--locked'],
    ['build', '--release', '-p', 'deckweave-workspace', '--locked'],
    ['build', '-p', 'deckweave-core', '--bin', 'deckweave-config', '--locked'],
  ]) {
    const run = spawnSync('cargo', args, { cwd: ROOT, stdio: 'inherit', windowsHide: true });
    if (run.status !== 0) throw Error(`cargo ${args.join(' ')} failed`);
  }
}
for (const [id, binary] of [
  ['audio', 'deckweave-system.exe'],
  ['monitor', 'deckweave-system.exe'],
  ['workspace', 'deckweave-workspace.exe'],
]) {
  const directory = path.join(ROOT, `plugins/dev.deckweave.${id}.sdPlugin/bin`);
  await mkdir(directory, { recursive: true });
  await copyFile(path.join(ROOT, 'target/release', binary), path.join(directory, binary));
  console.log(`${id}: ${binary}`);
}
