import { readdir, readFile, access } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const files = [];
async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (['.git', 'target', 'node_modules', '.local', 'dist', 'gen'].includes(entry.name)) continue;
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) await walk(file);
    else if (entry.name.endsWith('.md')) files.push(file);
  }
}
await walk(root);
const errors = [];
let links = 0;
for (const file of files) {
  const source = await readFile(file, 'utf8');
  for (const match of source.matchAll(/\]\((?:<([^>]+)>|([^\s)]+))(?:\s+"[^"]*")?\)/g)) {
    const target = match[1] || match[2];
    if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith('#')) continue;
    const local = decodeURIComponent(target.split('#')[0]);
    if (!local) continue;
    links++;
    try {
      await access(path.resolve(path.dirname(file), local));
    } catch {
      errors.push(`${path.relative(root, file)}: missing ${local}`);
    }
  }
}
const plan = await readFile(path.join(root, 'docs/PLAN.md'), 'utf8');
for (const line of plan.split('\n')) {
  if (/^- \[[ x~]\]/.test(line) && !line.includes(']('))
    errors.push(`PLAN task has no context link: ${line}`);
}
for (const file of files.filter((file) => path.dirname(file) === path.join(root, 'modules'))) {
  if (!plan.includes(`../modules/${path.basename(file)}`))
    errors.push(`Module missing from PLAN: ${file}`);
}
if (errors.length) {
  console.error(errors.join('\n'));
  process.exitCode = 1;
} else
  console.log(
    `Documentation OK: ${files.length} Markdown files, ${links} local links, all modules covered by PLAN.`,
  );
