<script lang="ts">
  import type { Client } from './client.svelte';
  import { text } from './i18n';
  import type { Device, ActionKind, Control, PluginAction, Plan } from './types';
  import { plan as createPlan } from './action-factory';
  import PlanEditor from './PlanEditor.svelte';
  import PanelEditor from './PanelEditor.svelte';
  import TouchTest from './TouchTest.svelte';
  let { client, device }: { client: Client; device: Device } = $props();
  let selectedId = $state<string | null>(null);
  let search = $state('');
  let filter = $state<'keys' | 'dials' | 'plugins'>('keys');
  let testing = $state(false);
  let profileSettings = $state(false);
  let newProfile = $state('');
  const snapshot = $derived(client.snapshot!);
  const geometry = $derived(snapshot.geometries[device.id]);
  const connection = $derived(snapshot.connections[device.id]);
  const t = $derived((key: Parameters<typeof text>[1]) => text(snapshot.configuration.locale, key));
  const page = $derived(device.profile.pages.find((p) => p.id === device.profile.active_page_id)!);
  const strip = $derived(snapshot.key_images[device.id]?.['touch-strip']);
  const stripVisible = $derived(strip?.page_id === page.id);
  const selected = $derived(geometry.controls.find((c) => c.id === selectedId));
  const binding = $derived(selectedId ? page.bindings[selectedId] : undefined);
  const hasInspector = $derived(
    !!(
      binding?.plugin &&
      client.plugins
        .find((p) => p.id === binding.plugin!.package)
        ?.actions.find((a) => a.id === binding.plugin!.action)?.inspector
    ),
  );
  const result = $derived(
    binding
      ? snapshot.feedback.find((f) => f.device_id === device.id && f.binding_id === binding.id)
      : undefined,
  );
  const items = $derived(
    (['counter', 'toggle'] as const).filter((kind) =>
      `${t(kind)} ${t(kind === 'counter' ? 'counterHint' : 'toggleHint')}`
        .toLocaleLowerCase()
        .includes(search.toLocaleLowerCase()),
    ),
  );
  const visiblePlugins = $derived(
    client.plugins
      .map((pack) => ({
        ...pack,
        actions: pack.name.toLocaleLowerCase().includes(search.toLocaleLowerCase())
          ? pack.actions
          : pack.actions.filter((a) =>
              a.name.toLocaleLowerCase().includes(search.toLocaleLowerCase()),
            ),
      }))
      .filter((pack) => pack.actions.length || (!search && pack.error)),
  );
  const matchingPlans = $derived(
    (filter === 'dials'
      ? ['dial_bindings', 'dial_stack', 'action_wheel']
      : ['page', 'profile', 'back', 'launch', 'hotkey', 'sequence', 'switch', 'key_logic']
    ).filter((type) =>
      t(type as Plan['type'])
        .toLocaleLowerCase()
        .includes(search.toLocaleLowerCase()),
    ) as Plan['type'][],
  );
  const folderVisible = $derived(
    filter === 'keys' && t('folder').toLocaleLowerCase().includes(search.toLocaleLowerCase()),
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
    if (kind === 'plugin' && binding?.plugin) {
      assignPlugin(binding.plugin, title);
      return;
    }
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
  function assignPlugin(plugin: PluginAction, label: string) {
    if (!selectedId) return;
    const controlId = selectedId;
    const pageId = page.id;
    void client.send((revision) => ({
      type: 'configure_plugin',
      device_id: device.id,
      page_id: pageId,
      control_id: controlId,
      plugin,
      label,
      expected_revision: revision,
    }));
  }
  function assignPlan(plan: Plan, label?: string) {
    if (!selectedId) return;
    const controlId = selectedId;
    const pageId = page.id;
    void client.send((revision) => ({
      type: 'configure_program',
      device_id: device.id,
      page_id: pageId,
      control_id: controlId,
      plan,
      label: label ?? binding?.label ?? t(plan.type),
      expected_revision: revision,
    }));
  }
  async function gesture(mode: 'double' | 'hold') {
    if (!selectedId || testing || binding?.plan?.type !== 'key_logic') return;
    const id = selectedId;
    const hold = binding.plan.hold_ms;
    testing = true;
    try {
      const tap = async (ms: number) => {
        if (await client.input(device.id, id, { type: 'down' })) {
          await new Promise((r) => setTimeout(r, ms));
          await client.input(device.id, id, { type: 'up' });
        }
      };
      if (mode === 'hold') {
        await tap(hold + 80);
      } else {
        await tap(35);
        await new Promise((r) => setTimeout(r, 80));
        await tap(35);
      }
    } finally {
      testing = false;
    }
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
        await new Promise((resolve) => setTimeout(resolve, 80));
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
          >
          <select
            aria-label={t('profile')}
            value={device.profile.id}
            onchange={(e) => {
              const profileId = e.currentTarget.value;
              void client.send((revision) => ({
                type: 'select_profile',
                device_id: device.id,
                profile_id: profileId,
                expected_revision: revision,
              }));
            }}
            >{#each [device.profile, ...device.profiles] as profile}<option value={profile.id}
                >{profile.name}</option
              >{/each}</select
          >
          <button
            class="quiet"
            aria-label={t('profileSettings')}
            onclick={() => (profileSettings = !profileSettings)}>⚙</button
          >
        </div>
      </div>
      <span class="pill muted">{t('preview')}</span>
    </div>
    {#if profileSettings}<div class="profile-settings">
        <label
          >{t('application')}<input
            value={device.profile.application ?? ''}
            placeholder="obs64.exe"
            onchange={(e) => {
              const application = e.currentTarget.value;
              void client.send((revision) => ({
                type: 'profile_application',
                device_id: device.id,
                application: application || null,
                expected_revision: revision,
              }));
            }}
          /><small>{t('applicationHint')}</small></label
        >
        <form
          onsubmit={(e) => {
            e.preventDefault();
            if (newProfile.trim()) {
              const name = newProfile.trim();
              void client.send((revision) => ({
                type: 'add_profile',
                device_id: device.id,
                name,
                expected_revision: revision,
              }));
              newProfile = '';
            }
          }}
        >
          <input
            aria-label={t('profileName')}
            maxlength="80"
            bind:value={newProfile}
            placeholder={t('profileName')}
          /><button type="submit" disabled={!newProfile.trim()}>＋ {t('addProfile')}</button>
        </form>
      </div>{/if}
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
            {:else if feedback?.image?.startsWith('data:image/')}<img
                class="plugin-image"
                src={feedback.image}
                alt=""
              />
            {:else if assignment}<span
                class="control-value"
                class:is-toggle={assignment.kind === 'toggle'}
                >{assignment.kind === 'counter'
                  ? (feedback?.value ?? 0)
                  : assignment.kind === 'plugin' || assignment.kind === 'program'
                    ? (feedback?.display ?? '')
                    : feedback?.active
                      ? '●'
                      : '○'}</span
              ><span class="control-label"
                >{feedback?.title ?? (assignment.label || t(assignment.kind))}</span
              >{:else}<span class="empty-control">{control.number}</span>{/if}
          </button>
        {/each}
      </div>
    </div>
    <div class="page-row" aria-label={t('currentPage')}>
      {#if page.parent}<button
          title={t('back')}
          onclick={() =>
            void client.send((revision) => ({
              type: 'select_page',
              device_id: device.id,
              page_id: page.parent!,
              expected_revision: revision,
            }))}>← {t('back')}</button
        ><span>{page.name}</span>{/if}
      {#each device.profile.pages.filter((p) => p.parent === null) as p (p.id)}<button
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
    <PanelEditor
      value={page.panel}
      controls={geometry.controls}
      locale={snapshot.configuration.locale}
      onchange={(layout) => {
        const pageId = page.id;
        void client.send((revision) => ({
          type: 'set_panel_layout',
          device_id: device.id,
          page_id: pageId,
          layout,
          expected_revision: revision,
        }));
      }}
    />
    <div class="inspector" class:composite={!!binding?.plan}>
      {#if selected}
        <div class="inspector-heading">
          <div>
            <span class="eyebrow">{t(selected.kind)} {selected.number}</span>
            <h2>{binding ? t(binding.plan?.type ?? binding.kind) : t('unassigned')}</h2>
          </div>
          <button
            class="primary test-button"
            disabled={!binding || testing || (binding.kind === 'plugin' && result?.running)}
            onclick={() => void check()}>{testing ? t('checking') : `▷ ${t('check')}`}</button
          >
        </div>
        {#if binding}
          {#if result?.error}<p class="connection-error">{result.error}</p>{/if}
          {#if selected.kind === 'key'}<label class="pin-field"
              ><input
                type="checkbox"
                checked={!!device.pinned[selectedId!]}
                onchange={(e) => {
                  const pinned = e.currentTarget.checked;
                  void client.send((revision) => ({
                    type: 'set_pinned',
                    device_id: device.id,
                    control_id: selectedId!,
                    pinned,
                    expected_revision: revision,
                  }));
                }}
              />{t('pinned')}</label
            >{/if}
          {#if binding.plugin}
            {#if selected.kind === 'touch'}
              <label class="form-row"
                ><span>{t('controller')}</span><select
                  value={binding.plugin.controller ?? 'Keypad'}
                  onchange={(e) =>
                    assignPlugin(
                      {
                        ...binding!.plugin!,
                        controller: e.currentTarget.value as 'Keypad' | 'Encoder' | 'Panel',
                      },
                      binding!.label,
                    )}
                >
                  {#each client.plugins
                    .find((p) => p.id === binding!.plugin!.package)
                    ?.actions.find((a) => a.id === binding!.plugin!.action)?.controllers ?? ['Keypad'] as c}<option
                      value={c}>{c}</option
                    >{/each}</select
                ></label
              >
            {/if}
            <div class="dial-tests">
              <button
                disabled={!hasInspector}
                onclick={() => void client.openInspector(binding!.id)}>{t('pluginSettings')}</button
              ><button onclick={() => void client.restartPlugin(binding!.plugin!.package)}
                >{t('restartPlugin')}</button
              >
            </div>
            {#if !hasInspector}<label class="form-row"
                ><span>{t('settingsJson')}</span><textarea
                  rows="3"
                  value={JSON.stringify(binding.plugin.settings, null, 2)}
                  onchange={(e) => {
                    try {
                      const settings = JSON.parse(e.currentTarget.value);
                      void client.send((revision) => ({
                        type: 'plugin_settings',
                        device_id: device.id,
                        binding_id: binding!.id,
                        settings,
                        expected_revision: revision,
                      }));
                    } catch {
                      client.error = 'Invalid JSON';
                    }
                  }}></textarea></label
              >{/if}
          {/if}
          {#if selected.kind === 'touch'}{#key selected.id}<TouchTest
                {client}
                deviceId={device.id}
                control={selected}
                {geometry}
                locale={snapshot.configuration.locale}
              />{/key}{/if}
          <label class="form-row"
            ><span>{t('title')}</span><input
              maxlength="80"
              value={binding.label}
              onchange={(e) => {
                const label = e.currentTarget.value;
                void client.send((revision) => ({
                  type: 'rename_binding',
                  device_id: device.id,
                  binding_id: binding!.id,
                  label,
                  expected_revision: revision,
                }));
              }}
            /></label
          >
          {#if binding.plan}<PlanEditor
              feedback={snapshot.feedback}
              openInspector={(context) => void client.openInspector(context)}
              value={binding.plan}
              {device}
              plugins={client.plugins}
              locale={snapshot.configuration.locale}
              onchange={(next) => assignPlan(next)}
            />
            {#if binding.plan.type === 'key_logic'}<div class="dial-tests">
                <button disabled={testing} onclick={() => void gesture('double')}
                  >{t('testDouble')}</button
                ><button disabled={testing} onclick={() => void gesture('hold')}
                  >{t('testHold')}</button
                >
              </div>{/if}
            <button
              class="quiet"
              onclick={() =>
                void client.send(() => ({ type: 'cancel_actions', device_id: device.id }))}
              >{t('cancelActions')}</button
            >
          {/if}
          <div class="form-row">
            <span>{t('actionResult')}</span>
            <div class="result-value" aria-live="polite">
              {result
                ? result.running
                  ? t('running')
                  : binding.plan?.type === 'switch'
                    ? `${t('nextAction')}: ${result.title ?? ''}`
                    : binding.plan?.type === 'dial_stack' || binding.plan?.type === 'action_wheel'
                      ? `${t('selectedAction')}: ${result.title ?? ''} ${result.display ?? ''}`
                      : binding.kind === 'program'
                        ? (result.display ?? t('noActionYet'))
                        : binding.kind === 'plugin'
                          ? (result.title ?? (result.error || t('connected')))
                          : binding.kind === 'counter'
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
            <p>{t(binding.plugin ? 'pluginTest' : binding.plan ? 'systemTest' : 'safeTest')}</p>
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
    {#if filter === 'plugins'}
      {#each visiblePlugins as pack}
        <div class="library-category">{pack.name} · {pack.version}</div>
        {#if pack.error}<p class="connection-error">{pack.error}</p>{:else}
          <div class="action-list">
            {#each pack.actions as action}<button
                class="action-item"
                disabled={!selected ||
                  !action.controllers.some((c) =>
                    selected.kind === 'touch'
                      ? ['Keypad', 'Encoder', 'Panel'].includes(c)
                      : c === (selected.kind === 'dial' ? 'Encoder' : 'Keypad'),
                  )}
                onclick={() =>
                  assignPlugin(
                    {
                      package: pack.id,
                      action: action.id,
                      settings: {},
                      controller:
                        selected?.kind === 'dial'
                          ? 'Encoder'
                          : selected?.kind === 'touch' && action.controllers.includes('Panel')
                            ? 'Panel'
                            : selected?.kind === 'touch' && action.controllers.includes('Encoder')
                              ? 'Encoder'
                              : 'Keypad',
                    },
                    action.name,
                  )}
                ><span class="action-icon purple">◈</span><span
                  ><strong>{action.name}</strong><small>{action.controllers.join(' · ')}</small
                  ></span
                ></button
              >{/each}
          </div>
        {/if}
      {/each}
      {#if !client.plugins.length}<p class="library-empty">{t('pluginsNote')}</p>{/if}
    {:else}<div class="library-category">
        ⌄ {t('builtin')}
      </div>
      <div class="action-list">
        {#each matchingPlans as type}<button
            class="action-item"
            disabled={!selectedId}
            onclick={() => assignPlan(createPlan(type, device), t(type))}
            ><span class="action-icon purple"
              >{type === 'sequence'
                ? '☷'
                : type === 'switch'
                  ? '⇄'
                  : type === 'key_logic'
                    ? '⌘'
                    : '◇'}</span
            ><strong>{t(type)}</strong></button
          >{/each}
        {#if folderVisible}<button
            class="action-item"
            disabled={!selectedId}
            onclick={() =>
              void client.send((revision) => ({
                type: 'add_folder',
                device_id: device.id,
                control_id: selectedId!,
                name: t('folder'),
                expected_revision: revision,
              }))}><span class="action-icon">▱</span><strong>{t('folder')}</strong></button
          >{/if}
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
          >{/each}{#if items.length === 0 && matchingPlans.length === 0 && !folderVisible}<p
            class="library-empty"
          >
            {t('noResults')}
          </p>{/if}
      </div>{/if}
  </aside>
</div>
