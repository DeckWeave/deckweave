// Fixed operations shared by the property inspector, hardware renderer and client.
export const operations = Object.freeze({
  'launch.code': ['VS Code', 'VS Code', 'code'],
  'launch.codex': ['Codex', 'Codex', 'chat'],
  'launch.browser': ['Браузер', 'Browser', 'browser'],
  'launch.telegram': ['Telegram', 'Telegram', 'chat'],
  'launch.telemost': ['Телемост', 'Telemost', 'video'],
  'launch.zotero': ['Zotero', 'Zotero', 'book'],
  'report.preview': ['Просмотр', 'Preview', 'preview'],
  'report.pdf': ['PDF', 'PDF', 'download'],
  'report.outline': ['Структура', 'Outline', 'list'],
  'report.bold': ['Жирный', 'Bold', 'bold'],
  'report.italic': ['Курсив', 'Italic', 'italic'],
  'report.formula': ['Формула', 'Formula', 'formula'],
  'report.reference': ['Ссылка', 'Reference', 'link'],
  'report.image': ['Рисунок', 'Image', 'image'],
  'report.table': ['Таблица', 'Table', 'table'],
  'report.format': ['Формат', 'Format', 'format'],
  'vscode.codexSidebar': ['Codex в Code', 'Codex in Code', 'code'],
  'vscode.codexNewChat': ['Новый в Code', 'New in Code', 'newChat'],
  'vscode.codexAddSelection': ['В контекст', 'Add selection', 'selection'],
  'browser.back': ['Назад', 'Back', 'back'],
  'browser.forward': ['Вперёд', 'Forward', 'forward'],
  'browser.reload': ['Обновить', 'Reload', 'refresh'],
  'browser.newTab': ['Новая вкладка', 'New tab', 'newTab'],
  'browser.nextTab': ['Следующая', 'Next tab', 'forward'],
  'browser.previousTab': ['Предыдущая', 'Previous tab', 'back'],
  'browser.address': ['Адрес / поиск', 'Address / search', 'search'],
  'browser.find': ['Найти', 'Find', 'search'],
  'browser.zoomIn': ['Крупнее', 'Zoom in', 'zoomIn'],
  'browser.zoomOut': ['Мельче', 'Zoom out', 'zoomOut'],
  'browser.zoomReset': ['Масштаб 100%', 'Zoom 100%', 'zoomReset'],
  'codex.commandMenu': ['Команды', 'Commands', 'list'],
  'codex.sidebar': ['Чаты', 'Sidebar', 'sidebar'],
  'codex.bottomPanel': ['Нижняя панель', 'Bottom panel', 'panel'],
  'codex.terminal': ['Терминал', 'Terminal', 'terminal'],
  'codex.find': ['Найти', 'Find', 'search'],
  'codex.nextChat': ['Следующий чат', 'Next chat', 'forward'],
  'codex.previousChat': ['Предыдущий чат', 'Previous chat', 'back'],
  'codex.newChat': ['Новый чат', 'New chat', 'newChat'],
});
export function settings(value = {}, language = 'en') {
  const operation = value.operation == null ? 'launch.code' : String(value.operation);
  const ru = String(value.language ?? language).startsWith('ru');
  const definition = operations[operation] || ['Команда', 'Command', 'code'];
  return {
    operation,
    label: String(value.label || definition[ru ? 0 : 1]).slice(0, 24),
    icon: Object.hasOwn(icons, value.icon) ? value.icon : definition[2],
    ru,
  };
}
export const icons = Object.freeze({
  code: '<path d="m9 7-6 5 6 5m6-10 6 5-6 5m-2-12-2 14"/>',
  chat: '<path d="M4 4h16v12H9l-5 4z"/>',
  browser: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c-5 6-5 12 0 18 5-6 5-12 0-18"/>',
  video: '<rect x="3" y="6" width="12" height="12" rx="2"/><path d="m15 10 6-3v10l-6-3"/>',
  book: '<path d="M3 4h6c2 0 3 1 3 2 0-1 1-2 3-2h6v15h-6c-2 0-3 1-3 2 0-1-1-2-3-2H3zM12 6v15"/>',
  preview: '<path d="M2 12c5-9 15-9 20 0-5 9-15 9-20 0z"/><circle cx="12" cy="12" r="3"/>',
  download: '<path d="M12 3v13m-5-5 5 5 5-5M4 17v4h16v-4"/>',
  list: '<path d="M8 5h13M8 12h13M8 19h13M3 5h.01M3 12h.01M3 19h.01"/>',
  bold: '<path d="M7 4h6a4 4 0 0 1 0 8H7m0-8v16h7a4 4 0 0 0 0-8H7"/>',
  italic: '<path d="M10 4h9M5 20h9M15 4 9 20"/>',
  formula: '<path d="M20 5h-8c-3 0-3 3-4 8s-1 7-4 7M5 10h11m1 3 5 7m0-7-5 7"/>',
  link: '<path d="m10 7 3-3a5 5 0 0 1 7 7l-3 3M14 17l-3 3a5 5 0 0 1-7-7l3-3m1 6 8-8"/>',
  image:
    '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8" cy="8" r="1"/><path d="m3 17 6-6 5 5 3-3 4 4"/>',
  table:
    '<rect x="3" y="3" width="18" height="18" rx="1"/><path d="M3 9h18M3 15h18M9 3v18M15 3v18"/>',
  format: '<path d="M4 5h16M4 10h10M4 15h16M4 20h10"/>',
  newChat: '<path d="M4 4h12v12H9l-5 4zM19 3v8m-4-4h8"/>',
  selection: '<path d="M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5M8 9h8M8 13h8M8 17h4"/>',
  back: '<path d="m12 4-8 8 8 8M4 12h17"/>',
  forward: '<path d="m12 4 8 8-8 8M3 12h17"/>',
  refresh: '<path d="M20 10a8 8 0 1 0 0 5M20 3v7h-7"/>',
  newTab: '<rect x="3" y="4" width="18" height="17" rx="2"/><path d="M3 9h18m-9 3v6m-3-3h6"/>',
  search: '<circle cx="10" cy="10" r="6"/><path d="m15 15 6 6"/>',
  zoomIn: '<circle cx="10" cy="10" r="6"/><path d="m15 15 6 6M10 7v6m-3-3h6"/>',
  zoomOut: '<circle cx="10" cy="10" r="6"/><path d="m15 15 6 6M7 10h6"/>',
  zoomReset: '<circle cx="10" cy="10" r="6"/><path d="m15 15 6 6M10 7v6"/>',
  sidebar: '<rect x="3" y="4" width="18" height="16" rx="1"/><path d="M9 4v16M5 8h2m-2 4h2"/>',
  panel: '<rect x="3" y="4" width="18" height="16" rx="1"/><path d="M3 14h18"/>',
  terminal: '<path d="m5 6 6 6-6 6m8 0h6"/>',
});
const xml = (s) =>
  String(s).replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' })[c],
  );
