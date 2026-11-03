// SPDX-License-Identifier: GPL-3.0-or-later
import { createPlugin, svgDataUrl } from './lib/sdk.mjs';
import { render, metricView, gpuMetric } from './monitor.mjs';
import { SharedSampler } from './sampler.mjs';
import { pathToFileURL } from 'node:url';
export function start(options) {
  const plugin = createPlugin(options);
  const language = plugin.info.application?.language?.startsWith('ru') ? 'ru' : 'en';
  const sampler = new SharedSampler(() => {
    for (const ctx of plugin.contexts.values()) draw(ctx);
    for (const ctx of plugin.services.values()) updatePanel(ctx);
  });
  const settings = (ctx) => ({ ...ctx.settings, language: ctx.settings.language ?? language });
  function updatePanel(ctx) {
    const metric = ctx.panel?.spec.content.detail || ctx.action;
    const view = metricView(metric, sampler.sample, settings(ctx));
    const detail = [view.detail, view.adapter?.name, view.reason].filter(Boolean).join(' · ');
    // One producer update in flight per panel. Sampling never builds an SDK queue.
    if (ctx.state.updating) return;
    ctx.state.updating = true;
    void ctx.panels
      .update(ctx.panel.id, { value: view.value, detail })
      .catch((error) => {
        if (!['closed', 'disposed', 'disconnected'].includes(error.code))
          console.error('[Monitor panel]', error.code, error.message);
      })
      .finally(() => {
        ctx.state.updating = false;
      });
  }
  function draw(ctx) {
    const image = svgDataUrl(render(ctx.action, sampler.sample, settings(ctx), ctx.canvas));
    if (image === ctx.state.lastImage) return;
    ctx.state.lastImage = image;
    if (ctx.controller === 'Encoder') ctx.setFeedback({ 'full-canvas': image });
    else ctx.setImage(image);
    ctx.setTitle('');
  }
  function diagnostics(ctx) {
    const view = metricView(ctx.action, sampler.sample, settings(ctx));
    ctx.sendToPropertyInspector({
      monitor: {
        status: view.status,
        reason: view.reason,
        adapter: view.adapter,
        sampledAt: sampler.sample.gpu?.sampled_at_ms,
        adapters:
          sampler.sample.gpu?.adapters.map(({ id, index, name }) => ({ id, index, name })) ?? [],
      },
    });
  }
  plugin.on('willAppear', (ctx) => {
    if (ctx.controller === 'Encoder') ctx.setFeedbackLayout('$A0');
    sampler.setDemand(`key:${ctx.id}`, gpuMetric(ctx.action));
    draw(ctx);
  });
  plugin.on('didReceiveSettings', (ctx) => {
    draw(ctx);
    diagnostics(ctx);
  });
  plugin.on('willDisappear', (ctx) => sampler.setDemand(`key:${ctx.id}`, null));
  plugin.on('sendToPlugin', (ctx, message) => {
    if (message.payload?.monitorDiagnostics) diagnostics(ctx);
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
    sampler.setDemand(`panel:${ctx.id}`, gpuMetric(ctx.panel?.spec.content.detail || ctx.action));
    updatePanel(ctx);
  });
  plugin.on('deckweaveServiceDidChange', (ctx) => {
    sampler.setDemand(`panel:${ctx.id}`, gpuMetric(ctx.panel?.spec.content.detail || ctx.action));
    updatePanel(ctx);
  });
  plugin.on('deckweaveServiceDidDisappear', (ctx) => sampler.setDemand(`panel:${ctx.id}`, null));
  plugin.onDispose(() => sampler.close());
  return plugin;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) start();
