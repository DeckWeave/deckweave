<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { emptyProviderInventory, type ProviderInventory } from './provider-ui';
  import type { Client } from './client.svelte';
  import Icon from './Icon.svelte';
  import { text } from './i18n';
  import { lifecycleText } from './device-lifecycle-i18n';
  import DeviceHardwareSettings from './DeviceHardwareSettings.svelte';
  import type { Device } from './types';
  let { client }: { client: Client } = $props();
  let chosen = $state<string | null>(null);
  const snapshot = $derived(client.snapshot!);
  const t = $derived((key: Parameters<typeof text>[1]) => text(snapshot.configuration.locale, key));
  let view = $state<'active' | 'saved' | 'discovered'>('active');
  let inventory = $state<ProviderInventory>(emptyProviderInventory());
  let discovering = $state(false);
  let discoveryError = $state('');
  const ru = $derived(snapshot.configuration.locale === 'ru');
  async function discover() {
    if (!client.available) return;
    discovering = true;
    try {
      inventory = await invoke<ProviderInventory>('device_provider_status');
      discoveryError = '';
    } catch (e) {
      discoveryError = String(e);
    } finally {
      discovering = false;
    }
  }
  type Discovered = ProviderInventory['discovered'][number];
  let pairing = $state<Discovered | null>(null);
  let pairingTarget = $state('');
  let pairingConfirmed = $state(false);
  let pairingDialog: HTMLDialogElement;
  const claimedPairing = $derived(
    pairing
      ? [...snapshot.configuration.devices, ...snapshot.configuration.removed_devices].find(
          (d) => d.provider_id === pairing!.provider_id && d.hardware_key === pairing!.instance_id,
        )
      : undefined,
  );
  const compatiblePairing = $derived(
    pairing
      ? [...snapshot.configuration.devices, ...snapshot.configuration.removed_devices].filter(
          (d) =>
            d.provider_id === pairing!.provider_id &&
            d.provider_descriptor?.definition.modelId === pairing!.descriptor.modelId &&
            typeof pairing!.descriptor.modelId === 'string' &&
            snapshot.connections[d.id]?.state !== 'connected' &&
            inventory.sessions[d.id]?.state !== 'connected' &&
            (!claimedPairing || d.id === claimedPairing.id),
        )
      : [],
  );
  function reviewPairing(found: Discovered) {
    pairing = found;
    pairingTarget = '';
    pairingConfirmed = false;
    discoveryError = '';
    pairingDialog.showModal();
  }
  async function confirmPairing() {
    if (!pairing || !pairingTarget || (pairingTarget !== 'new' && !pairingConfirmed)) return;
    await addDiscovered(
      pairing.provider_id,
      pairing.instance_id,
      pairingTarget === 'new' ? undefined : pairingTarget,
    );
  }
  async function addDiscovered(providerId: string, instanceId: string, deviceId?: string) {
    discovering = true;
    try {
      const id = await invoke<string>('add_provider_device', { providerId, instanceId, deviceId });
      await client.refresh();
      chosen = id;
      view = 'active';
      await discover();
      pairingDialog.close();
      pairing = null;
    } catch (e) {
      discoveryError = String(e);
    } finally {
      discovering = false;
    }
  }
  onMount(() => {
    void discover();
  });
  let confirmation = $state<{ action: 'remove_device' | 'forget_device'; device: Device } | null>(
    null,
  );
  let dialog: HTMLDialogElement;
  const lt = $derived((key: Parameters<typeof lifecycleText>[1]) =>
    lifecycleText(snapshot.configuration.locale, key),
  );
  const saved = $derived(snapshot.configuration.removed_devices);
  const forgotten = $derived(
    snapshot.configuration.device_exclusions.filter(
      (e) =>
        !saved.some((d) => d.provider_id === e.provider_id && d.hardware_key === e.hardware_key),
    ),
  );
  const devices = $derived(snapshot.configuration.devices);
  const device = $derived(
    devices.find((d) => d.id === chosen) ??
      devices.find((d) => snapshot.connections[d.id]?.state === 'connected') ??
      devices[0],
  );
  const connection = $derived(device ? snapshot.connections[device.id] : null);
  const geometry = $derived(device ? snapshot.geometries[device.id] : undefined);
  const editorAvailable = $derived(
    !!device &&
      !!geometry?.controls.length &&
      (['deckweave.n4', 'deckweave.virtual'].includes(device.provider_id) ||
        !!device.provider_descriptor),
  );
  const controlsSummary = $derived.by(() => {
    const controls = geometry?.controls ?? [];
    const keys = controls.filter((c) => c.kind === 'key').length;
    const dials = controls.filter((c) => c.kind === 'dial').length;
    const touch = controls.filter((c) => c.kind === 'touch').length;
    return (
      [
        keys ? keys + ' ' + t('keysLabel') : '',
        dials ? dials + ' ' + t('dialsLabel') : '',
        touch ? touch + ' ' + (ru ? 'сенсорных областей' : 'touch areas') : '',
      ]
        .filter(Boolean)
        .join(' · ') || '—'
    );
  });
  $effect(() => {
    if (client.settingsDevice) {
      chosen = client.settingsDevice;
      view = 'active';
    }
  });
  function ask(action: 'remove_device' | 'forget_device', device: Device) {
    confirmation = { action, device };
    dialog.showModal();
  }
  async function confirm() {
    const item = confirmation;
    if (!item) return;
    if (
      await client.send((revision) => ({
        type: item.action,
        device_id: item.device.id,
        expected_revision: revision,
      }))
    ) {
      dialog.close();
      confirmation = null;
      if (item.action === 'remove_device') {
        view = 'saved';
        chosen = null;
      }
    }
  }
  async function restore(id: string) {
    if (
      await client.send((revision) => ({
        type: 'restore_device',
        device_id: id,
        expected_revision: revision,
      }))
    ) {
      chosen = id;
      view = 'active';
    }
  }
  async function add() {
    if (await client.send((revision) => ({ type: 'add_device', expected_revision: revision }))) {
      chosen = client.snapshot!.configuration.devices.at(-1)!.id;
      view = 'active';
    }
  }
