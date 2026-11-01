<script lang="ts">
  import type { Client } from './client.svelte';
  import { text } from './i18n';
  import type { Device, ActionKind, Control, PluginAction, Plan } from './types';
  import { plan as createPlan } from './action-factory';
  import PlanEditor from './PlanEditor.svelte';
  import PanelEditor from './PanelEditor.svelte';
  import TouchTest from './TouchTest.svelte';
  import PaneSplitter from './PaneSplitter.svelte';
  import { pageLabel } from './page-label';
  let { client, device }: { client: Client; device: Device } = $props();
  let selectedId = $state<string | null>(null);
  let search = $state('');
  let filter = $state<'keys' | 'dials' | 'plugins'>('keys');
  let testing = $state(false);
  let profileSettings = $state(false);
  let newProfile = $state('');
  function preference(key: string, fallback: number) {
    try {
      const v = Number(localStorage.getItem(key));
      return Number.isFinite(v) && v > 0 ? v : fallback;
    } catch {
      return fallback;
    }
  }
  function saveSize(key: string, value: number) {
    try {
      localStorage.setItem(key, String(value));
    } catch {
      /* Layout remains usable without storage. */
    }
  }
  let viewportWidth = $state(1000);
  let viewportHeight = $state(800);
  let inspectorHeight = $state(preference('deckweave.inspectorHeight', 230));
  let libraryWidth = $state(preference('deckweave.libraryWidth', 280));
  const inspectorMax = $derived(Math.max(180, viewportHeight - 360));
  const libraryMax = $derived(Math.max(220, Math.min(600, viewportWidth - 420)));
  type LibraryAction =
    | { type: 'builtin'; kind: 'counter' | 'toggle' }
    | { type: 'plan'; plan: Plan['type'] }
    | { type: 'folder' }
    | { type: 'plugin'; package: string; action: string; label: string; controllers: string[] };
  let dragging = $state<LibraryAction | null>(null);
  let dropTarget = $state<string | null>(null);
  let dragSession = $state<{ id: number; x: number; y: number; item: LibraryAction } | null>(null);
  let dragX = $state(0);
  let dragY = $state(0);
  let suppressClick = false;
  function regionFor(control: Control) {
    return page.panel?.regions.find((r) =>
      control.kind === 'dial' ? r.dials.includes(control.id) : 'panel-' + r.id === control.id,
    );
  }
  function accepts(item: LibraryAction, control: Control): boolean {
    const region = regionFor(control);
    if (
      region &&
      region.dials.length > 1 &&
      (item.type !== 'plugin' || !item.controllers.includes('Panel'))
    )
      return false;
    if (item.type === 'folder') return control.kind === 'key';
    if (item.type === 'plan' && ['dial_stack', 'action_wheel', 'dial_bindings'].includes(item.plan))
      return control.kind !== 'key';
    return (
      item.type !== 'plugin' ||
      item.controllers.some((c) =>
        control.kind === 'touch'
          ? ['Panel', 'Encoder', 'Keypad'].includes(c)
          : c === (control.kind === 'dial' ? 'Encoder' : 'Keypad') ||
            (control.kind === 'dial' && !!region && c === 'Panel'),
      )
    );
  }
  function source(item: LibraryAction) {
    return {
      onpointerdown: (e: PointerEvent) => {
        if (e.button !== 0) return;
        suppressClick = false;
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
        dragSession = { id: e.pointerId, x: e.clientX, y: e.clientY, item };
      },
      onpointermove: (e: PointerEvent) => {
        if (dragSession?.id !== e.pointerId) return;
        if (!dragging && Math.hypot(e.clientX - dragSession.x, e.clientY - dragSession.y) < 6)
          return;
        e.preventDefault();
        dragging = dragSession.item;
        dragX = e.clientX;
        dragY = e.clientY;
        const id = document
          .elementFromPoint(e.clientX, e.clientY)
          ?.closest<HTMLElement>('[data-control-id]')?.dataset.controlId;
        const control = geometry.controls.find((c) => c.id === id);
        dropTarget = control && accepts(dragSession.item, control) ? control.id : null;
      },
      onpointerup: (e: PointerEvent) => {
        if (dragSession?.id !== e.pointerId) return;
        if (dragging) {
          e.preventDefault();
          suppressClick = true;
          const target = document
            .elementFromPoint(e.clientX, e.clientY)
            ?.closest<HTMLElement>('[data-control-id]')?.dataset.controlId;
          applyLibrary(
            dragSession.item,
            geometry.controls.find((c) => c.id === target),
          );
        }
        endDrag();
      },
      onpointercancel: endDrag,
      onlostpointercapture: endDrag,
      onclick: (e: MouseEvent) => {
        // A captured pointer produces a click on the source after releasing the drag.
        // Keep suppression until that click, rather than relying on timer ordering.
        if (suppressClick && e.detail > 0) {
          suppressClick = false;
          return;
        }
        suppressClick = false;
        applyLibrary(item, selected);
      },
    };
  }
  function endDrag() {
    dragSession = null;
    dragging = null;
    dropTarget = null;
  }
  function applyLibrary(item: LibraryAction, control: Control | undefined) {
    if (!control || !accepts(item, control)) return;
    select(control);
    if (item.type === 'builtin') assign(item.kind);
    else if (item.type === 'plan') assignPlan(createPlan(item.plan, device), t(item.plan));
    else if (item.type === 'folder')
      void client.send((revision) => ({
        type: 'add_folder',
        device_id: device.id,
        control_id: control.id,
        name: t('folder'),
        expected_revision: revision,
      }));
    else
      assignPlugin(
        {
          package: item.package,
          action: item.action,
          settings: {},
          controller: pluginController(item, control),
        },
        item.label,
      );
  }
  function pluginController(
    item: Extract<LibraryAction, { type: 'plugin' }>,
    control: Control,
  ): NonNullable<PluginAction['controller']> {
    if (control.kind === 'key') return 'Keypad';
    const region = regionFor(control);
    if (
      item.controllers.includes('Panel') &&
      (control.kind === 'touch' ||
        (region && (region.dials.length > 1 || !item.controllers.includes('Encoder'))))
    )
      return 'Panel';
    return control.kind === 'dial' || item.controllers.includes('Encoder') ? 'Encoder' : 'Keypad';
  }
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

