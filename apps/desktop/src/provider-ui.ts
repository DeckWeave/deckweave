import type { ProviderDefinition } from './types';
export interface ProviderSessionState {
  generation?: number;
  settings_edit?: number;
  read_sequence?: number;
  recovery?: { state?: string; pending?: unknown[] } | null;
  state: string;
  observed: Record<string, unknown>;
  requested: Record<string, unknown>;
  evidence: Record<string, string>;
  error: string | null;
}
export interface ProviderInventory {
  packages: { id: string; name: string; version: string; error: string | null }[];
  discovered: {
    provider_id: string;
    instance_id: string;
    name: string;
    descriptor: ProviderDefinition;
    device_id: string | null;
    state: string;
    error: string | null;
  }[];
  sessions: Record<string, ProviderSessionState>;
}
export const emptyProviderInventory = (): ProviderInventory => ({
  packages: [],
  discovered: [],
  sessions: {},
});
