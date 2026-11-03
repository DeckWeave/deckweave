import { spawn } from 'node:child_process';
import { access, readdir, readFile, stat } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomInt } from 'node:crypto';
import net from 'node:net';
import { operations } from './operations.mjs';

export async function findHelper(directory = dirname(fileURLToPath(import.meta.url))) {
  if (process.platform !== 'win32')
    throw new Error('Workspace application controls require Windows');
  const paths = [join(directory, 'bin', 'deckweave-workspace.exe')];
  const root = resolve(directory, '../..');
  try {
    await access(join(root, 'Cargo.toml'));
    paths.push(
      join(root, 'target/release/deckweave-workspace.exe'),
      join(root, 'target/debug/deckweave-workspace.exe'),
    );
  } catch {}
  for (const path of paths) {
    try {
      await access(path);
      return path;
    } catch {}
  }
  throw new Error('Install the complete Workspace plugin or build deckweave-workspace.exe');
}
export class NativeClient {
  constructor({ locate = findHelper, spawnProcess = spawn, timeout = 7000 } = {}) {
    Object.assign(this, { locate, spawnProcess, timeout });
    this.child = null;
    this.starting = null;
    this.pending = null;
    this.closed = false;
  }
  async start() {
    if (this.closed) throw new Error('Workspace controls are closed');
    if (this.child) return this.child;
    if (this.starting) return this.starting;
    this.starting = (async () => {
      const path = await this.locate();
      if (this.closed) throw new Error('Workspace controls are closed');
      const child = this.spawnProcess(path, [], {
        windowsHide: true,
        shell: false,
        stdio: ['pipe', 'pipe', 'ignore'],
      });
      this.child = child;
      let buffer = '';
      child.stdout.setEncoding('utf8');
      child.stdout.on('data', (chunk) => {
        if (this.child !== child) return;
        buffer += chunk;
        if (buffer.length > 4096)
          return this.fail(new Error('Workspace helper response exceeds limit'));
        const newline = buffer.indexOf('\n');
        if (newline < 0) return;
        const line = buffer.slice(0, newline);
        buffer = buffer.slice(newline + 1);
        const pending = this.pending;
        let response;
        try {
          response = JSON.parse(line);
        } catch {
          return this.fail(new Error('Invalid workspace helper response'));
        }
        if (!pending || response.id !== pending.id || buffer.length)
          return this.fail(new Error('Unexpected workspace helper response'));
        this.pending = null;
        clearTimeout(pending.timer);
        if (response.ok === true) pending.resolve();
        else
          pending.reject(
            new Error(String(response.error || 'Workspace command failed').slice(0, 300)),
          );
      });
      child.on('error', (error) => this.fail(error));
      child.on('exit', () => {
        if (this.child === child)
          this.fail(new Error('Workspace helper stopped; press the command again'));
      });
      child.stdin.on('error', (error) => this.fail(error));
      return child;
    })().finally(() => {
      this.starting = null;
    });
    return this.starting;
  }
  async request(operation) {
    if (this.pending) throw new Error('A workspace command is still running');
    const child = await this.start();
    if (this.pending) throw new Error('A workspace command is still running');
    const id = randomInt(1, 0xffff_ffff);
    return new Promise((resolveRequest, reject) => {
      const timer = setTimeout(
        () =>
          this.fail(
            new Error('Workspace command timed out; check the application before retrying'),
          ),
        this.timeout,
      );
      this.pending = { id, timer, resolve: resolveRequest, reject };
      child.stdin.write(JSON.stringify({ id, operation }) + '\n', (error) => {
        if (error) this.fail(error);
      });
    });
  }
  fail(error) {
    const child = this.child;
    this.child = null;
    if (this.pending) {
      clearTimeout(this.pending.timer);
      this.pending.reject(error);
      this.pending = null;
    }
    if (child) {
      child.stdin.destroy();
      child.kill();
    }
  }
  close() {
    this.closed = true;
    this.fail(new Error('Workspace controls are closed'));
  }
}

