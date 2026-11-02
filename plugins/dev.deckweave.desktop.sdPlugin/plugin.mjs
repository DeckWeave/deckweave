// SPDX-License-Identifier: GPL-3.0-or-later
import { createPlugin, escapeXml, svgDataUrl } from './lib/sdk.mjs';
import { pathToFileURL } from 'node:url';
export function start(options) {
  const plugin = createPlugin(options);
  const ru = plugin.info.application?.language?.startsWith('ru');
  function draw(ctx) {
    const type = ctx.action.split('.').at(-1);
    const title =
      ctx.settings.title ||
      (type === 'note'
        ? ru
          ? 'Заметка'
          : 'Note'
        : type === 'timer'
          ? ru
            ? 'Таймер'
            : 'Timer'
          : ru
            ? 'Звук'
            : 'Sound');
    const glyph =
      type === 'note'
        ? '<path d="M30 25h52v60H30zm12 17h28M42 54h28M42 66h20"/>'
        : type === 'timer'
          ? '<circle cx="56" cy="58" r="28"/><path d="M56 39v21l13 8M45 20h22M56 20v10"/>'
          : '<path d="M25 45h15l20-16v54L40 67H25zm48-8q24 19 0 38M68 47q10 9 0 18"/>';
    ctx.setImage(
      svgDataUrl(
        `<svg xmlns="http://www.w3.org/2000/svg" width="112" height="112"><rect width="112" height="112" fill="#252525"/><g stroke="#e4c780" stroke-width="4" fill="none" stroke-linecap="round" stroke-linejoin="round">${glyph}</g><text x="56" y="104" text-anchor="middle" font-size="13" fill="#ededed" font-family="sans-serif">${escapeXml(title.slice(0, 16))}</text></svg>`,
      ),
    );
    ctx.setTitle('');
  }
  function spec(ctx, s = ctx.settings) {
    const type = ctx.action.split('.').at(-1);
    return {
      title: s.title || (type === 'timer' ? (ru ? 'Таймер' : 'Timer') : ru ? 'Заметка' : 'Note'),
      content:
        type === 'timer'
          ? {
              type: 'timer',
              seconds: Number(s.seconds ?? 300),
              sound: s.sound === 'silent' ? null : s.sound || 'complete',
              volume: Number(s.volume ?? 35),
            }
          : { type: 'note', text: s.text || '' },
      placement: {
        anchor: s.anchor || 'top_right',
        width: Number(s.width ?? 320),
        height: Number(s.height ?? 160),
        background: s.background || '#252525',
        foreground: s.foreground || '#ededed',
        click_through: !!s.clickThrough,
      },
    };
  }
  plugin.on('willAppear', (ctx) => {
    ctx.state.lastSettings = { ...ctx.settings };
    draw(ctx);
  });
  plugin.on('didReceiveSettings', (ctx) => {
    const before = spec(ctx, ctx.state.lastSettings ?? ctx.settings),
      after = spec(ctx);
    ctx.state.lastSettings = { ...ctx.settings };
    draw(ctx);
    if (ctx.action.endsWith('.sound') || JSON.stringify(before) === JSON.stringify(after)) return;
    const changed = (a, b) =>
      Object.fromEntries(
        Object.entries(b).filter(
          ([key, value]) => JSON.stringify(a[key]) !== JSON.stringify(value),
        ),
      );
    // Coalesce pending settings; retain the first baseline so intermediate edits are not lost.
    ctx.state.pendingSettings = { before: ctx.state.pendingSettings?.before ?? before, after };
    if (ctx.state.syncing) return;
    ctx.state.syncing = true;
    void (async () => {
      try {
        while (ctx.active && ctx.state.pendingSettings) {
          const update = ctx.state.pendingSettings;
          ctx.state.pendingSettings = null;
          const saved = await ctx.panels.get('main');
          await ctx.panels.configure(saved.id, saved.revision, {
            ...saved.spec,
            ...(update.before.title !== update.after.title ? { title: update.after.title } : {}),
            content: {
              ...saved.spec.content,
              ...changed(update.before.content, update.after.content),
            },
            placement: {
              ...saved.spec.placement,
              ...changed(update.before.placement, update.after.placement),
            },
          });
        }
      } catch (error) {
        if (error.code !== 'not_found') {
          ctx.showAlert();
          ctx.sendToPropertyInspector({ error: error.message });
        }
      } finally {
        ctx.state.syncing = false;
      }
    })();
  });
  async function activate(ctx) {
    if (ctx.state.busy) return;
    ctx.state.busy = true;
    try {
      const s = ctx.settings,
        type = ctx.action.split('.').at(-1);
      if (type === 'sound') {
        await ctx.playSound({
          sound: s.sound || 'complete',
          volume: Number(s.volume ?? 35),
          eventId: crypto.randomUUID(),
        });
      } else {
        const panel = await ctx.panels.open('main', spec(ctx));
        if (type === 'timer') await ctx.panels.timer(panel.id, 'start');
      }
      ctx.showOk();
    } catch (error) {
      ctx.showAlert();
      ctx.sendToPropertyInspector({ error: error.message });
    } finally {
      ctx.state.busy = false;
    }
  }
  plugin.on('keyUp', (ctx) => activate(ctx));
  return plugin;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) start();
