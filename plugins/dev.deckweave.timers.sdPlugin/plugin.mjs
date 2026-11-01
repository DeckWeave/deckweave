// SPDX-License-Identifier: GPL-3.0-or-later
import { createPlugin, svgDataUrl } from './lib/sdk.mjs';
import { duration, restore, valueAt, toggle, reset, serialize, render } from './timer.mjs';
import { Handoffs } from './handoff.mjs';
const p = createPlugin(),
  now = () => performance.now();
const language = p.info.application?.language?.startsWith('ru') ? 'ru' : 'en';
const handoffs = new Handoffs();
function initialize(ctx) {
  ctx.state.receivedRuntime = JSON.stringify(ctx.settings.runtime);
  ctx.state.timer = restore(
    ctx.settings.runtime,
    ctx.action.endsWith('.countdown'),
    duration(ctx.settings.seconds),
    Date.now(),
    now(),
  );
  ctx.state.signature = JSON.stringify([ctx.settings.runtime, ctx.settings.seconds]);
}
function persist(ctx) {
  const settings = { ...ctx.settings, runtime: serialize(ctx.state.timer, Date.now(), now()) };
  ctx.state.signature = JSON.stringify([settings.runtime, settings.seconds]);
  ctx.state.savedRuntime = JSON.stringify(settings.runtime);
  ctx.setSettings(settings);
}
function draw(ctx) {
  let state = ctx.state.timer;
  if (state.status === 'running' && state.countdown && valueAt(state, now()) === 0) {
    ctx.state.timer = state = { ...state, status: 'done', base: 0 };
    persist(ctx);
  }
  const image = svgDataUrl(
    render(
      state,
      { ...ctx.settings, language: ctx.settings.language ?? language },
      ctx.canvas,
      now(),
    ),
  );
  if (image !== ctx.state.image) {
    ctx.state.image = image;
    if (ctx.controller === 'Encoder') ctx.setFeedback({ 'full-canvas': image });
    else ctx.setImage(image);
    ctx.setTitle('');
  }
}
function act(ctx, command) {
  ctx.state.timer =
    command === 'reset' ? reset(ctx.state.timer, now()) : toggle(ctx.state.timer, now());
  persist(ctx);
  draw(ctx);
}
p.on('willAppear', (ctx) => {
  const runtime = handoffs.take(ctx.id, ctx.settings, now());
  if (runtime) ctx.settings = { ...ctx.settings, runtime };
  initialize(ctx);
  if (ctx.controller === 'Encoder') ctx.setFeedbackLayout('$A0');
  draw(ctx);
  ctx.interval(() => draw(ctx), 250);
});
p.on('willDisappear', (ctx) => {
  const signatures = [ctx.state.receivedRuntime, ctx.state.savedRuntime];
  persist(ctx);
  signatures.push(ctx.state.savedRuntime);
  handoffs.put(ctx.id, ctx.settings.runtime, signatures, ctx.settings.seconds ?? 1500, now());
});
p.on('didReceiveSettings', (ctx) => {
  if (JSON.stringify([ctx.settings.runtime, ctx.settings.seconds]) !== ctx.state.signature)
    initialize(ctx);
  draw(ctx);
});
function down(ctx) {
  ctx.state.held = false;
  ctx.state.cancelHold?.();
  ctx.state.cancelHold = ctx.timeout(() => {
    ctx.state.held = true;
    act(ctx, 'reset');
  }, 800);
}
function up(ctx) {
  ctx.state.cancelHold?.();
  if (!ctx.state.held) act(ctx, 'toggle');
}
for (const event of ['keyDown', 'dialDown', 'panelDown']) p.on(event, down);
for (const event of ['keyUp', 'dialUp', 'panelUp']) p.on(event, up);
p.on('panelCancel', (ctx) => {
  ctx.state.cancelHold?.();
  ctx.state.held = true;
});
p.on('touchTap', (ctx, msg) => act(ctx, msg.payload?.hold ? 'reset' : 'toggle'));
p.on('dialRotate', (ctx, msg) => {
  const state = ctx.state.timer;
  if (!state.countdown || state.status === 'running') return;
  const seconds = Math.max(
    1,
    Math.min(86400, (ctx.settings.seconds || 1500) + (Number(msg.payload?.ticks) || 0) * 60),
  );
  ctx.settings = { ...ctx.settings, seconds };
  ctx.state.timer = reset({ ...state, durationMs: duration(seconds) }, now());
  persist(ctx);
  draw(ctx);
});
p.on('sendToPlugin', (ctx, msg) => {
  if (!ctx) return;
  const payload = msg.payload ?? {};
  if (payload.command === 'reset' || payload.command === 'toggle') act(ctx, payload.command);
  if (payload.command === 'configure') {
    const seconds = duration(payload.seconds) / 1000;
    const changed = seconds !== (ctx.settings.seconds || 1500);
    ctx.settings = {
      ...ctx.settings,
      label: String(payload.label ?? '').slice(0, 24),
      language: payload.language === 'ru' ? 'ru' : 'en',
      seconds,
    };
    if (changed) ctx.state.timer = reset({ ...ctx.state.timer, durationMs: seconds * 1000 }, now());
    persist(ctx);
    draw(ctx);
  }
});
