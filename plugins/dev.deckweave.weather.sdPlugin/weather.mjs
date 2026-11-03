// SPDX-License-Identifier: GPL-3.0-or-later
// Port of the validation/time-horizon ideas in old DeckWeave plugins/weather/src/data.rs.
export const PERIOD = 15 * 60_000;
export function validLocation(s) {
  return (
    typeof s?.name === 'string' &&
    s.name.trim().length > 0 &&
    s.name.length <= 120 &&
    Number.isFinite(s.latitude) &&
    Math.abs(s.latitude) <= 90 &&
    Number.isFinite(s.longitude) &&
    Math.abs(s.longitude) <= 180
  );
}
const number = (v, min, max) =>
  typeof v === 'number' && Number.isFinite(v) && v >= min && v <= max ? v : null;
const codes = new Set([
  0, 1, 2, 3, 45, 48, 51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 71, 73, 75, 77, 80, 81, 82, 85, 86,
  95, 96, 99,
]);
const code = (v) => (codes.has(v) ? v : null);
export function parseForecast(v, now = Date.now() / 1000) {
  const c = v.current ?? {};
  const temperature = number(c.temperature_2m, -100, 70);
  if (
    temperature === null ||
    !Number.isSafeInteger(c.time) ||
    c.time > now + 300 ||
    now - c.time > 7200
  )
    throw Error('Invalid or stale weather observation');
  if (typeof v.timezone !== 'string' || v.timezone.length > 80) throw Error('Invalid timezone');
  try {
    new Intl.DateTimeFormat('en', { timeZone: v.timezone }).format();
  } catch {
    throw Error('Invalid timezone');
  }
  const times = v.hourly?.time;
  if (!Array.isArray(times) || times.length > 96) throw Error('Invalid forecast timeline');
  const hourly = times.map((time, i) => {
    if (
      !Number.isSafeInteger(time) ||
      time < now - 36 * 3600 ||
      time > now + 4 * 86400 ||
      (i > 0 && time <= times[i - 1])
    )
      throw Error('Invalid forecast time');
    return {
      time,
      temperature: number(v.hourly.temperature_2m?.[i], -100, 70),
      feels: number(v.hourly.apparent_temperature?.[i], -120, 100),
      rain: number(v.hourly.precipitation_probability?.[i], 0, 100),
      code: code(v.hourly.weather_code?.[i]),
      day: v.hourly.is_day?.[i] === 1,
    };
  });
  if ((v.daily?.sunset?.length ?? 0) > 4) throw Error('Invalid solar timeline');
  return {
    observed: c.time,
    temperature,
    feels: number(c.apparent_temperature, -120, 100),
    code: code(c.weather_code),
    day: c.is_day === 1,
    timezone: v.timezone,
    hourly,
    sunsets: (v.daily?.sunset ?? []).filter(
      (n) => Number.isSafeInteger(n) && n > now - 36 * 3600 && n < now + 4 * 86400,
    ),
  };
}
export function forecastAt(data, hours, now = Date.now() / 1000) {
  const target = now + Math.max(1, Math.min(24, Number(hours) || 3)) * 3600;
  return data.hourly.find((h) => h.time >= target && h.time - target < 3600) ?? null;
}
export function sunset(data, now = Date.now() / 1000) {
  return data.sunsets.find((t) => t > now) ?? null;
}
export function condition(code, ru = false) {
  const text =
    code === 0
      ? ['Clear', 'Ясно']
      : code === 1 || code === 2
        ? ['Partly cloudy', 'Облачно']
        : code === 3
          ? ['Overcast', 'Пасмурно']
          : code === 45 || code === 48
            ? ['Fog', 'Туман']
            : code >= 51 && code <= 57
              ? ['Drizzle', 'Морось']
              : (code >= 71 && code <= 77) || code === 85 || code === 86
                ? ['Snow', 'Снег']
                : code >= 95
                  ? ['Thunderstorm', 'Гроза']
                  : code >= 61 && code <= 82
                    ? ['Rain', 'Дождь']
                    : ['Unknown', 'Нет данных'];
  return text[ru ? 1 : 0];
}
export const escape = (v) =>
  String(v).replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' })[c],
  );
