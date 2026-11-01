<script lang="ts">
  import { onMount } from 'svelte';
  import { Client } from './client.svelte';
  import { text } from './i18n';
  import DeviceEditor from './DeviceEditor.svelte';
  import Registry from './Registry.svelte';
  import type { Locale } from './types';

  const client = new Client();
  const locale = $derived(client.snapshot?.configuration.locale ?? 'ru');
  const t = $derived((key: Parameters<typeof text>[1]) => text(locale, key));
  const device = $derived(
    client.snapshot?.configuration.devices.find((d) => d.id === client.deviceId),
  );
  onMount(() => {
    void client.start();
    return () => client.stop();
  });
  $effect(() => {
    document.documentElement.lang = locale;
  });
</script>

<div class="app-shell">
  <header class="app-bar">
    <div class="brand">
      <span class="brand-mark">D</span><strong>DeckWeave</strong><span class="build-label"
        >0.1 · {t('foundation')}</span
      >
    </div>
    <div class="app-tools">
      <span class="save-status" aria-live="polite"
        >{client.busy ? t('saving') : client.snapshot ? t('saved') : ''}</span
      >
      <select
        aria-label="Language"
        value={locale}
        onchange={(e) => {
          const nextLocale = e.currentTarget.value as Locale;
          void client.send((revision) => ({
            type: 'set_locale',
            locale: nextLocale,
            expected_revision: revision,
          }));
        }}
        disabled={!client.snapshot}
      >
        <option value="ru">RU</option><option value="en">EN</option>
      </select>
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
    <Registry {client} />
  {:else if device}
    <DeviceEditor {client} {device} />
  {:else}
    <main class="empty-screen"><p>{t('error')}: {client.deviceId}</p></main>
  {/if}
</div>
