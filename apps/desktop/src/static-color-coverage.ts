export interface StaticRgb {
  r: number;
  g: number;
  b: number;
}
/** Only current readback can establish the guarded address coverage. */
export function observedStaticColors(
  value: unknown,
  declaredControlIds: readonly string[],
): Record<string, StaticRgb> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  const record = value as Record<string, unknown>;
  const colors: Record<string, StaticRgb> = {};
  for (const id of declaredControlIds) {
    if (!Object.hasOwn(record, id)) continue;
    const entry = record[id];
    if (!entry || typeof entry !== 'object' || Array.isArray(entry)) continue;
    const rgb = entry as Record<string, unknown>;
    if (
      !['r', 'g', 'b'].every(
        (channel) =>
          typeof rgb[channel] === 'number' &&
          Number.isInteger(rgb[channel]) &&
          Number(rgb[channel]) >= 0 &&
          Number(rgb[channel]) <= 255,
      )
    )
      continue;
    colors[id] = { r: Number(rgb.r), g: Number(rgb.g), b: Number(rgb.b) };
  }
  return colors;
}
export function unsupportedStaticColors(
  value: unknown,
  coverage: Record<string, StaticRgb>,
): boolean {
  if (value === undefined) return false;
  if (!value || typeof value !== 'object' || Array.isArray(value)) return true;
  return Object.keys(value).some((id) => !Object.hasOwn(coverage, id));
}
