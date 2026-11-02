import type { LibraryPreferences } from './types';

export const libraryDefaults = (): LibraryPreferences => ({
  hidden: [],
  order: [],
  show_development: false,
});
export function ordered<T>(items: T[], key: (item: T) => string, order: string[]): T[] {
  const rank = new Map(order.map((id, index) => [id, index]));
  return [...items].sort(
    (a, b) =>
      (rank.get(key(a)) ?? Number.MAX_SAFE_INTEGER) - (rank.get(key(b)) ?? Number.MAX_SAFE_INTEGER),
  );
}
export function moved(
  order: string[],
  siblings: string[],
  id: string,
  direction: -1 | 1,
): string[] {
  const current = ordered(siblings, (key) => key, order);
  const index = current.indexOf(id),
    target = index + direction;
  if (index < 0 || target < 0 || target >= current.length) return order;
  [current[index], current[target]] = [current[target], current[index]];
  return [...order.filter((key) => !siblings.includes(key)), ...current];
}

/** Reorder one sibling set without discarding preferences for hidden/other groups. */
export function placed(
  order: string[],
  siblings: string[],
  id: string,
  target: string,
  after = false,
): string[] {
  if (id === target || !siblings.includes(id) || !siblings.includes(target)) return order;
  const current = ordered(siblings, (key) => key, order).filter((key) => key !== id);
  current.splice(current.indexOf(target) + Number(after), 0, id);
  return [...order.filter((key) => !siblings.includes(key)), ...current];
}
