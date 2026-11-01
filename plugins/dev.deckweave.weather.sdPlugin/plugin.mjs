// SPDX-License-Identifier: GPL-3.0-or-later
import {
  PERIOD,
  validLocation,
  getJson,
  forecastUrl,
  parseForecast,
  searchPlaces,
  render,
} from './weather.mjs';
const args = Object.fromEntries(
  process.argv.slice(2).reduce((a, v, i, all) => (i % 2 ? a : [...a, [v, all[i + 1]]]), []),
);
let defaultLanguage = 'en';
try {
  defaultLanguage = JSON.parse(args['-info']).application?.language?.startsWith('ru') ? 'ru' : 'en';
} catch {}
const ws = new WebSocket(`ws://127.0.0.1:${args['-port']}`);
const contexts = new Map(),
  cache = new Map(),
  searches = new Map();
const send = (event, context, payload) => {
  if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ event, context, payload }));
};
const key = (s) => `${s.latitude},${s.longitude}`;
function draw(context) {
  const state = contexts.get(context);
  if (!state) return;
  const entry = cache.get(key(state.settings));
  const data = entry?.data;
  const error = entry?.error ?? '';
  const ru = state.settings.language === 'ru';
  send('setImage', context, {
    image:
      'data:image/svg+xml;base64,' +
      Buffer.from(
        render(data, state.settings, state.action, state.canvas, Date.now() / 1000, error),
      ).toString('base64'),
    target: 0,
  });
  send('setTitle', context, {
    title: !validLocation(state.settings)
      ? ru
        ? 'Выберите город'
        : 'Choose city'
      : error
        ? data
          ? ru
            ? 'Устаревшие данные'
            : 'Stale data'
          : ru
            ? 'Погода недоступна'
            : 'Weather unavailable'
        : state.settings.name,
    target: 0,
  });
}
async function refresh(context) {
  const state = contexts.get(context);
  if (!state || !validLocation(state.settings)) {
    draw(context);
    return;
  }
  const k = key(state.settings);
  let entry = cache.get(k);
  if (!entry) {
    if (cache.size >= 16) {
      const unused = [...cache.keys()].find(
        (k) => ![...contexts.values()].some((s) => key(s.settings) === k),
      );
      if (!unused) {
        send('setTitle', context, { title: 'Weather location limit (16)' });
        return;
      }
      cache.delete(unused);
    }
    entry = { data: null, requested: 0, pending: false, error: '' };
    cache.set(k, entry);
  }
  if (!entry.pending && Date.now() - entry.requested > (entry.error ? 60_000 : PERIOD)) {
    entry.pending = true;
    entry.requested = Date.now();
    try {
      entry.data = parseForecast(await getJson(forecastUrl(state.settings)));
      entry.error = '';
    } catch (e) {
      entry.error = e.message;
    } finally {
      entry.pending = false;
    }
    for (const [id, s] of contexts) if (key(s.settings) === k) draw(id);
  } else draw(context);
}
ws.addEventListener('open', () =>
  ws.send(JSON.stringify({ event: args['-registerEvent'], uuid: args['-pluginUUID'] })),
);
ws.addEventListener('message', (event) => {
  let m;
  try {
    m = JSON.parse(event.data);
  } catch {
    return;
  }
  const { context, payload = {} } = m;
  if (m.event === 'propertyInspectorDidDisappear') {
    searches.delete(context);
    return;
  }
  if (m.event === 'willDisappear') {
    contexts.delete(context);
    searches.delete(context);
    return;
  }
  if (m.event === 'willAppear' || m.event === 'didReceiveSettings') {
    const before = contexts.get(context);
    contexts.set(context, {
      settings: { language: defaultLanguage, ...(payload.settings ?? {}) },
      canvas: payload.canvas ?? before?.canvas ?? [112, 112],
      action: (m.action ?? before?.action ?? '').split('.').at(-1),
    });
    draw(context);
    void refresh(context);
  } else if (['keyUp', 'dialUp', 'panelUp'].includes(m.event)) void refresh(context);
  else if (m.event === 'sendToPlugin' && payload.command === 'search') {
    const token = payload.request;
    searches.set(context, token);
    void searchPlaces(payload.query, payload.language)
      .then((places) => {
        if (searches.get(context) === token)
          send('sendToPropertyInspector', context, {
            command: 'locations',
            request: token,
            places,
          });
      })
      .catch(() => {
        if (searches.get(context) === token)
          send('sendToPropertyInspector', context, {
            command: 'locations',
            request: token,
            places: [],
            error: 'Could not reach location service',
          });
      });
  }
});
const timer = setInterval(() => {
  for (const id of contexts.keys()) void refresh(id);
}, 30_000);
ws.addEventListener('close', () => {
  clearInterval(timer);
  process.exit(0);
});
