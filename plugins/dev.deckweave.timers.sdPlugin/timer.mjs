// SPDX-License-Identifier: GPL-3.0-or-later
import { escapeXml } from './lib/sdk.mjs';
export const duration = (value) =>
  Math.max(
    1,
    Math.min(
      24 * 60 * 60,
      Math.round(Number.isFinite(Number(value)) && Number(value) > 0 ? Number(value) : 25 * 60),
    ),
  ) * 1000;
export function restore(saved, countdown, durationMs, wallNow, monoNow) {
  const status = ['running', 'paused', 'done'].includes(saved?.status) ? saved.status : 'idle';
  const initial = countdown ? durationMs : 0;
  const base = Number.isFinite(saved?.value)
    ? Math.max(0, Math.min(7 * 86400000, saved.value))
    : initial;
  const elapsed =
    status === 'running' && Number.isFinite(saved?.startedAt)
      ? Math.max(0, wallNow - saved.startedAt)
      : 0;
  const value = Math.max(0, base + (countdown ? -elapsed : elapsed));
  return {
    countdown,
    durationMs,
    status: countdown && value === 0 && status === 'running' ? 'done' : status,
    base: value,
    anchor: monoNow,
  };
}
export function valueAt(state, now) {
  const delta = state.status === 'running' ? Math.max(0, now - state.anchor) : 0;
  return Math.max(0, state.base + (state.countdown ? -delta : delta));
}
export function toggle(state, now) {
  const value = valueAt(state, now);
  if (state.status === 'running') return { ...state, status: 'paused', base: value, anchor: now };
  return {
    ...state,
    status: 'running',
    base: state.status === 'done' ? (state.countdown ? state.durationMs : 0) : value,
    anchor: now,
  };
}
export function reset(state, now) {
  return { ...state, status: 'idle', base: state.countdown ? state.durationMs : 0, anchor: now };
}
export function serialize(state, wallNow, monoNow) {
  return {
    status: state.status,
    value: valueAt(state, monoNow),
    startedAt: state.status === 'running' ? wallNow : null,
  };
}
export function format(ms, countdown = true) {
  const seconds = Math.max(0, countdown ? Math.ceil(ms / 1000) : Math.floor(ms / 1000));
  return seconds >= 3600
    ? `${Math.floor(seconds / 3600)}:${String(Math.floor(seconds / 60) % 60).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`
    : `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}
export function render(state, settings, canvas, now) {
  const [w, h] = canvas,
    scale = Math.min(w / 112, h / 112),
    ru = settings.language === 'ru',
    value = valueAt(state, now);
  const label = String(
    settings.label ||
      (state.countdown ? (ru ? 'Фокус' : 'Focus') : ru ? 'Секундомер' : 'Stopwatch'),
  ).slice(0, 24);
  const time = format(value, state.countdown),
    done = state.status === 'done',
    color = done ? '#a6e3a1' : state.status === 'running' ? '#8bd5fb' : '#d2d8e4';
  const hint = done
    ? ru
      ? 'Готово · повтор'
      : 'Done · restart'
    : state.status === 'running'
      ? ru
        ? 'Пауза · удержать: сброс'
        : 'Pause · hold: reset'
      : ru
        ? 'Нажать: старт'
        : 'Press to start';
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}"><rect width="100%" height="100%" fill="${done ? '#15342a' : '#1b2233'}"/><text x="${w / 2}" y="${20 * scale}" text-anchor="middle" font-family="IBM Plex Sans" font-size="${14 * scale}" fill="#d8e6f5">${escapeXml(label)}</text><text x="${w / 2}" y="${h * 0.61}" text-anchor="middle" font-family="IBM Plex Sans" font-weight="600" font-size="${Math.min(h * 0.34, w / (time.length * 0.6))}" fill="${color}">${time}</text><text x="${w / 2}" y="${h - 13 * scale}" text-anchor="middle" font-family="IBM Plex Sans" font-size="${8.5 * scale}" fill="#d8e6f5">${escapeXml(hint)}</text>${state.countdown ? `<rect x="${6 * scale}" y="${h - 5 * scale}" width="${(w - 12 * scale) * Math.min(1, value / state.durationMs)}" height="${3 * scale}" fill="${color}"/>` : ''}</svg>`;
}