export function rpc(
  endpoint,
  operation,
  { timeout = 2000, connect = net.createConnection, signal } = {},
) {
  return new Promise((resolveRequest, reject) => {
    if (signal?.aborted) return reject(new DOMException('Workspace context closed', 'AbortError'));
    const id = randomInt(1, 0xffff_ffff);
    const socket = connect(endpoint.pipe);
    let buffer = Buffer.alloc(0),
      done = false;
    const abort = () => finish(new DOMException('Workspace context closed', 'AbortError'));
    const finish = (error, value) => {
      if (done) return;
      done = true;
      signal?.removeEventListener('abort', abort);
      socket.destroy();
      error ? reject(error) : resolveRequest(value);
    };
    signal?.addEventListener('abort', abort, { once: true });
    socket.setTimeout(timeout, () =>
      finish(new Error('VS Code command timed out; check its dialog before retrying')),
    );
    socket.on('error', (error) => finish(error));
    socket.on('end', () => {
      if (!done) finish(new Error('VS Code companion closed the connection'));
    });
    socket.on('connect', () =>
      socket.write(JSON.stringify({ id, token: endpoint.token, operation }) + '\n'),
    );
    socket.on('data', (data) => {
      buffer = Buffer.concat([buffer, data]);
      if (buffer.length > 4096)
        return finish(new Error('VS Code companion response exceeds limit'));
      const newline = buffer.indexOf(10);
      if (newline < 0) return;
      let response;
      try {
        response = JSON.parse(buffer.subarray(0, newline).toString('utf8'));
      } catch {
        return finish(new Error('Invalid VS Code companion response'));
      }
      if (response.id !== id) return finish(new Error('Unexpected VS Code companion response'));
      if (response.ok === true) finish(null, response);
      else
        finish(
          Object.assign(
            new Error(String(response.error || 'VS Code command failed').slice(0, 300)),
            { code: response.code },
          ),
        );
    });
  });
}
export async function endpoints(
  directory = join(process.env.LOCALAPPDATA || '', 'DeckWeave', 'workspace-vscode'),
) {
  let names;
  try {
    names = (await readdir(directory))
      .filter((name) => /^[a-f0-9]{40}\.json$/.test(name))
      .slice(-32);
  } catch {
    return [];
  }
  const found = [];
  for (const name of names) {
    try {
      const path = join(directory, name);
      if ((await stat(path)).size > 2048) continue;
      const endpoint = JSON.parse(await readFile(path, 'utf8'));
      if (
        endpoint.version === 1 &&
        /^\\\\\.\\pipe\\deckweave-vscode-[a-f0-9]{40}$/.test(endpoint.pipe) &&
        /^[a-f0-9]{64}$/.test(endpoint.token) &&
        Number.isSafeInteger(endpoint.pid)
      )
        found.push(endpoint);
    } catch {}
  }
  return found;
}
export class WorkspaceClient {
  constructor({
    native = new NativeClient(),
    discover = endpoints,
    call = rpc,
    settle = () => new Promise((resolveWait) => setTimeout(resolveWait, 180)),
  } = {}) {
    Object.assign(this, { native, discover, call, settle });
    this.busy = false;
  }
  async request(operation, { signal } = {}) {
    if (!Object.hasOwn(operations, operation))
      throw new Error('Choose a supported workspace command');
    const alive = () => {
      if (signal?.aborted) throw new DOMException('Workspace context closed', 'AbortError');
    };
    alive();
    if (this.busy)
      throw new Error('A workspace command is still running; finish it before retrying');
    this.busy = true;
    try {
      if (!operation.startsWith('report.') && !operation.startsWith('vscode.'))
        return await this.native.request(operation);
      await this.native.request('launch.code');
      await this.settle();
      alive();
      const candidates = await this.discover();
      if (!candidates.length)
        throw new Error('VS Code companion is unavailable; install it and reload VS Code');
      const focused = [];
      // Status requests never activate an extension or read document/selection content.
      for (const endpoint of candidates) {
        alive();
        try {
          if ((await this.call(endpoint, 'status', { timeout: 250, signal })).focused)
            focused.push(endpoint);
        } catch {}
      }
      if (focused.length !== 1)
        throw new Error(
          focused.length > 1
            ? 'Several VS Code windows report focus; focus the intended window and retry'
            : 'Focus a VS Code window with the DeckWeave companion enabled, then retry',
        );
      alive();
      return await this.call(focused[0], operation, { signal });
    } finally {
      this.busy = false;
    }
  }
  close() {
    this.native.close();
  }
}
