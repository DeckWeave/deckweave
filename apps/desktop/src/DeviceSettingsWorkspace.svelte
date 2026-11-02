<script lang="ts">
  import type { Client } from './client.svelte';
  import type { Device } from './types';
  import { text } from './i18n';
  import DeviceHardwareSettings from './DeviceHardwareSettings.svelte';
  import ProviderSettings from './ProviderSettings.svelte';
  import KeyboardSettings from './KeyboardSettings.svelte';
  import Icon from './Icon.svelte';
  let { client, device }: { client: Client; device: Device } = $props();
  const snapshot = $derived(client.snapshot!);
  const t = $derived((key: Parameters<typeof text>[1]) => text(snapshot.configuration.locale, key));
</script>

<div class="device-settings-workspace">
  <header>
    <h2>{t('deviceSettings')}</h2>
    <button onclick={() => void client.openSettings(device.id)}
      ><Icon name="device" />{t('devices')}</button
    >
  </header>
  <label class="settings-row"
    ><span>{t('deviceName')}</span><input
      maxlength="80"
      value={device.name}
      onchange={(e) => {
        const name = e.currentTarget.value;
        void client.send((revision) => ({
          type: 'rename_device',
          device_id: device.id,
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
        void client.send((revision) => ({
          type: 'device_preferences',
          device_id: device.id,
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
  {#if device.provider_id === 'deckweave.n4'}<DeviceHardwareSettings
      {client}
      {device}
      status={snapshot.hardware_settings?.[device.id]}
    />{:else if device.provider_descriptor}<ProviderSettings {client} {device} />
    {#if device.provider_descriptor.definition.keyboard}<KeyboardSettings
        {client}
        {device}
      />{/if}{/if}
</div>

<style>
  .device-settings-workspace {
    padding: 24px;
    overflow: auto;
    min-height: 0;
  }
  .device-settings-workspace header {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: center;
  }
  .device-settings-workspace :global(.provider-settings-workspace) {
    padding: 20px 0;
  }
</style>
