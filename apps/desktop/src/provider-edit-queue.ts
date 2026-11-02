export type SettingsPatch = Record<string, unknown>;
export interface ProviderEditState {
  pending: boolean;
  desired: SettingsPatch;
  error: string | null;
}

/** Merge complete gestures, not pointer samples. Retain only the newest queued value per field. */
export function mergeSettings(base: SettingsPatch, patch: SettingsPatch): SettingsPatch {
  const object = (v: unknown): v is SettingsPatch =>
    !!v && typeof v === 'object' && !Array.isArray(v);
  return Object.fromEntries(
    [...new Set([...Object.keys(base), ...Object.keys(patch)])].map((key) => [
      key,
      Object.hasOwn(patch, key)
        ? object(base[key]) && object(patch[key])
          ? mergeSettings(base[key], patch[key])
          : patch[key]
        : base[key],
    ]),
  );
}

/** One verified write at a time and one coalesced successor. A failed write never replays its successor. */
export class ProviderEditQueue {
  private queued: SettingsPatch = {};
  private active: SettingsPatch = {};
  private running = false;
  private stopped = false;
  private failed = false;
  constructor(
    private commit: (patch: SettingsPatch, cancelled: () => boolean) => Promise<void>,
    private publish: (state: ProviderEditState) => void,
  ) {}
  enqueue(patch: SettingsPatch): boolean {
    if (this.stopped || this.failed || !Object.keys(patch).length) return false;
    this.queued = mergeSettings(this.queued, patch);
    this.update();
    if (!this.running) void this.run();
    return true;
  }
  private update(error: string | null = null) {
    if (!this.stopped)
      this.publish({
        pending: this.running || !!Object.keys(this.queued).length,
        desired: mergeSettings(this.active, this.queued),
        error,
      });
  }
  private async run() {
    this.running = true;
    try {
      while (!this.stopped && Object.keys(this.queued).length) {
        this.active = this.queued;
        this.queued = {};
        this.update();
        await this.commit(this.active, () => this.stopped);
        this.active = {};
      }
    } catch (e) {
      this.failed = true;
      this.active = {};
      this.queued = {};
      this.running = false;
      this.update(String(e));
      return;
    }
    this.running = false;
    this.update();
  }
  dispose() {
    this.stopped = true;
    this.queued = {};
  }
}
