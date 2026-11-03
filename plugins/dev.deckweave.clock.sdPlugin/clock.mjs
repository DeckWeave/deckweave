// SPDX-License-Identifier: GPL-3.0-or-later
export const escape = (s) =>
  String(s).replace(
    /[<>&"']/g,
    (c) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;', "'": '&apos;' })[c],
  );
export function render(date = new Date(), settings = {}, canvas = [112, 112]) {
  const [width, height] = canvas;
  const wide = width / height > 1.3,
    h = wide ? 128 : 112,
    w = (width / height) * h;
  const locale = settings.language === 'en' ? 'en-GB' : 'ru-RU';
  const options = { hour: '2-digit', minute: '2-digit', hour12: false };
  if (settings.seconds) options.second = '2-digit';
  if (settings.timezone) options.timeZone = settings.timezone;
  let time, day, weekday;
  try {
    time = new Intl.DateTimeFormat(locale, options).format(date);
    day = new Intl.DateTimeFormat(locale, {
      day: 'numeric',
      month: 'short',
      ...(options.timeZone ? { timeZone: options.timeZone } : {}),
    })
      .format(date)
      .replace(/\s*г\.$/, '');
    weekday = new Intl.DateTimeFormat(locale, {
      weekday: 'long',
      ...(options.timeZone ? { timeZone: options.timeZone } : {}),
    }).format(date);
  } catch {
    time = '—';
    day = locale === 'ru-RU' ? 'Часовой пояс' : 'Time zone';
    weekday = locale === 'ru-RU' ? 'Проверьте настройки' : 'Check settings';
  }
  const text = (y, size, value, color = '#f4f2ee', weight = 500) =>
    `<text x="${w / 2}" y="${y}" fill="${color}" text-anchor="middle" font-family="IBM Plex Sans" font-weight="${weight}" font-size="${size}">${escape(value)}</text>`;
  const size = Math.min(wide ? 57 : 39, (w - 12) / (time.length * 0.59));
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${w} ${h}"><rect width="${w}" height="${h}" fill="#20201f"/>${text(wide ? 59 : 47, size, time, '#f4f2ee', 600)}${text(wide ? 89 : 76, wide ? 23 : 20, day)}${text(wide ? 116 : 101, wide ? 18 : 15, weekday, '#d3cabb')}</svg>`;
}
