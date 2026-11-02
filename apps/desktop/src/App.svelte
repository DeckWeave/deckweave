<script lang="ts">
  import { onMount } from 'svelte';
  import { Client } from './client.svelte';
  import { text } from './i18n';
  import DeviceEditor from './DeviceEditor.svelte';
  import Settings from './Settings.svelte';
  import DepthObservationControl from './DepthObservationControl.svelte';

  type Workspace = 'actions' | 'lighting' | 'settings';
  let workspace = $state<Workspace>('actions');
  let workspaceDevice = $state<string | null>(null);
  const client = new Client();
  const locale = $derived(client.snapshot?.configuration.locale ?? 'ru');
  const t = $derived((key: Parameters<typeof text>[1]) => text(locale, key));
  const device = $derived(
    client.snapshot?.configuration.devices.find((d) => d.id === client.deviceId),
  );
  const lightingSupported = $derived(!!device?.provider_descriptor?.definition.lighting?.length);
  $effect(() => {
    const current = device?.id ?? null;
    if (workspaceDevice !== current) {
      workspaceDevice = current;
      workspace = 'actions';
    }
    if (workspace === 'lighting' && !lightingSupported) workspace = 'actions';
  });
  onMount(() => {
    void client.start();
    return () => client.stop();
  });
  $effect(() => {
    document.documentElement.lang = locale;
  });
</script>

<div class="app-shell" class:compact={client.snapshot?.configuration.preferences.compact}>
  <header class="app-bar" class:device-header={client.deviceId !== 'main' && !!device}>
    <div class="brand">
      <span class="brand-mark">D</span><strong>DeckWeave</strong>
    </div>
    {#if client.deviceId !== 'main' && device}
      <nav class="editor-workspace-tabs" role="group" aria-label={t('deviceSettings')}>
        <button
          class:current={workspace === 'actions'}
          aria-pressed={workspace === 'actions'}
          onclick={() => (workspace = 'actions')}>{t('actions')}</button
        >
        {#if lightingSupported}<button
            class:current={workspace === 'lighting'}
            aria-pressed={workspace === 'lighting'}
            onclick={() => (workspace = 'lighting')}
            >{locale === 'ru' ? 'Освещение' : 'Lighting'}</button
          >{/if}
        <button
          class:current={workspace === 'settings'}
          aria-pressed={workspace === 'settings'}
          onclick={() => (workspace = 'settings')}>{t('deviceSettings')}</button
        >
      </nav>
    {/if}
    <div class="app-tools">
      {#if device?.provider_descriptor?.definition.observation?.keyTravel}{@const travel =
          device.provider_descriptor.definition.observation.keyTravel}<DepthObservationControl
          {client}
          {locale}
          maxUm={travel.maxUm}
          normalTyping={travel.normalTyping}
          connected={client.snapshot?.connections[device.id]?.state === 'connected'}
        />{/if}
      <span class="save-status" aria-live="polite"
        >{client.busy ? t('saving') : client.snapshot ? t('saved') : ''}</span
      >
      {#if client.deviceId !== 'main'}<button
          class="settings-button quiet"
          aria-label={t('settings')}
          title={t('settings')}
          onclick={() => void client.openSettings()}>⚙</button
        >{/if}
    </div>
  </header>

  {#if client.error}
    <div class="error-bar" role="alert">
      <div><strong>{t('error')}</strong><span>{client.error}</span></div>
      <button class="quiet" aria-label={t('dismiss')} onclick={() => (client.error = '')}>×</button>
    </div>
  {/if}

  {#if !client.available}
    <main class="empty-screen">
      <div class="empty-icon">▦</div>
      <h1>{t('desktopRequired')}</h1>
      <p>{t('desktopNote')}</p>
    </main>
  {:else if !client.snapshot}
    <main class="empty-screen">
      <h1>{t('loading')}</h1>
      {#if client.error}<button onclick={() => void client.refresh()}>{t('retry')}</button>{/if}
    </main>
  {:else if client.deviceId === 'main'}
    <Settings {client} />
  {:else if device}
    <DeviceEditor {client} {device} bind:workspace />
  {:else}
    <main class="empty-screen"><p>{t('error')}: {client.deviceId}</p></main>
  {/if}
</div>

<style>
  .app-bar.device-header {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 20px;
  }
  .editor-workspace-tabs {
    display: flex;
    justify-self: center;
    gap: 3px;
    min-width: 0;
    max-width: 100%;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .editor-workspace-tabs::-webkit-scrollbar {
    display: none;
  }
  .editor-workspace-tabs button {
    flex-shrink: 0;
    height: 31px;
    padding: 0 12px;
    border: 1px solid transparent;
    background: transparent;
    white-space: nowrap;
    font-size: 12px;
    border-radius: 5px;
    color: #b4bdcb;
  }
  .editor-workspace-tabs button:hover {
    background: #30353e;
    color: #e5edfa;
  }
  .editor-workspace-tabs button.current {
    background: #34465f;
    border-color: #4a6486;
    color: #eaf2ff;
  }
  .device-header .app-tools {
    gap: 10px;
    white-space: nowrap;
  }
  @media (max-width: 760px) {
    .app-bar.device-header {
      gap: 10px;
      padding: 0 12px;
    }
    .device-header .brand strong {
      display: none;
    }
    .editor-workspace-tabs button {
      padding: 0 9px;
      font-size: 11px;
    }
  }
  @media (max-width: 540px) {
    .device-header .save-status {
      display: none;
    }
    .app-bar.device-header {
      gap: 7px;
    }
    .editor-workspace-tabs {
      justify-self: start;
    }
    .editor-workspace-tabs button {
      padding: 0 7px;
    }
  }
</style>
