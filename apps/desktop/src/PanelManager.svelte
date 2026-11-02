<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Icon from './Icon.svelte';
  import {
    service,
    serviceError,
    emptyServices,
    defaultPlacement,
    type ServiceSnapshot,
    type PanelSpec,
  } from './services';
  let { ru }: { ru: boolean } = $props();
  let inventory = $state<ServiceSnapshot>(emptyServices()),
    selected = $state(''),
    busy = $state(false),
    error = $state(''),
    volume = $state(35),
    removing = $state('');
  const view = $derived(
    inventory.panels.find((v) => v.panel.id === selected) ?? inventory.panels[0],
  );
  async function refresh() {
    inventory = await invoke<ServiceSnapshot>('host_service_inventory');
  }
  onMount(() => {
    let off: (() => void) | undefined;
    let stopped = false;
    void (async () => {
      try {
        off = await listen(
          'deckweave:services',
          () => void refresh().catch((e) => (error = serviceError(e))),
        );
        if (stopped) {
          off();
          return;
        }
        await refresh();
      } catch (e) {
        error = serviceError(e);
      }
    })();
    return () => {
      stopped = true;
      off?.();
    };
  });
  async function command(operation: string, payload: unknown) {
    if (busy) return;
    busy = true;
    try {
      const result = await service(operation, payload);
      error = '';
      await refresh();
      return result;
    } catch (e) {
      error = serviceError(e);
    } finally {
      busy = false;
    }
  }
  async function create(type: 'note' | 'timer') {
    const offset = (inventory.panels.filter((v) => v.panel.open).length % 8) * 24;
    const spec: PanelSpec = {
      title: type === 'note' ? (ru ? 'Заметка' : 'Note') : ru ? 'Таймер' : 'Timer',
      content:
        type === 'note'
          ? { type: 'note', text: '' }
          : { type: 'timer', seconds: 300, sound: 'complete', volume: 35 },
      placement: { ...defaultPlacement(), x: 16 + offset, y: 16 + offset },
    };
    const p = await command('open', { key: crypto.randomUUID(), spec });
    if (p) selected = p.id;
  }
  function configure(patch: Partial<PanelSpec>) {
    if (!view) return;
    void command('configure', {
      id: view.panel.id,
      revision: view.panel.revision,
      spec: { ...view.panel.spec, ...patch },
    });
  }
  function placement(key: string, value: unknown) {
    if (!view) return;
    configure({ placement: { ...view.panel.spec.placement, [key]: value } });
  }
  function timerContent(patch: { sound?: string | null; volume?: number; seconds?: number }) {
    if (view?.panel.spec.content.type === 'timer')
      configure({ content: { ...view.panel.spec.content, ...patch } });
  }
</script>

<div class="panel-toolbar">
  <h2>{ru ? 'Панели' : 'Panels'}</h2>
  <button disabled={busy} onclick={() => void create('note')}
    ><Icon name="plus" />{ru ? 'Заметка' : 'Note'}</button
  ><button disabled={busy} onclick={() => void create('timer')}
    ><Icon name="timer" />{ru ? 'Таймер' : 'Timer'}</button
  >
