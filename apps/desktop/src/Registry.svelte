<script lang="ts">
  import type { Client } from './client.svelte';
  import { text } from './i18n';
  let { client }: { client: Client } = $props();
  let chosen = $state<string | null>(null);
  const snapshot = $derived(client.snapshot!);
  const t = $derived((key: Parameters<typeof text>[1]) => text(snapshot.configuration.locale, key));
  const devices = $derived(snapshot.configuration.devices);
  const device = $derived(
    devices.find((d) => d.id === chosen) ??
      devices.find((d) => snapshot.connections[d.id]?.state === 'connected') ??
      devices[0],
  );
  const connection = $derived(device ? snapshot.connections[device.id] : null);
  $effect(() => {
    if (client.settingsDevice) chosen = client.settingsDevice;
  });
  async function add() {
    if (await client.send((revision) => ({ type: 'add_device', expected_revision: revision })))
      chosen = client.snapshot!.configuration.devices.at(-1)!.id;
  }
</script>

<h2>{t('devices')}</h2>
<p class="settings-intro">{t('registryIntro')}</p>
{#if snapshot.provider_error}<p class="connection-error">{snapshot.provider_error}</p>{/if}
<div class="settings-devices">
  <div class="settings-device-list" aria-label={t('devices')}>
    {#each devices as item}<button
        class:current={device?.id === item.id}
        onclick={() => (chosen = item.id)}
        ><span class="device-list-icon" aria-hidden="true">▦</span><span
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
    <button class="add-device" disabled={client.busy} onclick={() => void add()}
      >＋ {t('addDevice')}</button
    >
  </div>
  {#if device && connection}<div class="device-preferences">
      <div class="settings-device-heading">
        <span class="device-hero" aria-hidden="true">▦</span>
        <div>
          <h3>{device.name}</h3>
          <p>{device.provider_id === 'deckweave.n4' ? 'Mirabox N4 Pro' : t('virtual')}</p>
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
          ><span>{t('defaultProfile')}<small>{t('defaultProfileHint')}</small></span><select
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
          <span>{t('controls')}</span><span
            >{snapshot.geometries[device.id].controls.filter((c) => c.kind === 'key').length}
            {t('keysLabel')} · {snapshot.geometries[device.id].controls.filter(
              (c) => c.kind === 'dial',
            ).length}
            {t('dialsLabel')} · {t('touchStripLabel')}</span
          >
        </div>
      </div>
      {#if connection.error}<p class="connection-error">{connection.error}</p>{/if}
      <button class="primary" onclick={() => void client.openDevice(device!.id)}
        >{t('openEditor')} →</button
      >
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
