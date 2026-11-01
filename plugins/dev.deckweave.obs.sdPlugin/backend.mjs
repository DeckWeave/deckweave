// GPL-3.0-or-later.
import { Obs } from './obs.mjs';
const args = Object.fromEntries(
  Array.from({ length: (process.argv.length - 2) / 2 }, (_, i) =>
    process.argv.slice(2 + i * 2, 4 + i * 2),
  ),
);
const socket = new WebSocket(`ws://127.0.0.1:${args['-port']}`);
const contexts = new Map();
const connections = new Map();
const send = (event, context, payload) => socket.send(JSON.stringify({ event, context, payload }));
const failed = (context, message) => {
  send('setTitle', context, { title: message });
  send('showAlert', context, {});
};
function attach(context, action, settings) {
  detach(context);
  const key = JSON.stringify([
    settings.host || '127.0.0.1',
    settings.port || 4455,
    settings.password || '',
  ]);
  let group = connections.get(key);
  if (!group) {
    group = {
      contexts: new Set(),
      obs: new Obs(settings, (event, message) => {
        for (const id of connections.get(key)?.contexts || []) {
          if (event === 'error') failed(id, message);
          else void refresh(id);
        }
      }),
    };
    connections.set(key, group);
  }
  group.contexts.add(context);
  contexts.set(context, { action, settings, key });
  if (group.obs.ready) void refresh(context);
  else send('setTitle', context, { title: 'OBS…' });
}
function detach(context) {
  const state = contexts.get(context);
  if (!state) return;
  const group = connections.get(state.key);
  group?.contexts.delete(context);
  if (group && !group.contexts.size) {
    group.obs.close();
    connections.delete(state.key);
  }
  contexts.delete(context);
}
async function refresh(context) {
  const state = contexts.get(context);
  if (!state) return;
  const obs = connections.get(state.key)?.obs;
  if (!obs?.ready) return;
  try {
    let title,
      active = false;
    switch (state.action.split('.').at(-1)) {
      case 'scene': {
        const data = await obs.request('GetCurrentProgramScene');
        title = state.settings.scene || data.sceneName;
        active = data.sceneName === state.settings.scene;
        break;
      }
      case 'mute': {
        if (!state.settings.input) {
          send('setTitle', context, { title: 'Choose input' });
          return;
        }
        const data = await obs.request('GetInputMute', { inputName: state.settings.input });
        active = !!data.inputMuted;
        title = `${state.settings.input} ${active ? 'MUTE' : 'ON'}`;
        break;
      }
      case 'record': {
        const data = await obs.request('GetRecordStatus');
        active = !!data.outputActive;
        title = active ? 'REC' : 'Record';
        break;
      }
      default: {
        const data = await obs.request('GetVersion');
        title = `OBS ${data.obsVersion}`;
        active = true;
      }
    }
    if (!contexts.has(context)) return;
    send('setTitle', context, { title });
    send('setState', context, { state: active ? 1 : 0 });
    send('showOk', context, {});
  } catch (e) {
    failed(context, e.message);
  }
}
socket.onopen = () =>
  socket.send(JSON.stringify({ event: args['-registerEvent'], uuid: args['-pluginUUID'] }));
socket.onmessage = async ({ data }) => {
  const { event, context, action, payload } = JSON.parse(data);
  if (event === 'willAppear' || event === 'didReceiveSettings') {
    attach(context, action, payload.settings || {});
    return;
  }
  if (event === 'willDisappear') {
    detach(context);
    return;
  }
  const state = contexts.get(context);
  if (!state) return;
  const obs = connections.get(state.key)?.obs;
  try {
    if (event === 'keyUp') {
      switch (state.action.split('.').at(-1)) {
        case 'scene':
          if (!state.settings.scene) throw Error('Choose scene');
          await obs.request('SetCurrentProgramScene', { sceneName: state.settings.scene });
          break;
        case 'mute':
          if (!state.settings.input) throw Error('Choose input');
          await obs.request('ToggleInputMute', { inputName: state.settings.input });
          break;
        case 'record':
          await obs.request('ToggleRecord');
          break;
      }
      await refresh(context);
    }
    if (event === 'sendToPlugin' && payload?.list) {
      const result = await obs.request(payload.list === 'scenes' ? 'GetSceneList' : 'GetInputList');
      send('sendToPropertyInspector', context, result);
    }
  } catch (e) {
    failed(context, e.message);
  }
};
socket.onclose = () => {
  for (const group of connections.values()) group.obs.close();
  process.exit(0);
};
