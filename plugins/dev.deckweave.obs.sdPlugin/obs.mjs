// GPL-3.0-or-later. obs-websocket 5.x JSON RPC; no Stream Deck-specific OBS module.
import { createHash, randomUUID } from 'node:crypto';
export const authentication = (password, salt, challenge) => {
  const hash = (value) => createHash('sha256').update(value).digest('base64');
  return hash(hash(password + salt) + challenge);
};
export class Obs {
  constructor(settings, onEvent) {
    this.settings = settings;
    this.onEvent = onEvent;
    this.pending = new Map();
    this.ready = false;
    this.closed = false;
    this.connect();
  }
  connect() {
    if (this.closed) return;
    let address;
    try {
      const host = this.settings.host || '127.0.0.1';
      const port = Number(this.settings.port || 4455);
      if (!/^[\w.:[\]-]+$/.test(host) || !Number.isInteger(port) || port < 1 || port > 65535)
        throw Error('Invalid OBS address');
      address = `ws://${host}:${port}`;
    } catch (e) {
      this.onEvent('error', e.message);
      return;
    }
    const socket = (this.socket = new WebSocket(address));
    const timeout = setTimeout(() => socket.close(), 5000);
    socket.onmessage = ({ data }) => {
      if (typeof data !== 'string' || data.length > 1024 * 1024) {
        socket.close();
        return;
      }
      let message;
      try {
        message = JSON.parse(data);
      } catch {
        socket.close();
        return;
      }
      const { op, d } = message;
      if (op === 0) {
        const identify = { rpcVersion: 1, eventSubscriptions: 76 };
        if (d.authentication)
          identify.authentication = authentication(
            this.settings.password || '',
            d.authentication.salt,
            d.authentication.challenge,
          );
        socket.send(JSON.stringify({ op: 1, d: identify }));
      } else if (op === 2) {
        clearTimeout(timeout);
        this.ready = true;
        this.onEvent('ready');
      } else if (op === 5) this.onEvent(d.eventType, d.eventData);
      else if (op === 7) {
        const pending = this.pending.get(d.requestId);
        if (!pending) return;
        clearTimeout(pending.timeout);
        this.pending.delete(d.requestId);
        if (d.requestStatus?.result) pending.resolve(d.responseData || {});
        else
          pending.reject(Error(d.requestStatus?.comment || `OBS error ${d.requestStatus?.code}`));
      }
    };
    socket.onerror = () => this.onEvent('error', 'Cannot connect to OBS');
    socket.onclose = ({ code }) => {
      clearTimeout(timeout);
      this.ready = false;
      for (const p of this.pending.values()) {
        clearTimeout(p.timeout);
        p.reject(Error('OBS disconnected'));
      }
      this.pending.clear();
      if (!this.closed) {
        this.onEvent('error', code === 4009 ? 'OBS authentication failed' : 'OBS disconnected');
        this.retry = setTimeout(() => this.connect(), 3000);
      }
    };
  }
  request(requestType, requestData = {}) {
    if (!this.ready) return Promise.reject(Error('OBS is not connected'));
    if (this.pending.size >= 128) return Promise.reject(Error('Too many pending OBS requests'));
    const requestId = randomUUID();
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(requestId);
        reject(Error('OBS request timed out'));
      }, 5000);
      this.pending.set(requestId, { resolve, reject, timeout });
      this.socket.send(JSON.stringify({ op: 6, d: { requestType, requestId, requestData } }));
    });
  }
  close() {
    this.closed = true;
    clearTimeout(this.retry);
    this.socket.close();
    for (const p of this.pending.values()) {
      clearTimeout(p.timeout);
      p.reject(Error('OBS connection closed'));
    }
    this.pending.clear();
  }
}