export function errorStatus(error, ru = false) {
  const message = String(error);
  if (/Typst editor/i.test(message)) return ru ? 'Не Typst' : 'Open Typst';
  if (/select.*text|selection/i.test(message)) return ru ? 'Нет выделения' : 'Select text';
  if (/still running|busy|dialog/i.test(message)) return ru ? 'Завершите диалог' : 'Finish dialog';
  if (/modifiers/i.test(message)) return ru ? 'Отпустите Ctrl' : 'Release Ctrl';
  if (/companion|authentication/i.test(message)) return ru ? 'Нет связи' : 'No connection';
  if (/Install or enable/i.test(message)) return ru ? 'Нет дополнения' : 'Enable extension';
  if (/window|focus|executable|application/i.test(message)) return ru ? 'Нет окна' : 'Focus app';
  if (/command.*unavailable|supported|listed command/i.test(message))
    return ru ? 'Нет команды' : 'Unavailable';
  if (/changed|expired/i.test(message)) return ru ? 'Повторите' : 'Press again';
  return ru ? 'Нет связи' : 'No connection';
}
function lines(label, limit = 12) {
  if (label.length <= limit) return [label];
  const words = label.split(/\s+/);
  let first = '',
    second = '';
  for (const word of words) {
    if (!second && (!first || first.length + word.length + 1 <= limit))
      first += (first ? ' ' : '') + word;
    else second += (second ? ' ' : '') + word;
  }
  if (!second) return [label.slice(0, limit), label.slice(limit, limit * 2)];
  return [first, second];
}
export function render(options, canvas = [112, 112], error = '', busy = false) {
  const w = Math.max(32, Math.min(4096, Number(canvas[0]) || 112));
  const h = Math.max(24, Math.min(1024, Number(canvas[1]) || 112));
  const wide = w / h > 2;
  const size = Math.min(wide ? h * 0.56 : w * 0.36, h * 0.4);
  const x = wide ? Math.max(7, h * 0.16) : (w - size) / 2;
  const y = wide ? (h - size) / 2 : h * 0.1;
  const labelX = wide ? x + size + h * 0.18 : w / 2;
  const available = wide ? w - labelX - 6 : w - 10;
  const label = error ? errorStatus(error, options.ru) : options.label;
  const rows = lines(label, Math.max(8, Math.floor(available / (Math.max(14, h * 0.145) * 0.54))));
  const font = Math.max(
    h < 80 ? 10 : 14,
    Math.min(
      wide ? h * 0.24 : h * 0.15,
      available / Math.max(...rows.map((row) => row.length * 0.54), 1),
    ),
  );
  const labelY = wide ? h * (rows.length > 1 ? 0.43 : 0.56) : h * (rows.length > 1 ? 0.7 : 0.77);
  const color = error ? '#ffb4a6' : busy ? '#c9c3b9' : '#e8b466';
  const text = rows
    .map(
      (row, index) =>
        `<text x="${labelX}" y="${labelY + index * font * 1.14}" text-anchor="${wide ? 'start' : 'middle'}" font-family="IBM Plex Sans,Segoe UI,sans-serif" font-size="${font}" font-weight="600" fill="${error ? '#ffb4a6' : '#f0f0f0'}">${xml(row)}</text>`,
    )
    .join('');
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}"><rect width="100%" height="100%" fill="#202020"/><svg x="${x}" y="${y}" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="${color}" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">${error ? '<path d="M12 3 2 21h20zM12 9v5m0 3v.01"/>' : icons[options.icon]}</svg>${text}</svg>`;
}
