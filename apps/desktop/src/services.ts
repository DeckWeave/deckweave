import { invoke } from '@tauri-apps/api/core';
export interface Placement {
  anchor: string;
  monitor: string | null;
  x: number;
  y: number;
  width: number;
  height: number;
  background: string;
  foreground: string;
  opacity: number;
  click_through: boolean;
}
export interface PanelSpec {
  title: string;
  content:
    | { type: 'note'; text: string }
    | { type: 'metric'; value: string; detail: string }
    | { type: 'timer'; seconds: number; sound?: string | null; volume?: number };
  placement: Placement;
}
export interface PanelRecord {
  id: string;
  service_id: string;
  owner: string;
  origin: string;
  local_key: string;
  revision: number;
  open: boolean;
  spec: PanelSpec;
}
export interface TimerRun {
  run_id: string;
  revision: number;
  status: 'idle' | 'running' | 'paused' | 'done';
  remaining_ms: number;
  due_ms: number | null;
  sound: string | null;
  volume: number;
}
export interface PanelView {
  locale: string;
  panel: PanelRecord;
  online: boolean;
  value: { value?: string; detail?: string } | null;
  timer: TimerRun | null;
  remaining_ms: number | null;
  error: string | null;
}
export interface ServiceSnapshot {
  locale: string;
  panels: PanelView[];
  unavailable: { id: string; error: string }[];
  error: string | null;
  outcomes: { status: string; error: string | null }[];
}
export const emptyServices = (): ServiceSnapshot => ({
  locale: 'ru',
  panels: [],
  unavailable: [],
  error: null,
  outcomes: [],
});
export const defaultPlacement = (): Placement => ({
  anchor: 'top_right',
  monitor: null,
  x: 16,
  y: 16,
  width: 320,
  height: 160,
  background: '#252525',
  foreground: '#ededed',
  opacity: 100,
  click_through: false,
});
export const service = (operation: string, payload: unknown) =>
  invoke<any>('host_service_command', { operation, payload });
export const serviceError = (error: unknown) =>
  typeof error === 'object' && error && 'message' in error ? String(error.message) : String(error);
