<script lang="ts">
  import type { Client } from './client.svelte';
  import type { Locale, Preferences } from './types';
  import { text } from './i18n';
  import Registry from './Registry.svelte';
  import PanelManager from './PanelManager.svelte';
  import PluginManager from './PluginManager.svelte';
  import Icon from './Icon.svelte';
  import appIcon from '../../../assets/branding/icon.png';
  let { client }: { client: Client } = $props();
  let tab = $state('devices');
  const config = $derived(client.snapshot!.configuration);
  const t = $derived((key: Parameters<typeof text>[1]) => text(config.locale, key));
  $effect(() => {
    if (client.settingsRequest) {
      tab = client.settingsSection;
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
    {#each [['general', 'sliders'], ['devices', 'device'], ['panels', 'pages'], ['plugins', 'plugin'], ['about', 'info']] as [name, icon]}
      <button class:current={tab === name} onclick={() => (tab = name)}
        ><Icon name={icon} />{name === 'panels'
          ? config.locale === 'ru'
            ? 'Панели'
            : 'Panels'
          : t(name as 'general' | 'devices' | 'plugins' | 'about')}</button
      >
    {/each}
    <button class="settings-quit" onclick={() => void client.quit()}
      ><Icon name="power" />{t('quit')}</button
    >
  </nav>
  <section class="settings-content">
    {#if tab === 'devices'}
      <Registry {client} />
    {:else if tab === 'panels'}
      <PanelManager ru={config.locale === 'ru'} />
    {:else if tab === 'general'}
      <h2>{t('general')}</h2>
      <div class="settings-group">
        <div class="settings-row">
          <span>{config.locale === 'ru' ? 'Тема' : 'Theme'}</span>
          <div
            class="segmented"
            role="group"
            aria-label={config.locale === 'ru' ? 'Тема' : 'Theme'}
          >
            {#each ['dark', 'light'] as theme}<button
                class:current={config.preferences.theme === theme}
                aria-pressed={config.preferences.theme === theme}
                onclick={() => preference({ theme: theme as 'dark' | 'light' })}
                >{theme === 'dark'
                  ? config.locale === 'ru'
                    ? 'Тёмная'
                    : 'Dark'
                  : config.locale === 'ru'
                    ? 'Светлая'
                    : 'Light'}</button
              >{/each}
          </div>
        </div>
        <div class="settings-row">
          <span>{config.locale === 'ru' ? 'Акцентный цвет' : 'Accent color'}</span>
          <div class="accent-options">
            {#each ['#e8b466', '#8fb9ff', '#8dceac', '#d5a1df', '#ed9b8b'] as color}
              <button
                class="accent-swatch"
                style:--swatch={color}
                aria-label={color}
                title={color}
                aria-pressed={config.preferences.accent_color === color}
                class:current={config.preferences.accent_color === color}
                onclick={() => preference({ accent_color: color })}><span></span></button
              >
            {/each}
            <input
              type="color"
              aria-label={config.locale === 'ru' ? 'Свой акцентный цвет' : 'Custom accent color'}
              value={config.preferences.accent_color}
              onchange={(e) => preference({ accent_color: e.currentTarget.value })}
            />
          </div>
        </div>
        <div class="settings-row">
          <span>{t('language')}</span>
          <div class="segmented" role="group" aria-label={t('language')}>
            {#each [['ru', 'Русский'], ['en', 'English']] as [code, name]}<button
                class:current={config.locale === code}
                aria-pressed={config.locale === code}
                onclick={() => {
                  const locale = code as Locale;
                  void client.send((revision) => ({
                    type: 'set_locale',
                    locale,
                    expected_revision: revision,
                  }));
                }}>{name}</button
              >{/each}
          </div>
        </div>
        <div class="settings-row">
          <span title={t('previewScaleHint')}>{t('previewScale')}</span>
          <div class="segmented" role="group" aria-label={t('previewScale')}>
            {#each [75, 100, 125, 150] as value}<button
                class:current={config.preferences.preview_scale === value}
                aria-pressed={config.preferences.preview_scale === value}
                onclick={() => preference({ preview_scale: value })}>{value}%</button
              >{/each}
          </div>
        </div>
        <label class="settings-row"
          ><span title={t('smartProfilesHint')}>Smart Profiles</span><input
            type="checkbox"
            checked={config.preferences.smart_profiles}
            onchange={(e) => preference({ smart_profiles: e.currentTarget.checked })}
          /></label
        >
      </div>
      <p class="settings-intro">{t('backgroundHint')}</p>
    {:else if tab === 'plugins'}
      <PluginManager {client} />
    {:else}
      <div class="about-app">
        <img class="brand-mark" src={appIcon} alt="" />
        <h2>DeckWeave</h2>
        <p>0.1.0</p>
        <p>{t('aboutDescription')}</p>
        <small>GPL-3.0-or-later · Rust · Tauri 2 · Svelte</small>
      </div>
    {/if}
  </section>
</main>
