// SPDX-License-Identifier: GPL-3.0-or-later
// A host may enqueue reappearance before it receives final disappearance settings.
// Retain only a short, bounded bridge; never replace a different owner's settings.
export class Handoffs {
  entries = new Map();
  prune(now) {
    for (const [id, entry] of this.entries) if (entry.until <= now) this.entries.delete(id);
  }
  put(id, runtime, signatures, seconds, now) {
    this.prune(now);
    this.entries.delete(id);
    this.entries.set(id, { runtime, signatures: new Set(signatures), seconds, until: now + 10000 });
    while (this.entries.size > 256) this.entries.delete(this.entries.keys().next().value);
  }
  take(id, settings, now) {
    this.prune(now);
    const entry = this.entries.get(id);
    this.entries.delete(id);
    return entry &&
      entry.seconds === (settings.seconds ?? 1500) &&
      entry.signatures.has(JSON.stringify(settings.runtime))
      ? entry.runtime
      : null;
  }
}
