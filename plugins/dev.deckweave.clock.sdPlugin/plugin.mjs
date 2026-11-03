// SPDX-License-Identifier: GPL-3.0-or-later
import { createPlugin, svgDataUrl } from './lib/sdk.mjs';
import { render } from './clock.mjs';
const p = createPlugin();
const language = p.info.application?.language?.startsWith('en') ? 'en' : 'ru';
function draw(ctx) {
  const image = svgDataUrl(render(new Date(), { language, ...ctx.settings }, ctx.canvas));
  if (ctx.state.image === image) return;
  ctx.state.image = image;
  ctx.setImage(image);
  ctx.setTitle('');
}
p.on('willAppear', draw);
p.on('didReceiveSettings', draw);
const timer = setInterval(() => {
  for (const ctx of p.contexts.values()) draw(ctx);
}, 1000);
p.onDispose(() => clearInterval(timer));
