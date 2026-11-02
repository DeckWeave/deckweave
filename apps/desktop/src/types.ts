export type Locale = 'ru' | 'en';
export interface Preferences {
  library: LibraryPreferences;
  disabled_plugins: string[];
  last_device: string | null;
  smart_profiles: boolean;
  preview_scale: number;
  compact: boolean;
}
export interface LibraryPreferences {
  hidden: string[];
  order: string[];
  show_development: boolean;
}
export type ActionKind = 'counter' | 'toggle' | 'plugin' | 'program';
export type Plan =
  | { type: 'page_cycle'; direction: 'previous' | 'next' | 'dial' }
  | {
      type: 'dial_bindings';
      left: Binding | null;
      right: Binding | null;
      press: Binding | null;
      tap: Binding | null;
    }
  | { type: 'page'; page_id: string }
  | { type: 'profile'; profile_id: string }
  | { type: 'back' }
  | { type: 'lighting_preset'; preset_id: string; temporary_ms: number | null }
  | { type: 'lighting_return' }
  | { type: 'launch'; program: string; args: string[] }
  | { type: 'hotkey'; keys: string[] }
  | { type: 'sequence'; steps: { delay_ms: number; action: Binding }[] }
  | { type: 'switch'; states: Binding[] }
  | { type: 'dial_stack' | 'action_wheel'; actions: Binding[] }
  | {
      type: 'key_logic';
      single: Binding | null;
      double: Binding | null;
      hold: Binding | null;
      double_ms: number;
      hold_ms: number;
    };
export interface PluginAction {
  controller?: 'Keypad' | 'Encoder' | 'Panel' | null;
  package: string;
  action: string;
  settings: Record<string, unknown>;
}
export interface PluginPackage {
  id: string;
  name: string;
  version: string;
  error: string | null;
  resource_only: boolean;
  development: boolean;
  actions: {
    id: string;
    name: string;
    visible: boolean;
    controllers: string[];
    inspector: string | null;
  }[];
}
export type ControlKind = 'key' | 'dial' | 'touch';
export interface Binding {
  id: string;
  kind: ActionKind;
  label: string;
  plugin: PluginAction | null;
  plan: Plan | null;
}
export interface Page {
  close_after_action: boolean;
  panel: PanelLayout | null;
  id: string;
  name: string;
  bindings: Record<string, Binding>;
  parent: string | null;
}
export interface Profile {
  lighting_preset: string | null;
  id: string;
  name: string;
  active_page_id: string;
  pages: Page[];
  application: string | null;
}
export interface DeviceHardwareSettings {
  screen_brightness?: number;
  dial_brightness?: number;
  vibration?: boolean;
}
export interface DeviceExclusion {
  provider_id: string;
  hardware_key: string;
}
export interface ProviderSetting {
  id: string;
  label: { en?: string; ru?: string };
  type: 'boolean' | 'number' | 'choice';
  min?: number;
  max?: number;
  step?: number;
  choices?: (string | number | { value: string | number; label?: { en?: string; ru?: string } })[];
  group?: string;
  persistence?: 'volatile' | 'onboard';
  readback?: boolean;
}
export interface ProviderDefinition {
  keyboard?: {
    onboardMappings?: {
      settingId: string;
      address: string;
      layers: string[];
      bindingFormat: string;
      readback?: boolean;
      persistence?: string;
    };
    actuation?: {
      settingId: string;
      address: string;
      minUm: number;
      maxUm: number;
      stepUm: number;
      rapidMinUm?: number;
      fields?: string[];
      readback?: boolean;
      persistence?: string;
    };
  };
  controls?: { id: string; label?: string | { en?: string; ru?: string } }[];
  settings: ProviderSetting[];
  lighting?: {
    id: string;
    label?: { en?: string; ru?: string };
    kind?: string;
    controls?: string[];
    settings?: string[];
    staticColors?: { settingId: string; address: string };
  }[];
  displays?: { id: string; width: number; height: number; mime: string[] }[];
  [key: string]: unknown;
}
export interface ProviderDescriptor {
  geometry: Snapshot['geometries'][string];
  definition: ProviderDefinition;
}
export interface LightingPreset {
  id: string;
  name: string;
  settings: Record<string, unknown>;
}
export interface Device {
  lighting_presets: LightingPreset[];
  manual_lighting_preset: string | null;
  provider_descriptor: ProviderDescriptor | null;
  provider_settings: Record<string, unknown>;
  hardware_settings: DeviceHardwareSettings;
  id: string;
  name: string;
  provider_id: string;
  hardware_key: string | null;
  profile: Profile;
  profiles: Profile[];
  pinned: Record<string, Binding>;
  default_profile: string | null;
}
export interface Control {
  pointer: boolean;
  id: string;
  kind: ControlKind;
  rect: [number, number, number, number];
  number: number;
}
export interface Snapshot {
  lighting: Record<
    string,
    {
      source: string;
      manual_editing: boolean;
      temporary: boolean;
      preset_id: string | null;
      error: string | null;
    }
  >;
  hardware_settings?: Record<string, import('./device-settings-i18n').N4SettingsStatus>;
  connections: Record<
    string,
    {
      state: 'virtual' | 'offline' | 'connected' | 'error';
      generation: number;
      error: string | null;
      input_events: number;
      output_frames: number;
      last_input: string | null;
    }
  >;
  provider_error: string | null;
  key_images: Record<
    string,
    Record<string, { page_id: string; binding_id: string | null; data_url: string }>
  >;
  configuration: {
    revision: number;
    locale: Locale;
    preferences: Preferences;
    devices: Device[];
    removed_devices: Device[];
    device_exclusions: DeviceExclusion[];
  };
  geometries: Record<
    string,
    {
      width: number;
      height: number;
      surface: [number, number, number, number];
      surface_pixels: [number, number];
      controls: Control[];
    }
  >;
  feedback: {
    device_id: string;
    binding_id: string;
    value: number;
    active: boolean;
    title: string | null;
    image: string | null;
    error: string | null;
    display: string | null;
    running: boolean;
  }[];
  pressed: { device_id: string; control_id: string }[];
  editing_windows: number;
  smart_profiles_paused: boolean;
  event_sequence: number;
}
export type InputEvent =
  | { type: 'down' | 'up' | 'cancel' | 'click' }
  | { type: 'rotate'; ticks: number }
  | { type: 'swipe'; right: boolean }
  | { type: 'pointer'; phase: 'down' | 'move' | 'up' | 'cancel'; x: number; y: number | null };
