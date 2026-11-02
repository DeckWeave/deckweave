import type { ProviderColorChannels, ProviderCondition, ProviderSetting } from './types';

/** Presentation uses observed values only. A queued request is not device state. */
export function conditionMatches(
  condition: ProviderCondition | undefined,
  observed: Record<string, unknown>,
) {
  return (
    !condition ||
    (Object.hasOwn(observed, condition.settingId) &&
      condition.values.some((v) => v === observed[condition.settingId]))
  );
}
export function colorVisible(
  color: ProviderColorChannels,
  settings: ProviderSetting[],
  observed: Record<string, unknown>,
) {
  return (
    conditionMatches(color.visibleWhen, observed) &&
    [color.r, color.g, color.b].every((id) => {
      const setting = settings.find((s) => s.id === id);
      return setting && conditionMatches(setting.visibleWhen, observed);
    })
  );
}
export function colorHex(
  color: ProviderColorChannels,
  observed: Record<string, unknown>,
): string | null {
  const values = [color.r, color.g, color.b].map((id) => observed[id]);
  return values.every((v) => typeof v === 'number' && Number.isInteger(v) && v >= 0 && v <= 255)
    ? '#' + values.map((v) => Number(v).toString(16).padStart(2, '0')).join('')
    : null;
}
/** All channels form one edit/one onboard transaction; never three independent writes. */
export function colorChanges(
  color: ProviderColorChannels,
  hex: string,
): Record<string, number> | null {
  if (!/^#[0-9a-f]{6}$/i.test(hex)) return null;
  return Object.fromEntries(
    [color.r, color.g, color.b].map((id, i) => [id, parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16)]),
  );
}

export function colorForeground(hex: string | null): string {
  if (!hex || !/^#[0-9a-f]{6}$/i.test(hex)) return '#eef1f8';
  const rgb = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722 > 145 ? '#11141b' : '#ffffff';
}

/** A partial per-key patch may be confirmed by a larger readback map. */
export function patchMatches(
  changes: Record<string, unknown>,
  observed: Record<string, unknown>,
): boolean {
  return Object.entries(changes).every(([id, value]) => {
    const actual = observed[id];
    if (value !== null && typeof value === 'object' && !Array.isArray(value))
      return (
        actual !== null &&
        typeof actual === 'object' &&
        !Array.isArray(actual) &&
        patchMatches(value as Record<string, unknown>, actual as Record<string, unknown>)
      );
    return Object.hasOwn(observed, id) && JSON.stringify(value) === JSON.stringify(actual);
  });
}

export function hexHsv(hex: string): { h: number; s: number; v: number } {
  if (!/^#[0-9a-f]{6}$/i.test(hex)) return { h: 0, s: 0, v: 100 };
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const max = Math.max(r, g, b),
    min = Math.min(r, g, b),
    d = max - min;
  const h =
    d === 0 ? 0 : max === r ? ((g - b) / d + 6) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return { h: h * 60, s: max === 0 ? 0 : (d / max) * 100, v: max * 100 };
}
export function hsvHex(h: number, s: number, v: number): string {
  h = ((h % 360) + 360) % 360;
  s = Math.max(0, Math.min(100, s)) / 100;
  v = Math.max(0, Math.min(100, v)) / 100;
  const c = v * s,
    x = c * (1 - Math.abs(((h / 60) % 2) - 1)),
    m = v - c;
  const rgb =
    h < 60
      ? [c, x, 0]
      : h < 120
        ? [x, c, 0]
        : h < 180
          ? [0, c, x]
          : h < 240
            ? [0, x, c]
            : h < 300
              ? [x, 0, c]
              : [c, 0, x];
  return (
    '#' +
    rgb
      .map((n) =>
        Math.round((n + m) * 255)
          .toString(16)
          .padStart(2, '0'),
      )
      .join('')
  );
}
