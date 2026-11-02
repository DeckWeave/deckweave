import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { Command, InputEvent, Snapshot, PluginPackage } from './types';

type SettingsContext = {
  device_id: string | null;
  section: 'general' | 'devices' | 'plugins' | 'about';
};
export class Client {
  snapshot = $state<Snapshot | null>(null);
  busy = $state(false);
  error = $state('');
  plugins = $state<PluginPackage[]>([]);
  settingsDevice = $state<string | null>(null);
  settingsSection = $state<SettingsContext['section']>('general');
  settingsRequest = $state(0);
  available = isTauri();
  deviceId = this.available ? getCurrentWindow().label.replace(/^device-/, '') : 'main';
  private queue: Promise<unknown> = Promise.resolve();
  private unlisten?: UnlistenFn;
  private unlistenSettings?: UnlistenFn;
  private unlistenPlugins?: UnlistenFn;
  private stopped = false;
  private refreshing?: Promise<void>;
  private refreshAgain = false;

  async start() {
    if (!this.available) return;
    try {
      this.unlistenPlugins = await listen('deckweave:plugins', () => void this.reloadPlugins());
      this.unlistenSettings = await listen<SettingsContext>('deckweave:settings', (event) => {
        this.settingsDevice = event.payload.device_id;
        this.settingsSection = event.payload.section;
        this.settingsRequest++;
      });
      if (this.deviceId === 'main') {
        const context = await invoke<SettingsContext>('settings_context');
        this.settingsDevice = context.device_id;
        this.settingsSection = context.section;
        this.settingsRequest++;
      }
      this.unlisten = await listen('deckweave:changed', () => {
        void this.refresh();
      });
      if (this.stopped) {
        this.unlisten();
        this.unlistenSettings?.();
        this.unlistenPlugins?.();
        return;
      }
      await this.refresh();
      this.plugins = await invoke<PluginPackage[]>('list_plugins');
    } catch (e) {
      this.fail(e);
    }
  }
  stop() {
    this.stopped = true;
    this.unlisten?.();
    this.unlistenSettings?.();
    this.unlistenPlugins?.();
  }
  async reloadPlugins() {
    try {
      const plugins = await invoke<PluginPackage[]>('list_plugins');
      if (!this.stopped) this.plugins = plugins;
    } catch (e) {
      this.fail(e);
    }
  }
  private adopt(snapshot: Snapshot) {
    if (
      !this.stopped &&
      (!this.snapshot || snapshot.event_sequence >= this.snapshot.event_sequence)
    )
      this.snapshot = snapshot;
  }
  private fail(error: unknown) {
    this.error =
      typeof error === 'object' && error && 'message' in error
        ? String(error.message)
        : String(error);
  }
  refresh(): Promise<void> {
    this.refreshAgain = true;
    if (this.refreshing) return this.refreshing;
    this.refreshing = (async () => {
      try {
        while (this.refreshAgain && !this.stopped) {
          this.refreshAgain = false;
          this.adopt(await invoke<Snapshot>('get_snapshot'));
        }
      } catch (e) {
        this.fail(e);
      } finally {
        this.refreshing = undefined;
      }
    })();
    return this.refreshing;
  }
  send(make: (revision: number) => Command): Promise<boolean> {
    const next = this.queue.then(async () => {
      if (!this.snapshot || this.stopped) return false;
      this.busy = true;
      try {
        this.adopt(
          await invoke<Snapshot>('dispatch', {
            command: make(this.snapshot.configuration.revision),
          }),
        );
        this.error = '';
        return true;
      } catch (e) {
        this.fail(e);
        await this.refresh();
        return false;
      } finally {
        this.busy = false;
      }
    });
    this.queue = next;
    return next;
  }
  input(deviceId: string, controlId: string, event: InputEvent) {
    return this.send(() => ({ type: 'input', device_id: deviceId, control_id: controlId, event }));
  }
  async openDevice(deviceId: string) {
    try {
      await invoke('open_device', { deviceId });
    } catch (e) {
      this.fail(e);
    }
  }
  async quit() {
    try {
      await invoke('quit_application');
    } catch (e) {
      this.fail(e);
    }
  }
  async openSettings(deviceId: string | null = null, section?: SettingsContext['section']) {
    try {
      await invoke('open_settings', { deviceId, section });
    } catch (e) {
      this.fail(e);
    }
  }
  async openInspector(context: string) {
    try {
      await invoke('open_inspector', { context });
    } catch (e) {
      this.fail(e);
    }
  }
  async restartPlugin(packageId: string) {
    try {
      await invoke('restart_plugin', { package: packageId });
    } catch (e) {
      this.fail(e);
    }
  }
}
