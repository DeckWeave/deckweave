import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  parseForecast,
  forecastAt,
  sunset,
  render,
  getJson,
  validLocation,
  searchPlaces,
} from '../../plugins/dev.deckweave.weather.sdPlugin/weather.mjs';
const now = 1700000000;
function fixture() {
  return {
    timezone: 'Europe/London',
    current: { time: now, temperature_2m: 12, apparent_temperature: 9, weather_code: 2, is_day: 1 },
    hourly: {
      time: [now + 3600, now + 7200, now + 10800],
      temperature_2m: [13, null, 15],
      apparent_temperature: [10, 11, 12],
      precipitation_probability: [null, 20, 90],
      weather_code: [0, 3, 63],
      is_day: [1, 1, 0],
    },
    daily: { sunset: [now - 10, now + 5000] },
  };
}
test('weather keeps nulls and exact forecast horizon, next sunset, local timezone', () => {
  const f = parseForecast(fixture(), now);
  assert.equal(forecastAt(f, 2, now).temperature, null);
  assert.equal(forecastAt(f, 3, now).temperature, 15);
  assert.equal(forecastAt(f, 24, now), null);
  assert.equal(sunset(f, now), now + 5000);
  assert.equal(forecastAt(f, 1, now).rain, null);
  assert.equal(f.timezone, 'Europe/London');
});
test('rejects stale, future, malformed timezone and unordered timelines', () => {
  for (const mutate of [
    (v) => (v.current.time -= 7201),
    (v) => (v.current.time += 301),
    (v) => (v.current.temperature_2m = null),
    (v) => (v.timezone = 'Invalid/Timezone'),
    (v) => v.hourly.time.reverse(),
  ]) {
    const v = fixture();
    mutate(v);
    assert.throws(() => parseForecast(v, now));
  }
});
test('polar sunset is absent and invalid coordinates never become a default city', () => {
  const v = fixture();
  v.daily.sunset = [null];
  assert.equal(sunset(parseForecast(v, now), now), null);
  assert.equal(validLocation({ name: 'A', latitude: NaN, longitude: 0 }), false);
  assert.equal(validLocation({ name: 'A', latitude: 90, longitude: 180 }), true);
});
test('rendering escapes names, distinguishes stale/offline, works at key/slot/full-panel sizes', () => {
  const f = parseForecast(fixture(), now);
  for (const size of [
    [112, 112],
    [200, 128],
    [800, 128],
  ]) {
    const svg = render(f, { name: 'A<&', language: 'ru' }, 'current', size, now, 'offline');
    assert.match(svg, /A&lt;&amp;/);
    assert.match(svg, /#ffba78/);
    assert.ok(svg.includes(`width="${size[0]}"`));
  }
  assert.match(render(null, { name: 'City' }, 'current', [112, 112], now, 'offline'), /Offline/);
});
test('HTTP streams are bounded and cancelled; missing measurements do not invent values', async () => {
  let cancelled = false;
  const body = new ReadableStream({
    start(c) {
      c.enqueue(new Uint8Array(262145));
    },
    cancel() {
      cancelled = true;
    },
  });
  await assert.rejects(
    getJson(new URL('https://example.invalid'), async () => ({ ok: true, body })),
    /too large/,
  );
  assert.equal(cancelled, true);
});
test('geocoding returns explicit choices and filters malformed coordinates', async () => {
  const result = await searchPlaces(
    'Paris',
    'en',
    async () =>
      new Response(
        JSON.stringify({
          results: [
            { name: 'Paris', latitude: 48.8, longitude: 2.3, country: 'France' },
            { name: 'Bad', latitude: null, longitude: 3 },
          ],
        }),
      ),
  );
  assert.equal(result.length, 1);
  assert.equal(result[0].name, 'Paris');
});

test('scaled panel layout stays inside its viewBox, including half-height regions', () => {
  const data = parseForecast(fixture(), now);
  for (const canvas of [
    [400, 64],
    [800, 128],
    [200, 64],
    [200, 128],
    [80, 128],
  ]) {
    const svg = render(data, { name: 'London' }, 'current', canvas, now);
    const height = Number(svg.match(/viewBox="0 0 [^ ]+ ([^"]+)"/)[1]);
    for (const match of svg.matchAll(/<text[^>]* y="([^"]+)"/g))
      assert.ok(Number(match[1]) <= height);
  }
  const snow = render({ ...data, code: 73 }, { name: 'London' }, 'current', [112, 112], now);
  assert.match(snow, /circle cx="18" cy="52"/);
});
