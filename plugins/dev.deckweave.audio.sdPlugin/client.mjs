// SPDX-License-Identifier: GPL-3.0-or-later
import { spawn } from 'node:child_process';
import { access } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

export async function findHelper(directory = dirname(fileURLToPath(import.meta.url))) {
  if (process.platform !== 'win32') throw new Error('Master audio currently requires Windows');
  const filename = 'deckweave-system.exe';
  // Distributed plugins carry their helper. Source-tree fallback is enabled only beside Cargo.toml.
  const paths = [resolve(directory, 'bin', filename)];
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
  throw new Error('Audio helper missing: build deckweave-system or install the complete plugin');
}

/** One pooled helper per plugin process. Bounded framing, pending requests and response deadlines. */
export class AudioClient {
  constructor({ locate = findHelper, spawnProcess = spawn, timeout = 2000 } = {}) {
    this.locate = locate;
    this.spawnProcess = spawnProcess;
    this.timeout = timeout;
    this.pending = new Map();
    this.child = null;
    this.starting = null;
    this.closed = false;
    this.nextId = 0;
    this.retryAfter = 0;
  }
  async start() {
    if (this.closed) throw new Error('Audio client closed');
    if (this.child) return this.child;
    if (this.starting) return this.starting;
    if (Date.now() < this.retryAfter) throw new Error('Audio helper reconnecting');
    this.starting = (async () => {
      const path = await this.locate();
      if (this.closed) throw new Error('Audio client closed');
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
        if (buffered.length > 16_384)
          return this.fail(child, new Error('Audio response exceeds limit'));
        let newline;
        while ((newline = buffered.indexOf('\n')) >= 0) {
          const line = buffered.slice(0, newline);
          buffered = buffered.slice(newline + 1);
          let response;
          try {
            response = JSON.parse(line);
          } catch {
            return this.fail(child, new Error('Invalid audio response'));
          }
          const request = this.pending.get(response.id);
          if (!request) return this.fail(child, new Error('Unexpected audio response'));
          this.pending.delete(response.id);
          clearTimeout(request.timer);
          if (
            response.ok &&
            Number.isFinite(response.state?.volume) &&
            response.state.volume >= 0 &&
            response.state.volume <= 100 &&
            typeof response.state.muted === 'boolean'
          )
            request.resolve(response.state);
          else
            request.reject(
              new Error(String(response.error ?? 'Invalid audio state').slice(0, 300)),
            );
        }
      });
      child.on('error', (error) => this.fail(child, error));
      child.on('exit', () => this.fail(child, new Error('Audio helper stopped')));
      child.stdin.on('error', (error) => this.fail(child, error));
      return child;
    })()
      .catch((error) => {
        this.retryAfter = Date.now() + 2000;
        throw error;
      })
      .finally(() => {
        this.starting = null;
      });
    return this.starting;
  }
  async request(command, values = {}, { signal } = {}) {
    if (signal?.aborted) throw new DOMException('Audio context disposed', 'AbortError');
    if (this.pending.size >= 32) throw new Error('Audio request queue full');
    const child = await this.start();
    if (signal?.aborted) throw new DOMException('Audio context disposed', 'AbortError');
    if (this.closed || this.child !== child) throw new Error('Audio helper stopped');
    if (this.pending.size >= 32) throw new Error('Audio request queue full');
    const id = (this.nextId = this.nextId >= 0xffff_fffe ? 1 : this.nextId + 1);
    return new Promise((resolveRequest, reject) => {
      const timer = setTimeout(
        () => this.fail(child, new Error('Audio helper timed out')),
        this.timeout,
      );
      this.pending.set(id, { resolve: resolveRequest, reject, timer });
      const message = JSON.stringify({ id, command, ...values });
      if (message.length > 1023) {
        this.fail(child, new Error('Audio request exceeds limit'));
        return;
      }
      child.stdin.write(message + '\n', (error) => {
        if (error) this.fail(child, error);
      });
    });
  }
  fail(child, error) {
    if (this.child !== child) return;
    this.child = null;
    this.retryAfter = Date.now() + 2000;
    for (const request of this.pending.values()) {
      clearTimeout(request.timer);
      request.reject(error);
    }
    this.pending.clear();
    child.stdin.destroy();
    child.kill();
  }
  close() {
    this.closed = true;
    if (this.child) this.fail(this.child, new Error('Audio client closed'));
  }
}
