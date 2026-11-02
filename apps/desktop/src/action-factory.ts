import type { Binding, Plan, Device } from './types';
export const planTypes = [
  'page_cycle',
  'page',
  'profile',
  'back',
  'lighting_preset',
  'lighting_return',
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
    case 'page_cycle':
      return { type, direction: 'next' };
    case 'dial_bindings':
      return { type, left: null, right: null, press: null, tap: null };
    case 'page':
      return { type, page_id: device.profile.active_page_id };
    case 'profile':
      return { type, profile_id: device.profile.id };
    case 'back':
    case 'lighting_return':
      return { type };
    case 'lighting_preset':
      return {
        type,
        preset_id: device.lighting_presets[0]?.id ?? '0'.repeat(32),
        temporary_ms: null,
      };
    case 'launch':
      return { type, program: 'notepad.exe', args: [] };
    case 'hotkey':
      return { type, keys: ['Ctrl', 'C'] };
    case 'sequence':
      return { type, steps: [] };
    case 'switch':
      return {
        type,
        states: ['1', '2'].map((label) => ({
          ...action('program', label),
          plan: { type: 'sequence', steps: [] },
        })),
      };
    case 'key_logic':
      return {
        type,
        single: { ...action('program', 'Single'), plan: { type: 'sequence', steps: [] } },
        double: null,
        hold: null,
        double_ms: 350,
        hold_ms: 650,
      };
    case 'dial_stack':
    case 'action_wheel':
      return {
        type,
        actions: ['1', '2'].map((label) => ({
          ...action('program', label),
          plan: { type: 'sequence', steps: [] },
        })),
      };
  }
}
