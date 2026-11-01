<script lang="ts">
  import type { Client } from './client.svelte';
  import type { Locale, Preferences } from './types';
  import { text } from './i18n';
  import Registry from './Registry.svelte';
  let { client }: { client: Client } = $props();
  let tab = $state('devices');
  const config = $derived(client.snapshot!.configuration);
  const t = $derived((key: Parameters<typeof text>[1]) => text(config.locale, key));
  $effect(() => {
    if (client.settingsRequest) {
      tab = client.settingsDevice ? 'devices' : 'general';
    }
  });
  function preference(update: Partial<Preferences>) {
    void client.send((revision) => ({
      type: 'set_preferences',
      preferences: { ...client.snapshot!.configuration.preferences, ...update },
      expected_revision: revision,
    }));
  }
</script>

<main class="settings-layout">
  <nav class="settings-nav" aria-label={t('settings')}>
    <h1>{t('settings')}</h1>
    {#each [['general', '☷'], ['devices', '▦'], ['plugins', '◈'], ['about', 'ⓘ']] as [name, icon]}
      <button class:current={tab === name} onclick={() => (tab = name)}
        ><span aria-hidden="true">{icon}</span>{t(
          name as 'general' | 'devices' | 'plugins' | 'about',
        )}</button
      >
    {/each}
    <button class="settings-quit" onclick={() => void client.quit()}>{t('quit')}</button>
  </nav>
  <section class="settings-content">
    {#if tab === 'devices'}
      <Registry {client} />
    {:else if tab === 'general'}
      <h2>{t('general')}</h2>
      <p class="settings-intro">{t('settingsSaveHint')}</p>
      <div class="settings-group">
        <label class="settings-row"
          ><span>{t('language')}</span><select
            value={config.locale}
            onchange={(e) => {
              const locale = e.currentTarget.value as Locale;
              void client.send((revision) => ({
                type: 'set_locale',
                locale,
                expected_revision: revision,
              }));
            }}><option value="ru">Русский</option><option value="en">English</option></select
          ></label
        >
        <label class="settings-row"
          ><span>{t('previewScale')}<small>{t('previewScaleHint')}</small></span><select
            value={config.preferences.preview_scale}
            onchange={(e) => preference({ preview_scale: Number(e.currentTarget.value) })}
            >{#each [75, 100, 125, 150] as value}<option {value}>{value}%</option>{/each}</select
          ></label
        >
        <label class="settings-row"
          ><span>{t('compactInterface')}<small>{t('compactHint')}</small></span><input
            type="checkbox"
            checked={config.preferences.compact}
            onchange={(e) => preference({ compact: e.currentTarget.checked })}
          /></label
        >
        <label class="settings-row"
          ><span>Smart Profiles<small>{t('smartProfilesHint')}</small></span><input
            type="checkbox"
            checked={config.preferences.smart_profiles}
            onchange={(e) => preference({ smart_profiles: e.currentTarget.checked })}
          /></label
        >
      </div>
      <div class="settings-callout">
        <strong>{t('backgroundWork')}</strong>
        <p>{t('backgroundHint')}</p>
      </div>
    {:else if tab === 'plugins'}
      <h2>{t('plugins')}</h2>
      <p class="settings-intro">{t('pluginCatalogHint')}</p>
      <div class="settings-group">
        <label class="settings-row"
          ><span
            >{t('elgatoWeatherPermission')}<small>{t('elgatoWeatherPermissionHint')}</small></span
          ><input
            type="checkbox"
            checked={config.preferences.allow_weather_geolocation}
            onchange={(e) => preference({ allow_weather_geolocation: e.currentTarget.checked })}
          /></label
        >
      </div>
      <div class="installed-plugins">
        {#each client.plugins as pack}<article>
            <span class="package-symbol">◈</span>
            <div>
              <h3>{pack.name}</h3>
              <p>
                {pack.version} · {pack.resource_only
                  ? t('resourcePackage')
                  : `${pack.actions.filter((a) => a.visible).length} ${t('actions').toLocaleLowerCase()}`}
              </p>
              {#if pack.actions.length && pack.actions.every((a) => !a.visible)}<p>
                  {t('hiddenVendorActions')}
                </p>{/if}
              {#if pack.error}<p class="connection-error">{pack.error}</p>{/if}
              {#if pack.actions.some((a) => a.visible)}<details>
                  <summary>{t('actions')}</summary>{#each pack.actions as action}<p>
                      {action.name} <small>{action.controllers.join(' · ')}</small>
                    </p>{/each}
                </details>{/if}
            </div>
            <span class="pill" class:plugin-error={!!pack.error}
              >{pack.error
                ? t('unavailable')
                : pack.id === 'com.elgato.weather' && !config.preferences.allow_weather_geolocation
                  ? t('disabledWeather')
                  : t('available')}</span
            >
          </article>{/each}
      </div>
    {:else}
      <div class="about-app">
        <div class="brand-mark">D</div>
        <h2>DeckWeave</h2>
        <p>0.1.0</p>
        <p>{t('aboutDescription')}</p>
        <small>GPL-3.0-or-later · Rust · Tauri 2 · Svelte</small>
      </div>
    {/if}
  </section>
</main>
