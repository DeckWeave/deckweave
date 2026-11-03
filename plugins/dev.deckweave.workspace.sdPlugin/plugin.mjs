import { createPlugin, svgDataUrl } from './lib/sdk.mjs';
import { settings, render } from './operations.mjs';
import { WorkspaceClient } from './client.mjs';
import { pathToFileURL } from 'node:url';
export function start(options, client = new WorkspaceClient()) {
  const plugin = createPlugin(options);
  const language = plugin.info.application?.language ?? 'en';
  function draw(ctx) {
    const options = settings(ctx.settings, language);
    const image = svgDataUrl(render(options, ctx.canvas, ctx.state.error, ctx.state.busy));
    if (ctx.state.image === image) return;
    ctx.state.image = image;
    if (ctx.controller === 'Encoder') {
      if (!ctx.state.layout) {
        ctx.setFeedbackLayout('$A0');
        ctx.state.layout = true;
      }
      ctx.setFeedback({ 'full-canvas': image, title: '' });
    } else ctx.setImage(image);
    ctx.setTitle('');
  }
  async function run(ctx) {
    if (!ctx || ctx.state.busy) return;
    const operation = settings(ctx.settings, language).operation;
    ctx.state.busy = true;
    ctx.state.error = '';
    draw(ctx);
    try {
      await client.request(operation, { signal: ctx.signal });
      if (ctx.active) ctx.showOk();
    } catch (error) {
      if (error.name === 'AbortError') return;
      ctx.state.error = error.message;
      if (ctx.active) {
        ctx.showAlert(error.message);
        ctx.sendToPropertyInspector({ error: error.message });
      }
    } finally {
      ctx.state.busy = false;
      if (ctx.active) draw(ctx);
    }
  }
  plugin.on('willAppear', draw);
  plugin.on('didReceiveSettings', (ctx) => {
    if (ctx) {
      ctx.state.error = '';
      ctx.showOk();
      draw(ctx);
    }
  });
  plugin.on('propertyInspectorDidAppear', (ctx) => {
    if (ctx) ctx.sendToPropertyInspector({ error: ctx.state.error ?? '' });
  });
  for (const event of ['keyUp', 'dialUp', 'panelUp']) plugin.on(event, (ctx) => void run(ctx));
  const stop = () => {
    plugin.dispose();
    process.exit(0);
  };
  const exit = () => client.close();
  for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, stop);
  process.once('exit', exit);
  plugin.onDispose(() => {
    client.close();
    for (const signal of ['SIGINT', 'SIGTERM']) process.removeListener(signal, stop);
    process.removeListener('exit', exit);
  });
  return plugin;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) start();
