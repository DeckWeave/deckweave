// The original DeckWeave artwork is the canonical application identity.
// Copy tracked assets rather than synthesizing a different logo during builds.
import { copyFile, mkdir } from 'node:fs/promises';
const source = new URL('../assets/branding/', import.meta.url);
const destination = new URL('../apps/desktop/src-tauri/icons/', import.meta.url);
await mkdir(destination, { recursive: true });
for (const name of ['icon.png', 'icon.ico', 'icon.icns']) {
  await copyFile(new URL(name, source), new URL(name, destination));
}
console.log('Copied original DeckWeave application icons.');
