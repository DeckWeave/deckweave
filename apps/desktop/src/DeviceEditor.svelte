<script lang="ts">
  import type { Client } from './client.svelte';
  import { text } from './i18n';
  import type { Device, ActionKind, Control } from './types';
  let { client, device }: { client: Client; device: Device } = $props();
  let selectedId = $state<string | null>(null);
  let search = $state('');
  let filter = $state<'keys' | 'dials' | 'plugins'>('keys');
  let testing = $state(false);
  const snapshot = $derived(client.snapshot!);
  const geometry = $derived(snapshot.geometries[device.id]);
  const connection = $derived(snapshot.connections[device.id]);
  const t = $derived((key: Parameters<typeof text>[1]) => text(snapshot.configuration.locale, key));
  const page = $derived(device.profile.pages.find((p) => p.id === device.profile.active_page_id)!);
  const strip = $derived(snapshot.key_images[device.id]?.['touch-strip']);
  const stripVisible = $derived(strip?.page_id === page.id);
  const selected = $derived(geometry.controls.find((c) => c.id === selectedId));
  const binding = $derived(selectedId ? page.bindings[selectedId] : undefined);
  const result = $derived(
    binding
      ? snapshot.feedback.find((f) => f.device_id === device.id && f.binding_id === binding.id)
      : undefined,
  );
  const items = $derived(
    (['counter', 'toggle'] as ActionKind[]).filter((kind) =>
      `${t(kind)} ${t(kind === 'counter' ? 'counterHint' : 'toggleHint')}`
        .toLocaleLowerCase()
        .includes(search.toLocaleLowerCase()),
    ),
  );

  function select(control: Control) {
    selectedId = control.id;
    filter = control.kind === 'dial' ? 'dials' : 'keys';
  }
  function assign(kind: ActionKind | null, label?: string) {
    if (!selectedId) return;
    const controlId = selectedId;
    const pageId = page.id;
    const title = label ?? (kind ? t(kind) : '');
    void client.send((revision) => ({
      type: 'configure_control',
      device_id: device.id,
      page_id: pageId,
      control_id: controlId,
      kind,
      label: title,
      expected_revision: revision,
    }));
  }
  async function check() {
    if (!selectedId || !binding || testing) return;
    const controlId = selectedId;
    testing = true;
    try {
      if (device.provider_id === 'deckweave.n4' && selected?.kind === 'dial') {
        await client.input(device.id, controlId, { type: 'click' });
        return;
      }
      if (await client.input(device.id, controlId, { type: 'down' })) {
        // Visible virtual press; this is not the future Multi Action Delay scheduler.
        await new Promise((resolve) => setTimeout(resolve, 220));
        await client.input(device.id, controlId, { type: 'up' });
      }
    } finally {
      testing = false;
    }
  }
</script>

