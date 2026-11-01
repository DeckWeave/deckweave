// SPDX-License-Identifier: GPL-3.0-or-later
/** Ordered, bounded mutations. Context disposal cancels work that has not reached the helper. */
export class AudioQueue {
  constructor(execute, { delay = 30, onError = () => {} } = {}) {
    this.execute = execute;
    this.delay = delay;
    this.onError = onError;
    this.pending = [];
    this.timer = null;
    this.flushing = false;
    this.closed = false;
  }
  enqueue(owner, command, values = {}) {
    if (this.closed || !owner.active) return false;
    const previous = this.pending.at(-1);
    if (previous?.owner === owner && previous.command === command) {
      if (command === 'set_volume') {
        previous.values = values;
        return true;
      }
      // A direction change must stay ordered: +100 then -50 differs from +50 near a boundary.
      if (
        command === 'adjust_volume' &&
        Math.sign(previous.values.delta) === Math.sign(values.delta)
      ) {
        previous.values.delta = Math.max(-100, Math.min(100, previous.values.delta + values.delta));
        return true;
      }
    }
    if (this.pending.length >= 128) {
      this.onError(new Error('Audio input queue full'));
      return false;
    }
    this.pending.push({ owner, command, values: { ...values } });
    if (!this.flushing && !this.timer) this.timer = setTimeout(() => void this.flush(), this.delay);
    return true;
  }
  cancel(owner, command) {
    this.pending = this.pending.filter(
      (operation) => operation.owner !== owner || (command && operation.command !== command),
    );
    if (!this.pending.length && this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }
  async flush() {
    clearTimeout(this.timer);
    this.timer = null;
    if (this.flushing || this.closed) return;
    this.flushing = true;
    try {
      while (!this.closed && this.pending.length) {
        const operation = this.pending.shift();
        if (!operation.owner.active) continue;
        try {
          await this.execute(operation.command, operation.values, operation.owner.signal);
        } catch (error) {
          this.onError(error);
        }
      }
    } finally {
      this.flushing = false;
    }
  }
  close() {
    this.closed = true;
    clearTimeout(this.timer);
    this.timer = null;
    this.pending = [];
  }
}
