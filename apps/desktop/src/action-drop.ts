import type { Binding, Plan } from './types';

export type LibraryAction =
  | { type: 'builtin'; kind: 'counter' | 'toggle' }
  | { type: 'plan'; plan: Plan['type'] }
  | { type: 'folder' }
  | { type: 'plugin'; package: string; action: string; label: string; controllers: string[] };
export const actionDropContext = Symbol('action library');
export type DropScope = { controller: 'Keypad' | 'Encoder'; depth: number; inSequence: boolean };
export function acceptsNested(item: LibraryAction, scope: DropScope): boolean {
  if (item.type === 'folder') return false;
  if (item.type === 'plugin') return item.controllers.includes(scope.controller);
  if (item.type !== 'plan') return true;
  if (['key_logic', 'dial_bindings', 'dial_stack', 'action_wheel'].includes(item.plan))
    return false;
  if (scope.inSequence && ['sequence', 'switch'].includes(item.plan)) return false;
  return scope.depth < 6;
}
export interface ActionLibraryBridge {
  selected: () => LibraryAction | null;
  enabled: (item: LibraryAction) => boolean;
  make: (item: LibraryAction, controller: 'Keypad' | 'Encoder') => Binding | null;
}
type Target = {
  node: HTMLElement;
  accepts: (item: LibraryAction) => boolean;
  identity: () => string;
  drop: (item: LibraryAction) => void;
};
const targets = new WeakMap<Element, Target>();
export function registerDrop(node: HTMLElement, target: Omit<Target, 'node'>) {
  targets.set(node, { ...target, node });
  return { destroy: () => targets.delete(node) };
}
export function findDrop(x: number, y: number): Target | undefined {
  const node = document.elementFromPoint(x, y)?.closest('[data-action-drop]');
  return node ? targets.get(node) : undefined;
}
export function clearDrop(target?: Target) {
  if (!target) return;
  delete target.node.dataset.dropActive;
  delete target.node.dataset.dropRejected;
}
export function reordered<T>(list: T[], from: number, to: number): T[] {
  if (from < 0 || to < 0 || from >= list.length || to >= list.length || from === to) return list;
  const next = [...list];
  next.splice(to, 0, next.splice(from, 1)[0]);
  return next;
}
