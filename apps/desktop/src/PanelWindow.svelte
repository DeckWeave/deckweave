<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import { service, serviceError, type PanelView } from './services';
  let { id }: { id: string } = $props();
  let view = $state<PanelView | null>(null),
    error = $state(''),
    busy = $state(false);
  const ru = $derived(view?.locale !== 'en');
  const seconds = $derived(
    Math.max(
      0,
      Math.ceil(
        (view?.remaining_ms ??
          (view?.panel.spec.content.type === 'timer'
            ? view.panel.spec.content.seconds * 1000
            : 0)) / 1000,
      ),
    ),
  );
  const time = $derived(Math.floor(seconds / 60) + ':' + String(seconds % 60).padStart(2, '0'));
  onMount(() => {
    let stopped = false;
    let off: (() => void) | undefined;
    void (async () => {
      off = await listen<PanelView>('deckweave:panel', (e) => {
        view = e.payload;
      });
      if (stopped) {
        off();
        return;
      }
      try {
        view = await invoke<PanelView>('host_panel_view', { id });
      } catch (e) {
        error = serviceError(e);
      }
    })();
    return () => {
      stopped = true;
      off?.();
    };
  });
  async function command(operation: string, extra: Record<string, unknown> = {}) {
    if (!view || busy) return;
    busy = true;
    try {
      await service(operation, { id, revision: view.panel.revision, ...extra });
      error = '';
    } catch (e) {
      error = serviceError(e);
    } finally {
      busy = false;
    }
  }
</script>

{#if view}<main
    class="overlay"
    style:background={view.panel.spec.placement.background}
    style:color={view.panel.spec.placement.foreground}
    style:opacity={view.panel.spec.placement.opacity / 100}
  >
    <header>
      <button
        class="drag"
        aria-label={ru ? 'Переместить панель' : 'Move panel'}
        onpointerdown={() => void command('drag')}
        ><Icon name="grip" /><strong>{view.panel.spec.title}</strong></button
      ><button
        disabled={busy}
        aria-label={ru ? 'Закрыть панель' : 'Close panel'}
        onclick={() => void command('close')}><Icon name="close" /></button
      >
    </header>
    {#if view.panel.spec.content.type === 'note'}<div class="note">
        {view.panel.spec.content.text ||
          (ru
            ? 'Заметку можно изменить в Настройки → Панели'
            : 'Edit this note in Settings → Panels')}
      </div>
    {:else if view.panel.spec.content.type === 'metric'}<div class="metric">
        <strong>{view.online ? (view.value?.value ?? view.panel.spec.content.value) : '—'}</strong
        ><span
          >{view.online
            ? (view.value?.detail ?? view.panel.spec.content.detail)
            : ru
              ? 'Плагин недоступен'
              : 'Plugin unavailable'}</span
        >
      </div>
    {:else}<div class="timer">
        <strong>{time}</strong>
        <div class="timer-controls">
          <button
            disabled={busy}
            onclick={() =>
              void command('timer', {
                command: view!.timer?.status === 'running' ? 'pause' : 'start',
              })}
            ><Icon name={view.timer?.status === 'running' ? 'pause' : 'play'} />{view.timer
              ?.status === 'running'
              ? ru
                ? 'Пауза'
                : 'Pause'
              : ru
                ? 'Старт'
                : 'Start'}</button
          ><button
            disabled={busy}
            aria-label={ru ? 'Сбросить таймер' : 'Reset timer'}
            onclick={() =>
              void command('timer', {
                command: 'reset',
              })}><Icon name="reset" /></button
          >
        </div>
      </div>{/if}
    {#if error || view.error}<p class="error" role="alert">{error || view.error}</p>{/if}
  </main>{:else if error}<p role="alert">{error}</p>{/if}

<style>
  :global(html:has(.overlay)),
  :global(body:has(.overlay)),
  :global(#app:has(.overlay)) {
    background: transparent;
    margin: 0;
    overflow: hidden;
    width: 100%;
    height: 100%;
  }
  .overlay {
    height: 100vh;
    display: flex;
    flex-direction: column;
    font:
      13px system-ui,
      sans-serif;
    border: 1px solid currentColor;
    border-radius: 8px;
    box-sizing: border-box;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    padding: 4px 6px;
    gap: 4px;
    border-bottom: 1px solid color-mix(in srgb, currentColor 25%, transparent);
  }
  button {
    background: transparent;
    color: inherit;
    border: 0;
    border-radius: 4px;
    display: inline-flex;
    gap: 6px;
    align-items: center;
    padding: 4px;
    cursor: pointer;
  }
  button:hover {
    background: color-mix(in srgb, currentColor 15%, transparent);
  }
  button:focus-visible {
    outline: 2px solid currentColor;
    outline-offset: -2px;
  }
  .drag {
    flex: 1;
    min-width: 0;
    cursor: move;
    text-align: left;
  }
  .drag strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
  }
  .note {
    padding: 10px;
    white-space: pre-wrap;
    overflow: auto;
    overflow-wrap: anywhere;
    flex: 1;
  }
  .metric,
  .timer {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    gap: 6px;
  }
  .metric strong,
  .timer > strong {
    font-size: clamp(24px, 11vw, 64px);
    font-variant-numeric: tabular-nums;
  }
  .metric span {
    font-size: 12px;
  }
  .timer-controls {
    display: flex;
    gap: 8px;
  }
  .error {
    padding: 4px 8px;
    margin: 0;
    font-size: 12px;
    overflow-wrap: anywhere;
  }
</style>
