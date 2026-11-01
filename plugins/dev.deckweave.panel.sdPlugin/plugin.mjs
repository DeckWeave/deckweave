// DeckWeave's Panel extension. No OS actions or network connections other than the host.
const args = Object.fromEntries(
  process.argv.slice(2).reduce((pairs, v, i, a) => (i % 2 ? pairs : [...pairs, [v, a[i + 1]]]), []),
);
const ws = new WebSocket(`ws://127.0.0.1:${args['-port']}`);
const contexts = new Map();
const send = (event, context, payload) => ws.send(JSON.stringify({ event, context, payload }));
function draw(context, state) {
  const [w, h] = state.canvas;
  const fill = state.active ? '#57c9a5' : '#759aea';
  const y = state.y === null ? 'Y: unknown' : `Y: ${state.y.toFixed(2)}`;
  const padding = Math.min(16, w * 0.04, h * 0.1);
  const wide = w / h >= 4;
  const detail = `${state.event} · ${y}`;
  const detailWidth = wide ? w * 0.4 - padding : w - 2 * padding;
  const detailSize = Math.min(wide ? 21 : 14, h * 0.16, detailWidth / (detail.length * 0.62));
  const barWidth = w - 2 * padding;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
    <rect width="${w}" height="${h}" fill="#151c29"/>
    <rect x="${padding}" y="${h * 0.59}" width="${barWidth}" height="${h * 0.29}" rx="${Math.min(8, h * 0.06)}" fill="#30394b"/>
    <rect x="${padding}" y="${h * 0.59}" width="${(barWidth * state.value) / 100}" height="${h * 0.29}" rx="${Math.min(8, h * 0.06)}" fill="${fill}"/>
    <text x="${padding}" y="${h * 0.35}" fill="#ffffff" font-size="${Math.min(30, h * 0.28, w * 0.13)}" font-family="IBM Plex Sans">Panel ${state.value}%</text>
    <text x="${wide ? w * 0.59 : padding}" y="${h * (wide ? 0.35 : 0.52)}" fill="#b6c7df" font-size="${detailSize}" font-family="IBM Plex Sans">${detail}</text>
  </svg>`;
  send('setImage', context, {
    image: `data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}`,
    target: 0,
  });
  send('setTitle', context, { title: `Panel ${state.value}%`, target: 0 });
}
ws.addEventListener('open', () =>
  ws.send(JSON.stringify({ event: args['-registerEvent'], uuid: args['-pluginUUID'] })),
);
ws.addEventListener('message', (event) => {
  const message = JSON.parse(event.data);
  const { context, payload = {} } = message;
  if (message.event === 'willDisappear') {
    contexts.delete(context);
    return;
  }
  if (message.event === 'willAppear')
    contexts.set(context, {
      value: 50,
      active: false,
      canvas: payload.canvas ?? [800, 128],
      y: null,
      event: 'ready',
    });
  const state = contexts.get(context);
  if (!state) return;
  if (message.event === 'dialRotate')
    state.value = Math.max(0, Math.min(100, state.value + payload.ticks));
  else if (message.event === 'dialUp') state.active = !state.active;
  else if (['panelDown', 'panelMove', 'panelUp'].includes(message.event) && payload.position) {
    state.value = Math.round(payload.position.x * 100);
    state.y = payload.position.y;
  } else if (message.event === 'panelSwipe') state.active = !state.active;
  else if (message.event !== 'willAppear' && message.event !== 'panelCancel') return;
  if (payload.canvas) state.canvas = payload.canvas;
  state.event = message.event + (payload.control?.startsWith('dial-') ? ` ${payload.control}` : '');
  draw(context, state);
});
