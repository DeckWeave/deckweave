// One stroke vocabulary for commands. Plugins without artwork use semantic glyphs.
export const icons: Record<string, string> = {
  volume: 'M4 9h4l5-4v14l-5-4H4ZM16 8a6 6 0 0 1 0 8M19 5a10 10 0 0 1 0 14',
  reset: 'M4 10a8 8 0 1 1 0 5M4 4v6h6',
  pause: 'M8 5v14M16 5v14',
  settings:
    'M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1 1-3ZM12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z',
  sliders: 'M4 6h5m4 0h7M4 12h9m4 0h3M4 18h2m4 0h10M9 3v6m4 0v6m-7 0v6',
  device: 'M4 4h16v16H4ZM8 8h2v2H8Zm6 0h2v2h-2Zm-6 6h2v2H8Zm6 0h2v2h-2Z',
  keyboard: 'M3 6h18v12H3ZM6 10h1m3 0h1m3 0h1m3 0h0M7 14h10',
  dial: 'M8 3h8M12 3v4M4 14a8 8 0 1 0 16 0 8 8 0 0 0-16 0Zm8 0 4-4',
  plugin: 'M8 3v5H3v8h5v5h8v-5h5V8h-5V3Z',
  folder: 'M3 6h7l2 3h9v11H3Z',
  page: 'M6 3h9l4 4v14H6ZM14 3v5h5M9 12h7m-7 4h7',
  pages: 'M4 5h12v14H4Zm5-3h11v14M7 10h6m-6 4h6',
  profile: 'M5 4h14v16H5ZM9 8a3 3 0 1 0 6 0 3 3 0 0 0-6 0Zm-1 9c0-5 8-5 8 0',
  back: 'M10 5 3 12l7 7M3 12h18',
  launch: 'M9 3H3v18h18v-6M14 3h7v7M21 3 10 14',
  hotkey: 'M8 3a3 3 0 1 0 0 6h8a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H8a3 3 0 1 0 3 3V6a3 3 0 0 0-3-3Z',
  sequence: 'M8 5h13M8 12h13M8 19h13M3 4v2m0 5v2m0 5v2',
  switch: 'M3 7h17l-4-4m4 4-4 4M21 17H4l4 4m-4-4 4-4',
  key_logic: 'M4 5h6v5H4ZM14 14h6v5h-6M7 10v7h7M10 7h7v7',
  dial_stack: 'M4 5h10M4 9h7M4 13h4M12 16a5 5 0 1 0 10 0 5 5 0 0 0-10 0Zm5 0 2-2',
  action_wheel:
    'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Zm0 0v6m8 8-5-3M4 17l5-3M9 12a3 3 0 1 0 6 0 3 3 0 0 0-6 0Z',
  dial_bindings:
    'M3 6h7m-3-3L4 6l3 3M21 6h-7m3-3 3 3-3 3M7 17a5 5 0 1 0 10 0 5 5 0 0 0-10 0Zm5-3v3',
  light: 'M8 15c-6-6-1-13 4-13s10 7 4 13l-1 3H9ZM9 22h6M9 18h6',
  lighting_preset: 'M8 15c-6-6-1-13 4-13s10 7 4 13l-1 3H9ZM9 22h6M9 18h6',
  lighting_return: 'M3 10a8 8 0 1 1 1 8M3 4v6h6M12 7v5l3 2',
  clock: 'M3 12a9 9 0 1 0 18 0 9 9 0 0 0-18 0Zm9-5v5l4 2',
  timer: 'M9 2h6M12 2v3M18 5l2 2M4 14a8 8 0 1 0 16 0 8 8 0 0 0-16 0Zm8-5v5l3 2',
  weather: 'M5 14a4 4 0 1 0 0 8h12a5 5 0 1 0-2-10 6 6 0 0 0-10 2ZM7 2v3M2 6l2 2m9-6-1 3',
  audio: 'M3 9h4l5-5v16l-5-5H3ZM16 8c3 2 3 6 0 8m3-11c5 4 5 10 0 14',
  mute: 'M3 9h4l5-5v16l-5-5H3Zm13-1 6 8m0-8-6 8',
  monitor: 'M3 4h18v13H3ZM8 21h8m-4-4v4M6 12h3l2-5 3 7 2-4h2',
  record: 'M3 12a9 9 0 1 0 18 0 9 9 0 0 0-18 0Zm5 0a4 4 0 1 0 8 0 4 4 0 0 0-8 0Z',
  scene: 'M3 4h18v16H3ZM3 9h18M8 4v5m8-5v5m-6 3 5 3-5 3Z',
  code: 'm8 6-6 6 6 6m8-12 6 6-6 6M14 3l-4 18',
  color:
    'M12 3C6 3 2 7 2 12s4 9 9 9c2 0 3-2 2-4-1-2 1-3 3-3h2c6 0 3-11-6-11ZM7 8h.01M13 6h.01M18 9h.01M6 13h.01',
  counter: 'M10 3 8 21M17 3l-2 18M4 9h16M3 15h16',
  toggle: 'M7 6h10a6 6 0 0 1 0 12H7A6 6 0 0 1 7 6Zm0 3a3 3 0 1 0 0 6 3 3 0 0 0 0-6Z',
  play: 'm8 4 12 8-12 8Z',
  stop: 'M5 5h14v14H5Z',
  close: 'm6 6 12 12M18 6 6 18',
  trash: 'M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7',
  plus: 'M12 4v16M4 12h16',
  minus: 'M4 12h16',
  up: 'm5 14 7-7 7 7',
  down: 'm5 10 7 7 7-7',
  rotate_left: 'M3 4v6h6M3 10a8 8 0 1 1 1 8',
  rotate_right: 'M21 4v6h-6m6 0a8 8 0 1 0-1 8',
  check: 'm4 12 5 5L20 6',
  grip: 'M8 5h.01M16 5h.01M8 12h.01M16 12h.01M8 19h.01M16 19h.01',
  chevron: 'm9 5 7 7-7 7',
  refresh: 'M20 8a8 8 0 0 0-14-3L3 8m0-5v5h5M4 16a8 8 0 0 0 14 3l3-3m0 5v-5h-5',
  install: 'M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5',
  info: 'M3 12a9 9 0 1 0 18 0 9 9 0 0 0-18 0Zm9-2v7m0-10h.01',
  power: 'M12 2v10M6 5a9 9 0 1 0 12 0',
  search: 'M3 10a7 7 0 1 0 14 0 7 7 0 0 0-14 0Zm12 5 6 6',
};

export function actionIcon(id: string, label = ''): string {
  const key = id.replace(/^(plan|builtin):/, '');
  if (icons[key]) return key;
  if (key === 'page_cycle') return 'pages';
  id += ' ' + label;
  if (/forecast|weather/i.test(id)) return 'weather';
  if (/color|palette/i.test(id)) return 'color';
  if (/countdown|stopwatch|timer/i.test(id)) return 'timer';
  if (/desktop\.note/i.test(id)) return 'page';
  if (/desktop\.sound/i.test(id)) return 'volume';
  if (/clock|uptime/i.test(id)) return 'clock';
  if (/mute/i.test(id)) return 'mute';
  if (/volume|audio/i.test(id)) return 'audio';
  if (/record/i.test(id)) return 'record';
  if (/scene|obs/i.test(id)) return 'scene';
  if (/cpu|memory|monitor/i.test(id)) return 'monitor';
  if (/vscode|studio.code|terminal|snippet|command/i.test(id)) return 'code';
  return 'plugin';
}
