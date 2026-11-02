<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onDestroy } from 'svelte';
  import type { Client } from './client.svelte';
  import { text } from './i18n';
  let { client }: { client: Client } = $props();
  type Entry = {
    id: string;
    name: string;
    author: string;
    description: string;
    repository: string;
  };
  type Asset = { id: number; name: string; size: number; version: string };
  type Preview = {
    token: string;
    id: string;
    name: string;
    version: string;
    actions: number;
    sha256: string;
    source: string;
    installed_version: string | null;
  };
  let catalogue = $state<Entry[] | null>(null),
    browsing = $state(false),
    search = $state(''),
    limit = $state(30);
  let selected = $state<Entry | null>(null),
    assets = $state<Asset[]>([]),
    assetId = $state('');
  let preview = $state<Preview | null>(null),
    busy = $state(false),
    error = $state(''),
    notice = $state('');
  let disposed = false;
  const config = $derived(client.snapshot!.configuration);
  const t = $derived((key: Parameters<typeof text>[1]) => text(config.locale, key));
  const results = $derived(
    (catalogue ?? []).filter((p) =>
      `${p.name} ${p.id} ${p.author} ${p.description}`
        .toLocaleLowerCase()
        .includes(search.toLocaleLowerCase()),
    ),
  );
  async function run(work: () => Promise<void>) {
    if (busy) return;
    busy = true;
    error = '';
    notice = '';
    try {
      await work();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
  function enabled(id: string, checked: boolean) {
    void client.send((revision) => ({
      type: 'set_preferences',
      expected_revision: revision,
      preferences: {
        ...client.snapshot!.configuration.preferences,
        disabled_plugins: checked
          ? client.snapshot!.configuration.preferences.disabled_plugins.filter((p) => p !== id)
          : [...new Set([...client.snapshot!.configuration.preferences.disabled_plugins, id])],
      },
    }));
  }
  function browse() {
    void run(async () => {
      catalogue = await invoke<Entry[]>('plugin_catalogue');
      browsing = true;
      selected = null;
      assets = [];
      limit = 30;
    });
  }
  function choose(entry: Entry) {
    void run(async () => {
      selected = entry;
      assets = [];
      assetId = '';
      assets = await invoke<Asset[]>('plugin_release_assets', { id: entry.id });
      if (assets.length === 1) assetId = String(assets[0].id);
    });
  }
  function pick() {
    void run(async () => {
      await discard();
      await adoptPreview(await invoke<Preview | null>('plugin_pick_archive'));
    });
  }
  function prepare() {
    if (!selected || !assetId) return;
    const id = selected.id,
      chosen = Number(assetId);
    void run(async () => {
      await discard();
      await adoptPreview(await invoke<Preview>('plugin_prepare_asset', { id, assetId: chosen }));
    });
  }
  async function discard() {
    if (preview) {
      const token = preview.token;
      preview = null;
      await invoke('plugin_discard', { token });
    }
  }
  async function adoptPreview(value: Preview | null) {
    if (disposed) {
      if (value) await invoke('plugin_discard', { token: value.token });
    } else preview = value;
  }
  function install() {
    if (!preview) return;
    const token = preview.token;
    void run(async () => {
      await invoke('plugin_commit', { token });
      preview = null;
      await client.reloadPlugins();
      notice = t('installSuccess');
    });
  }
  onDestroy(() => {
    disposed = true;
    if (preview) void invoke('plugin_discard', { token: preview.token });
  });
</script>

<h2>{t('plugins')}</h2>
<div class="plugin-toolbar">
  <button disabled={busy} onclick={pick}>{t('installFromFile')}</button>
  <button disabled={busy} class:current={browsing} onclick={browse}>{t('openDeckCatalogue')}</button
  >
  {#if browsing}<button
      disabled={busy}
      class="quiet"
      onclick={() => {
        browsing = false;
        selected = null;
      }}>{t('installedPlugins')}</button
    >{/if}
</div>
{#if busy}<p role="status">{t('pluginLoading')}</p>{/if}
{#if error}<p class="connection-error" role="alert">{error}</p>{/if}
{#if notice}<p role="status">{notice}</p>{/if}
{#if preview}
  <section class="install-preview" aria-label={t('installReview')}>
    <h3>{t('installReview')}: {preview.name}</h3>
    <p>{preview.version} · {t('actions')}: {preview.actions}</p>
    <p><code>{preview.id}</code></p>
    {#if preview.installed_version}<p>
        {t('installedVersion')}: {preview.installed_version} → {preview.version}
      </p>{/if}
    <p class="plugin-source">{preview.source}</p>
    <details><summary>SHA-256</summary><code class="plugin-source">{preview.sha256}</code></details>
    <div class="plugin-toolbar">
      <button class="primary" disabled={busy} onclick={install}
        >{t(preview.installed_version ? 'updatePlugin' : 'installPlugin')}</button
      ><button disabled={busy} onclick={() => void run(discard)}>{t('cancelInstall')}</button>
    </div>
  </section>
{/if}
{#if browsing}
  <p class="settings-intro">{t('catalogueHint')}</p>
  <input
    class="plugin-search"
    placeholder={t('searchPlugins')}
    aria-label={t('searchPlugins')}
    bind:value={search}
    oninput={() => (limit = 30)}
  />
  {#if selected}<section class="plugin-release">
      <h3>{selected.name}</h3>
      <p class="plugin-source">{selected.repository}</p>
      {#if assets.length}<label
          >{t('releaseFiles')}<select bind:value={assetId}
            ><option value="">{t('chooseReleaseFile')}</option>{#each assets as asset}<option
                value={String(asset.id)}
                >{asset.version} · {asset.name} · {(asset.size / 1048576).toFixed(1)} MiB</option
              >{/each}</select
          ></label
        >
        <button disabled={busy || !assetId} onclick={prepare}>{t('prepareInstall')}</button>
      {:else if !busy}<p>{t('noReleaseFiles')}</p>{/if}
    </section>{/if}
  <div class="installed-plugins">
    {#each results.slice(0, limit) as entry (entry.id)}<article>
        <div>
          <h3>{entry.name}</h3>
          <p>{entry.author} · <code>{entry.id}</code></p>
          <p>{entry.description}</p>
        </div>
        <button disabled={busy} onclick={() => choose(entry)}>{t('releaseFiles')}</button>
      </article>{/each}
  </div>
  {#if results.length > limit}<button onclick={() => (limit += 30)}
      >{t('morePlugins')} ({results.length - limit})</button
    >{/if}
{:else}
  <p class="settings-intro">{t('pluginCatalogHint')}</p>
  <div class="installed-plugins">
    {#each client.plugins as pack (pack.id)}<article>
        <span class="package-symbol">◈</span>
        <div>
          <h3>{pack.name}</h3>
          <p>
            {pack.version} · {pack.resource_only
              ? t('resourcePackage')
              : `${t('actions')}: ${pack.actions.filter((a) => a.visible).length}`}
          </p>
          <code class="package-id">{pack.id}</code>
          {#if pack.error}<p class="connection-error">{pack.error}</p>{/if}
          {#if pack.actions.length && pack.actions.every((a) => !a.visible)}<p>
              {t('hiddenVendorActions')}
            </p>{/if}
          {#if pack.actions.some((a) => a.visible)}<details>
              <summary>{t('actions')}</summary
              >{#each pack.actions.filter((a) => a.visible) as action}<p>
                  {action.name} <small>{action.controllers.join(' · ')}</small>
                </p>{/each}
            </details>{/if}
        </div>
        {#if !pack.resource_only && pack.actions.some((a) => a.visible)}<label class="plugin-enable"
            ><input
              type="checkbox"
              checked={!config.preferences.disabled_plugins.includes(pack.id)}
              onchange={(e) => enabled(pack.id, e.currentTarget.checked)}
              aria-label={`${t('pluginEnabled')}: ${pack.name}`}
            />{t('pluginEnabled')}</label
          >
        {:else}<span class="pill">{t(pack.error ? 'unavailable' : 'available')}</span>{/if}
      </article>{/each}
  </div>
{/if}

<style>
  .plugin-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 14px 0;
  }
  .plugin-toolbar button.current {
    background: #3d475f;
  }
  .plugin-search {
    width: 100%;
    margin: 0 0 14px;
  }
  .install-preview,
  .plugin-release {
    padding: 16px;
    border: 1px solid #515b78;
    border-radius: 9px;
    background: #292e3a;
    margin: 16px 0;
  }
  .plugin-release label {
    display: grid;
    gap: 8px;
    margin: 14px 0;
  }
  .plugin-source,
  .package-id {
    overflow-wrap: anywhere;
    color: #aab7cf;
    font-size: 12px;
  }
  .installed-plugins article > div {
    min-width: 0;
    flex: 1;
  }
  .installed-plugins article > button {
    flex-shrink: 0;
  }
  .plugin-enable {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
    font-size: 12px;
  }
  h3 {
    margin: 0 0 8px;
  }
  p {
    line-height: 1.5;
  }
</style>
