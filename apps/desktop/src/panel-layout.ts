import type { PanelRegion } from './types';
export function overlaps(a: number[], b: number[]): boolean {
  return (
    a[0] < b[0] + b[2] - 1e-9 &&
    a[0] + a[2] > b[0] + 1e-9 &&
    a[1] < b[1] + b[3] - 1e-9 &&
    a[1] + a[3] > b[1] + 1e-9
  );
}
export function validRegion(rect: number[], siblings: PanelRegion[], id: string): boolean {
  const [x, y, w, h] = rect;
  return (
    rect.every(Number.isFinite) &&
    x >= 0 &&
    y >= 0 &&
    w >= 13 / 800 &&
    h >= 13 / 128 &&
    x + w <= 1 + 1e-9 &&
    y + h <= 1 + 1e-9 &&
    siblings.every((r) => r.id === id || !overlaps(rect, r.rect))
  );
}
export function freeRegion(regions: PanelRegion[]): PanelRegion['rect'] | null {
  for (let y = 0; y < 2; y++)
    for (let x = 0; x < 8; x++) {
      const rect: PanelRegion['rect'] = [x / 8, y / 2, 1 / 8, 1 / 2];
      if (regions.every((r) => !overlaps(rect, r.rect))) return rect;
    }
  return null;
}
export function splitRegion(
  region: PanelRegion,
): [PanelRegion['rect'], PanelRegion['rect']] | null {
  const [x, y, w, h] = region.rect;
  if (w / 2 < 13 / 800) return null;
  return [
    [x, y, w / 2, h],
    [x + w / 2, y, w / 2, h],
  ];
}
