<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { Client } from './client.svelte';
  import type { Device } from './types';
  type Summary = { groups?: string[]; keys?: string[]; changedBytes?: number };
  type RecordInfo = {
    snapshotId: string;
    summary?: Summary;
    createdAt?: string;
    createdAtMs?: number;
  };
  type Preview = RecordInfo & {
    previewToken: string;
    expiresInMs: number;
    confirmedInstanceId: string;
    requiresConfirmation: true;
    identityConfidence: string;
  };
  type Selection = { requiresSnapshotSelection: true; pending: RecordInfo[] };
  let {
    client,
    device,
    recovery,
    oncomplete,
  }: {
    client: Client;
    device: Device;
    recovery?: { state?: string; pending?: unknown[] } | null;
    oncomplete: () => void | Promise<void>;
  } = $props();
  let dialog: HTMLDialogElement;
  let preview = $state<Preview | null>(null);
  let selection = $state<RecordInfo[]>([]);
  let busy = $state(false);
  let error = $state('');
  let confirmed = $state(false);
  let result = $state('');
  const ru = $derived(client.snapshot?.configuration.locale === 'ru');
  function groupName(value: string) {
    const names: Record<string, [string, string]> = {
      lighting: ['Режим освещения', 'Lighting mode'],
      colors: ['Цвета клавиш', 'Key colors'],
      staticColors: ['Цвета клавиш', 'Key colors'],
      base: ['Основные назначения', 'Main assignments'],
      main: ['Основные назначения', 'Main assignments'],
      fn: ['Назначения Fn', 'Fn assignments'],
      function: ['Назначения Fn', 'Fn assignments'],
      actuation: ['Срабатывание клавиш', 'Key actuation'],
    };
    return names[value]?.[ru ? 0 : 1] ?? value;
  }
  function date(record: RecordInfo) {
    const value = record.createdAt ?? record.createdAtMs;
    const date = value === undefined ? null : new Date(value);
    return date && Number.isFinite(date.getTime())
      ? date.toLocaleString(ru ? 'ru-RU' : 'en-US')
      : ru
        ? 'Дата не предоставлена драйвером'
        : 'Date not provided by device support';
  }
  async function read(snapshotId?: string) {
    busy = true;
    error = '';
    preview = null;
    selection = [];
    confirmed = false;
    try {
      const value = await invoke<Preview | Selection>('provider_recovery_preview', {
        deviceId: device.id,
        snapshotId,
      });
      if ('requiresSnapshotSelection' in value) {
        selection = value.pending;
      } else {
        preview = value;
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
  function open() {
    result = '';
    dialog.showModal();
    void read();
  }
  async function restore() {
    if (!preview || !confirmed) return;
    busy = true;
    error = '';
    try {
      const state = await invoke<{ recovery?: { state?: string } | string }>(
        'recover_provider_settings',
        {
          deviceId: device.id,
          previewToken: preview.previewToken,
          confirmedInstanceId: preview.confirmedInstanceId,
        },
      );
      if (
        state.recovery === 'pending' ||
        (typeof state.recovery === 'object' && state.recovery.state === 'pending')
      ) {
        error = ru
          ? 'Восстановление ещё не завершено. Сохранённый снимок остаётся доступным.'
          : 'Recovery remains pending. The saved snapshot is still available.';
      } else if (
        state.recovery === 'completed' ||
        (typeof state.recovery === 'object' &&
          ['not_needed', 'completed'].includes(state.recovery.state ?? ''))
      ) {
        result = ru
          ? 'Драйвер сообщил о завершении восстановления.'
          : 'Device support reported that recovery completed.';
        dialog.close();
        await oncomplete();
      } else {
        error = ru
          ? 'Драйвер не подтвердил завершение восстановления. Снимок остаётся доступным.'
          : 'Device support did not confirm completion. The saved snapshot remains available.';
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

{#if recovery?.state === 'pending'}<section class="provider-recovery" role="status">
    <h3>{ru ? 'Требуется восстановление настроек' : 'Settings recovery required'}</h3>
    <p>
      {ru
        ? 'Предыдущая запись завершилась частично или её результат не подтверждён. Новые изменения заблокированы. Можно прочитать сохранённый снимок и явно восстановить выбранное устройство.'
        : 'A previous write partially completed or could not be verified. New edits are blocked. Review the saved snapshot and explicitly restore the selected device.'}
    </p>
    <button onclick={open}>{ru ? 'Посмотреть восстановление…' : 'Review recovery…'}</button>
  </section>{/if}
{#if result}<p role="status">{result}</p>{/if}
<dialog bind:this={dialog} class="recovery-dialog">
  <h2>{ru ? 'Восстановление настроек' : 'Recover device settings'}</h2>
  <p><strong>{device.name}</strong></p>
  {#if busy}<p>{ru ? 'Загрузка…' : 'Working…'}</p>{/if}
  {#if error}<p class="connection-error" role="alert">{error}</p>{/if}
  {#if selection.length}<p>
      {ru
        ? 'Выберите сохранённый снимок. Автоматического выбора нет.'
        : 'Choose a saved snapshot. No snapshot is selected automatically.'}
    </p>
    {#each selection as record}<button disabled={busy} onclick={() => void read(record.snapshotId)}
        >{date(record)} · {(record.summary?.groups ?? []).map(groupName).join(', ')}</button
      >{/each}{/if}
  {#if preview}<dl>
      <dt>{ru ? 'Исходный снимок' : 'Original snapshot'}</dt>
      <dd>{date(preview)}</dd>
      <dt>{ru ? 'Разделы настроек' : 'Settings groups'}</dt>
      <dd>{(preview.summary?.groups ?? []).map(groupName).join(', ') || '—'}</dd>
      <dt>{ru ? 'Клавиши' : 'Keys'}</dt>
      <dd>{preview.summary?.keys?.join(', ') || '—'}</dd>
    </dl>
    <p>
      {ru
        ? 'Постоянный серийный номер не подтверждён. Восстановление относится к устройству, которое вы сейчас явно подключили к этому экземпляру.'
        : 'A permanent serial identity is unverified. Recovery targets the hardware you explicitly paired with this instance now.'}
    </p>
    <label class="recovery-confirm"
      ><input type="checkbox" bind:checked={confirmed} />{ru
        ? 'Подтверждаю: восстановить выбранное устройство'
        : 'I confirm recovery of the selected device'} «{device.name}»</label
    >
  {/if}
  <footer>
    <button disabled={busy} onclick={() => dialog.close()}>{ru ? 'Отмена' : 'Cancel'}</button
    >{#if preview}<button
        class="primary"
        disabled={busy || !confirmed}
        onclick={() => void restore()}
        >{ru ? 'Восстановить настройки устройства' : 'Restore device settings'}</button
      >{/if}
  </footer>
</dialog>

<style>
  .provider-recovery {
    border: 1px solid #b58b40;
    border-radius: 8px;
    padding: 16px;
    margin: 16px 0;
  }
  .provider-recovery h3 {
    margin-top: 0;
  }
  .provider-recovery p {
    font-size: 13px;
    line-height: 1.5;
  }
  .recovery-dialog {
    width: min(650px, 90vw);
    max-height: 85vh;
    overflow: auto;
    background: #252830;
    color: #e4e7ee;
    border: 1px solid #6c7079;
    border-radius: 12px;
    padding: 24px;
  }
  .recovery-dialog::backdrop {
    background: #000a;
  }
  .recovery-dialog p {
    line-height: 1.5;
  }
  .recovery-dialog dl {
    display: grid;
    grid-template-columns: 160px 1fr;
    gap: 12px;
  }
  .recovery-dialog dd {
    margin: 0;
  }
  .recovery-dialog footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 24px;
  }
  .recovery-confirm {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    line-height: 1.5;
  }
</style>
