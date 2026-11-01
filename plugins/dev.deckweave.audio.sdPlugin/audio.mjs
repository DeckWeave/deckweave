// SPDX-License-Identifier: GPL-3.0-or-later
export function settings(input = {}, language = 'en') {
  return {
    step: Math.max(1, Math.min(10, Math.round(Number(input.step) || 2))),
    label: typeof input.label === 'string' ? input.label.trim().slice(0, 24) : '',
    language: String(input.language ?? language).startsWith('ru') ? 'ru' : 'en',
  };
}
export const escapeXml = (value) =>
  String(value).replace(
    /[<>&"']/g,
    (c) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;', "'": '&apos;' })[c],
  );

export function render(state, options, canvas = [112, 112], error = '') {
  const w = Math.max(16, Math.min(2048, Number(canvas[0]) || 112));
  const h = Math.max(16, Math.min(2048, Number(canvas[1]) || 112));
  const wide = w / h > 2;
  const ru = options.language === 'ru';
  const volume = state ? Math.round(state.volume) : null;
  const muted = state?.muted;
  const title = options.label || (ru ? 'Громкость' : 'Volume');
  const value = error
    ? ru
      ? 'Нет связи'
      : 'Offline'
    : volume === null
      ? '…'
      : muted
        ? ru
          ? 'ВЫКЛ'
          : 'MUTE'
        : `${volume}%`;
  const color = error ? '#e7a96b' : muted ? '#f08c95' : '#78d8b0';
  const pad = Math.min(w * 0.08, h * 0.14);
  const labelSize = Math.min(h * 0.18, w / (title.length * 0.65 + 1));
  const valueSize = Math.min(h * (wide ? 0.43 : 0.32), w / (value.length * 0.68 + 1));
  const barX = wide ? w * 0.43 : pad;
  const barWidth = wide ? w * 0.53 - pad : w - pad * 2;
  const barY = wide ? h * 0.49 : h * 0.79;
  const barHeight = h * (wide ? 0.22 : 0.09);
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"><rect width="${w}" height="${h}" fill="#18232b"/><text x="${pad}" y="${h * 0.25}" fill="#d9e6ef" font-size="${labelSize}" font-family="IBM Plex Sans">${escapeXml(title)}</text><text x="${pad}" y="${h * (wide ? 0.76 : 0.65)}" fill="${color}" font-weight="600" font-size="${valueSize}" font-family="IBM Plex Sans">${escapeXml(value)}</text><rect x="${barX}" y="${barY}" width="${barWidth}" height="${barHeight}" rx="3" fill="#354650"/><rect x="${barX}" y="${barY}" width="${(barWidth * (volume ?? 0)) / 100}" height="${barHeight}" rx="3" fill="${color}" opacity="${muted ? 0.35 : 1}"/></svg>`;
}

export function touchVolume(position) {
  return typeof position?.x === 'number' && Number.isFinite(position.x)
    ? Math.round(Math.max(0, Math.min(1, position.x)) * 100)
    : null;
}
