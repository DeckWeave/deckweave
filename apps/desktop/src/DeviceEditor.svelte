<script lang="ts">
  import { untrack, setContext } from 'svelte';
  import {
    actionDropContext,
    findDrop,
    clearDrop,
    type LibraryAction,
    type ActionLibraryBridge,
  } from './action-drop';
  import { invoke } from '@tauri-apps/api/core';
  import type { Client } from './client.svelte';
  import { text } from './i18n';
  import type { Device, ActionKind, Binding, Control, PluginAction, Plan } from './types';
  import { plan as createPlan, action as createAction } from './action-factory';
  import KeyboardAssignmentSetup from './KeyboardAssignmentSetup.svelte';
  import {
    onboardAssignment,
    keyboardOverlay,
    keyboardOwner,
    keyboardPreviewCurrent,
    keyboardPreviewForContext,
    type KeyboardLayer,
    type OnboardAssignment,
  } from './keyboard-assignments';
  import type { ProviderSessionState } from './provider-ui';
  import KeyLightingAlertEditor from './KeyLightingAlertEditor.svelte';
  import PlanEditor from './PlanEditor.svelte';
  import PanelEditor from './PanelEditor.svelte';
  import ProfileEditor from './ProfileEditor.svelte';
  import TouchTest from './TouchTest.svelte';
  import PaneSplitter from './PaneSplitter.svelte';
  import LibraryOptions from './LibraryOptions.svelte';
  import { ordered } from './library';
  import { pageLabel } from './page-label';
  import DeviceSettingsWorkspace from './DeviceSettingsWorkspace.svelte';
  import ProviderSettings from './ProviderSettings.svelte';
  import DeviceDecorations from './DeviceDecorations.svelte';
  import DepthRing from './DepthRing.svelte';
  import Icon from './Icon.svelte';
  import { actionIcon } from './icons';
  let {
    client,
    device,
    workspace = $bindable<'actions' | 'lighting' | 'settings'>('actions'),
  }: { client: Client; device: Device; workspace?: 'actions' | 'lighting' | 'settings' } = $props();
  let contextMenu = $state<{ x: number; y: number } | null>(null);
  const ru = $derived(client.snapshot?.configuration.locale === 'ru');
  const lightingSupported = $derived(!!device.provider_descriptor?.definition.lighting?.length);
  const travel = $derived(device.provider_descriptor?.definition.observation?.keyTravel);
  let selectedId = $state<string | null>(null);
  let search = $state('');
  let editingLibrary = $state(false);
  let filter = $state<'keys' | 'dials' | 'plugins'>('keys');
  let testing = $state(false);
  let profileSettings = $state(false);
  let panelEditing = $state(false);
  let panelContext = $state('');
  $effect(() => {
    const context = device.id + ':' + device.profile.id + ':' + device.profile.active_page_id;
    if (panelContext !== context) {
      panelContext = context;
      panelEditing = false;
    }
  });
  let actionLayer = $state<'main' | 'fn'>('main');
  let keyboardSession = $state<ProviderSessionState | null>(null);
  let keyboardRoutes = $state<
    Record<string, { state: string; error?: string | null; source?: string }>
  >({});
  let keyboardStatusError = $state('');
  let pendingKeyboard = $state<{
    context: string;
    controlId: string;
    layer: KeyboardLayer;
    binding: Binding;
    native: OnboardAssignment;
    generation: number | null;
  } | null>(null);
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
  setContext<ActionLibraryBridge>(actionDropContext, {
    selected: () => selectedLibrary?.item ?? null,
    enabled: (item) =>
      item.type !== 'plugin' ||
      !snapshot.configuration.preferences.disabled_plugins.includes(item.package),
    make: (item, controller) => {
      if (item.type === 'folder') return null;
      if (item.type === 'builtin') return createAction(item.kind, t(item.kind));
      if (item.type === 'plan')
        return { ...createAction('program', t(item.plan)), plan: createPlan(item.plan, device) };
      return {
        ...createAction('plugin', item.label),
        plugin: { package: item.package, action: item.action, settings: {}, controller },
      };
    },
  });
  let nestedTarget: ReturnType<typeof findDrop>;
  let nestedIdentity = '';
  const dragContext = () =>
    [
      device.id,
      device.profile.id,
      device.profile.active_page_id,
      selectedId,
      actionLayer,
      workspace,
    ].join(':');
  let dragStartedIn = '';
  let dragging = $state<LibraryAction | null>(null);
  let selectedLibrary = $state<{ item: LibraryAction; name: string; id: string } | null>(null);
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
    if (physicalKeyboard && controlLabel(control).trim().toLowerCase() === 'fn') return false;
    if (
      physicalKeyboard &&
      (item.type === 'folder' ||
        (item.type === 'plan' &&
          ['key_logic', 'dial_stack', 'dial_bindings', 'action_wheel', 'page'].includes(item.plan)))
    )
      return false;
    if (
      item.type === 'plugin' &&
      snapshot.configuration.preferences.disabled_plugins.includes(item.package)
    )
      return false;
    const region = regionFor(control);
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
  function source(item: LibraryAction, name: string, id: string) {
    return {
      onpointerdown: (e: PointerEvent) => {
        if (e.button !== 0) return;
        suppressClick = false;
        dragStartedIn = dragContext();
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
        clearDrop(nestedTarget);
        nestedTarget = findDrop(e.clientX, e.clientY);
        nestedIdentity = nestedTarget?.identity() ?? '';
        if (nestedTarget) {
          const allowed = nestedTarget.accepts(dragSession.item);
          nestedTarget.node.dataset.dropActive = String(allowed);
          nestedTarget.node.dataset.dropRejected = String(!allowed);
          dropTarget = null;
          return;
        }
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
          if (dragStartedIn !== dragContext()) {
            endDrag();
            return;
          }
          const nested = findDrop(e.clientX, e.clientY);
          if (nested) {
            if (
              nested === nestedTarget &&
              nestedIdentity === nested.identity() &&
              nested.accepts(dragSession.item)
            )
              nested.drop(dragSession.item);
            endDrag();
            return;
          }
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
        // A library click selects a tool. Only a drop or the explicit Assign
        // command may change a device assignment, including keyboard activation.
        selectedLibrary = { item, name, id };
      },
    };
  }
  function endDrag() {
    clearDrop(nestedTarget);
    nestedTarget = undefined;
    nestedIdentity = '';
    dragSession = null;
    dragging = null;
    dropTarget = null;
  }
  function applyLibrary(item: LibraryAction, control: Control | undefined) {
    if (!control || !accepts(item, control)) return;
    select(control);
    panelEditing = false;
    if (item.type === 'builtin') assign(item.kind);
    else if (item.type === 'plan') {
      const next = createPlan(item.plan, device);
      if (next.type === 'page_cycle' && control.kind === 'dial') next.direction = 'dial';
      assignPlan(next, t(item.plan));
    } else if (item.type === 'folder')
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
  const physicalKeyboard = $derived(!!device.provider_descriptor?.definition.keyboard);
  const keyboardPollingContext = $derived(
    physicalKeyboard && workspace === 'actions' ? device.id : null,
  );
  const keyboardEditingContext = $derived(
    [
      device.id,
      device.profile.id,
      device.profile.active_page_id,
      actionLayer,
      selectedId,
      workspace,
    ].join('\0'),
  );
  // One read-only session projection serves every key and the inspector. It never
  // reads HID directly, writes a mapping, or changes the manual Read button state.
  $effect(() => {
    const id = keyboardPollingContext;
    if (!id) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    keyboardSession = null;
    keyboardRoutes = {};
    async function poll() {
      try {
        const next = await invoke<{
          session: ProviderSessionState | null;
          routes: typeof keyboardRoutes;
        }>('keyboard_assignment_status', { deviceId: id });
        if (!disposed) {
          keyboardSession = next.session;
          keyboardRoutes = next.routes;
          keyboardStatusError = '';
        }
      } catch (e) {
        if (!disposed) keyboardStatusError = String(e);
      }
      if (!disposed) timer = setTimeout(() => void poll(), 1000);
    }
    void poll();
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  });
  $effect(() => {
    const context = keyboardEditingContext;
    untrack(() => {
      pendingKeyboard = keyboardPreviewForContext(pendingKeyboard, context);
    });
  });
  function nativeFor(controlId: string) {
    return onboardAssignment(
      keyboardSession,
      device.provider_descriptor?.definition.keyboard?.onboardMappings?.settingId,
      controlId,
      actionLayer,
      ru ? 'ru' : 'en',
    );
  }
  function ownerFor(controlId: string) {
    return keyboardOwner(device.keyboard_aliases, controlId, actionLayer);
  }
  function overlayFor(controlId: string) {
    const owner = ownerFor(controlId);
    return keyboardOverlay(
      nativeFor(controlId),
      visibleBindings[controlId],
      owner && owner.phase !== 'released' ? owner.installed[2] : null,
    );
  }
  function setKeyboard(next: Binding | null) {
    if (!selectedId) return;
    const controlId = selectedId;
    const owner = ownerFor(controlId);
    if (
      next &&
      (!owner ||
        owner.phase === 'released' ||
        (owner.provenance === 'legacy' && owner.phase === 'legacy_unverified'))
    ) {
      previewKeyboard(next);
      return;
    }
    pendingKeyboard = null;
    void client.assignKeyboard(device.id, controlId, actionLayer, next);
  }
  function previewKeyboard(next: Binding) {
    if (!selectedId) return;
    pendingKeyboard = {
      context: keyboardEditingContext,
      controlId: selectedId,
      layer: actionLayer,
      binding: next,
      native: nativeFor(selectedId),
      generation: keyboardSession?.generation ?? null,
    };
  }
  function previewCurrent() {
    const preview = pendingKeyboard;
    return (
      !!keyboardPreviewForContext(preview, keyboardEditingContext) &&
      !!preview &&
      keyboardPreviewCurrent(
        preview.native,
        preview.generation,
        nativeFor(preview.controlId),
        keyboardSession?.generation,
      )
    );
  }
  async function confirmKeyboard() {
    const pending = pendingKeyboard;
    if (
      !pending ||
      client.busy ||
      !previewCurrent() ||
      !pending.native.record ||
      pending.generation === null
    )
      return;
    if (
      await client.assignKeyboard(
        device.id,
        pending.controlId,
        pending.layer,
        pending.binding,
        true,
        { record: pending.native.record, generation: pending.generation },
      )
    )
      pendingKeyboard = null;
  }
  function previewScale(value: number) {
    void client.send((revision) => ({
      type: 'set_preferences',
      preferences: {
        ...client.snapshot!.configuration.preferences,
        preview_scale: Math.max(75, Math.min(150, value)),
      },
      expected_revision: revision,
    }));
  }
  const connection = $derived(snapshot.connections[device.id]);
  const t = $derived((key: Parameters<typeof text>[1]) => text(snapshot.configuration.locale, key));
  const page = $derived(device.profile.pages.find((p) => p.id === device.profile.active_page_id)!);
  const strip = $derived(snapshot.key_images[device.id]?.['touch-strip']);
  function hasDisplay(control: Control) {
    return (
      !device.provider_descriptor ||
      !!device.provider_descriptor.definition.displays?.some((d) => d.id === control.id)
    );
  }
  function controlLabel(control: Control) {
    const label = device.provider_descriptor?.definition.controls?.find(
      (c) => c.id === control.id,
    )?.label;
    return typeof label === 'string'
      ? label
      : (label?.[snapshot.configuration.locale] ?? label?.en ?? String(control.number));
  }
  const stripVisible = $derived(
    strip?.page_id === page.id &&
      (!device.provider_descriptor ||
        device.provider_descriptor.definition.displays?.some((d) =>
          ['strip', 'touch-strip'].includes(d.id),
        )),
  );
  const selected = $derived(geometry.controls.find((c) => c.id === selectedId));
  const visibleBindings = $derived(
    actionLayer === 'fn'
      ? Object.fromEntries(
          Object.entries(device.profile.fn_bindings ?? {}).map(([id, entry]) => [
            id,
            entry.binding,
          ]),
        )
      : page.bindings,
  );
  const binding = $derived(selectedId ? visibleBindings[selectedId] : undefined);
  const selectedDepth = $derived(
    travel && selected?.kind === 'key' ? client.depth(selected.id, client.depthScaleUm) : null,
  );
  const folder = $derived(
    binding?.plan?.type === 'page'
      ? device.profile.pages.find(
          (p) =>
            p.id === (binding!.plan as Extract<Plan, { type: 'page' }>).page_id &&
            p.parent !== null,
        )
      : undefined,
  );
  function openFolder(id: string) {
    void client.send((revision) => ({
      type: 'select_page',
      device_id: device.id,
      page_id: id,
      expected_revision: revision,
    }));
  }
  function folderBehavior(id: string, enabled: boolean) {
    void client.send((revision) => ({
      type: 'folder_behavior',
      device_id: device.id,
      page_id: id,
      close_after_action: enabled,
      expected_revision: revision,
    }));
  }
  const selectedPackage = $derived(
    binding?.plugin ? client.plugins.find((p) => p.id === binding.plugin!.package) : undefined,
  );
  const pluginUnavailable = $derived(
    !!binding?.plugin &&
      (!selectedPackage ||
        !selectedPackage.actions.some((a) => a.id === binding!.plugin!.action) ||
        !!selectedPackage.error ||
        snapshot.configuration.preferences.disabled_plugins.includes(binding.plugin.package)),
  );
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
  type LibraryGroup = {
    id: string;
    name: string;
    actions: { id: string; name: string; item: LibraryAction }[];
  };
  const libraryPrefs = $derived(snapshot.configuration.preferences.library);
  const libraryGroups = $derived.by(() => {
    const groups: LibraryGroup[] = [
      {
        id: 'group:navigation',
        name: t('navigationGroup'),
        actions: [
          ...(['page', 'page_cycle', 'profile', 'back'] as const).map((type) => ({
            id: 'plan:' + type,
            name: t(type),
            item: { type: 'plan', plan: type } as LibraryAction,
          })),
          { id: 'plan:folder', name: t('folder'), item: { type: 'folder' } },
        ],
      },
      {
        id: 'group:scenarios',
        name: t('multiActionGroup'),
        actions: (
          [
            'launch',
            'hotkey',
            'sequence',
            'switch',
            'key_logic',
            'dial_bindings',
            'dial_stack',
            'action_wheel',
          ] as const
        ).map((type) => ({
          id: 'plan:' + type,
          name: t(type),
          item: { type: 'plan', plan: type },
        })),
      },
      ...(lightingSupported
        ? [
            {
              id: 'group:lighting',
              name: ru ? 'Освещение' : 'Lighting',
              actions: (['lighting_preset', 'lighting_return'] as const).map((type) => ({
                id: 'plan:' + type,
                name: t(type),
                item: { type: 'plan', plan: type } as LibraryAction,
              })),
            },
          ]
        : []),
      ...client.plugins
        .filter(
          (pack) =>
            !pack.error &&
            !pack.resource_only &&
            (!pack.development || libraryPrefs.show_development) &&
            !snapshot.configuration.preferences.disabled_plugins.includes(pack.id),
        )
        .map((pack) => ({
          id: 'plugin:' + pack.id,
          name: pack.name,
          actions: pack.actions
            .filter((action) => action.visible)
            .map((action) => ({
              id: 'action:' + action.id,
              name: action.name,
              item: {
                type: 'plugin',
                package: pack.id,
                action: action.id,
                label: action.name,
                controllers: action.controllers,
              } as LibraryAction,
            })),
        })),
      ...(libraryPrefs.show_development
        ? [
            {
              id: 'group:test',
              name: t('testActionsGroup'),
              actions: (['counter', 'toggle'] as const).map((kind) => ({
                id: 'builtin:' + kind,
                name: t(kind),
                item: { type: 'builtin', kind } as LibraryAction,
              })),
            },
          ]
        : []),
    ];
    return ordered(
      groups.filter((group) => group.actions.length),
      (g) => g.id,
      libraryPrefs.order,
    ).map((group) => ({
      ...group,
      actions: ordered(group.actions, (a) => a.id, libraryPrefs.order),
    }));
  });
  const visibleGroups = $derived(
    libraryGroups
      .filter((group) => !libraryPrefs.hidden.includes(group.id))
      .map((group) => ({
        ...group,
        actions: group.actions.filter((action) => {
          if (libraryPrefs.hidden.includes(action.id)) return false;
          const item = action.item;
          if (filter === 'plugins' && item.type !== 'plugin') return false;
          if (filter === 'dials') {
            if (item.type === 'folder' || item.type === 'builtin') return false;
            if (
              item.type === 'plan' &&
              !['page_cycle', 'dial_bindings', 'dial_stack', 'action_wheel'].includes(item.plan)
            )
              return false;
            if (
              item.type === 'plugin' &&
              !item.controllers.some((c) => ['Encoder', 'Panel'].includes(c))
            )
              return false;
          }
          if (filter === 'keys') {
            if (
              item.type === 'plan' &&
              ['dial_bindings', 'dial_stack', 'action_wheel'].includes(item.plan)
            )
              return false;
            if (item.type === 'plugin' && !item.controllers.includes('Keypad')) return false;
          }
          return (group.name + ' ' + action.name)
            .toLocaleLowerCase()
            .includes(search.toLocaleLowerCase());
        }),
      }))
      .filter((group) => group.actions.length),
  );
  $effect(() => {
    if (
      selectedLibrary &&
      !libraryGroups.some((group) =>
        group.actions.some((action) => action.id === selectedLibrary!.id),
      )
    )
      selectedLibrary = null;
  });

  function select(control: Control) {
    if (control.kind !== 'touch') panelEditing = false;
    selectedId = control.id;
    filter = control.kind === 'dial' ? 'dials' : 'keys';
  }
  function assign(kind: ActionKind | null, label?: string) {
    if (!selectedId) return;
    const controlId = selectedId;
    const pageId = page.id;
    const title = label ?? (kind ? t(kind) : '');
    if (physicalKeyboard) {
      setKeyboard(
        kind ? { ...createAction(kind, title), id: binding?.id ?? createAction().id } : null,
      );
      return;
    }
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
    if (physicalKeyboard) {
      setKeyboard({
        ...createAction('plugin', label),
        id: binding?.id ?? createAction().id,
        plugin,
      });
      return;
    }
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
    if (physicalKeyboard) {
      setKeyboard({
        ...createAction('program', label ?? binding?.label ?? t(plan.type)),
        id: binding?.id ?? createAction().id,
        plan,
      });
      return;
    }
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
    if (!selectedId || !binding || testing || pluginUnavailable) return;
    const controlId = selectedId;
    testing = true;
    try {
      if (actionLayer === 'fn') {
        await client.send(() => ({
          type: 'test_fn_binding',
          device_id: device.id,
          control_id: controlId,
        }));
        return;
      }
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
    if (e.key === 'Escape') contextMenu = null;
    if (
      e.key === 'Delete' &&
      workspace === 'actions' &&
      selectedId &&
      binding &&
      !testing &&
      e.target instanceof Element &&
      !!e.target.closest('[data-control-id], .inspector') &&
      !(
        e.target instanceof Element &&
        e.target.closest('input,textarea,select,[contenteditable="true"],[role="dialog"],dialog')
      )
    ) {
      e.preventDefault();
      assign(null);
      contextMenu = null;
    }
    if (e.key === 'Escape' && dragging) {
      e.preventDefault();
      suppressClick = true;
      endDrag();
    }
  }}
  onclick={() => (contextMenu = null)}
  onblur={() => {
    endDrag();
    contextMenu = null;
  }}
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
{#if workspace === 'settings'}<div class="device-mode-content">
    <DeviceSettingsWorkspace {client} {device} />
  </div>
{:else if workspace === 'lighting'}<div class="device-mode-content">
    <ProviderSettings {client} {device} lighting />
  </div>
{:else}
  <div
    class="editor-layout"
    style:--library-width={Math.max(220, Math.min(libraryMax, libraryWidth)) + 'px'}
    style:--inspector-height={Math.max(150, Math.min(inspectorMax, inspectorHeight)) + 'px'}
  >
    <section class="device-workspace">
      <div class="device-toolbar">
        <div>
          <select
            class="device-selector"
            aria-label={t('devices')}
            value={device.id}
            onchange={(e) => {
              const id = e.currentTarget.value;
              e.currentTarget.value = device.id;
              void client.openDevice(id);
            }}
            >{#each snapshot.configuration.devices as item}<option value={item.id}
                >{item.name}</option
              >{/each}</select
          >
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
              aria-label={ru ? 'Управление профилями' : 'Manage profiles'}
              title={ru
                ? 'Наборы страниц и действия каждого профиля'
                : 'Page sets and actions for each profile'}
              onclick={() => (profileSettings = !profileSettings)}
              ><Icon name="profile" />{ru ? 'Профили' : 'Profiles'}</button
            >
          </div>
        </div>
        <div class="device-preview-tools">
          {#if physicalKeyboard}<div
              class="preview-zoom"
              role="group"
              aria-label={ru ? 'Слой клавиатуры' : 'Keyboard layer'}
            >
              <button
                class:current={actionLayer === 'main'}
                aria-pressed={actionLayer === 'main'}
                title={ru
                  ? 'Аппаратный слой общий для всех профилей'
                  : 'Onboard layer is shared by every profile'}
                onclick={() => (actionLayer = 'main')}>{ru ? 'Основной' : 'Main'}</button
              >
              <button
                class:current={actionLayer === 'fn'}
                aria-pressed={actionLayer === 'fn'}
                title={ru
                  ? 'Аппаратный слой общий для всех профилей'
                  : 'Onboard layer is shared by every profile'}
                onclick={() => (actionLayer = 'fn')}>Fn</button
              >
            </div>{/if}
          {#if physicalKeyboard}<div
              class="preview-zoom"
              role="group"
              aria-label={ru ? 'Масштаб клавиатуры' : 'Keyboard zoom'}
            >
              <button
                disabled={client.busy || snapshot.configuration.preferences.preview_scale <= 75}
                aria-label={ru ? 'Уменьшить клавиатуру' : 'Zoom out keyboard'}
                onclick={() => previewScale(snapshot.configuration.preferences.preview_scale - 25)}
                >−</button
              >
              <button
                class="zoom-fit"
                title={ru ? 'По размеру доступной области' : 'Fit available space'}
                disabled={client.busy}
                onclick={() => previewScale(100)}
                >{snapshot.configuration.preferences.preview_scale === 100
                  ? ru
                    ? 'По размеру'
                    : 'Fit'
                  : snapshot.configuration.preferences.preview_scale + '%'}</button
              >
              <button
                disabled={client.busy || snapshot.configuration.preferences.preview_scale >= 150}
                aria-label={ru ? 'Увеличить клавиатуру' : 'Zoom in keyboard'}
                onclick={() => previewScale(snapshot.configuration.preferences.preview_scale + 25)}
                ><Icon name="plus" /></button
              >
            </div>{/if}
          {#if geometry.surface_pixels[0] > 0 && geometry.surface_pixels[1] > 0}<button
              class:current={panelEditing}
              aria-pressed={panelEditing}
              onclick={() => {
                if (!panelEditing && selected?.kind !== 'touch')
                  selectedId = page.panel?.regions[0]
                    ? 'panel-' + page.panel.regions[0].id
                    : (geometry.controls.find((c) => c.kind === 'touch')?.id ?? null);
                panelEditing = !panelEditing;
              }}><Icon name="device" />{t('panelLayout')}</button
            >{/if}
          <button
            class="quiet device-settings-link"
            aria-label={t('deviceSettings')}
            title={t('deviceSettings')}
            onclick={() => (workspace = 'settings')}><Icon name="sliders" /></button
          >
        </div>
      </div>
      {#if profileSettings}<ProfileEditor {client} {device} />{/if}
      <div class="panel-stage" class:keyboard-stage={physicalKeyboard}>
        <div
          class="device-panel"
          class:keyboard-panel={physicalKeyboard}
          style:aspect-ratio={`${geometry.width}/${geometry.height}`}
          style:--device-ratio={geometry.width / geometry.height}
          style:--preview-scale={snapshot.configuration.preferences.preview_scale / 100}
        >
          <DeviceDecorations
            definition={device.provider_descriptor?.definition}
            {geometry}
            locale={snapshot.configuration.locale}
          />
          {#if geometry.controls.some((c) => c.kind === 'touch')}<div
              class="touch-surface"
              style:left={`${(geometry.surface[0] / geometry.width) * 100}%`}
              style:top={`${(geometry.surface[1] / geometry.height) * 100}%`}
              style:width={`${(geometry.surface[2] / geometry.width) * 100}%`}
              style:height={`${(geometry.surface[3] / geometry.height) * 100}%`}
            >
              {#if stripVisible}<img class="hardware-key-image" src={strip.data_url} alt="" />{/if}
            </div>{/if}
          {#each geometry.controls as control (control.id)}
            {@const depth =
              travel && control.kind === 'key'
                ? client.depth(control.id, client.depthScaleUm)
                : null}
            {@const assignment = visibleBindings[control.id]}
            {@const keyboardView = physicalKeyboard ? overlayFor(control.id) : null}
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
              class:physical-key={physicalKeyboard && control.kind === 'key'}
              class:dial-control={control.kind === 'dial'}
              class:touch-control={control.kind === 'touch'}
              class:panel-layout-control={panelEditing && control.kind === 'touch'}
              class:strip-overlay={control.kind === 'touch' && stripVisible}
              class:selected={selectedId === control.id}
              class:pressed
              class:depth-pressed={!!depth && depth.um >= 300}
              class:active={feedback?.active}
              class:drop-target={dropTarget === control.id}
              style:left={`${(control.rect[0] / geometry.width) * 100}%`}
              style:top={`${(control.rect[1] / geometry.height) * 100}%`}
              style:width={`${(control.rect[2] / geometry.width) * 100}%`}
              style:height={`${(control.rect[3] / geometry.height) * 100}%`}
              title={`${controlLabel(control)} · ${keyboardView?.label || assignment?.label || t('unassigned')}${depth ? ' · ' + (depth.um / 1000).toFixed(2) + (ru ? ' мм' : ' mm') : ''}`}
              aria-label={`${controlLabel(control)}: ${keyboardView?.label || assignment?.label || t('unassigned')}`}
              aria-pressed={selectedId === control.id}
              oncontextmenu={(e) => {
                e.preventDefault();
                select(control);
                contextMenu = {
                  x: Math.min(e.clientX, viewportWidth - 220),
                  y: Math.min(e.clientY, viewportHeight - 110),
                };
              }}
              onclick={() => select(control)}
              ondblclick={() => {
                const plan = assignment?.plan;
                if (
                  plan?.type === 'page' &&
                  device.profile.pages.some((p) => p.id === plan.page_id && p.parent !== null)
                )
                  openFolder(plan.page_id);
              }}
            >
              {#if panelEditing && control.kind === 'touch'}<span class="panel-layout-overlay"
                  >{page.panel?.regions.find((r) => 'panel-' + r.id === control.id)?.name ??
                    'T' + control.number}</span
                >{/if}
              <DepthRing reading={depth} />
              {#if control.kind === 'key' && !hasDisplay(control)}
                <span class="physical-key-label">{controlLabel(control)}</span><span
                  class="physical-key-binding"
                  class:host-assignment={!!assignment}
                  >{keyboardView?.label ?? assignment?.label ?? ''}</span
                >
                {#if physicalKeyboard && assignment}<span
                    class="physical-key-assigned"
                    aria-hidden="true">●</span
                  >{/if}
              {:else if control.kind === 'dial'}
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
      {#if actionLayer !== 'fn'}<div class="page-row" aria-label={t('currentPage')}>
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
              }))}><Icon name="plus" /></button
          >
        </div>{/if}
      {#if page.parent}<div class="folder-toolbar">
          <span>{t('editingFolder')}: {page.name}</span><label
            ><input
              type="checkbox"
              checked={page.close_after_action}
              onchange={(e) => folderBehavior(page.id, e.currentTarget.checked)}
            />{t('closeFolderAfterAction')}</label
          >
        </div>{/if}
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
        {#if panelEditing}
          <div class="inspector-heading">
            <h2>{t('panelLayout')}</h2>
            <button
              class="quiet"
              aria-label={ru ? 'Закрыть' : 'Close'}
              title={ru ? 'Закрыть' : 'Close'}
              onclick={() => (panelEditing = false)}><Icon name="close" /></button
            >
          </div>
          <PanelEditor
            context={device.id + ':' + device.profile.id + ':' + page.id}
            value={page.panel}
            controls={geometry.controls}
            locale={snapshot.configuration.locale}
            pixels={geometry.surface_pixels}
            bindings={visibleBindings}
            {selectedId}
            onselect={(id) => (selectedId = id)}
            onaction={(id) => {
              selectedId = id;
              panelEditing = false;
            }}
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
        {:else if selected}
          <div class="inspector-heading">
            <div class="selected-control-heading">
              <h2>
                {binding?.label ||
                  (binding
                    ? t(binding.plan?.type ?? binding.kind)
                    : physicalKeyboard
                      ? nativeFor(selected.id).label
                      : t('unassigned'))}
              </h2>
              {#if travel && selected.kind === 'key'}<div class="selected-key-depth">
                  <span>{controlLabel(selected)}</span><DepthRing
                    reading={selectedDepth}
                    inline
                  /><strong
                    >{selectedDepth ? (selectedDepth.um / 1000).toFixed(2) : '—'}
                    {ru ? 'мм' : 'mm'}</strong
                  >
                </div>{/if}
            </div>
            {#if binding?.plugin && hasInspector && !pluginUnavailable}<button
                onclick={() => void client.openInspector(binding!.id)}
                ><Icon name="sliders" />{t('pluginSettings')}</button
              >{/if}
            <button
              class="primary test-button"
              disabled={!binding ||
                testing ||
                pluginUnavailable ||
                (binding.kind === 'plugin' && result?.running)}
              title={physicalKeyboard
                ? ru
                  ? 'Программная проверка действия; физический ввод не проверяется'
                  : 'Run action in software; physical input is not tested'
                : t('check')}
              onclick={() => void check()}
              ><Icon name="play" />{testing ? t('checking') : t('check')}</button
            >
            {#if binding}<button
                class="quiet icon-button danger-text"
                title={t('remove') + ' · Delete'}
                aria-label={t('remove')}
                onclick={() => assign(null)}><Icon name="trash" /></button
              >{/if}
          </div>
          {#if physicalKeyboard}<KeyboardAssignmentSetup
              native={nativeFor(selected.id)}
              owner={ownerFor(selected.id)}
              {binding}
              route={keyboardRoutes[actionLayer + ':' + selected.id] ?? null}
              connected={keyboardSession?.state === 'connected'}
              canReconcile={!!keyboardSession &&
                ['connected', 'error', 'recovery_pending'].includes(keyboardSession.state)}
              busy={client.busy}
              {ru}
              profileName={device.profile.name}
              candidate={pendingKeyboard?.controlId === selected.id &&
              pendingKeyboard.layer === actionLayer
                ? pendingKeyboard.binding
                : null}
              previewNative={pendingKeyboard?.native ?? null}
              previewCurrent={previewCurrent()}
              affectedProfiles={[device.profile, ...device.profiles]
                .filter((p) =>
                  actionLayer === 'fn'
                    ? !!p.fn_bindings?.[selected!.id]
                    : p.pages.some((p) => !!p.bindings[selected!.id]),
                )
                .map((p) => p.name)}
              onconfirm={() => void confirmKeyboard()}
              oncancel={() => (pendingKeyboard = null)}
              onsetup={() => {
                if (binding) previewKeyboard(binding);
              }}
              onrestore={() => void client.restoreKeyboard(device.id, selected!.id, actionLayer)}
              onreconcile={() =>
                void client.reconcileKeyboard(device.id, selected!.id, actionLayer)}
            />
            {#if keyboardStatusError}<p class="connection-error" role="alert">
                {keyboardStatusError}
              </p>{/if}
          {/if}
          {#if binding}
            <div class="inspector-fields">
              {#if result?.error}<p class="connection-error">{result.error}</p>{/if}
              {#if !physicalKeyboard && actionLayer !== 'fn' && (selected.kind === 'key' || (selected.kind === 'dial' && binding.plugin?.controller !== 'Panel'))}<label
                  class="pin-field"
                  title={t('pinned')}
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
                {#if binding.plugin.package === 'dev.deckweave.timers' && binding.plugin.action.endsWith('.countdown')}<KeyLightingAlertEditor
                    {client}
                    {device}
                    {binding}
                  />{/if}
                {#if pluginUnavailable}<div class="unavailable-action" role="status">
                    <strong
                      >{ru ? 'Плагин действия недоступен' : 'Action plugin unavailable'}</strong
                    >
                    <p>
                      {selectedPackage?.error ??
                        (ru
                          ? 'Установите или подключите плагин в настройках расширений. Назначение и его параметры сохранены.'
                          : 'Install or reconnect the plugin in Extensions. The assignment and its settings are retained.')}
                    </p>
                    <button onclick={() => void client.openSettings(null, 'plugins')}
                      >{ru ? 'Управление расширениями' : 'Manage extensions'} →</button
                    >
                  </div>{/if}
                {@const controllers =
                  selectedPackage?.actions.find((a) => a.id === binding!.plugin!.action)
                    ?.controllers ?? []}
                {#if selected.kind === 'touch' && controllers.length > 1}
                  <div class="direction-choice">
                    <span>{t('controller')}</span>
                    <div class="segmented" role="group" aria-label={t('controller')}>
                      {#each controllers as controller}<button
                          class:current={(binding.plugin.controller ?? 'Keypad') === controller}
                          aria-pressed={(binding.plugin.controller ?? 'Keypad') === controller}
                          onclick={() =>
                            assignPlugin(
                              {
                                ...binding!.plugin!,
                                controller: controller as 'Keypad' | 'Encoder' | 'Panel',
                              },
                              binding!.label,
                            )}
                          >{controller === 'Panel'
                            ? ru
                              ? 'Своя панель'
                              : 'Custom panel'
                            : controller === 'Encoder'
                              ? ru
                                ? 'Крутилка'
                                : 'Dial'
                              : ru
                                ? 'Кнопка'
                                : 'Key'}</button
                        >{/each}
                    </div>
                  </div>
                {/if}
                <details class="plugin-options">
                  <summary>{t('advancedSettings')}</summary>
                  <button
                    class="quiet"
                    disabled={pluginUnavailable}
                    onclick={() => void client.restartPlugin(binding!.plugin!.package)}
                    >{t('restartPlugin')}</button
                  >
                  {#if !hasInspector && !pluginUnavailable}<label class="form-row"
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
              {#if selected.kind === 'touch'}{#key selected.id}<details
                    class="touch-test-disclosure"
                  >
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
              {#if folder}<div class="folder-controls">
                  <button onclick={() => openFolder(folder!.id)}
                    ><Icon name="folder" />{t('openFolder')}</button
                  ><label
                    ><input
                      type="checkbox"
                      checked={folder.close_after_action}
                      onchange={(e) => folderBehavior(folder!.id, e.currentTarget.checked)}
                    />{t('closeFolderAfterAction')}</label
                  >
                </div>{/if}
              {#if binding.plan && !folder}<PlanEditor
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
                  ><Icon name="stop" />{t('cancelActions')}</button
                >
              {/if}
              {#if result && (result.running || result.error || result.display || result.title || ['counter', 'toggle'].includes(binding.kind) || ['switch', 'dial_stack', 'action_wheel'].includes(binding.plan?.type ?? ''))}<div
                  class="form-row"
                >
                  <span>{t('actionResult')}</span>
                  <div class="result-value" aria-live="polite">
                    {result
                      ? result.running
                        ? t('running')
                        : binding.plan?.type === 'switch'
                          ? `${t('nextAction')}: ${result.title ?? ''}`
                          : binding.plan?.type === 'dial_stack' ||
                              binding.plan?.type === 'action_wheel'
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
                </div>{/if}
              {#if selected.kind === 'dial'}<div class="dial-tests">
                  <button
                    onclick={() =>
                      void client.input(device.id, selectedId!, { type: 'rotate', ticks: -1 })}
                    ><Icon name="rotate_left" />{t('rotateLeft')}</button
                  ><button
                    onclick={() =>
                      void client.input(device.id, selectedId!, { type: 'rotate', ticks: 1 })}
                    ><Icon name="rotate_right" />{t('rotateRight')}</button
                  >
                </div>{/if}
            </div>
          {:else}<p class="inspector-hint">{t('assignHint')}</p>{/if}
        {:else}<div class="inspector-empty">
            <Icon name="device" size={24} />
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
        <div class="library-tabs">
          {#each ['keys', 'dials', 'plugins'] as tab}<button
              class:current={filter === tab}
              onclick={() => (filter = tab as typeof filter)}
              >{tab === 'keys' ? 'Keys' : tab === 'dials' ? 'Dials' : 'Plugins'}</button
            >{/each}
        </div>
        <div class="library-search-row">
          <input
            class="search-field"
            aria-label={t('search')}
            placeholder={t('search')}
            bind:value={search}
          />
          <button
            class="library-display-button"
            class:current={editingLibrary}
            aria-label={t('libraryDisplay')}
            title={t('libraryDisplay')}
            aria-pressed={editingLibrary}
            onclick={() => (editingLibrary = !editingLibrary)}
          >
            <Icon name={editingLibrary ? 'check' : 'sliders'} />
          </button>
        </div>
      </div>
      {#if editingLibrary}
        <LibraryOptions {client} groups={libraryGroups} {search} />
      {:else}
        <div class="library-scroll">
          {#each visibleGroups as group (group.id)}
            <details class="library-group" open>
              <summary>{group.name}</summary>
              <div class="action-list">
                {#each group.actions as action (action.id)}
                  <button
                    class="action-item"
                    class:library-selected={selectedLibrary?.id === action.id}
                    aria-pressed={selectedLibrary?.id === action.id}
                    title={ru
                      ? 'Перетащите на элемент устройства. Клик только выбирает действие.'
                      : 'Drag onto a device control. Clicking only selects the action.'}
                    {...source(action.item, action.name, action.id)}
                  >
                    <span class="action-icon" aria-hidden="true"
                      ><Icon name={actionIcon(action.id, action.name)} size={18} /></span
                    >
                    <strong>{action.name}</strong>
                  </button>
                {/each}
              </div>
            </details>
          {/each}
          {#if !visibleGroups.length}<p class="library-empty">{t('noResults')}</p>{/if}
        </div>
        <div class="library-assignment-bar">
          {#if selectedLibrary}<span
              ><Icon name={actionIcon(selectedLibrary.id)} /><strong>{selectedLibrary.name}</strong
              ></span
            ><button
              class="primary"
              disabled={!selected || client.busy || !accepts(selectedLibrary.item, selected)}
              onclick={() => applyLibrary(selectedLibrary!.item, selected)}
              >{binding ? (ru ? 'Заменить' : 'Replace') : ru ? 'Назначить' : 'Assign'}{selected
                ? ' · ' + controlLabel(selected)
                : ''}</button
            >
          {:else}<span class="subtle"
              >{ru ? 'Перетащите действие на устройство' : 'Drag an action onto the device'}</span
            >{/if}
        </div>
      {/if}
    </aside>
  </div>
{/if}
{#if contextMenu}<div
    class="control-context-menu"
    role="menu"
    style:left={contextMenu.x + 'px'}
    style:top={contextMenu.y + 'px'}
  >
    <button
      role="menuitem"
      disabled={!binding || pluginUnavailable || testing}
      onclick={() => {
        void check();
        contextMenu = null;
      }}><Icon name="play" />{t('check')}</button
    ><button
      role="menuitem"
      disabled={!binding}
      onclick={() => {
        assign(null);
        contextMenu = null;
      }}><Icon name="trash" />{t('remove')} · Del</button
    >
  </div>{/if}

<style>
  .control.depth-pressed {
    border-color: #77e2ff;
    box-shadow: inset 0 0 9px #77e2ff40;
  }
  .physical-key-label {
    font-size: 12px;
    line-height: 1.1;
  }
  .physical-key-assigned {
    display: none;
  }
  .device-preview-tools {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-shrink: 0;
  }
  .preview-zoom {
    display: flex;
    gap: 1px;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
  }
  .preview-zoom button {
    border: 0;
    border-radius: 0;
    padding: 4px 7px;
    font-size: 12px;
    background: var(--panel);
  }
  .preview-zoom .zoom-fit {
    min-width: 76px;
  }
  .keyboard-scope {
    align-self: center;
    padding: 0 7px;
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }
  .selected-control-heading {
    min-width: 0;
    display: flex;
    gap: 12px;
    align-items: center;
    flex-wrap: wrap;
  }
  .selected-key-depth {
    display: flex;
    align-items: center;
    gap: 8px;
    white-space: nowrap;
    color: var(--depth);
  }
  .selected-key-depth > span {
    font-size: 12px;
    font-weight: 600;
  }
  .selected-key-depth strong {
    font-size: 16px;
    font-variant-numeric: tabular-nums;
  }
  .physical-key-binding {
    font-size: 10px;
    line-height: 1.2;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .physical-key-binding.host-assignment {
    color: var(--depth);
  }
  .device-mode-content {
    height: calc(100vh - 44px);
    overflow: auto;
  }
  .unavailable-action {
    padding: 12px;
    border: 1px solid #be884d;
    border-radius: 8px;
    grid-column: 1/-1;
  }
  .unavailable-action p {
    font-size: 12px;
    line-height: 1.5;
  }
  .control-context-menu {
    position: fixed;
    z-index: 1000;
    padding: 6px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 8px;
    box-shadow: 0 8px 30px #0008;
    display: grid;
    min-width: 200px;
  }
  .control-context-menu button {
    text-align: left;
  }
</style>
