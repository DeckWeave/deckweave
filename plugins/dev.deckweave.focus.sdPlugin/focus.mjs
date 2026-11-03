// SPDX-License-Identifier: GPL-3.0-or-later
import { escapeXml } from './lib/sdk.mjs';
export const defaults = Object.freeze({
  workMinutes: 25,
  shortMinutes: 5,
  longMinutes: 15,
  rounds: 4,
  sound: 'complete',
  volume: 35,
});
export function workspace(settings) {
  const value = settings.workspace ?? 'personal';
  if (typeof value !== 'string' || !/^[\w.-]{1,160}$/.test(value))
    throw new Error('Workspace must use letters, numbers, dots, hyphens or underscores');
  return value;
}
export function config(settings) {
  const seconds = (name) => {
    const value = Number(settings[name] ?? defaults[name]);
    if (!Number.isFinite(value) || value < 1 / 60 || value > 1440)
      throw new Error('Duration must be between one second and 24 hours');
    return Math.round(value * 60);
  };
  const rounds = Number(settings.rounds ?? defaults.rounds),
    volume = Number(settings.volume ?? defaults.volume),
    sound = settings.sound === undefined ? defaults.sound : settings.sound;
  if (
    !Number.isInteger(rounds) ||
    rounds < 1 ||
    rounds > 16 ||
    !Number.isInteger(volume) ||
    volume < 0 ||
    volume > 100 ||
    ![null, 'complete', 'notice', 'warning'].includes(sound)
  )
    throw new Error('Invalid rounds, volume or sound');
  return {
    work_seconds: seconds('workMinutes'),
    short_seconds: seconds('shortMinutes'),
    long_seconds: seconds('longMinutes'),
    rounds,
    sound,
    volume,
  };
}
export function settingsFromConfig(value) {
  return {
    workMinutes: value.work_seconds / 60,
    shortMinutes: value.short_seconds / 60,
    longMinutes: value.long_seconds / 60,
    rounds: value.rounds,
    sound: value.sound,
    volume: value.volume,
  };
}
export function time(ms) {
  const seconds = Math.max(0, Math.ceil(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}
export function render(job, settings, canvas, error) {
  const [w, h] = canvas,
    ru = settings.language === 'ru',
    wide = w / h >= 2.4;
  const phases = ru
    ? { work: 'Фокус', short_break: 'Перерыв', long_break: 'Отдых' }
    : { work: 'Focus', short_break: 'Break', long_break: 'Rest' };
  const statuses = ru
    ? { idle: 'Старт', paused: 'Пауза', ready: 'Далее' }
    : { idle: 'Start', paused: 'Paused', ready: 'Next' };
  const operations = ru
    ? { start: 'Старт', pause: 'Пауза', reset: 'Сброс', skip: 'Далее' }
    : { start: 'Start', pause: 'Pause', reset: 'Reset', skip: 'Next' };
  const label = error
    ? ru
      ? 'Нет связи'
      : 'Unavailable'
    : job
      ? (operations[settings.operation] ?? phases[job.phase])
      : ru
        ? 'Связь…'
        : 'Connecting…';
  const value = error || !job ? '—' : time(job.live_remaining_ms);
  const state = job
    ? `${statuses[job.status] ?? phases[job.phase]} ${job.round}/${job.config.rounds}`
    : '';
  const accent = job?.status === 'running' ? '#efbb72' : '#ededed';
  const valueSize = Math.min(
    h * (wide ? 0.64 : 0.38),
    (w * (wide ? 0.5 : 0.92)) / (value.length * 0.58),
  );
  const small = Math.min(h * 0.17, w / (Math.max(label.length, state.length) * 0.58 + 1));
  const text = (x, y, size, color, content, anchor = 'middle', weight = '400') =>
    `<text x="${x}" y="${y}" text-anchor="${anchor}" font-family="IBM Plex Sans" font-size="${size}" font-weight="${weight}" fill="${color}">${escapeXml(content)}</text>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"><rect width="${w}" height="${h}" fill="#20201f"/>${
    wide
      ? text(w * 0.04, h * 0.72, valueSize, accent, value, 'start', '600') +
        text(w * 0.95, h * 0.4, small, '#ededed', label, 'end') +
        text(w * 0.95, h * 0.75, small, '#cdcdcd', state, 'end')
      : text(w / 2, h * 0.22, small, '#ededed', label) +
        text(w / 2, h * 0.66, valueSize, accent, value, 'middle', '600') +
        text(w / 2, h * 0.9, small, '#cdcdcd', state)
  }</svg>`;
}
