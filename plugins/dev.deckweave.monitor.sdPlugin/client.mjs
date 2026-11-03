// SPDX-License-Identifier: GPL-3.0-or-later
import { spawn } from 'node:child_process';
import { access } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

export async function findHelper(directory = dirname(fileURLToPath(import.meta.url))) {
  if (process.platform !== 'win32') throw new Error('GPU telemetry requires Windows');
  const filename = 'deckweave-system.exe',
    paths = [resolve(directory, 'bin', filename)];
  const root = resolve(directory, '../..');
  try {
    await access(resolve(root, 'Cargo.toml'));
    paths.push(resolve(root, 'target/release', filename), resolve(root, 'target/debug', filename));
  } catch {}
  for (const path of paths) {
    try {
      await access(path);
      return path;
    } catch {}
  }
  throw new Error('GPU helper missing: install the complete System Monitor plugin');
}

const statuses = new Set(['ready', 'warming_up', 'unavailable']);
const bytes = (value) => Number.isSafeInteger(value) && value >= 0;
export function validSample(value) {
  return (
    value &&
    statuses.has(value.status) &&
    bytes(value.sampled_at_ms) &&
    Array.isArray(value.adapters) &&
    value.adapters.length <= 16 &&
    value.adapters.every(
      (adapter) =>
        typeof adapter.id === 'string' &&
        /^[0-9a-f]{8}:[0-9a-f]{8}$/.test(adapter.id) &&
        Number.isInteger(adapter.index) &&
        adapter.index >= 0 &&
        adapter.index < 16 &&
        typeof adapter.name === 'string' &&
        adapter.name.length <= 128 &&
        statuses.has(adapter.gpu_status) &&
        statuses.has(adapter.memory_status) &&
        bytes(adapter.dedicated_total_bytes) &&
        (adapter.dedicated_used_bytes === null || bytes(adapter.dedicated_used_bytes)) &&
        (adapter.busiest_engine_percent === null ||
          (Number.isFinite(adapter.busiest_engine_percent) &&
            adapter.busiest_engine_percent >= 0 &&
            adapter.busiest_engine_percent <= 100)) &&
        (adapter.gpu_status !== 'ready' || adapter.busiest_engine_percent !== null) &&
        (adapter.memory_status !== 'ready' ||
          (adapter.dedicated_total_bytes > 0 && adapter.dedicated_used_bytes !== null)),
    ) &&
    (value.selected_adapter_id === null ||
      value.adapters.some((a) => a.id === value.selected_adapter_id))
  );
}

/** A single persistent process, one coalesced request, bounded framing and deadline. */
export class GpuClient {
  constructor({ locate = findHelper, spawnProcess = spawn, timeout = 1500 } = {}) {
    Object.assign(this, {
      locate,
      spawnProcess,
      timeout,
      child: null,
      pending: null,
      inflight: null,
      closed: false,
      nextId: 0,
      retryAfter: 0,
    });
  }
  sample() {
    if (this.closed) return Promise.reject(new Error('GPU sampler closed'));
    if (this.inflight) return this.inflight;
    this.inflight = this.request().finally(() => {
      this.inflight = null;
    });
    return this.inflight;
  }
  async request() {
    if (Date.now() < this.retryAfter) throw new Error('GPU helper reconnecting');
    if (!this.child) {
      const path = await this.locate();
      if (this.closed) throw new Error('GPU sampler closed');
      const child = this.spawnProcess(path, [], {
        stdio: ['pipe', 'pipe', 'ignore'],
        shell: false,
        windowsHide: true,
      });
      this.child = child;
      let buffered = '';
      child.stdout.setEncoding('utf8');
      child.stdout.on('data', (chunk) => {
        if (this.child !== child) return;
        buffered += chunk;
        if (buffered.length > 65_536)
          return this.fail(child, new Error('GPU response exceeds limit'));
        let newline;
        while ((newline = buffered.indexOf('\n')) >= 0) {
          const line = buffered.slice(0, newline);
          buffered = buffered.slice(newline + 1);
          let response;
          try {
            response = JSON.parse(line);
          } catch {
            return this.fail(child, new Error('Invalid GPU response'));
          }
          const request = this.pending;
          if (!request || request.id !== response.id)
            return this.fail(child, new Error('Unexpected GPU response'));
          this.pending = null;
          clearTimeout(request.timer);
          if (response.ok && validSample(response.state)) request.resolve(response.state);
          else
            request.reject(new Error(String(response.error ?? 'Invalid GPU state').slice(0, 300)));
        }
      });
      child.on('error', (error) => this.fail(child, error));
      child.on('exit', () => this.fail(child, new Error('GPU helper stopped')));
      child.stdin.on('error', (error) => this.fail(child, error));
    }
    if (this.closed) throw new Error('GPU sampler closed');
    const child = this.child;
    const id = (this.nextId = this.nextId >= 0xffff_fffe ? 1 : this.nextId + 1);
    return new Promise((resolveRequest, reject) => {
      const timer = setTimeout(
        () => this.fail(child, new Error('GPU helper timed out')),
        this.timeout,
      );
      this.pending = { id, resolve: resolveRequest, reject, timer };
      child.stdin.write(JSON.stringify({ id, command: 'gpu_sample' }) + '\n', (error) => {
        if (error) this.fail(child, error);
      });
    });
  }
  fail(child, error) {
    if (this.child !== child) return;
    this.child = null;
    this.retryAfter = Date.now() + 2000;
    if (this.pending) {
      clearTimeout(this.pending.timer);
      this.pending.reject(error);
      this.pending = null;
    }
    child.stdin.destroy();
    child.kill();
  }
  close() {
    this.closed = true;
    if (this.child) this.fail(this.child, new Error('GPU sampler closed'));
  }
}
