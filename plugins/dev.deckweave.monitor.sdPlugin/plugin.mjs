// SPDX-License-Identifier: GPL-3.0-or-later
import os from 'node:os';
import { createPlugin, svgDataUrl } from './lib/sdk.mjs';
import { cpuSample, utilization, render, uptime } from './monitor.mjs';
import { pathToFileURL } from 'node:url';
export function start(options) {
  const plugin = createPlugin(options);
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
    for (const ctx of plugin.services.values()) updatePanel(ctx);
  }
  function updatePanel(ctx) {
    const metric = ctx.panel?.spec.content.detail;
    const value =
      metric === 'memory'
        ? `${Math.round(sample.memory)}%`
        : metric === 'uptime'
          ? uptime(sample.uptime)
          : sample.cpu === null
            ? '…'
            : `${Math.round(sample.cpu)}%`;
    const detail =
      metric === 'memory'
        ? `${sample.usedGiB.toFixed(1)} / ${sample.totalGiB.toFixed(1)} GiB`
        : metric === 'uptime'
          ? language === 'ru'
            ? 'С запуска ОС'
            : 'Since OS boot'
          : 'CPU';
    void ctx.panels.update(ctx.panel.id, { value, detail }).catch((error) => {
      if (!['closed', 'disposed', 'disconnected'].includes(error.code))
        console.error('[Monitor panel]', error);
    });
  }
  function startSampler() {
    if (interval) return;
    previous = cpuSample(os.cpus());
    collect();
    interval = setInterval(collect, 2000);
  }
  function stopSampler(ctx, collection) {
    const visible =
      plugin.contexts.size - (collection === plugin.contexts && collection.has(ctx.id) ? 1 : 0);
    const services =
      plugin.services.size - (collection === plugin.services && collection.has(ctx.id) ? 1 : 0);
    if (!visible && !services) {
      clearInterval(interval);
      interval = undefined;
      sample.cpu = null;
    }
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
    startSampler();
    draw(ctx);
  });
  plugin.on('didReceiveSettings', (ctx) => draw(ctx));
  plugin.on('willDisappear', (ctx) => {
    stopSampler(ctx, plugin.contexts);
  });
  plugin.on('keyUp', async (ctx) => {
    if (!ctx.settings.desktopPanel) return;
    try {
      await ctx.panels.open('monitor', {
        title: ctx.settings.label || ctx.action.split('.').at(-1).toUpperCase(),
        content: { type: 'metric', value: '…', detail: ctx.action.split('.').at(-1) },
      });
      ctx.showOk();
    } catch (error) {
      ctx.showAlert();
      ctx.sendToPropertyInspector({ error: error.message });
    }
  });
  plugin.on('deckweaveServiceDidAppear', (ctx) => {
    startSampler();
    updatePanel(ctx);
  });
  plugin.on('deckweaveServiceDidChange', (ctx) => updatePanel(ctx));
  plugin.on('deckweaveServiceDidDisappear', (ctx) => stopSampler(ctx, plugin.services));
  plugin.onDispose(() => clearInterval(interval));

  return plugin;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) start();
