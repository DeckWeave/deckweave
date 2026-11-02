// Verify the actual Windows PE resources, not merely the source filenames.
import { readFile, readdir } from 'node:fs/promises';
import assert from 'node:assert/strict';
const exe = await readFile(new URL('../target/release/deckweave-desktop.exe', import.meta.url));
const ico = await readFile(new URL('../assets/branding/icon.ico', import.meta.url));
const pe = exe.readUInt32LE(0x3c);
assert.equal(exe.toString('ascii', pe, pe + 4), 'PE\0\0');
const optional = pe + 24;
const directories = optional + (exe.readUInt16LE(optional) === 0x20b ? 112 : 96);
const sections = optional + exe.readUInt16LE(pe + 20);
function offset(rva) {
  for (let i = 0; i < exe.readUInt16LE(pe + 6); i++) {
    const s = sections + i * 40;
    const base = exe.readUInt32LE(s + 12);
    const size = Math.max(exe.readUInt32LE(s + 8), exe.readUInt32LE(s + 16));
    if (rva >= base && rva < base + size) return exe.readUInt32LE(s + 20) + rva - base;
  }
  throw new Error('Resource RVA outside the PE sections');
}
const root = offset(exe.readUInt32LE(directories + 16));
const icons = [];
function visit(relative, path = []) {
  const dir = root + relative;
  const count = exe.readUInt16LE(dir + 12) + exe.readUInt16LE(dir + 14);
  for (let i = 0; i < count; i++) {
    const entry = dir + 16 + i * 8;
    const name = exe.readUInt32LE(entry);
    const child = exe.readUInt32LE(entry + 4);
    const next = [...path, name];
    if (child & 0x80000000) visit(child & 0x7fffffff, next);
    else if (next[0] === 3) {
      const data = root + child;
      const start = offset(exe.readUInt32LE(data));
      icons.push(exe.subarray(start, start + exe.readUInt32LE(data + 4)));
    }
  }
}
visit(0);
for (let i = 0; i < ico.readUInt16LE(4); i++) {
  const entry = 6 + i * 16;
  const start = ico.readUInt32LE(entry + 12);
  const frame = ico.subarray(start, start + ico.readUInt32LE(entry + 8));
  assert.ok(
    icons.some((image) => image.equals(frame)),
    `Application icon frame ${i} is missing/stale in the exe`,
  );
}
console.log(`Windows executable contains all ${ico.readUInt16LE(4)} canonical icon frames.`);
const bundles = (await readdir(new URL('../apps/desktop/dist/assets/', import.meta.url))).filter(
  (name) => /\.(js|css)$/.test(name),
);
assert.ok(bundles.length >= 2, 'Build the web assets before checking the executable');
for (const name of bundles)
  assert.ok(exe.includes(Buffer.from(name)), `Stale frontend in exe: missing ${name}`);
console.log('Windows executable embeds the current JavaScript and CSS bundles.');