<div class="editor-layout">
  <section class="device-workspace">
    <div class="device-toolbar">
      <div>
        <input
          class="device-name"
          aria-label={t('deviceName')}
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
        />
        <div class="toolbar-meta">
          <span
            class="status-dot"
            class:paused={connection.state === 'offline' || connection.state === 'error'}
          ></span>{t(connection.state === 'error' ? 'connectionError' : connection.state)}<span
            class="meta-divider">/</span
          >{t('profile')}: {device.profile.name}
        </div>
      </div>
      <span class="pill muted">{t('preview')}</span>
    </div>
    <div class="panel-stage">
      <div
        class="device-panel"
        style:aspect-ratio={`${geometry.width}/${geometry.height}`}
        style:--device-ratio={geometry.width / geometry.height}
      >
        <div
          class="touch-surface"
          style:left={`${(geometry.surface[0] / geometry.width) * 100}%`}
          style:top={`${(geometry.surface[1] / geometry.height) * 100}%`}
          style:width={`${(geometry.surface[2] / geometry.width) * 100}%`}
          style:height={`${(geometry.surface[3] / geometry.height) * 100}%`}
        >
          {#if stripVisible}<img class="hardware-key-image" src={strip.data_url} alt="" />{/if}
        </div>
        {#each geometry.controls as control (control.id)}
          {@const assignment = page.bindings[control.id]}
          {@const feedback = assignment
            ? snapshot.feedback.find(
                (f) => f.device_id === device.id && f.binding_id === assignment.id,
              )
            : undefined}
          {@const pressed = snapshot.pressed.some(
            (p) => p.device_id === device.id && p.control_id === control.id,
          )}
          {@const frame = snapshot.key_images[device.id]?.[control.id]}
          <button
            class="control"
            class:key-control={control.kind === 'key'}
            class:dial-control={control.kind === 'dial'}
            class:touch-control={control.kind === 'touch'}
            class:strip-overlay={control.kind === 'touch' && stripVisible}
            class:selected={selectedId === control.id}
            class:pressed
            class:active={feedback?.active}
            style:left={`${(control.rect[0] / geometry.width) * 100}%`}
            style:top={`${(control.rect[1] / geometry.height) * 100}%`}
            style:width={`${(control.rect[2] / geometry.width) * 100}%`}
            style:height={`${(control.rect[3] / geometry.height) * 100}%`}
            title={`${t(control.kind)} ${control.number} · ${assignment?.label || t('unassigned')}`}
            aria-label={`${t(control.kind)} ${control.number}: ${assignment?.label || t('unassigned')}`}
            aria-pressed={selectedId === control.id}
            onclick={() => select(control)}
          >
            {#if control.kind === 'touch' && stripVisible}
              <span class="sr-only">{assignment?.label || t('unassigned')}</span>
            {:else if frame && frame.page_id === page.id && frame.binding_id === (assignment?.id ?? null)}
              <img class="hardware-key-image" src={frame.data_url} alt="" />
            {:else if assignment}<span
                class="control-value"
                class:is-toggle={assignment.kind === 'toggle'}
                >{assignment.kind === 'counter'
                  ? (feedback?.value ?? 0)
                  : feedback?.active
                    ? '●'
                    : '○'}</span
              ><span class="control-label">{assignment.label || t(assignment.kind)}</span
              >{:else}<span class="empty-control">{control.number}</span>{/if}
          </button>
        {/each}
      </div>
    </div>
    <div class="page-row" aria-label={t('currentPage')}>
      {#each device.profile.pages as p (p.id)}<button
          class:current={p.id === page.id}
          title={`${t('page')} ${p.name}`}
          aria-label={`${t('page')} ${p.name}`}
          aria-current={p.id === page.id ? 'page' : undefined}
          onclick={() =>
            void client.send((revision) => ({
              type: 'select_page',
              device_id: device.id,
              page_id: p.id,
              expected_revision: revision,
            }))}>{p.name}</button
        >{/each}
      <button
        class="add-page"
        title={t('addPage')}
        aria-label={t('addPage')}
        onclick={() =>
          void client.send((revision) => ({
            type: 'add_page',
            device_id: device.id,
            expected_revision: revision,
          }))}>＋</button
      >
    </div>
    <div class="inspector">
      {#if selected}
        <div class="inspector-heading">
          <div>
            <span class="eyebrow">{t(selected.kind)} {selected.number}</span>
            <h2>{binding ? t(binding.kind) : t('unassigned')}</h2>
          </div>
          <button
            class="primary test-button"
            disabled={!binding || testing}
            onclick={() => void check()}>{testing ? t('checking') : `▷ ${t('check')}`}</button
          >
        </div>
        {#if binding}
          <label class="form-row"
            ><span>{t('title')}</span><input
              maxlength="80"
              value={binding.label}
              onchange={(e) => assign(binding!.kind, e.currentTarget.value)}
            /></label
          >
          <div class="form-row">
            <span>{t('actionResult')}</span>
            <div class="result-value" aria-live="polite">
              {result
                ? binding.kind === 'counter'
                  ? result.value
                  : result.active
                    ? t('on')
                    : t('off')
                : t('noActionYet')}
            </div>
          </div>
          {#if selected.kind === 'dial'}<div class="dial-tests">
              <button
                onclick={() =>
                  void client.input(device.id, selectedId!, { type: 'rotate', ticks: -1 })}
                >↶ {t('rotateLeft')}</button
              ><button
                onclick={() =>
                  void client.input(device.id, selectedId!, { type: 'rotate', ticks: 1 })}
                >↷ {t('rotateRight')}</button
              >
            </div>{/if}
          <div class="inspector-bottom">
            <p>{t('safeTest')}</p>
            <button class="text-button" onclick={() => assign(null)}>{t('remove')}</button>
          </div>
        {:else}<p class="inspector-hint">{t('assignHint')}</p>{/if}
      {:else}<div class="inspector-empty">
          <div class="select-icon">⌖</div>
          <h2>{t('selectControl')}</h2>
          <p>{t('selectHint')}</p>
        </div>{/if}
    </div>
    <footer class="editor-footer">
      <span class="status-dot paused"></span>{t('automaticPaused')}
      {#if device.provider_id === 'deckweave.n4'}
        <span class="hardware-counts"
          >IN {connection.input_events} · OUT {connection.output_frames}</span
        >
      {/if}
    </footer>
    {#if connection.error}<p class="connection-error hardware-error">{connection.error}</p>{/if}
    {#if device.provider_id === 'deckweave.n4'}<p class="hardware-note hardware-limit">
        {t('n4Limit')}
      </p>{/if}
  </section>
  <aside class="action-library">
    <div class="library-header">
      <h2>{t('actions')}</h2>
      <div class="library-tabs">
        {#each ['keys', 'dials', 'plugins'] as tab}<button
            class:current={filter === tab}
            onclick={() => (filter = tab as typeof filter)}
            >{tab === 'keys' ? 'Keys' : tab === 'dials' ? 'Dials' : 'Plugins'}</button
          >{/each}
      </div>
      <input
        class="search-field"
        aria-label={t('search')}
        placeholder={t('search')}
        bind:value={search}
      />
    </div>
    {#if filter === 'plugins'}<p class="library-empty">{t('pluginsNote')}</p>{:else}<div
        class="library-category"
      >
        ⌄ {t('builtin')}
      </div>
      <div class="action-list">
        {#each items as kind}<button
            class="action-item"
            disabled={!selectedId}
            title={selectedId
              ? t(kind === 'counter' ? 'counterHint' : 'toggleHint')
              : t('chooseFirst')}
            onclick={() => assign(kind)}
            ><span class="action-icon" class:purple={kind === 'toggle'}
              >{kind === 'counter' ? '＋' : '◉'}</span
            ><span
              ><strong>{t(kind)}</strong><small
                >{t(kind === 'counter' ? 'counterHint' : 'toggleHint')}</small
              ></span
            ></button
          >{/each}{#if items.length === 0}<p class="library-empty">{t('noResults')}</p>{/if}
      </div>{/if}
  </aside>
</div>
