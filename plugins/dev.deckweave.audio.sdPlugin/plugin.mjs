// SPDX-License-Identifier: GPL-3.0-or-later
import { createPlugin, svgDataUrl } from './lib/sdk.mjs';
import { AudioClient } from './client.mjs';
import { settings, render, touchVolume } from './audio.mjs';
import { AudioQueue } from './queue.mjs';

const plugin = createPlugin();
const client = new AudioClient();
const language = plugin.info.application?.language ?? 'en';
let observed = null,
  error = '',
  polling = false;
function draw(ctx) {
  const options = settings(ctx.settings, language);
  const image = svgDataUrl(render(observed, options, ctx.canvas, error));
  if (ctx.state.image === image) return;
  ctx.state.image = image;
  if (ctx.controller === 'Encoder') {
    if (!ctx.state.layout) {
      ctx.setFeedbackLayout('$A0');
      ctx.state.layout = true;
    }
    ctx.setFeedback({ 'full-canvas': image, title: '' });
  } else ctx.setImage(image);
  ctx.setTitle(error ? (options.language === 'ru' ? 'Звук недоступен' : 'Audio unavailable') : '');
}
function redraw() {
  for (const ctx of plugin.contexts.values()) draw(ctx);
}
async function command(name, values, signal) {
  try {
    observed = await client.request(name, values, { signal });
    error = '';
  } catch (e) {
    if (e.name === 'AbortError') return;
    error = e.message;
  }
  redraw();
}
const mutations = new AudioQueue(command, {
  onError(e) {
    error = e.message;
    redraw();
  },
});
async function poll() {
  if (polling || !plugin.contexts.size) return;
  polling = true;
  try {
    await command('status');
  } finally {
    polling = false;
  }
}
plugin.on('willAppear', (ctx) => {
  ctx.onDispose(() => mutations.cancel(ctx));
  draw(ctx);
  void poll();
});
plugin.on('didReceiveSettings', (ctx) => {
  if (ctx) {
    ctx.state.image = '';
    draw(ctx);
  }
});
plugin.on('dialRotate', (ctx, message) => {
  if (!ctx || !Number.isFinite(message.payload?.ticks)) return;
  const delta = Math.max(
    -100,
    Math.min(100, message.payload.ticks * settings(ctx.settings, language).step),
  );
  if (delta) mutations.enqueue(ctx, 'adjust_volume', { delta });
});
for (const event of ['keyUp', 'dialUp'])
  plugin.on(event, (ctx) => {
    if (ctx) mutations.enqueue(ctx, 'toggle_mute');
  });
for (const event of ['panelDown', 'panelMove', 'panelUp'])
  plugin.on(event, (ctx, message) => {
    if (!ctx) return;
    const value = touchVolume(message.payload?.position);
    if (value === null) return;
    mutations.enqueue(ctx, 'set_volume', { value });
  });
plugin.on('panelCancel', (ctx) => {
  if (ctx) mutations.cancel(ctx, 'set_volume');
});
const timer = setInterval(() => void poll(), 1000);
plugin.onDispose(() => {
  clearInterval(timer);
  mutations.close();
  client.close();
});
for (const signal of ['SIGINT', 'SIGTERM'])
  process.once(signal, () => {
    plugin.dispose();
    process.exit(0);
  });
process.once('exit', () => client.close());
