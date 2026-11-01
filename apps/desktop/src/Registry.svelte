<script lang="ts">
  import type { Client } from './client.svelte';
  import { text } from './i18n';
  let { client }: { client: Client } = $props();
  const state = $derived(client.snapshot!);
  const t = $derived((key: Parameters<typeof text>[1]) => text(state.configuration.locale, key));
</script>

<main class="registry">
  <div class="registry-heading">
    <div>
      <div class="eyebrow">DECKWEAVE STUDIO</div>
      <h1>{t('yourDevices')}</h1>
      <p>{t('registryIntro')}</p>
    </div>
    <button
      class="primary"
      disabled={client.busy}
      onclick={() =>
        void client.send((revision) => ({ type: 'add_device', expected_revision: revision }))}
      >＋ {t('addDevice')}</button
    >
  </div>
  <p class="hardware-note">{t('n4Note')}</p>
  {#if state.provider_error}<p class="connection-error">{state.provider_error}</p>{/if}
  <div class="device-list">
    {#each state.configuration.devices as device (device.id)}
      {@const connection = state.connections[device.id]}
      <article class="device-card">
        <div class="mini-device" aria-hidden="true">
          <div class="mini-keys">
            {#each Array(10) as _}<i></i>{/each}
          </div>
          <div class="mini-strip"></div>
          <div class="mini-dials">
            {#each Array(4) as _}<i></i>{/each}
          </div>
        </div>
        <div class="device-info">
          <div class="device-title">
            <h2>{device.name}</h2>
            <span class="pill"
              >{t(connection.state === 'error' ? 'connectionError' : connection.state)}</span
            >
          </div>
          <p>{t('independent')}</p>
          <span class="subtle"
            >{t('profile')}: {device.profile.name} · {device.profile.pages.length}
            {t('page').toLocaleLowerCase()}</span
          >
          {#if connection.state !== 'virtual'}
            <p class="subtle">
              {t('inputEvents')}: {connection.input_events} · {t('outputFrames')}: {connection.output_frames}
            </p>
            {#if connection.error}<p class="connection-error">{connection.error}</p>{/if}
          {/if}
        </div>
        <button class="primary" onclick={() => void client.openDevice(device.id)}
          >{t('open')} →</button
        >
      </article>
    {/each}
  </div>
  <div class="registry-footnote">
    <span class:paused={state.smart_profiles_paused} class="status-dot"></span><span
      >{state.smart_profiles_paused ? t('automaticPaused') : t('automaticIdle')}</span
    >
  </div>
  <p class="foundation-note">{t('foundationNote')}</p>
</main>
