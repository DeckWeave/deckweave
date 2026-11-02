<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onDestroy } from 'svelte';
  import type { Client } from './client.svelte';
  import { text } from './i18n';
  import type { PluginPackage } from './types';
  import Icon from './Icon.svelte';
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
    kind: 'action' | 'device';
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
  let operation = 0;
  let returnFocus: HTMLButtonElement | undefined;
  let flowOpen = $state(false),
    installing = $state(false);
  let dialog = $state<HTMLDialogElement>();
  type Installed = PluginPackage & {
    kind: 'action' | 'device';
    source: 'managed' | 'repository' | 'elgato';
    sources: { kind: string; version: string; active: boolean }[];
    blocked: boolean;
    available: boolean;
    phase: string | null;
  };
  type RemovalPreview = {
    kind: 'action' | 'device';
    devices: string[];
    id: string;
    name: string;
    source: string;
    revision: number;
    dependencies: {
      device: string;
      profile: string;
      page: string;
      control: string;
      action: string;
      saved_device: boolean;
    }[];
  };
  let inventory = $state<Installed[]>([]);
  let kindFilter = $state('all');
  let installedKind = $state<'action' | 'device'>('action');
  let lifecycleHistory = $state<{ id: string; phase: string; error: string | null }[]>([]);
  let management = $state<Installed | null>(null);
  let removalPreview = $state<RemovalPreview | null>(null);
  let managementOpen = $state(false);
  let managementDialog = $state<HTMLDialogElement>();
  let managementInvoker: HTMLButtonElement | undefined;
  let inventoryGeneration = 0;
  $effect(() => {
    client.plugins;
    void loadInventory();
  });
  $effect(() => {
    if (managementOpen && managementDialog && !managementDialog.open) managementDialog.showModal();
    else if (!managementOpen && managementDialog?.open) {
      managementDialog.close();
      if (managementInvoker?.isConnected) managementInvoker.focus({ preventScroll: true });
      managementInvoker = undefined;
    }
  });
  const installedResults = $derived(
    inventory.filter(
      (p) =>
        (kindFilter === 'all' || p.kind === kindFilter) &&
        `${p.name} ${p.id}`.toLocaleLowerCase().includes(search.toLocaleLowerCase()),
    ),
  );
  async function loadInventory() {
    const generation = ++inventoryGeneration;
    try {
      const items = await invoke<Installed[]>('plugin_inventory');
      if (!disposed && generation === inventoryGeneration) inventory = items;
    } catch (e) {
      if (!disposed && generation === inventoryGeneration) error = String(e);
    }
  }
  function sourceLabel(source: string) {
    return t(
      source === 'managed'
        ? 'sourceManaged'
        : source === 'elgato'
          ? 'sourceElgato'
          : 'sourceRepository',
    );
  }
  async function loadHistory() {
    try {
      lifecycleHistory = await invoke('plugin_lifecycle_history');
    } catch (e) {
      error = String(e);
    }
  }
  function askRemove(entry: Installed, invoker: HTMLButtonElement) {
    if (busy) return;
    management = entry;
    managementInvoker = invoker;
    removalPreview = null;
    managementOpen = true;
    void run(async (current) => {
      const value = await invoke<RemovalPreview>('plugin_removal_preview', {
        id: entry.id,
        kind: entry.kind,
      });
      if (current()) removalPreview = value;
    });
  }
  function closeManagement() {
    if (installing) return;
    operation++;
    busy = false;
    managementOpen = false;
    management = null;
    removalPreview = null;
    error = '';
    notice = '';
  }
  function removePlugin() {
    if (!removalPreview || !management) return;
    const item = removalPreview;
    void run(async (current) => {
      installing = true;
      try {
        await invoke('plugin_remove', {
          id: item.id,
          kind: item.kind,
          expectedRevision: item.revision,
        });
        await client.reloadPlugins();
        await loadInventory();
        if (current())
          notice = t(item.source === 'managed' ? 'pluginRemoved' : 'pluginDisconnected');
      } finally {
        installing = false;
      }
    });
  }
  function reconnect(entry: Installed) {
    void run(async (current) => {
      await invoke('plugin_reconnect', { id: entry.id, kind: entry.kind });
      await client.reloadPlugins();
      await loadInventory();
      if (current()) notice = t('pluginRestored');
    });
  }
  $effect(() => {
    if (flowOpen && dialog && !dialog.open) dialog.showModal();
    else if (!flowOpen && dialog?.open) {
      dialog.close();
      if (returnFocus?.isConnected) returnFocus.focus({ preventScroll: true });
      returnFocus = undefined;
    }
  });
  const config = $derived(client.snapshot!.configuration);
  const t = $derived((key: Parameters<typeof text>[1]) => text(config.locale, key));
  const results = $derived(
    (catalogue ?? []).filter((p) =>
      `${p.name} ${p.id} ${p.author} ${p.description}`
        .toLocaleLowerCase()
        .includes(search.toLocaleLowerCase()),
    ),
  );
  async function run(work: (current: () => boolean) => Promise<void>) {
    if (busy) return;
    const id = ++operation;
    const current = () => !disposed && id === operation;
    busy = true;
    error = '';
    notice = '';
    try {
      await work(current);
    } catch (e) {
      if (current()) error = String(e);
    } finally {
      if (current()) busy = false;
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
    void run(async (current) => {
      const entries = await invoke<Entry[]>('plugin_catalogue');
      if (!current()) return;
      catalogue = entries;
      browsing = true;
      selected = null;
      assets = [];
      limit = 30;
    });
  }
  function choose(entry: Entry, invoker?: HTMLButtonElement) {
    if (busy) return;
    if (invoker) returnFocus = invoker;
    selected = entry;
    assets = [];
    assetId = '';
    flowOpen = true;
    void run(async (current) => {
      const files = await invoke<Asset[]>('plugin_release_assets', { id: entry.id });
      if (!current()) return;
      assets = files;
      if (assets.length === 1) assetId = String(assets[0].id);
    });
  }
  function pick(invoker: HTMLButtonElement) {
    if (busy) return;
    returnFocus = invoker;
    flowOpen = true;
    void run(async (current) => {
      const value = await invoke<Preview | null>('plugin_pick_archive');
      await adoptPreview(value, current);
      if (!value && current()) closeFlow();
    });
  }
  function prepare() {
    if (!selected || !assetId) return;
    const id = selected.id,
      chosen = Number(assetId);
    void run(async (current) => {
      await discard();
      await adoptPreview(
        await invoke<Preview>('plugin_prepare_asset', { id, assetId: chosen }),
        current,
      );
    });
  }
  async function discard() {
    if (preview) {
      const token = preview.token;
      preview = null;
      await invoke('plugin_discard', { token });
    }
  }
  async function adoptPreview(value: Preview | null, current: () => boolean) {
    if (!current()) {
      if (value) await invoke('plugin_discard', { token: value.token });
    } else preview = value;
  }
  function closeFlow() {
    if (installing) return;
    // Invalidate pending release/download results without moving the catalogue.
    operation++;
    busy = false;
    flowOpen = false;
    selected = null;
    assets = [];
    error = '';
    notice = '';
    if (preview) void discard().catch(() => {});
  }
  function install() {
    if (!preview) return;
    const token = preview.token;
    installedKind = preview.kind;
    void run(async (current) => {
      installing = true;
      try {
        await invoke('plugin_commit', { token });
        if (!current()) return;
        preview = null;
        await client.reloadPlugins();
        if (current())
          notice = t(installedKind === 'device' ? 'deviceInstallSuccess' : 'installSuccess');
      } finally {
        installing = false;
      }
    });
  }
  onDestroy(() => {
    disposed = true;
    operation++;
    if (preview) void invoke('plugin_discard', { token: preview.token }).catch(() => {});
  });
</script>

<div class="catalogue-tools">
  <h2>{t('plugins')}</h2>
  <div class="plugin-toolbar">
    <button disabled={busy} onclick={(e) => pick(e.currentTarget)}
      ><Icon name="install" />{t('installFromFile')}</button
    >
    <button disabled={busy} class:current={browsing} onclick={browse}
      ><Icon name="plugin" />{t('openDeckCatalogue')}</button
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
  <input
    class="plugin-search"
    placeholder={t(browsing ? 'searchPlugins' : 'searchInstalled')}
    aria-label={t(browsing ? 'searchPlugins' : 'searchInstalled')}
    bind:value={search}
    oninput={() => (limit = 30)}
  />
</div>
{#if busy && !flowOpen && !managementOpen}<p role="status">{t('pluginLoading')}</p>{/if}
{#if error && !flowOpen && !managementOpen}<p class="connection-error" role="alert">{error}</p>{/if}
{#if notice && !flowOpen && !managementOpen}<p role="status">{notice}</p>{/if}
{#if browsing}
  <p class="settings-intro">{t('catalogueHint')}</p>
  <div class="installed-plugins">
    {#each results.slice(0, limit) as entry (entry.id)}
      {@const installLabel = t(
        client.plugins.some((pack) => pack.id === entry.id) ? 'updatePlugin' : 'installPlugin',
      )}
      <article>
        <div>
          <h3>{entry.name}</h3>
          <p>{entry.author} · <code>{entry.id}</code></p>
          <p>{entry.description}</p>
        </div>
        <button
          disabled={busy}
          aria-label={`${installLabel}: ${entry.name}`}
          onclick={(e) => choose(entry, e.currentTarget)}>{installLabel}…</button
        >
      </article>{/each}
  </div>
  {#if results.length > limit}<button onclick={() => (limit += 30)}
      >{t('morePlugins')} ({results.length - limit})</button
    >{/if}
{:else}
  <div class="segmented extension-filter" role="group" aria-label={t('extensionType')}>
    {#each [['all', 'allExtensions'], ['action', 'extensionTypeActions'], ['device', 'extensionTypeDevices']] as [value, name]}<button
        class:current={kindFilter === value}
        aria-pressed={kindFilter === value}
        onclick={() => (kindFilter = value)}
        >{t(name as 'allExtensions' | 'extensionTypeActions' | 'extensionTypeDevices')}</button
      >{/each}
  </div>

  <div class="installed-plugins">
    {#if !installedResults.length}<p>{t('noInstalledPlugins')}</p>{/if}
    {#each installedResults as pack (`${pack.kind}:${pack.id}`)}<article>
        <span class="package-symbol"
          ><Icon name={pack.kind === 'device' ? 'device' : 'plugin'} size={20} /></span
        >
        <div>
          <h3>{pack.name}</h3>
          <p>
            {pack.version} · {pack.kind === 'device'
              ? t('extensionTypeDevices')
              : pack.resource_only
                ? t('resourcePackage')
                : `${t('actions')}: ${pack.actions.filter((a) => a.visible).length}`}
          </p>
          {#if pack.blocked}<p class="pill">{t('unavailable')}</p>
            {#if !pack.available}<p>{t('reinstallHint')}</p>{/if}
            {#if pack.phase !== 'removed'}<p class="connection-error">{t('removalPending')}</p>{/if}
          {/if}
          {#if pack.sources.length > 1}<details>
              <summary>{t('sourceCopies')}</summary>{#each pack.sources as source}<p>
                  {sourceLabel(source.kind)} · {source.version}
                </p>{/each}
            </details>{/if}
          {#if pack.error}<p class="connection-error">{pack.error}</p>{/if}
          {#if pack.actions.length && pack.actions.every((a) => !a.visible)}<p>
              {t('hiddenVendorActions')}
            </p>{/if}
          <details class="package-details">
            <summary>{config.locale === 'ru' ? 'Сведения' : 'Details'}</summary><code
              class="package-id">{pack.id}</code
            >
            <p class="plugin-source">{sourceLabel(pack.source)}</p>
            {#if pack.actions.some((a) => a.visible)}<div
                class="package-action-names"
                title={pack.actions
                  .filter((a) => a.visible)
                  .map((a) => a.name)
                  .join(' · ')}
              >
                {#each pack.actions.filter((a) => a.visible) as action}<span>
                    {action.name} <small>{action.controllers.join(' · ')}</small>
                  </span>{/each}
              </div>{/if}
          </details>
        </div>
        <div class="package-operations">
          {#if pack.kind === 'action' && !pack.blocked && !pack.resource_only && pack.actions.some((a) => a.visible)}<label
              class="plugin-enable"
              ><input
                type="checkbox"
                checked={!config.preferences.disabled_plugins.includes(pack.id)}
                onchange={(e) => enabled(pack.id, e.currentTarget.checked)}
                aria-label={`${t('pluginEnabled')}: ${pack.name}`}
              />{t('pluginEnabled')}</label
            >
          {:else if !pack.blocked}<span class="pill"
              >{t(pack.error ? 'unavailable' : 'available')}</span
            >{/if}
          {#if pack.blocked}
            {#if pack.phase !== 'removed'}<button
                disabled={busy}
                onclick={(e) => askRemove(pack, e.currentTarget)}>{t('retry')}</button
              >{/if}
            {#if pack.available}<button disabled={busy} onclick={() => reconnect(pack)}
                >{t('reconnectPlugin')}</button
              >{/if}
          {:else}<button
              class="quiet"
              disabled={busy}
              onclick={(e) => askRemove(pack, e.currentTarget)}
              ><Icon name="trash" />{t(
                pack.source === 'managed' ? 'uninstallPlugin' : 'disconnectPlugin',
              )}</button
            >{/if}
        </div>
      </article>{/each}
  </div>
  <details
    class="device-diagnostics"
    ontoggle={(e) => {
      if (e.currentTarget.open) void loadHistory();
    }}
  >
    <summary>{t('diagnostics')}</summary>{#each lifecycleHistory.slice().reverse() as event}<p>
        <code>{event.id}</code> · {event.phase}{#if event.error}<span class="connection-error">
            · {event.error}</span
          >{/if}
      </p>{/each}
  </details>
{/if}

<dialog
  class="install-dialog"
  bind:this={dialog}
  aria-labelledby="plugin-flow-title"
  oncancel={(e) => {
    e.preventDefault();
    closeFlow();
  }}
>
  <header>
    <div>
      <p>{t(preview || !selected ? 'installReview' : 'releaseFiles')}</p>
      <h2 id="plugin-flow-title">{preview?.name ?? selected?.name ?? t('installReview')}</h2>
    </div>
    <button class="quiet" disabled={installing} aria-label={t('close')} onclick={closeFlow}
      ><Icon name="close" /></button
    >
  </header>
  <div class="dialog-body" aria-busy={busy}>
    {#if busy}<p role="status">{t('pluginLoading')}</p>{/if}
    {#if error}<p class="connection-error" role="alert">{error}</p>{/if}
    {#if notice}<p role="status">{notice}</p>
    {:else if preview}
      <p>
        {preview.version} · {preview.kind === 'device'
          ? t('extensionTypeDevices')
          : `${t('actions')}: ${preview.actions}`}
      </p>
      <p><code>{preview.id}</code></p>
      {#if preview.installed_version}<p>
          {t('installedVersion')}: {preview.installed_version} → {preview.version}
        </p>{/if}
      <p class="plugin-source">{preview.source}</p>
      <details>
        <summary>SHA-256</summary><code class="plugin-source">{preview.sha256}</code>
      </details>
    {:else if selected}
      <p class="plugin-source">{selected.repository}</p>
      {#if assets.length}<label
          >{t('releaseFiles')}<select bind:value={assetId} disabled={busy}
            ><option value="">{t('chooseReleaseFile')}</option>{#each assets as asset}<option
                value={String(asset.id)}
                >{asset.version} · {asset.name} · {(asset.size / 1048576).toFixed(1)} MiB</option
              >{/each}</select
          ></label
        >
      {:else if !busy && !error}<p>{t('noReleaseFiles')}</p>{/if}
    {/if}
  </div>
  <footer>
    {#if notice}{#if installedKind === 'device'}<button
          onclick={() => {
            closeFlow();
            void client.openSettings(null, 'devices');
          }}>{t('devices')}</button
        >{/if}<button class="primary" onclick={closeFlow}>{t('done')}</button>
    {:else}
      <button disabled={installing} onclick={closeFlow}>{t('cancelInstall')}</button>
      {#if preview}<button class="primary" disabled={busy} onclick={install}
          >{t(preview.installed_version ? 'updatePlugin' : 'installPlugin')}</button
        >
      {:else if assets.length}<button class="primary" disabled={busy || !assetId} onclick={prepare}
          >{t('prepareInstall')}</button
        >
      {:else if selected && error}<button
          disabled={busy}
          onclick={() => selected && choose(selected)}>{t('retry')}</button
        >{/if}
    {/if}
  </footer>
</dialog>

<dialog
  class="install-dialog"
  bind:this={managementDialog}
  aria-labelledby="plugin-removal-title"
  oncancel={(e) => {
    e.preventDefault();
    closeManagement();
  }}
>
  <header>
    <div>
      <p>{t(management?.source === 'managed' ? 'removalTitle' : 'disconnectTitle')}</p>
      <h2 id="plugin-removal-title">
        {config.locale === 'ru'
          ? management?.source === 'managed'
            ? 'Удалить плагин'
            : 'Отключить плагин'
          : management?.source === 'managed'
            ? 'Remove plugin'
            : 'Disconnect plugin'} · {management?.name}
      </h2>
    </div>
    <button class="quiet" disabled={installing} aria-label={t('close')} onclick={closeManagement}
      ><Icon name="close" /></button
    >
  </header>
  <div class="dialog-body" aria-busy={busy}>
    {#if busy}<p role="status">{t('pluginLoading')}</p>{/if}
    {#if error}<p class="connection-error" role="alert">{error}</p>{/if}
    {#if notice}<p role="status">{notice}</p>
    {:else}
      <p>{t(management?.source === 'managed' ? 'removalHint' : 'disconnectHint')}</p>
      {#if management?.source !== 'managed'}<p>{t('removalHint')}</p>{/if}
      {#if removalPreview}
        {#if removalPreview.kind === 'device'}<h3>
            {t('affectedDevices')} ({removalPreview.devices.length})
          </h3>
          <ul>
            {#each removalPreview.devices as name}<li>{name}</li>{/each}
          </ul>{:else}
          <h3>{t('removalUses')} ({removalPreview.dependencies.length})</h3>
          {#if !removalPreview.dependencies.length}<p>{t('removalNoUses')}</p>{/if}
          <ul class="dependency-list">
            {#each removalPreview.dependencies as use}<li>
                <strong>{use.action}</strong><small
                  >{use.device} → {use.profile} → {use.page} · {use.control}</small
                >
              </li>{/each}
          </ul>
        {/if}
      {/if}
    {/if}
  </div>
  <footer>
    {#if notice}<button class="primary" onclick={closeManagement}>{t('done')}</button>{:else}<button
        disabled={installing}
        onclick={closeManagement}>{t('cancelInstall')}</button
      ><button class="danger" disabled={busy || !removalPreview} onclick={removePlugin}
        >{t(management?.source === 'managed' ? 'removalConfirm' : 'disconnectConfirm')}</button
      >{/if}
  </footer>
</dialog>

<style>
  .package-operations {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 10px;
  }
  .dependency-list {
    padding-left: 20px;
  }
  .dependency-list li {
    margin: 10px 0;
  }
  .dependency-list small {
    display: block;
    color: var(--muted);
    margin-top: 3px;
  }
  .danger {
    background: #853d4c;
    color: var(--text);
  }
  .plugin-toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 14px 0;
  }
  .plugin-toolbar button.current {
    background: var(--selection);
  }
  .plugin-search {
    width: 100%;
    margin: 0 0 14px;
  }
  .catalogue-tools {
    position: sticky;
    top: calc(-1 * var(--settings-padding));
    z-index: 1;
    background: var(--inset);
    padding: 16px 0 1px;
  }
  .install-dialog {
    width: min(640px, calc(100vw - 48px));
    max-height: calc(100dvh - 64px);
    padding: 0;
    margin: auto;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--panel);
    color: inherit;
    overflow: hidden;
    box-shadow: 0 20px 80px #0008;
  }
  .install-dialog[open] {
    display: flex;
    flex-direction: column;
  }
  .install-dialog::backdrop {
    background: #080b12b3;
  }
  .install-dialog header {
    flex-shrink: 0;
    display: flex;
    justify-content: space-between;
    align-items: start;
    gap: 20px;
    padding: 20px;
    border-bottom: 1px solid var(--line);
  }
  .install-dialog header p {
    font-size: 12px;
    margin-bottom: 5px;
  }
  .install-dialog h2 {
    overflow-wrap: anywhere;
  }
  .dialog-body {
    min-height: 0;
    overflow: auto;
    padding: 20px;
    display: grid;
    gap: 14px;
  }
  .dialog-body label {
    display: grid;
    gap: 8px;
  }
  .install-dialog footer {
    flex-shrink: 0;
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 10px;
    padding: 16px 20px;
    border-top: 1px solid var(--line);
  }
  .plugin-source,
  .package-id {
    overflow-wrap: anywhere;
    color: var(--muted);
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