<svelte:window
  bind:innerWidth={viewportWidth}
  bind:innerHeight={viewportHeight}
  onkeydown={(e) => {
    if (e.key === 'Escape' && dragging) {
      e.preventDefault();
      suppressClick = true;
      endDrag();
    }
  }}
  onblur={endDrag}
/>
{#if dragging}
  <div class="action-drag-preview" style:left={dragX + 14 + 'px'} style:top={dragY + 14 + 'px'}>
    ＋ {dragging.type === 'plugin'
      ? dragging.label
      : t(
          dragging.type === 'builtin'
            ? dragging.kind
            : dragging.type === 'plan'
              ? dragging.plan
              : 'folder',
        )}
  </div>
{/if}
<div
  class="editor-layout"
  style:--library-width={Math.max(220, Math.min(libraryMax, libraryWidth)) + 'px'}
  style:--inspector-height={Math.max(150, Math.min(inspectorMax, inspectorHeight)) + 'px'}
>
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
            data-control-id={control.id}
            class:key-control={control.kind === 'key'}
            class:dial-control={control.kind === 'dial'}
            class:touch-control={control.kind === 'touch'}
            class:strip-overlay={control.kind === 'touch' && stripVisible}
            class:selected={selectedId === control.id}
            class:pressed
            class:active={feedback?.active}
            class:drop-target={dropTarget === control.id}
            style:left={`${(control.rect[0] / geometry.width) * 100}%`}
            style:top={`${(control.rect[1] / geometry.height) * 100}%`}
            style:width={`${(control.rect[2] / geometry.width) * 100}%`}
            style:height={`${(control.rect[3] / geometry.height) * 100}%`}
            title={`${t(control.kind)} ${control.number} · ${assignment?.label || t('unassigned')}`}
            aria-label={`${t(control.kind)} ${control.number}: ${assignment?.label || t('unassigned')}`}
            aria-pressed={selectedId === control.id}
            onclick={() => select(control)}
          >
            {#if control.kind === 'dial'}
              <span class="dial-marker" aria-hidden="true"></span>
              <span class="dial-number">{control.number}</span>
              <span class="sr-only">{assignment?.label || t('unassigned')}</span>
            {:else if control.kind === 'touch' && stripVisible}
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
          title={`${t('page')} ${pageLabel(device.profile, p)}`}
          aria-label={`${t('page')} ${pageLabel(device.profile, p)}`}
          aria-current={p.id === page.id ? 'page' : undefined}
          onclick={() =>
            void client.send((revision) => ({
              type: 'select_page',
              device_id: device.id,
              page_id: p.id,
              expected_revision: revision,
            }))}>{pageLabel(device.profile, p)}</button
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
    <PaneSplitter
      orientation="horizontal"
      label={t('resizeInspector')}
      value={Math.max(150, Math.min(inspectorMax, inspectorHeight))}
      min={150}
      max={inspectorMax}
      onchange={(v) => {
        inspectorHeight = v;
        saveSize('deckweave.inspectorHeight', v);
      }}
    />
    <div class="inspector">
      {#if selected}
        <div class="inspector-heading">
          <div>
            <h2>
              {binding?.label ||
                (binding ? t(binding.plan?.type ?? binding.kind) : t('unassigned'))}
            </h2>
          </div>
          <button
            class="primary test-button"
            disabled={!binding || testing || (binding.kind === 'plugin' && result?.running)}
            onclick={() => void check()}>{testing ? t('checking') : `▷ ${t('check')}`}</button
          >
        </div>
        {#if binding}
          <div class="inspector-fields">
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
              {#if hasInspector}<div class="dial-tests">
                  <button onclick={() => void client.openInspector(binding!.id)}
                    >{t('pluginSettings')}</button
                  >
                </div>{/if}
              <details class="plugin-options">
                <summary>{t('advancedSettings')}</summary>
                <button
                  class="quiet"
                  onclick={() => void client.restartPlugin(binding!.plugin!.package)}
                  >{t('restartPlugin')}</button
                >
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
              </details>
            {/if}
            {#if selected.kind === 'touch'}{#key selected.id}<details class="touch-test-disclosure">
                  <summary>{t('touchTest')}</summary><TouchTest
                    {client}
                    deviceId={device.id}
                    control={selected}
                    {geometry}
                    locale={snapshot.configuration.locale}
                  />
                </details>{/key}{/if}
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
  <PaneSplitter
    orientation="vertical"
    label={t('resizeLibrary')}
    value={Math.max(220, Math.min(libraryMax, libraryWidth))}
    min={220}
    max={libraryMax}
    onchange={(v) => {
      libraryWidth = v;
      saveSize('deckweave.libraryWidth', v);
    }}
  />
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
                {...source({
                  type: 'plugin',
                  package: pack.id,
                  action: action.id,
                  label: action.name,
                  controllers: action.controllers,
                })}
                aria-disabled={!selected ||
                  !accepts(
                    {
                      type: 'plugin',
                      package: pack.id,
                      action: action.id,
                      label: action.name,
                      controllers: action.controllers,
                    },
                    selected,
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
            {...source({ type: 'plan', plan: type })}
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
        {#if folderVisible}<button class="action-item" {...source({ type: 'folder' })}
            ><span class="action-icon">▱</span><strong>{t('folder')}</strong></button
          >{/if}
        {#each items as kind}<button
            class="action-item"
            {...source({ type: 'builtin', kind })}
            title={selectedId
              ? t(kind === 'counter' ? 'counterHint' : 'toggleHint')
              : t('chooseFirst')}
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
