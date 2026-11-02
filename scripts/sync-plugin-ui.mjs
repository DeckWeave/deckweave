import { readFile, writeFile } from 'node:fs/promises';
const style = await readFile(new URL('../assets/branding/inspector.css', import.meta.url), 'utf8');
const theme = await readFile(
  new URL('../assets/branding/inspector-theme.js', import.meta.url),
  'utf8',
);
for (const name of ['audio', 'diagnostics', 'monitor', 'obs', 'timers', 'weather']) {
  const root = new URL(`../plugins/dev.deckweave.${name}.sdPlugin/`, import.meta.url);
  const file = new URL('settings.html', root);
  let html = await readFile(file, 'utf8');
  if (!html.includes('src="inspector-theme.js"'))
    html = html.replace(/<script(?=[\s>])/, '<script src="inspector-theme.js"></script>\n<script');
  if (!html.includes('DeckWeaveAppearance.apply'))
    html = html.replace(
      /((?:function connectElgatoStreamDeckSocket|window\.connectElgatoStreamDeckSocket\s*=\s*)[^\{]+\{\r?\n)/,
      '$1      DeckWeaveAppearance.apply(JSON.parse(info).deckweave);\n',
    );
  if (!html.includes('DeckWeaveAppearance.receive'))
    html = html.replace(
      /(const (m|message) = JSON\.parse\(data\);)/,
      '$1\n        DeckWeaveAppearance.receive($2);',
    );
  if (!html.includes('DeckWeaveAppearance.subscribe'))
    html = html.replace(
      /(socket = new WebSocket\([^;]+;)/,
      '$1\n      DeckWeaveAppearance.subscribe(socket, uuid);',
    );
  if (!html.includes('href="inspector.css"'))
    html = html.replace('</style>', '</style>\n  <link rel="stylesheet" href="inspector.css" />');
  if (!html.includes('class="inspector-help"'))
    html = html.replace(
      /<p id="(hint|help)">([\s\S]*?)<\/p>/g,
      '<details class="inspector-help"><summary><span class="help-ru">Справка</span><span class="help-en">Help</span></summary><p id="$1">$2</p></details>',
    );
  await writeFile(file, html);
  await writeFile(new URL('inspector.css', root), style);
  await writeFile(new URL('inspector-theme.js', root), theme);
}
console.log('Synced compact inspectors for six DeckWeave plugins.');
