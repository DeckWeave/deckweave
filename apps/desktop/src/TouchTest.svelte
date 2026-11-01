<script lang="ts">
  import type { Client } from './client.svelte';
  import type { Control, Locale, Snapshot } from './types';
  import { text } from './i18n';
  import { onDestroy } from 'svelte';
  let {
    client,
    deviceId,
    control,
    geometry,
    locale,
  }: {
    client: Client;
    deviceId: string;
    control: Control;
    geometry: Snapshot['geometries'][string];
    locale: Locale;
  } = $props();
  let active: number | null = null;
  let last = 0;
  onDestroy(() => {
    if (active !== null) void client.input(deviceId, control.id, { type: 'cancel' });
  });
  const t = $derived((key: Parameters<typeof text>[1]) => text(locale, key));
  function send(e: PointerEvent, phase: 'down' | 'move' | 'up' | 'cancel') {
    const target = e.currentTarget as HTMLElement;
    const box = target.getBoundingClientRect();
    const [sx, sy, sw, sh] = geometry.surface;
    const [x, y, w, h] = control.rect;
    const px = Math.max(0, Math.min(1, (e.clientX - box.left) / box.width));
    const py = Math.max(0, Math.min(1, (e.clientY - box.top) / box.height));
    void client.input(deviceId, control.id, {
      type: 'pointer',
      phase,
      x: Math.min(1, Math.max(0, (x - sx + px * w) / sw)),
      y: Math.min(1, Math.max(0, (y - sy + py * h) / sh)),
    });
  }
</script>

<div class="touch-test-tools">
  <p>{t('touchTestHint')}</p>
  <div
    role="application"
    aria-label={t('touchTest')}
    class="touch-test-pad"
    onpointerdown={(e) => {
      if (active !== null) return;
      active = e.pointerId;
      e.currentTarget.setPointerCapture(e.pointerId);
      send(e, 'down');
    }}
    onpointermove={(e) => {
      if (active === e.pointerId && performance.now() - last > 30) {
        last = performance.now();
        send(e, 'move');
      }
    }}
    onpointerup={(e) => {
      if (active === e.pointerId) {
        send(e, 'up');
        active = null;
        e.currentTarget.releasePointerCapture(e.pointerId);
      }
    }}
    onpointercancel={(e) => {
      if (active === e.pointerId) {
        send(e, 'cancel');
        active = null;
      }
    }}
  >
    {t('touchTest')}
  </div>
  <div class="dial-tests">
    <button onclick={() => void client.input(deviceId, control.id, { type: 'swipe', right: false })}
      >← {t('swipe')}</button
    ><button onclick={() => void client.input(deviceId, control.id, { type: 'swipe', right: true })}
      >{t('swipe')} →</button
    >
  </div>
</div>
