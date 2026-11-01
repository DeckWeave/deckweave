export type Locale = 'ru' | 'en';
export type ActionKind = 'counter' | 'toggle';
export type ControlKind = 'key' | 'dial' | 'touch';
export interface Binding {
  id: string;
  kind: ActionKind;
  label: string;
}
export interface Page {
  id: string;
  name: string;
  bindings: Record<string, Binding>;
}
export interface Device {
  id: string;
  name: string;
  provider_id: string;
  hardware_key: string | null;
  profile: { id: string; name: string; active_page_id: string; pages: Page[] };
}
export interface Control {
  id: string;
  kind: ControlKind;
  rect: [number, number, number, number];
  number: number;
}
export interface Snapshot {
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
  configuration: { revision: number; locale: Locale; devices: Device[] };
  geometries: Record<
    string,
    {
      width: number;
      height: number;
      surface: [number, number, number, number];
      controls: Control[];
    }
  >;
  feedback: { device_id: string; binding_id: string; value: number; active: boolean }[];
  pressed: { device_id: string; control_id: string }[];
  editing_windows: number;
  smart_profiles_paused: boolean;
  event_sequence: number;
}
export type InputEvent =
  { type: 'down' | 'up' | 'cancel' | 'click' } | { type: 'rotate'; ticks: number };
export type Command =
  | { type: 'add_device'; expected_revision: number }
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
