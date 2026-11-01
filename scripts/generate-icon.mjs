// Reproducible code-drawn app icon; no external image or font dependency.
import { mkdir, writeFile } from 'node:fs/promises';
import { deflateSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';

function crc32(data) {
  let crc = 0xffffffff;
  for (const byte of data) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const name = Buffer.from(type);
  const result = Buffer.alloc(12 + data.length);
  result.writeUInt32BE(data.length);
  name.copy(result, 4);
  data.copy(result, 8);
  result.writeUInt32BE(crc32(Buffer.concat([name, data])), 8 + data.length);
  return result;
}
const size = 256;
const pixels = Buffer.alloc((size * 4 + 1) * size);
for (let y = 0; y < size; y++)
  for (let x = 0; x < size; x++) {
    const offset = y * (size * 4 + 1) + 1 + x * 4;
    const dx = Math.max(38 - x, 0, x - 217),
      dy = Math.max(38 - y, 0, y - 217);
    const opaque = dx * dx + dy * dy <= 34 * 34 && x >= 4 && x <= 251 && y >= 4 && y <= 251;
    const outer =
      x >= 62 &&
      y >= 50 &&
      y <= 205 &&
      (x <= 116 || ((x - 116) / 80) ** 2 + ((y - 128) / 78) ** 2 <= 1);
    const inner =
      x >= 102 &&
      y >= 85 &&
      y <= 170 &&
      (x <= 119 || ((x - 119) / 39) ** 2 + ((y - 128) / 43) ** 2 <= 1);
    const color = outer && !inner ? [196, 219, 255] : [39, 62, 96];
    pixels[offset] = color[0];
    pixels[offset + 1] = color[1];
    pixels[offset + 2] = color[2];
    pixels[offset + 3] = opaque ? 255 : 0;
  }
const header = Buffer.alloc(13);
header.writeUInt32BE(size);
header.writeUInt32BE(size, 4);
header[8] = 8;
header[9] = 6;
const png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk('IHDR', header),
  chunk('IDAT', deflateSync(pixels)),
  chunk('IEND', Buffer.alloc(0)),
]);
const icoHeader = Buffer.alloc(22);
icoHeader.writeUInt16LE(1, 2);
icoHeader.writeUInt16LE(1, 4);
icoHeader.writeUInt16LE(1, 10);
icoHeader.writeUInt16LE(32, 12);
icoHeader.writeUInt32LE(png.length, 14);
icoHeader.writeUInt32LE(22, 18);
const folder = fileURLToPath(new URL('../apps/desktop/src-tauri/icons/', import.meta.url));
await mkdir(folder, { recursive: true });
await writeFile(`${folder}/icon.png`, png);
await writeFile(`${folder}/icon.ico`, Buffer.concat([icoHeader, png]));
console.log('Generated DeckWeave PNG/ICO.');
