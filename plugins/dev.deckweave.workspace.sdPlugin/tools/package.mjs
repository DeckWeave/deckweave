// SPDX-License-Identifier: GPL-3.0-or-later
// Reproducible ZIP (stored entries, fixed 1980 timestamp), no third-party tools.
import { readFile, readdir, lstat, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const excluded = new Set(['.git', 'node_modules', 'dist', '.env']);
async function list(directory, relative = '') {
  const result = [];
  for (const name of (await readdir(directory)).sort()) {
    if (excluded.has(name) || name.startsWith('.env.')) continue;
    const absolute = path.join(directory, name),
      rel = relative ? `${relative}/${name}` : name;
    const stat = await lstat(absolute);
    if (stat.isSymbolicLink()) throw new Error(`Symlinks are not packaged: ${rel}`);
    if (stat.isDirectory()) result.push(...(await list(absolute, rel)));
    else if (stat.isFile()) result.push(rel);
    else throw new Error(`Unsupported file: ${rel}`);
  }
  return result;
}
export async function validate() {
  const manifest = JSON.parse(await readFile(path.join(root, 'manifest.json'), 'utf8'));
  if (!/^[a-z][a-z0-9]*(?:\.[a-z][a-z0-9_-]*){2,}$/.test(manifest.UUID ?? ''))
    throw new Error('Manifest needs reverse-DNS UUID');
  if (
    typeof manifest.Name !== 'string' ||
    !manifest.Name.trim() ||
    !/^\d+\.\d+\.\d+(?:[-+][a-zA-Z0-9.-]+)?$/.test(manifest.Version ?? '')
  )
    throw new Error('Manifest needs Name and semantic Version');
  const files = await list(root),
    set = new Set(files);
  const entry = (value, label) => {
    if (
      typeof value !== 'string' ||
      value.includes('\\') ||
      value.startsWith('/') ||
      value.split('/').some((p) => p === '..' || p === '.') ||
      !set.has(value)
    )
      throw new Error(`${label} must name a bundled relative file: ${value}`);
  };
  entry(manifest.CodePath, 'CodePath');
  if (manifest.PropertyInspectorPath)
    entry(manifest.PropertyInspectorPath, 'PropertyInspectorPath');
  if (manifest.DeckWeave?.RequiredFiles !== undefined) {
    if (!Array.isArray(manifest.DeckWeave.RequiredFiles))
      throw new Error('DeckWeave.RequiredFiles must be an array');
    for (const file of manifest.DeckWeave.RequiredFiles) entry(file, 'DeckWeave.RequiredFiles');
  }
  for (const required of ['LICENSE', 'lib/sdk.mjs', 'lib/SDK-VERSION'])
    if (!set.has(required)) throw new Error(`Missing ${required}`);
  if (!Array.isArray(manifest.Actions) || !manifest.Actions.length)
    throw new Error('Manifest has no actions');
  const ids = new Set();
  for (const action of manifest.Actions) {
    if (
      typeof action.UUID !== 'string' ||
      !action.UUID.startsWith(manifest.UUID + '.') ||
      ids.has(action.UUID)
    )
      throw new Error('Action UUID must be unique and inside plugin namespace');
    ids.add(action.UUID);
    if (typeof action.Name !== 'string' || !action.Name.trim())
      throw new Error('Action needs a name');
    if (
      !Array.isArray(action.Controllers) ||
      !action.Controllers.length ||
      action.Controllers.some((c) => !['Keypad', 'Encoder', 'Panel'].includes(c))
    )
      throw new Error('Unsupported Controllers');
    if (action.PropertyInspectorPath) entry(action.PropertyInspectorPath, 'PropertyInspectorPath');
  }
  return { manifest, files };
}
const table = Array.from({ length: 256 }, (_, n) => {
  for (let k = 0; k < 8; k++) n = n & 1 ? 0xedb88320 ^ (n >>> 1) : n >>> 1;
  return n >>> 0;
});
function crc32(data) {
  let n = 0xffffffff;
  for (const b of data) n = table[(n ^ b) & 255] ^ (n >>> 8);
  return (n ^ 0xffffffff) >>> 0;
}
export function zip(entries) {
  const local = [],
    central = [];
  let offset = 0;
  for (const { name, data } of entries) {
    const nameBuffer = Buffer.from(name),
      crc = crc32(data);
    if (data.length > 0xffffffff || offset > 0xffffffff || nameBuffer.length > 65535)
      throw new Error('ZIP64 is not supported');
    const header = Buffer.alloc(30);
    header.writeUInt32LE(0x04034b50, 0);
    header.writeUInt16LE(20, 4);
    header.writeUInt16LE(0x0800, 6);
    header.writeUInt16LE(33, 12);
    header.writeUInt32LE(crc, 14);
    header.writeUInt32LE(data.length, 18);
    header.writeUInt32LE(data.length, 22);
    header.writeUInt16LE(nameBuffer.length, 26);
    const record = Buffer.alloc(46);
    record.writeUInt32LE(0x02014b50, 0);
    record.writeUInt16LE(20, 4);
    record.writeUInt16LE(20, 6);
    record.writeUInt16LE(0x0800, 8);
    record.writeUInt16LE(33, 14);
    record.writeUInt32LE(crc, 16);
    record.writeUInt32LE(data.length, 20);
    record.writeUInt32LE(data.length, 24);
    record.writeUInt16LE(nameBuffer.length, 28);
    record.writeUInt32LE(offset, 42);
    local.push(header, nameBuffer, data);
    central.push(record, nameBuffer);
    offset += header.length + nameBuffer.length + data.length;
  }
  if (entries.length > 65535) throw new Error('Too many ZIP entries');
  const directory = Buffer.concat(central),
    end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...local, directory, end]);
}
async function run() {
  const command = process.argv[2];
  if (!['validate', 'pack'].includes(command))
    throw new Error('Usage: node tools/package.mjs validate|pack');
  const { manifest, files } = await validate();
  console.log(
    `Valid ${manifest.UUID} ${manifest.Version}: ${manifest.Actions.length} action(s), ${files.length} files`,
  );
  if (command === 'validate') return;
  const entries = [];
  for (const file of files)
    entries.push({
      name: `${manifest.UUID}.sdPlugin/${file}`,
      data: await readFile(path.join(root, file)),
    });
  const bytes = zip(entries),
    digest = createHash('sha256').update(bytes).digest('hex'),
    name = `${manifest.UUID}-${manifest.Version}.deckweave-plugin.zip`;
  await mkdir(path.join(root, 'dist'), { recursive: true });
  await writeFile(path.join(root, 'dist', name), bytes);
  await writeFile(path.join(root, 'dist', name + '.sha256'), `${digest}  ${name}\n`);
  console.log(`dist/${name}\nSHA-256 ${digest}`);
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
  run().catch((e) => {
    console.error(e.message);
    process.exitCode = 1;
  });
