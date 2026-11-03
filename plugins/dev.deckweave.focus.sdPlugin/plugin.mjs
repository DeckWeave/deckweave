// SPDX-License-Identifier: GPL-3.0-or-later
import { randomUUID } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { createPlugin, svgDataUrl } from './lib/sdk.mjs';
import { workspace, config, settingsFromConfig, render } from './focus.mjs';

/** The host owns all timing and sound. This process only projects host views. */
export function start(options) {
  const plugin = createPlugin(options),
    jobs = new Map(),
    loading = new Map(),
    operations = new Map();
  const language = plugin.info.application?.language?.startsWith('ru') ? 'ru' : 'en';
  function draw(ctx) {
    let key;
    try {
      key = workspace(ctx.settings);
    } catch (error) {
      ctx.state.error = error.message;
    }
    const job = jobs.get(key);
    const image = svgDataUrl(
      render(
        job,
        { ...ctx.settings, language: ctx.settings.language ?? language },
        ctx.canvas,
        ctx.state.error,
      ),
    );
    if (image === ctx.state.image) return;
    ctx.state.image = image;
    if (ctx.controller === 'Encoder') ctx.setFeedback({ 'full-canvas': image });
    else ctx.setImage(image);
    ctx.setTitle('');
  }
  function accept(job) {
    const before = jobs.get(job.workspace);
    if (before && before.revision > job.revision) return;
    if (before?.revision === job.revision && job.status === 'running')
      job = {
        ...job,
        live_remaining_ms: Math.min(job.live_remaining_ms, before.live_remaining_ms),
      };
    jobs.set(job.workspace, job);
    for (const ctx of plugin.contexts.values())
      if ((ctx.settings.workspace ?? 'personal') === job.workspace) {
        ctx.state.error = null;
        draw(ctx);
        ctx.sendToPropertyInspector({ hostFocus: job });
      }
  }
  function fail(ctx, error) {
    if (!ctx.active) return;
    ctx.state.error = error.code === 'conflict' ? null : error.message;
    draw(ctx);
    ctx.showAlert();
    ctx.sendToPropertyInspector({ error: error.message });
  }
  async function load(ctx, refresh = false) {
    const key = workspace(ctx.settings);
    if (!refresh && jobs.has(key)) return jobs.get(key);
    if (!loading.has(key)) {
      const promise = ctx
        .service('focus_get', { workspace: key, config: config(ctx.settings) })
        .then((job) => {
          accept(job);
          return job;
        })
        .finally(() => loading.delete(key));
      loading.set(key, promise);
    }
    return loading.get(key);
  }
  function command(ctx, operation, changedConfig) {
    const key = workspace(ctx.settings);
    const next = (operations.get(key) ?? Promise.resolve())
      .catch(() => {})
      .then(async () => {
        const job = await load(ctx);
        const commandId = randomUUID();
        try {
          const value = await ctx.service('focus_control', {
            workspace: key,
            revision: job.revision,
            commandId,
            command: operation,
            ...(changedConfig ? { config: changedConfig } : {}),
          });
          accept(value);
        } catch (error) {
          // Refresh stale state; never repeat a toggle against a newer phase automatically.
          if (error.code === 'conflict') await load(ctx, true);
          throw error;
        }
      });
    operations.set(key, next);
    void next
      .catch((error) => fail(ctx, error))
      .finally(() => {
        if (operations.get(key) === next) operations.delete(key);
      });
    return next;
  }
  function action(ctx) {
    const operation = ctx.settings.operation ?? 'toggle';
    if (!['toggle', 'start', 'pause', 'reset', 'skip'].includes(operation))
      throw new Error('Unknown Pomodoro operation');
    return command(ctx, operation);
  }
  plugin.on('willAppear', async (ctx) => {
    try {
      if (ctx.controller === 'Encoder') ctx.setFeedbackLayout('$A0');
      draw(ctx);
      await load(ctx, true);
    } catch (error) {
      fail(ctx, error);
    }
  });
  plugin.on('deckweaveFocusDidChange', (_ctx, message) => {
    for (const job of message.payload?.jobs ?? []) accept(job);
  });
  plugin.on('propertyInspectorDidAppear', async (ctx) => {
    try {
      await load(ctx, true);
    } catch (error) {
      fail(ctx, error);
    }
  });
  plugin.on('didReceiveSettings', async (ctx) => {
    try {
      draw(ctx);
      await load(ctx, true);
    } catch (error) {
      fail(ctx, error);
    }
  });
  function down(ctx) {
    ctx.state.held = false;
    ctx.state.cancelHold?.();
    if (ctx.settings.holdReset !== false)
      ctx.state.cancelHold = ctx.timeout(() => {
        ctx.state.held = true;
        void command(ctx, 'reset').catch(() => {});
      }, 800);
  }
  function up(ctx) {
    ctx.state.cancelHold?.();
    if (!ctx.state.held) void action(ctx).catch(() => {});
  }
  for (const event of ['keyDown', 'dialDown', 'panelDown']) plugin.on(event, down);
  for (const event of ['keyUp', 'dialUp', 'panelUp']) plugin.on(event, up);
  plugin.on('panelCancel', (ctx) => {
    ctx.state.cancelHold?.();
    ctx.state.held = true;
  });
  plugin.on('touchTap', (ctx, message) => {
    void (
      message.payload?.hold && ctx.settings.holdReset !== false
        ? command(ctx, 'reset')
        : action(ctx)
    ).catch(() => {});
  });
  plugin.on('sendToPlugin', async (ctx, message) => {
    if (!ctx) return;
    const payload = message.payload ?? {};
    try {
      if (payload.command === 'configure') {
        const patch = payload.settings ?? {},
          next = { ...ctx.settings, ...patch };
        workspace(next);
        // Select the new workspace before applying its shared configuration.
        if (next.workspace !== ctx.settings.workspace) {
          ctx.setSettings(next);
          await load(ctx, true);
        }
        const host = await load(ctx);
        const hasConfig = [
          'workMinutes',
          'shortMinutes',
          'longMinutes',
          'rounds',
          'sound',
          'volume',
        ].some((k) => Object.hasOwn(patch, k));
        if (hasConfig)
          await command(ctx, 'configure', config({ ...settingsFromConfig(host.config), ...patch }));
        ctx.setSettings(next);
        draw(ctx);
        await load(ctx, true);
      } else if (['toggle', 'start', 'pause', 'reset', 'skip'].includes(payload.command))
        await command(ctx, payload.command);
      else if (payload.command === 'refresh') await load(ctx, true);
    } catch (error) {
      fail(ctx, error);
    }
  });
  return plugin;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) start();
