// GPL-3.0-or-later. Protocol fixture shipped with DeckWeave; requires Node 22+.
const args = Object.fromEntries(
  Array.from({ length: (process.argv.length - 2) / 2 }, (_, i) =>
    process.argv.slice(2 + i * 2, 4 + i * 2),
  ),
);
const socket = new WebSocket(`ws://127.0.0.1:${args['-port']}`);
const contexts = new Map();
const send = (event, context, payload) => socket.send(JSON.stringify({ event, context, payload }));
const render = (context) => {
  const state = contexts.get(context);
  send('setTitle', context, { title: `${state.settings.prefix || 'Plugin'} ${state.value}` });
  send('setState', context, { state: Math.abs(state.value) % 2 });
};
socket.onopen = () =>
  socket.send(JSON.stringify({ event: args['-registerEvent'], uuid: args['-pluginUUID'] }));
socket.onmessage = ({ data }) => {
  const { event, context, payload } = JSON.parse(data);
  if (event === 'willAppear') {
    contexts.set(context, { value: 0, settings: payload.settings || {} });
    render(context);
  }
  if (event === 'willDisappear') contexts.delete(context);
  const state = contexts.get(context);
  if (!state) return;
  if (event === 'didReceiveSettings') {
    state.settings = payload.settings || {};
    render(context);
  }
  if (event === 'keyUp' || event === 'dialUp' || event === 'dialRotate') {
    state.value += event === 'dialRotate' ? payload.ticks : 1;
    render(context);
  }
  if (event === 'sendToPlugin' && payload.reset) {
    state.value = 0;
    render(context);
  }
};
