<script lang="ts">
  import type { Client } from './client.svelte';
  import type { Device, DeviceHardwareSettings } from './types';
  import {
    deviceSettingsText,
    type N4SettingsStatus,
    type SettingStatus,
  } from './device-settings-i18n';

  let { client, device, status }: { client: Client; device: Device; status?: N4SettingsStatus } =
    $props();
  const t = $derived((key: Parameters<typeof deviceSettingsText>[1]) =>
    deviceSettingsText(client.snapshot!.configuration.locale, key),
  );
  const screen = $derived(device.hardware_settings.screen_brightness);
  const dials = $derived(device.hardware_settings.dial_brightness);
  let screenPreview = $state<number | undefined>();
  let dialsPreview = $state<number | undefined>();
  $effect(() => {
    screenPreview = screen;
  });
  $effect(() => {
    dialsPreview = dials;
  });
  function update(patch: Partial<DeviceHardwareSettings>) {
    const id = device.id;
    void client.send((revision) => ({
      type: 'set_device_hardware_settings',
      device_id: id,
      settings: {
        ...client.snapshot!.configuration.devices.find((item) => item.id === id)!.hardware_settings,
        ...patch,
      },
      expected_revision: revision,
    }));
  }
  function settingStateName<T>(field: SettingStatus<T> | undefined, configured: T | undefined) {
    return field?.state ?? (configured === undefined ? 'unchanged' : 'offline');
  }
</script>

{#snippet settingState(
  field: SettingStatus<number | boolean> | undefined,
  configured: number | boolean | undefined,
)}
  <small class:error={field?.state === 'error'} aria-live="polite">
    {t(settingStateName(field, configured))}{#if field?.error}: {field.error}{/if}
  </small>
{/snippet}

<section class="hardware-settings" aria-label={t('title')}>
  <h4>{t('title')}</h4>
  <p>{t('hint')}</p>
  <div class="hardware-setting">
    <label for={`screen-${device.id}`}>{t('screen')}</label>
    <output for={`screen-${device.id}`}
      >{screenPreview === undefined ? t('unknown') : `${screenPreview}%`}</output
    >
    <input
      id={`screen-${device.id}`}
      type="range"
      min="0"
      max="100"
      step="1"
      value={screen ?? 50}
      aria-valuetext={screenPreview === undefined ? t('unknown') : `${screenPreview}%`}
      oninput={(event) => (screenPreview = Number(event.currentTarget.value))}
      onchange={(event) => update({ screen_brightness: Number(event.currentTarget.value) })}
    />
    {@render settingState(status?.screen_brightness, screen)}
  </div>
  <div class="hardware-setting">
    <label for={`dials-${device.id}`}>{t('dials')}</label>
    <output for={`dials-${device.id}`}
      >{dialsPreview === undefined ? t('unknown') : `${dialsPreview}%`}</output
    >
    <input
      id={`dials-${device.id}`}
      type="range"
      min="0"
      max="100"
      step="1"
      value={dials ?? 50}
      aria-valuetext={dialsPreview === undefined ? t('unknown') : `${dialsPreview}%`}
      oninput={(event) => (dialsPreview = Number(event.currentTarget.value))}
      onchange={(event) => update({ dial_brightness: Number(event.currentTarget.value) })}
    />
    {@render settingState(status?.dial_brightness, dials)}
  </div>
  <div class="hardware-setting vibration">
    <span>{t('vibration')}</span>
    <div class="vibration-options" role="group" aria-label={t('vibration')}>
      <button
        type="button"
        aria-pressed={device.hardware_settings.vibration === true}
        onclick={() => update({ vibration: true })}>{t('on')}</button
      >
      <button
        type="button"
        aria-pressed={device.hardware_settings.vibration === false}
        onclick={() => update({ vibration: false })}>{t('off')}</button
      >
    </div>
    {@render settingState(status?.vibration, device.hardware_settings.vibration)}
  </div>
</section>

<style>
  .hardware-settings {
    border-top: 1px solid var(--border, var(--line));
    padding-top: 18px;
    margin-top: 18px;
  }
  h4 {
    font-size: 14px;
    margin: 0 0 8px;
  }
  p,
  small {
    color: var(--muted, var(--muted));
    font-size: 12px;
    line-height: 1.45;
  }
  p {
    margin: 0 0 18px;
    max-width: 65ch;
  }
  .hardware-setting {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 7px 12px;
    margin: 0 0 18px;
  }
  label,
  .vibration > span {
    font-size: 13px;
  }
  output {
    font-variant-numeric: tabular-nums;
    font-size: 12px;
  }
  input[type='range'],
  small {
    grid-column: 1 / -1;
  }
  input[type='range'] {
    width: 100%;
    margin: 0;
    accent-color: var(--focus);
  }
  .vibration {
    align-items: center;
  }
  .vibration-options {
    display: flex;
    gap: 4px;
  }
  button {
    padding: 5px 10px;
    font-size: 12px;
    border: 1px solid var(--border, var(--line));
    border-radius: 5px;
    background: transparent;
    color: inherit;
  }
  button[aria-pressed='true'] {
    background: var(--accent, var(--selection));
    border-color: var(--accent, var(--focus));
    color: white;
  }
  small.error {
    color: var(--danger);
  }
</style>
