'use strict';
const net = require('node:net');
const { validateRequest } = require('./core.cjs');
// One bounded request per connection. Tokens never travel over TCP/HTTP.
async function startServer({ pipe, token, dispatch, onError = () => {} }) {
  const sockets = new Set();
  const seen = new Set();
  const server = net.createServer((socket) => {
    if (sockets.size >= 4) {
      socket.destroy();
      return;
    }
    sockets.add(socket);
    let buffer = Buffer.alloc(0),
      completed = false;
    const abort = new AbortController();
    socket.setTimeout(2500, () => socket.destroy());
    socket.on('close', () => {
      sockets.delete(socket);
      abort.abort();
    });
    socket.on('error', () => {});
    socket.on('data', async (data) => {
      if (completed) return;
      buffer = Buffer.concat([buffer, data]);
      if (buffer.length > 1024) {
        socket.destroy();
        return;
      }
      const newline = buffer.indexOf(10);
      if (newline < 0) return;
      completed = true;
      let request;
      try {
        if (newline !== buffer.length - 1) throw new Error('Only one workspace request is allowed');
        request = JSON.parse(buffer.subarray(0, newline).toString('utf8'));
        validateRequest(request, token);
        const identity = `${request.id}`;
        if (seen.has(identity))
          throw Object.assign(new Error('Duplicate workspace request; press the key again'), {
            code: 'replay',
          });
        seen.add(identity);
        if (seen.size > 256) seen.delete(seen.values().next().value);
        const result = await dispatch(request.operation, { signal: abort.signal });
        if (!socket.destroyed)
          socket.end(JSON.stringify({ id: request.id, ok: true, ...result }) + '\n');
      } catch (error) {
        const code = error.code || 'command';
        const message = String(error.message || error).slice(0, 300);
        if (!['not_focused', 'auth', 'request', 'replay'].includes(code)) onError(message);
        if (!socket.destroyed)
          socket.end(
            JSON.stringify({
              id: Number.isSafeInteger(request?.id) ? request.id : null,
              ok: false,
              code,
              error: message,
            }) + '\n',
          );
      }
    });
  });
  server.on('error', onError);
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(pipe, resolve);
  });
  return {
    close: () => {
      for (const socket of sockets) socket.destroy();
      server.close();
    },
  };
}
module.exports = { startServer };