function icon(code, day, x, y, size) {
  const sun = day
    ? '<circle cx="32" cy="26" r="12" fill="#ffe09a"/><path d="M32 5v5M32 42v5M11 26h5M48 26h5M17 11l4 4M43 37l4 4M17 41l4-4M43 15l4-4" stroke="#ffe09a" stroke-width="3" stroke-linecap="round"/>'
    : '<path d="M39 8A21 21 0 1 0 54 40A23 23 0 0 1 39 8Z" fill="#d1dcff"/>';
  const cloud =
    '<path d="M15 42C2 42 3 24 16 25C18 8 44 9 46 27C60 24 64 44 49 44H15Z" fill="#e6eefc"/>';
  const snow = [71, 73, 75, 77, 85, 86].includes(code);
  const wet = snow
    ? '<g fill="#eaf4ff"><circle cx="18" cy="52" r="3"/><circle cx="32" cy="57" r="3"/><circle cx="47" cy="52" r="3"/></g>'
    : (code >= 51 && code <= 67) || (code >= 80 && code <= 82)
      ? '<path d="M19 49l-3 7M33 49l-3 7M47 49l-3 7" stroke="#8ccaff" stroke-width="4" stroke-linecap="round"/>'
      : code >= 95
        ? '<path d="M34 36L22 51H32L27 63L46 44H35L42 36Z" fill="#ffe09a"/>'
        : code === 45 || code === 48
          ? '<path d="M10 49H54M15 56H48" stroke="#b8d3e6" stroke-width="3" stroke-linecap="round"/>'
          : '';
  return `<svg x="${x}" y="${y}" width="${size}" height="${size}" viewBox="0 0 64 64">${code === null ? '<text x="20" y="45" fill="#a5b2c9" font-size="40">?</text>' : code === 0 ? sun : (code === 1 || code === 2 ? sun : '') + cloud + wet}</svg>`;
}
export function render(
  data,
  settings = {},
  action = 'current',
  canvas = [112, 112],
  now = Date.now() / 1000,
  error = '',
) {
  const [outputWidth, outputHeight] = canvas;
  const h = outputWidth / outputHeight > 1.3 ? 128 : 112;
  const w = outputWidth / outputHeight > 1.3 ? (outputWidth / outputHeight) * h : 112;
  const ru = settings.language === 'ru';
  const wide = w / h >= 5;
  const panel = w / h > 1.3;
  const units = settings.units === 'f' ? '°F' : '°';
  const temp = (n) =>
    n === null || n === undefined
      ? '—'
      : `${Math.round(settings.units === 'f' ? (n * 9) / 5 + 32 : n)}${units}`;
  const text = (x, y, size, value, fill = '#ffffff', anchor = 'start') =>
    `<text x="${x}" y="${y}" font-family="IBM Plex Sans" font-size="${size}" font-weight="500" fill="${fill}" text-anchor="${anchor}">${escape(value)}</text>`;
  const stale = data && (!!error || now - data.observed > 7200);
  const selected = data && (action === 'forecast' ? forecastAt(data, settings.hours, now) : data);
  const place = String(settings.name || (ru ? 'Выберите город' : 'Choose city')).slice(
    0,
    wide ? 32 : panel ? 22 : 13,
  );
  let body = '';
  if (!data) {
    body =
      text(7, h * 0.28, Math.min(15, w / 11), place) +
      text(
        7,
        h * 0.57,
        Math.min(18, w / 7),
        error
          ? ru
            ? 'Нет сети'
            : 'Offline'
          : !validLocation(settings)
            ? ru
              ? 'Настройки'
              : 'Settings'
            : ru
              ? 'Загрузка…'
              : 'Loading…',
        '#bfcee6',
      );
  } else if (wide) {
    body =
      text(18, 23, 17, place, '#c6d5ee') +
      icon(selected?.code ?? null, selected?.day, 145, 9, 55) +
      text(18, 82, 51, temp(selected?.temperature));
    body += text(18, 112, 15, `${ru ? 'Ощущ.' : 'Feels'} ${temp(selected?.feels)}`, '#cedbf0');
    for (let i = 0; i < 3; i++) {
      const hours = [Number(settings.hours) || 3, 6, 12][i],
        f = forecastAt(data, hours, now);
      const x = w * 0.29 + i * w * 0.19;
      body +=
        text(x, 24, 16, `+${hours} ${ru ? 'ч' : 'h'}`, '#b7c9e8') +
        icon(f?.code ?? null, f?.day, x + 70, 30, 44) +
        text(x, 77, 31, temp(f?.temperature)) +
        text(x, 108, 14, `${ru ? 'Осадки' : 'Rain'} ${f?.rain ?? '—'}%`, '#b7c9e8');
    }
    const set = sunset(data, now);
    body +=
      text(w - 14, 24, 14, ru ? 'Закат' : 'Sunset', '#b7c9e8', 'end') +
      text(
        w - 14,
        55,
        22,
        set
          ? new Intl.DateTimeFormat(ru ? 'ru' : 'en', {
              timeZone: data.timezone,
              hour: '2-digit',
              minute: '2-digit',
              hour12: false,
            }).format(set * 1000)
          : '—',
        '#ffffff',
        'end',
      );
  } else if (panel) {
    body = text(10, 25, Math.min(19, w / 13), place, '#e4ded4');
    body += icon(selected?.code ?? null, selected?.day, w - 82, 36, 68);
    body += text(9, 85, 57, temp(selected?.temperature));
    body += text(10, 115, 20,
      action === 'forecast' ? `+${Number(settings.hours) || 3} ${ru ? 'ч' : 'h'}` : `${ru ? 'Ощущ.' : 'Feels'} ${temp(selected?.feels)}`,
      '#ded8cf');
  } else {
    body = text(7, h * 0.16, Math.min(14, w / 12), place, '#d8e4f7');
    body += icon(
      selected?.code ?? null,
      selected?.day,
      panel ? w - 59 : w - 48,
      h * 0.24,
      panel ? 51 : 43,
    );
    body += text(6, h * 0.63, panel ? 45 : 37, temp(selected?.temperature));
    body += text(
      7,
      h * 0.8,
      Math.min(13, w / 8.5),
      action === 'forecast'
        ? `+${Number(settings.hours) || 3} ${ru ? 'ч' : 'h'}`
        : `${ru ? 'Ощущ.' : 'Feels'} ${temp(selected?.feels)}`,
      '#d7e4f5',
    );
    const set = sunset(data, now);
    const bottom =
      action === 'forecast'
        ? `${ru ? 'Осадки' : 'Rain'} ${selected?.rain ?? '—'}%`
        : set
          ? `${ru ? 'Закат' : 'Sunset'} ${new Intl.DateTimeFormat(ru ? 'ru' : 'en', { timeZone: data.timezone, hour: '2-digit', minute: '2-digit', hour12: false }).format(set * 1000)}`
          : condition(selected?.code ?? null, ru);
    body += text(7, h * 0.95, Math.min(12, w / 10), bottom, '#b8d1ec');
  }
  if (stale) body += `<rect x="${w - 11}" y="4" width="7" height="7" rx="2" fill="#ffba78"/>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${outputWidth}" height="${outputHeight}" viewBox="0 0 ${w} ${h}"><rect width="${w}" height="${h}" fill="#20201f"/>${body}</svg>`;
}
export async function getJson(url, fetcher = fetch) {
  const response = await fetcher(url, {
    signal: AbortSignal.timeout(8000),
    redirect: 'error',
    headers: { 'User-Agent': 'DeckWeave-Weather/0.1' },
  });
  if (!response.ok) throw Error(`HTTP ${response.status}`);
  const reader = response.body.getReader();
  let size = 0;
  const chunks = [];
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.length;
      if (size > 262144) throw Error('Response too large');
      chunks.push(value);
    }
  } finally {
    await reader.cancel();
  }
  return JSON.parse(Buffer.concat(chunks).toString('utf8'));
}
export function forecastUrl(s) {
  if (!validLocation(s)) throw Error('Choose a location');
  const u = new URL('https://api.open-meteo.com/v1/forecast');
  for (const [k, v] of Object.entries({
    latitude: s.latitude,
    longitude: s.longitude,
    current: 'temperature_2m,apparent_temperature,weather_code,is_day',
    hourly: 'temperature_2m,apparent_temperature,precipitation_probability,weather_code,is_day',
    daily: 'sunset,sunrise',
    forecast_days: 3,
    timeformat: 'unixtime',
    timezone: 'auto',
  }))
    u.searchParams.set(k, String(v));
  return u;
}
export async function searchPlaces(query, language = 'en', fetcher = fetch) {
  if (typeof query !== 'string' || query.trim().length < 2 || query.length > 120) return [];
  const url = new URL('https://geocoding-api.open-meteo.com/v1/search');
  url.search = new URLSearchParams({
    name: query.trim(),
    count: '8',
    language: language === 'ru' ? 'ru' : 'en',
    format: 'json',
  });
  const v = await getJson(url, fetcher);
  return (v.results ?? [])
    .slice(0, 8)
    .map((p) => ({
      name: p.name,
      latitude: p.latitude,
      longitude: p.longitude,
      country: p.country ?? '',
      region: p.admin1 ?? '',
    }))
    .filter(validLocation);
}