export interface PanelRegion {
  id: string;
  name: string;
  rect: [number, number, number, number];
  dials: string[];
}
export interface PanelLayout {
  regions: PanelRegion[];
  swipe_pages: boolean;
}
export type Command =
  | { type: 'set_preferences'; preferences: Preferences; expected_revision: number }
  | {
      type: 'device_preferences';
      device_id: string;
      default_profile: string | null;
      expected_revision: number;
    }
  | {
      type: 'folder_behavior';
      device_id: string;
      page_id: string;
      close_after_action: boolean;
      expected_revision: number;
    }
  | {
      type: 'set_panel_layout';
      device_id: string;
      page_id: string;
      layout: PanelLayout | null;
      expected_revision: number;
    }
  | {
      type: 'configure_program';
      device_id: string;
      page_id: string;
      control_id: string;
      plan: Plan;
      label: string;
      expected_revision: number;
    }
  | {
      type: 'rename_binding';
      device_id: string;
      binding_id: string;
      label: string;
      expected_revision: number;
    }
  | {
      type: 'set_pinned';
      device_id: string;
      control_id: string;
      pinned: boolean;
      expected_revision: number;
    }
  | { type: 'add_profile'; device_id: string; name: string; expected_revision: number }
  | { type: 'select_profile'; device_id: string; profile_id: string; expected_revision: number }
  | {
      type: 'profile_application';
      device_id: string;
      application: string | null;
      expected_revision: number;
    }
  | {
      type: 'add_folder';
      device_id: string;
      control_id: string;
      name: string;
      expected_revision: number;
    }
  | { type: 'cancel_actions'; device_id: string }
  | {
      type: 'configure_plugin';
      device_id: string;
      page_id: string;
      control_id: string;
      plugin: PluginAction;
      label: string;
      expected_revision: number;
    }
  | {
      type: 'plugin_settings';
      device_id: string;
      binding_id: string;
      settings: Record<string, unknown>;
      expected_revision: number;
    }
  | {
      type: 'save_lighting_preset';
      device_id: string;
      preset: LightingPreset;
      expected_revision: number;
    }
  | {
      type: 'delete_lighting_preset' | 'select_lighting_preset';
      device_id: string;
      preset_id: string;
      expected_revision: number;
    }
  | {
      type: 'profile_lighting_preset';
      device_id: string;
      preset_id: string | null;
      expected_revision: number;
    }
  | { type: 'release_lighting_edit' | 'return_to_manual_lighting'; device_id: string }
  | {
      type: 'set_provider_settings';
      device_id: string;
      changes: Record<string, unknown>;
      expected_revision: number;
    }
  | { type: 'add_device'; expected_revision: number }
  | {
      type: 'remove_device' | 'restore_device' | 'forget_device';
      device_id: string;
      expected_revision: number;
    }
  | { type: 'allow_hardware'; provider_id: string; hardware_key: string; expected_revision: number }
  | {
      type: 'set_device_hardware_settings';
      device_id: string;
      settings: DeviceHardwareSettings;
      expected_revision: number;
    }
  | { type: 'rename_device'; device_id: string; name: string; expected_revision: number }
  | {
      type: 'configure_control';
      device_id: string;
      page_id: string;
      control_id: string;
      kind: ActionKind | null;
      label: string;
      expected_revision: number;
    }
  | { type: 'add_page'; device_id: string; expected_revision: number }
  | { type: 'select_page'; device_id: string; page_id: string; expected_revision: number }
  | { type: 'set_locale'; locale: Locale; expected_revision: number }
  | { type: 'input'; device_id: string; control_id: string; event: InputEvent };