</script>

<h2>{t('devices')}</h2>
{#if snapshot.provider_error}<p class="connection-error">{snapshot.provider_error}</p>{/if}
<div class="device-lifecycle-toolbar">
  <div class="device-lifecycle-tabs" role="group" aria-label={t('devices')}>
    <button class:current={view === 'active'} onclick={() => (view = 'active')}
      >{lt('active')} ({devices.length})</button
    >
    <button class:current={view === 'saved'} onclick={() => (view = 'saved')}
      >{lt('saved')} ({saved.length + forgotten.length})</button
    >
  </div>
  <button
    class:current={view === 'discovered'}
    onclick={() => {
      view = 'discovered';
      void discover();
    }}>{ru ? 'Обнаруженное оборудование' : 'Discovered hardware'}</button
  >
  <button disabled={client.busy} onclick={() => void add()}
    ><Icon name="plus" />{lt('create')}</button
  >
</div>
{#if view === 'discovered'}
  <div class="device-lifecycle-toolbar">
    <h3>{ru ? 'Добавить устройство' : 'Add device'}</h3>
    <button disabled={discovering} onclick={() => void discover()}
      >{ru ? 'Обновить список' : 'Refresh list'}</button
    >
  </div>
  <p class="settings-intro">
    {ru
      ? 'Новое оборудование внешних драйверов добавляется только по вашему выбору. Подключение не меняет настройки клавиатуры.'
      : 'External-provider hardware is added only when you choose it. Connecting does not change keyboard settings.'}
  </p>
  {#if discoveryError}<p class="connection-error" role="alert">{discoveryError}</p>{/if}
  <div class="saved-device-list">
    {#each inventory.discovered as found (found.provider_id + found.instance_id)}
      {@const retained = saved.find(
        (d) => d.provider_id === found.provider_id && d.hardware_key === found.instance_id,
      )}
      <article class="saved-device-card">
        <div>
          <h3>{found.name}</h3>
          <p>
            {inventory.packages.find((p) => p.id === found.provider_id)?.name ?? found.provider_id}
          </p>
          {#if found.error}<p class="connection-error">{found.error}</p>{/if}
        </div>
        {#if found.device_id && devices.some((d) => d.id === found.device_id)}<button
            onclick={() => {
              chosen = found.device_id;
              view = 'active';
            }}>{t('deviceSettings')}</button
          >
        {:else if retained || (found.device_id && saved.some((d) => d.id === found.device_id))}<button
            onclick={() => reviewPairing(found)}>{lt('restore')}</button
          >
        {:else}<button
            class="primary"
            disabled={discovering || !!found.error}
            onclick={() => reviewPairing(found)}
            >{found.state === 'pairing_required'
              ? ru
                ? 'Подключить сохранённое'
                : 'Rebind saved device'
              : ru
                ? 'Добавить'
                : 'Add device'}</button
          >{/if}
      </article>
    {/each}
    {#each inventory.packages.filter((p) => p.error) as provider}<p class="connection-error">
        {provider.name}: {provider.error}
      </p>{/each}
    {#if !inventory.discovered.length}<p>
        {ru
          ? 'Оборудование внешних драйверов не обнаружено. Mirabox появляется в списке устройств автоматически.'
          : 'No external-provider hardware discovered. Mirabox appears in Your devices automatically.'}
      </p>{/if}
  </div>
  <p class="settings-intro">{lt('addSupport')}</p>
{:else if view === 'saved'}
  <div class="saved-device-list">
    {#if !saved.length && !forgotten.length}<p>{lt('emptySaved')}</p>{/if}
    {#each saved as item (item.id)}
      <article class="saved-device-card">
        <div>
          <h3>{item.name}</h3>
          <p>
            {item.provider_id === 'deckweave.n4'
              ? 'Mirabox N4 Pro'
              : item.provider_id === 'deckweave.virtual'
                ? t('virtual')
                : item.provider_id} · {item.profiles.length + 1}
            {lt('profiles')}
          </p>
          <small>{lt('retained')}</small>
        </div>
        <div class="saved-device-actions">
          <button class="primary" disabled={client.busy} onclick={() => void restore(item.id)}
            >{lt('restore')}</button
          ><button
            class="device-danger"
            disabled={client.busy}
            onclick={() => ask('forget_device', item)}>{lt('forget')}</button
          >
        </div>
      </article>
    {/each}
    {#if saved.length}<p class="settings-intro">{lt('restoring')}</p>{/if}
    {#each forgotten as item, index}
      <article class="saved-device-card">
        <div>
          <h3>{lt('forgotten')} {index + 1}</h3>
          <p>{item.provider_id === 'deckweave.n4' ? 'Mirabox N4 Pro' : item.provider_id}</p>
          <small>{lt('forgottenHint')}</small>
        </div>
        <button
          disabled={client.busy}
          onclick={() =>
            void client.send((revision) => ({
              type: 'allow_hardware',
              provider_id: item.provider_id,
              hardware_key: item.hardware_key,
              expected_revision: revision,
            }))}>{lt('readd')}</button
        >
      </article>
    {/each}
  </div>
{:else}
  {#if !devices.length}<p>{lt('emptyActive')}</p>{/if}
  <div class="settings-devices">
    <div class="settings-device-list" aria-label={t('devices')}>
      {#each devices as item}<button
          class:current={device?.id === item.id}
          onclick={() => (chosen = item.id)}
          ><span class="device-list-icon" aria-hidden="true"
            ><Icon
              name={item.provider_descriptor?.definition.keyboard ? 'keyboard' : 'device'}
              size={20}
            /></span
          ><span
            ><strong>{item.name}</strong><small
              ><i
                class="status-dot"
                class:paused={['offline', 'error'].includes(snapshot.connections[item.id].state)}
              ></i>{t(
                snapshot.connections[item.id].state === 'error'
                  ? 'connectionError'
                  : snapshot.connections[item.id].state,
              )}</small
            ></span
          ></button
        >{/each}
    </div>
    {#if device && connection}<div class="device-preferences">
        <div class="settings-device-heading">
          <span class="device-hero" aria-hidden="true"
            ><Icon
              name={device.provider_descriptor?.definition.keyboard ? 'keyboard' : 'device'}
              size={22}
            /></span
          >
          <div>
            <h3>{device.name}</h3>
            <p>
              {device.provider_id === 'deckweave.n4'
                ? 'Mirabox N4 Pro'
                : device.provider_id === 'deckweave.virtual'
                  ? t('virtual')
                  : device.provider_id}
            </p>
          </div>
        </div>
        <div class="settings-group">
          <label class="settings-row"
            ><span>{t('deviceName')}</span><input
              maxlength="80"
              value={device.name}
              onchange={(e) => {
                const name = e.currentTarget.value;
                const id = device!.id;
                void client.send((revision) => ({
                  type: 'rename_device',
                  device_id: id,
                  name,
                  expected_revision: revision,
                }));
              }}
            /></label
          >
          <label class="settings-row"
            ><span title={t('defaultProfileHint')}>{t('defaultProfile')}</span><select
              value={device.default_profile ?? ''}
              onchange={(e) => {
                const default_profile = e.currentTarget.value || null;
                const id = device!.id;
                void client.send((revision) => ({
                  type: 'device_preferences',
                  device_id: id,
                  default_profile,
                  expected_revision: revision,
                }));
              }}
              ><option value="">{t('keepCurrentProfile')}</option
              >{#each [device.profile, ...device.profiles] as profile}<option value={profile.id}
                  >{profile.name}</option
                >{/each}</select
            ></label
          >
          <div class="settings-row">
            <span>{t('connection')}</span><span class="pill"
              >{t(connection.state === 'error' ? 'connectionError' : connection.state)}</span
            >
          </div>
          <div class="settings-row">
            <span>{t('controls')}</span><span>{controlsSummary}</span>
          </div>
        </div>
        {#if !device.provider_descriptor && !['deckweave.n4', 'deckweave.virtual'].includes(device.provider_id)}<p
            class="connection-error"
          >
            {lt('missing')}
          </p>{/if}
        {#if device.provider_id === 'deckweave.n4'}<DeviceHardwareSettings
            {client}
            {device}
            status={snapshot.hardware_settings?.[device.id]}
          />{/if}
        {#if connection.error}<p class="connection-error">{connection.error}</p>{/if}
        <div class="device-lifecycle-toolbar">
          <button
            class="device-danger"
            disabled={client.busy}
            onclick={() => ask('remove_device', device!)}>{lt('remove')}</button
          >
          <button
            class="primary"
            disabled={!editorAvailable}
            onclick={() => void client.openDevice(device!.id)}>{t('openEditor')} →</button
          >
        </div>
        {#if device.provider_id === 'deckweave.n4'}<details class="device-diagnostics">
            <summary>{t('diagnostics')}</summary>
            <p>
              {t('inputEvents')}: {connection.input_events} · {t('outputFrames')}: {connection.output_frames}
            </p>
            <p>{t('n4Limit')}</p>
            <p>{t('n4Note')}</p>
          </details>{/if}
      </div>{/if}
  </div>
{/if}
<dialog bind:this={dialog} class="device-confirm-dialog" onclose={() => (confirmation = null)}>
  {#if confirmation}
    <h3>{lt(confirmation.action === 'remove_device' ? 'removeTitle' : 'forgetTitle')}</h3>
    <p><strong>{confirmation.device.name}</strong></p>
    <p>{lt(confirmation.action === 'remove_device' ? 'removeHint' : 'forgetHint')}</p>
    <div class="saved-device-actions">
      <button disabled={client.busy} onclick={() => dialog.close()}>{lt('cancel')}</button><button
        class="device-danger"
        disabled={client.busy}
        onclick={() => void confirm()}
        >{lt(confirmation.action === 'remove_device' ? 'confirmRemove' : 'confirmForget')}</button
      >
    </div>
  {/if}
</dialog>

<dialog
  bind:this={pairingDialog}
  class="device-confirm-dialog"
  onclose={() => {
    pairing = null;
    pairingConfirmed = false;
  }}
>
  {#if pairing}
    <h3>{ru ? 'Подключить устройство' : 'Connect device'}</h3>
    <p><strong>{pairing.name}</strong></p>
    <p>
      {ru
        ? 'Выберите новую настройку или сохранённое устройство той же модели. Постоянный серийный номер не подтверждён: программа не выбирает соответствие автоматически.'
        : 'Choose a new configuration or a saved device of the same model. Permanent identity is unverified; no match is chosen automatically.'}
    </p>
    <label class="pairing-choice"
      >{ru ? 'Использовать настройки' : 'Use configuration'}<select
        bind:value={pairingTarget}
        onchange={() => (pairingConfirmed = false)}
      >
        <option value="" disabled>{ru ? 'Выберите вариант' : 'Choose an option'}</option>
        <option value="new" disabled={!!claimedPairing}
          >{ru ? 'Создать новое устройство' : 'Create a new device'}</option
        >
        {#each compatiblePairing as retained (retained.id)}<option value={retained.id}
            >{retained.name} · {retained.profiles.length + 1}
            {lt('profiles')}{snapshot.configuration.removed_devices.some(
              (d) => d.id === retained.id,
            )
              ? ru
                ? ' · сохранённое'
                : ' · removed'
              : ''}</option
          >{/each}
      </select></label
    >
    {#if pairingTarget && pairingTarget !== 'new'}
      <p>
        {ru
          ? 'Профили, действия, пресеты и имя выбранного устройства сохранятся. Предыдущее подключение будет заменено; выполняемые действия остановятся. Настройки в оборудование автоматически не записываются.'
          : 'Profiles, actions, presets and the selected device name will be preserved. Its previous connection will be replaced and running actions stopped. Saved settings will not be written to hardware automatically.'}
      </p>
      <label class="pairing-confirm"
        ><input type="checkbox" bind:checked={pairingConfirmed} />{ru
          ? 'Подтверждаю перенос выбранных настроек на это подключённое устройство'
          : 'I confirm attaching the selected saved configuration to this connected hardware'}</label
      >
    {/if}
    {#if discoveryError}<p class="connection-error" role="alert">{discoveryError}</p>{/if}
    <div class="saved-device-actions">
      <button disabled={discovering} onclick={() => pairingDialog.close()}>{lt('cancel')}</button
      ><button
        class="primary"
        disabled={discovering || !pairingTarget || (pairingTarget !== 'new' && !pairingConfirmed)}
        onclick={() => void confirmPairing()}
        >{pairingTarget === 'new'
          ? ru
            ? 'Создать и подключить'
            : 'Create and connect'
          : ru
            ? 'Восстановить подключение'
            : 'Rebind saved device'}</button
      >
    </div>
  {/if}
</dialog>

<style>
  .pairing-choice {
    display: grid;
    gap: 8px;
    margin: 18px 0;
  }
  .pairing-confirm {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    line-height: 1.5;
    margin: 18px 0;
  }
  .device-lifecycle-toolbar,
  .device-lifecycle-tabs,
  .saved-device-actions {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .device-lifecycle-toolbar {
    justify-content: space-between;
    margin: 16px 0;
  }
  .device-lifecycle-tabs .current {
    background: var(--accent, var(--selection));
    color: white;
  }
  .saved-device-list {
    display: grid;
    gap: 12px;
  }
  .saved-device-card {
    display: flex;
    justify-content: space-between;
    gap: 20px;
    align-items: center;
    padding: 16px;
    border: 1px solid #ffffff20;
    border-radius: 10px;
  }
  .saved-device-card h3 {
    margin: 0 0 6px;
  }
  .saved-device-card p {
    margin: 4px 0;
  }
  .saved-device-card small {
    opacity: 0.72;
  }
  .saved-device-actions {
    justify-content: flex-end;
  }
  .device-danger {
    color: #ff9292;
  }
  .device-confirm-dialog {
    max-width: 520px;
    background: var(--surface);
    color: inherit;
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 24px;
  }
  .device-confirm-dialog::backdrop {
    background: #0009;
  }
  .device-confirm-dialog p {
    line-height: 1.5;
  }
  @media (max-width: 650px) {
    .saved-device-card {
      align-items: flex-start;
      flex-direction: column;
    }
  }
</style>
