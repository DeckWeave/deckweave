// SPDX-License-Identifier: GPL-3.0-or-later
import os from 'node:os';
import { createPlugin, svgDataUrl } from './lib/sdk.mjs';
import { cpuSample, utilization, render } from './monitor.mjs';
const plugin = createPlugin();
const language = plugin.info.application?.language?.startsWith('ru') ? 'ru' : 'en';
let previous = cpuSample(os.cpus()),
  sample = { cpu: null, memory: 0, usedGiB: 0, totalGiB: 0, uptime: 0 },
  interval;
function collect() {
  const next = cpuSample(os.cpus()),
    total = os.totalmem(),
    used = Math.max(0, total - os.freemem());
  sample = {
    cpu: utilization(previous, next),
    memory: total ? (used / total) * 100 : 0,
    usedGiB: used / 2 ** 30,
    totalGiB: total / 2 ** 30,
    uptime: os.uptime(),
  };
  previous = next;
  for (const ctx of plugin.contexts.values()) draw(ctx);
}
function draw(ctx) {
  const image = svgDataUrl(
    render(
      ctx.action,
      sample,
      { ...ctx.settings, language: ctx.settings.language ?? language },
      ctx.canvas,
    ),
  );
  if (image === ctx.state.lastImage) return;
  ctx.state.lastImage = image;
  if (ctx.controller === 'Encoder') ctx.setFeedback({ 'full-canvas': image });
  else ctx.setImage(image);
  ctx.setTitle('');
}
plugin.on('willAppear', (ctx) => {
  if (ctx.controller === 'Encoder') ctx.setFeedbackLayout('$A0');
  if (!interval) {
    previous = cpuSample(os.cpus());
    collect();
    interval = setInterval(collect, 2000);
  }
  draw(ctx);
});
plugin.on('didReceiveSettings', (ctx) => draw(ctx));
plugin.on('willDisappear', (ctx) => {
  if (plugin.contexts.size === 1 && plugin.contexts.has(ctx.id)) {
    clearInterval(interval);
    interval = undefined;
    sample.cpu = null;
  }
});
plugin.onDispose(() => clearInterval(interval));
