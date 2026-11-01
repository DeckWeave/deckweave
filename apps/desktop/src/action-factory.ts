import type { Binding, Plan, Device } from './types';
export const planTypes = [
  'page',
  'profile',
  'back',
  'launch',
  'hotkey',
  'sequence',
  'switch',
  'key_logic',
  'dial_stack',
  'dial_bindings',
  'action_wheel',
] as const;
export function action(kind = 'counter', label = 'Counter'): Binding {
  return {
    id: crypto.randomUUID().replaceAll('-', ''),
    kind: kind as Binding['kind'],
    label,
    plugin: null,
    plan: null,
  };
}
export function plan(type: Plan['type'], device: Device): Plan {
  switch (type) {
    case 'dial_bindings':
      return { type, left: null, right: null, press: null, tap: null };
    case 'page':
      return { type, page_id: device.profile.active_page_id };
    case 'profile':
      return { type, profile_id: device.profile.id };
    case 'back':
      return { type };
    case 'launch':
      return { type, program: 'notepad.exe', args: [] };
    case 'hotkey':
      return { type, keys: ['Ctrl', 'C'] };
    case 'sequence':
      return { type, steps: [{ delay_ms: 0, action: action() }] };
    case 'switch':
      return { type, states: [action('counter', 'A'), action('counter', 'B')] };
    case 'key_logic':
      return {
        type,
        single: action('counter', 'Single'),
        double: action('counter', 'Double'),
        hold: action('counter', 'Hold'),
        double_ms: 350,
        hold_ms: 650,
      };
    case 'dial_stack':
    case 'action_wheel':
      return { type, actions: [action('counter', 'A'), action('counter', 'B')] };
  }
}