</div>
{#if error || inventory.error}<p class="connection-error" role="alert">
    {error || inventory.error}
  </p>{/if}
<div class="sound-toolbar">
  <Icon name="volume" /><span>{ru ? 'Звуки' : 'Sounds'}</span
  >{#each ['complete', 'notice', 'warning'] as cue}<button
      disabled={busy}
      onclick={() => void command('sound', { sound: cue, volume, eventId: crypto.randomUUID() })}
      ><Icon name="play" />{ru
        ? { complete: 'Завершение', notice: 'Уведомление', warning: 'Внимание' }[cue]
        : cue}</button
    >{/each}<label
    >{ru ? 'Громкость' : 'Volume'}
    <input
      aria-label={ru ? 'Громкость проверки звука' : 'Sound preview volume'}
      type="range"
      min="0"
      max="100"
      bind:value={volume}
    /><span>{volume}%</span></label
  >
</div>
<div class="panels-workspace">
  <div class="panel-list" role="group" aria-label={ru ? 'Сохранённые панели' : 'Saved panels'}>
    {#each inventory.panels as item (item.panel.id)}<button
        class:current={view?.panel.id === item.panel.id}
        onclick={() => {
          selected = item.panel.id;
          removing = '';
        }}
        ><Icon name={item.panel.spec.content.type === 'timer' ? 'timer' : 'page'} /><span
          ><strong>{item.panel.spec.title}</strong><small
            >{item.panel.open ? (ru ? 'Открыта' : 'Open') : ru ? 'Закрыта' : 'Closed'} · {item.panel
              .owner === 'deckweave.host'
              ? 'DeckWeave'
              : item.panel.owner}</small
          ></span
        ></button
      >{/each}
  </div>
  {#if view}{@const p = view.panel}
    <section class="panel-properties">
      <div class="panel-toolbar">
        <input
          aria-label={ru ? 'Название панели' : 'Panel title'}
          maxlength="120"
          value={p.spec.title}
          disabled={busy}
          onchange={(e) => configure({ title: e.currentTarget.value })}
        /><button
          disabled={busy}
          onclick={() =>
            void command(p.open ? 'close' : 'reopen', { id: p.id, revision: p.revision })}
          ><Icon name={p.open ? 'close' : 'play'} />{p.open
            ? ru
              ? 'Закрыть'
              : 'Close'
            : ru
              ? 'Открыть'
              : 'Open'}</button
        ><button
          disabled={busy}
          title={ru ? 'Вернуть панель в видимую область' : 'Reset panel position'}
          onclick={() => void command('locate', { id: p.id })}><Icon name="reset" /></button
        ><button
          disabled={busy}
          aria-label={ru ? 'Удалить панель' : 'Remove panel'}
          onclick={() => (removing = p.id)}><Icon name="trash" /></button
        >
      </div>
      {#if removing === p.id}<div class="panel-toolbar" role="alert">
          <span
            >{ru
              ? 'Удалить сохранённую панель и её таймер?'
              : 'Remove this saved panel and its timer?'}</span
          ><button
            disabled={busy}
            onclick={() => {
              void command('remove', { id: p.id, revision: p.revision });
              removing = '';
            }}>{ru ? 'Удалить' : 'Remove'}</button
          ><button onclick={() => (removing = '')}>{ru ? 'Отмена' : 'Cancel'}</button>
        </div>{/if}
      {#if p.spec.content.type === 'note'}<textarea
          aria-label={ru ? 'Текст заметки' : 'Note text'}
          maxlength="8192"
          rows="5"
          disabled={busy}
          value={p.spec.content.text}
          onchange={(e) => configure({ content: { type: 'note', text: e.currentTarget.value } })}
        ></textarea>
      {:else if p.spec.content.type === 'timer'}<div class="panel-toolbar">
          <label
            >{ru ? 'Секунды' : 'Seconds'}
            <input
              aria-label={ru ? 'Длительность таймера' : 'Timer duration'}
              type="number"
              min="1"
              max="86400"
              value={p.spec.content.seconds}
              disabled={busy}
              onchange={(e) =>
                configure({
                  content: {
                    ...p.spec.content,
                    type: 'timer',
                    seconds: Number(e.currentTarget.value),
                  },
                })}
            /></label
          ><button
            disabled={busy}
            onclick={() =>
              void command('timer', {
                id: p.id,
                command: view.timer?.status === 'running' ? 'pause' : 'start',
              })}
            >{view.timer?.status === 'running'
              ? ru
                ? 'Пауза'
                : 'Pause'
              : ru
                ? 'Старт'
                : 'Start'}</button
          ><button
            disabled={busy}
            onclick={() => void command('timer', { id: p.id, command: 'reset' })}
            >{ru ? 'Сброс' : 'Reset'}</button
          ><select
            disabled={busy}
            aria-label={ru ? 'Звук завершения' : 'Completion sound'}
            value={p.spec.content.sound ?? ''}
            onchange={(e) => timerContent({ sound: e.currentTarget.value || null })}
            ><option value="">{ru ? 'Без звука' : 'Silent'}</option><option value="complete"
              >{ru ? 'Завершение' : 'Complete'}</option
            ><option value="notice">{ru ? 'Уведомление' : 'Notice'}</option><option value="warning"
              >{ru ? 'Внимание' : 'Warning'}</option
            ></select
          >
          <label
            >{ru ? 'Громкость' : 'Volume'}<input
              disabled={busy}
              aria-label={ru ? 'Громкость таймера' : 'Timer volume'}
              type="number"
              min="0"
              max="100"
              value={p.spec.content.volume ?? 35}
              onchange={(e) => timerContent({ volume: Number(e.currentTarget.value) })}
            />%</label
          >
        </div>
        <p class="panel-hint">
          {ru
            ? 'Длительность и звук применяются при следующем старте. Закрытие панели не останавливает таймер.'
            : 'Duration and sound apply at the next start. Closing the panel keeps the timer running.'}
        </p>
      {:else}<p>
          {view.online
            ? (view.value?.value ?? p.spec.content.value)
            : ru
              ? 'Плагин недоступен'
              : 'Plugin unavailable'}
        </p>{/if}
      <div class="placement-grid">
        <label
          >{ru ? 'Положение' : 'Position'}<select
            disabled={busy}
            value={p.spec.placement.anchor}
            onchange={(e) => placement('anchor', e.currentTarget.value)}
            >{#each [['top_left', ru ? 'Слева сверху' : 'Top left'], ['top_right', ru ? 'Справа сверху' : 'Top right'], ['bottom_left', ru ? 'Слева снизу' : 'Bottom left'], ['bottom_right', ru ? 'Справа снизу' : 'Bottom right'], ['free', ru ? 'Свободное' : 'Free']] as [value, label]}<option
                {value}>{label}</option
              >{/each}</select
          ></label
        >{#each [['x', 'X'], ['y', 'Y'], ['width', ru ? 'Ширина' : 'Width'], ['height', ru ? 'Высота' : 'Height']] as [key, label]}<label
            >{label}<input
              aria-label={label}
              disabled={busy}
              type="number"
              value={p.spec.placement[key as 'x' | 'y' | 'width' | 'height']}
              onchange={(e) => placement(key, Number(e.currentTarget.value))}
            /></label
          >{/each}
      </div>
      <div class="panel-toolbar">
        <label
          >{ru ? 'Фон' : 'Background'}<input
            type="color"
            aria-label={ru ? 'Цвет фона панели' : 'Panel background color'}
            disabled={busy}
            value={p.spec.placement.background}
            onchange={(e) => placement('background', e.currentTarget.value)}
          /></label
        ><label
          >{ru ? 'Текст' : 'Text'}<input
            type="color"
            aria-label={ru ? 'Цвет текста панели' : 'Panel text color'}
            disabled={busy}
            value={p.spec.placement.foreground}
            onchange={(e) => placement('foreground', e.currentTarget.value)}
          /></label
        ><label
          >{ru ? 'Непрозрачность' : 'Opacity'}<input
            type="number"
            min="0"
            max="100"
            aria-label={ru ? 'Непрозрачность панели' : 'Panel opacity'}
            disabled={busy}
            value={p.spec.placement.opacity}
            onchange={(e) => placement('opacity', Number(e.currentTarget.value))}
          />%</label
        ><label
          ><input
            type="checkbox"
            disabled={busy}
            checked={p.spec.placement.click_through}
            onchange={(e) => placement('click_through', e.currentTarget.checked)}
          />{ru ? 'Пропускать клики' : 'Click-through'}</label
        >
      </div>
      {#if p.spec.placement.click_through}<small
          >{ru
            ? 'Вернуть управление панелью можно здесь, отключив «Пропускать клики».'
            : 'Disable Click-through here to interact with this panel again.'}</small
        >{/if}
    </section>{:else}<p>
      {ru
        ? 'Создайте заметку или таймер. Панели сохраняются между профилями и после перезапуска.'
        : 'Create a note or timer. Panels survive profile changes and restart.'}
    </p>{/if}
</div>
{#each inventory.unavailable as item}<p class="connection-error">{item.id}: {item.error}</p>{/each}
{#if inventory.outcomes.at(-1)?.status === 'failed'}<p class="connection-error" role="alert">
    {inventory.outcomes.at(-1)?.error}
  </p>{/if}

<style>
  .panel-toolbar,
  .sound-toolbar {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }
  .panel-toolbar h2 {
    margin: 0 auto 0 0;
  }
  .panel-toolbar > input {
    flex: 1;
    min-width: 100px;
  }
  .sound-toolbar {
    padding-bottom: 8px;
    border-bottom: 1px solid var(--line);
  }
  .sound-toolbar label {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .sound-toolbar input {
    width: 100px;
  }
  .sound-toolbar span:last-child {
    min-width: 32px;
  }
  .panels-workspace {
    display: grid;
    grid-template-columns: minmax(150px, 1fr) minmax(300px, 3fr);
    gap: 10px;
  }
  .panel-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: 500px;
    overflow: auto;
  }
  .panel-list button {
    display: flex;
    align-items: center;
    gap: 7px;
    text-align: left;
    min-width: 0;
  }
  .panel-list button span {
    min-width: 0;
  }
  .panel-list strong,
  .panel-list small {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .panel-list small {
    font-size: 11px;
    color: var(--muted);
  }
  .panel-properties {
    min-width: 0;
  }
  .panel-properties textarea {
    box-sizing: border-box;
    width: 100%;
    resize: vertical;
    margin-bottom: 8px;
  }
  .panel-toolbar label {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .panel-toolbar input[type='number'] {
    width: 65px;
  }
  .placement-grid {
    display: grid;
    grid-template-columns: 2fr repeat(4, 1fr);
    gap: 6px;
    margin-bottom: 8px;
  }
  .placement-grid label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .placement-grid input,
  .placement-grid select {
    min-width: 0;
    width: 100%;
    box-sizing: border-box;
  }
  @media (max-width: 900px) {
    .panels-workspace {
      grid-template-columns: 1fr;
    }
    .panel-list {
      max-height: 130px;
    }
    .placement-grid {
      grid-template-columns: repeat(3, 1fr);
    }
  }
</style>
