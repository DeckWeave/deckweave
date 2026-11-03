// SPDX-License-Identifier: GPL-3.0-or-later
let socket,
  context,
  settings = {},
  job,
  ru = false,
  ready = false;
const el = (id) => document.getElementById(id);
const fields = [
  'workspace',
  'operation',
  'workMinutes',
  'shortMinutes',
  'longMinutes',
  'rounds',
  'sound',
  'volume',
  'holdReset',
];
const defaults = {
  workspace: 'personal',
  operation: 'toggle',
  workMinutes: 25,
  shortMinutes: 5,
  longMinutes: 15,
  rounds: 4,
  sound: 'complete',
  volume: 35,
  holdReset: true,
};
function status(message) {
  el('status').textContent = message;
}
function show() {
  const shared = job
    ? {
        workMinutes: job.config.work_seconds / 60,
        shortMinutes: job.config.short_seconds / 60,
        longMinutes: job.config.long_seconds / 60,
        rounds: job.config.rounds,
        sound: job.config.sound,
        volume: job.config.volume,
      }
    : {};
  for (const id of fields)
    if (document.activeElement !== el(id)) {
      const value = Object.hasOwn(shared, id) ? shared[id] : (settings[id] ?? defaults[id]);
      if (id === 'holdReset') el(id).checked = value;
      else el(id).value = id === 'sound' && value === null ? 'none' : value;
    }
  if (job) {
    const seconds = Math.ceil(job.live_remaining_ms / 1000),
      phases = ru
        ? { work: 'Фокус', short_break: 'Перерыв', long_break: 'Отдых' }
        : { work: 'Focus', short_break: 'Break', long_break: 'Rest' };
    const states = ru
      ? { idle: 'Старт', ready: 'Далее', paused: 'Пауза', running: 'Идёт' }
      : { idle: 'Start', ready: 'Next', paused: 'Paused', running: 'Running' };
    el('current').textContent =
      `${phases[job.phase]} ${job.round}/${job.config.rounds} · ${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')} · ${states[job.status]}`;
  }
}
function send(payload) {
  if (ready) socket.send(JSON.stringify({ event: 'sendToPlugin', context, payload }));
}
function enabled(value) {
  for (const element of document.querySelectorAll('input,select,button')) element.disabled = !value;
}
enabled(false);
window.connectElgatoStreamDeckSocket = (port, uuid, event, info, actionInfo) => {
  const host = JSON.parse(info);
  DeckWeaveAppearance.apply(host.deckweave);
  context = uuid;
  ru = host.application?.language?.startsWith('ru') ?? false;
  document.documentElement.lang = ru ? 'ru' : 'en';
  for (const node of document.querySelectorAll('[data-en]'))
    node.textContent = node.dataset[ru ? 'ru' : 'en'];
  settings = JSON.parse(actionInfo).payload?.settings ?? {};
  show();
  socket = new WebSocket(`ws://127.0.0.1:${port}`);
  DeckWeaveAppearance.subscribe(socket, uuid);
  socket.onopen = () => {
    socket.send(JSON.stringify({ event, uuid }));
    ready = true;
    enabled(true);
    status(ru ? 'Готово' : 'Ready');
    send({ command: 'refresh' });
  };
  socket.onmessage = ({ data }) => {
    try {
      const message = JSON.parse(data);
      DeckWeaveAppearance.receive(message);
      if (message.event === 'didReceiveSettings') {
        settings = message.payload?.settings ?? {};
        show();
      }
      if (message.event === 'sendToPropertyInspector') {
        if (message.payload?.hostFocus) {
          job = message.payload.hostFocus;
          show();
          status(ru ? 'Сохранено' : 'Saved');
        }
        if (message.payload?.error) status(message.payload.error);
      }
    } catch (error) {
      status(error.message);
    }
  };
  socket.onclose = () => {
    ready = false;
    enabled(false);
    status(ru ? 'Нет связи' : 'Disconnected');
  };
};
for (const id of fields)
  el(id).onchange = () => {
    if (!ready) return;
    if (!el(id).checkValidity()) {
      el(id).reportValidity();
      return;
    }
    const value =
      id === 'holdReset'
        ? el(id).checked
        : id === 'sound' && el(id).value === 'none'
          ? null
          : ['workMinutes', 'shortMinutes', 'longMinutes', 'rounds', 'volume'].includes(id)
            ? Number(el(id).value)
            : el(id).value;
    settings = { ...settings, [id]: value };
    if (id === 'workspace') job = undefined;
    send({ command: 'configure', settings: { [id]: value } });
  };
for (const button of document.querySelectorAll('[data-command]'))
  button.onclick = () => send({ command: button.dataset.command });
