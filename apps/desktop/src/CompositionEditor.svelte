<script lang="ts">
  import type { Binding, Device, Plan, PluginPackage, Locale, Snapshot } from './types';
  import { text } from './i18n';
  import { actionIcon } from './icons';
  import { reordered } from './action-drop';
  import ActionEditor from './ActionEditor.svelte';
  import PlanEditor from './PlanEditor.svelte';
  import ActionDrop from './ActionDrop.svelte';
  import Icon from './Icon.svelte';
  let {
    value,
    device,
    plugins,
    locale,
    depth,
    controller = 'Keypad',
    inSequence = false,
    onchange,
    openInspector,
    feedback,
  }: {
    value: Plan;
    device: Device;
    plugins: PluginPackage[];
    locale: Locale;
    depth: number;
    controller?: 'Keypad' | 'Encoder';
    inSequence?: boolean;
    onchange: (plan: Plan) => void;
    openInspector: (id: string) => void;
    feedback: Snapshot['feedback'];
  } = $props();
  const t = $derived((key: Parameters<typeof text>[1]) => text(locale, key));
  const ru = $derived(locale === 'ru');
  const sequence = $derived(value.type === 'sequence' ? value : null);
  const list = $derived(
    value.type === 'sequence'
      ? value.steps.map((s) => s.action)
      : value.type === 'switch'
        ? value.states
        : value.type === 'dial_stack' || value.type === 'action_wheel'
          ? value.actions
          : [],
  );
  const branches = $derived(
    value.type === 'key_logic'
      ? ['single', 'double', 'hold']
      : value.type === 'dial_bindings'
        ? ['left', 'right', 'press', 'tap']
        : [],
  );
  let selectedId = $state<string | null>(null);
  let branch = $state('');
  const selectedIndex = $derived(
    Math.max(
      0,
      list.findIndex((b) => b.id === selectedId),
    ),
  );
  const selected = $derived(list[selectedIndex]);
  const activeBranch = $derived(branches.includes(branch) ? branch : branches[0]);
  const branchValue = $derived(
    branches.length ? (value as unknown as Record<string, Binding | null>)[activeBranch] : null,
  );
  const scope = $derived({
    controller:
      value.type === 'dial_stack'
        ? ('Encoder' as const)
        : value.type === 'sequence'
          ? ('Keypad' as const)
          : controller,
    depth: depth + 1,
    inSequence: inSequence || value.type === 'sequence',
  });
  const identity = $derived(JSON.stringify(value));
  function updateList(next: Binding[]) {
    if (value.type === 'switch') onchange({ ...value, states: next });
    else if (value.type === 'dial_stack' || value.type === 'action_wheel')
      onchange({ ...value, actions: next });
  }
  function replace(next: Binding) {
    if (value.type === 'sequence')
      onchange({
        ...value,
        steps: value.steps.map((s) => (s.action.id === selected?.id ? { ...s, action: next } : s)),
      });
    else if (branches.length) onchange({ ...value, [activeBranch]: next } as Plan);
    else updateList(list.map((b, i) => (i === selectedIndex ? next : b)));
    selectedId = next.id;
  }
  function append(next: Binding) {
    if (value.type === 'sequence')
      onchange({ ...value, steps: [...value.steps, { delay_ms: 0, action: next }] });
    else updateList([...list, next]);
    selectedId = next.id;
  }
  function remove(index: number) {
    if (value.type === 'sequence')
      onchange({ ...value, steps: value.steps.filter((_, i) => i !== index) });
    else if (list.length > 1) updateList(list.filter((_, i) => i !== index));
    selectedId = list[Math.max(0, index - 1)]?.id ?? null;
  }
  function move(from: number, to: number) {
    if (value.type === 'sequence') onchange({ ...value, steps: reordered(value.steps, from, to) });
    else updateList(reordered(list, from, to));
  }
  let reorder: {
    pointer: number;
    from: number;
    to: number;
    before: string;
    node: HTMLElement;
  } | null = null;
  let reorderTarget = $state<number | null>(null);
  function reset() {
    reorder = null;
    reorderTarget = null;
  }
  function handle(index: number) {
    return {
      onpointerdown: (e: PointerEvent) => {
        if (e.button !== 0) return;
        const node = e.currentTarget as HTMLElement;
        node.setPointerCapture(e.pointerId);
        reorder = {
          pointer: e.pointerId,
          from: index,
          to: index,
          before: identity,
          node: node.closest('.composite-list')!,
        };
      },
      onpointermove: (e: PointerEvent) => {
        if (reorder?.pointer !== e.pointerId) return;
        const el = document
          .elementFromPoint(e.clientX, e.clientY)
          ?.closest<HTMLElement>('[data-step-index]');
        reorder.to =
          el && el.closest('.composite-list') === reorder.node ? Number(el.dataset.stepIndex) : -1;
        reorderTarget = reorder.to;
      },
      onpointerup: () => {
        if (reorder && reorder.before === identity && reorder.to >= 0)
          move(reorder.from, reorder.to);
        reset();
      },
      onpointercancel: reset,
      onlostpointercapture: reset,
      onkeydown: (e: KeyboardEvent) => {
        if (e.altKey && ['ArrowUp', 'ArrowDown'].includes(e.key)) {
          e.preventDefault();
          move(index, index + (e.key === 'ArrowUp' ? -1 : 1));
        }
      },
    };
  }
</script>

{#if branches.length}
  {#if value.type === 'key_logic'}{@const logic = value}
    <div class="gesture-thresholds">
      <label
        >{t('doubleWindow')}<input
          type="number"
          min="100"
          max="1000"
          value={logic.double_ms}
          onchange={(e) => onchange({ ...logic, double_ms: e.currentTarget.valueAsNumber })}
        /> ms</label
      >
      <label
        >{t('holdThreshold')}<input
          type="number"
          min="200"
          max="5000"
          value={logic.hold_ms}
          onchange={(e) => onchange({ ...logic, hold_ms: e.currentTarget.valueAsNumber })}
        /> ms</label
      >
    </div>{/if}
  <div class="composite-tabs" role="group" aria-label={t('actions')}>
    {#each branches as key}<button
        class:current={activeBranch === key}
        aria-pressed={activeBranch === key}
        onclick={() => (branch = key)}
      >
        {key === 'left'
          ? t('rotateLeft')
          : key === 'right'
            ? t('rotateRight')
            : key === 'press'
              ? t('dialPress')
              : key === 'tap'
                ? t('panelTap')
                : t(key as 'single' | 'double' | 'hold')}
      </button>{/each}
    {#if branchValue}<button
        class="quiet"
        title={t('remove')}
        aria-label={t('remove')}
        onclick={() => onchange({ ...value, [activeBranch]: null } as Plan)}
        ><Icon name="trash" /></button
      >{/if}
  </div>
  {#if branchValue}<ActionEditor
      value={branchValue}
      {device}
      {plugins}
      {locale}
      {openInspector}
      {feedback}
      controller={scope.controller}
      depth={depth + 1}
      {inSequence}
      onchange={replace}
    />
  {:else}<ActionDrop {scope} identity={identity + activeBranch} {ru} onchange={replace} />{/if}
{:else if sequence}
  <div class="composite-sequence">
    <div class="composite-steps">
      <div class="composite-list">
        {#each sequence.steps as step, i (step.action.id)}<div
            class="composite-row"
            class:active={selected?.id === step.action.id}
            data-step-index={i}
            data-reorder-active={reorderTarget === i}
          >
            <button
              class="quiet composite-reorder"
              title={ru ? 'Перетащить · Alt+↑/↓' : 'Drag · Alt+↑/↓'}
              aria-label={ru ? 'Переместить шаг' : 'Move step'}
              {...handle(i)}><Icon name="grip" /></button
            >
            <button class="action-summary" onclick={() => (selectedId = step.action.id)}
              ><Icon
                name={actionIcon(
                  step.action.plan?.type ?? step.action.plugin?.action ?? step.action.kind,
                )}
              /><span>{i + 1}. {step.action.label}</span></button
            >
            <label class="step-delay" title={ru ? 'Пауза перед действием' : 'Delay before action'}
              ><Icon name="clock" /><input
                type="number"
                aria-label={t('delay')}
                min="0"
                max="3600000"
                value={step.delay_ms}
                onchange={(e) =>
                  onchange({
                    ...sequence,
                    steps: sequence.steps.map((s, n) =>
                      n === i ? { ...s, delay_ms: e.currentTarget.valueAsNumber } : s,
                    ),
                  })}
              /> ms</label
            >
            <button
              class="quiet"
              aria-label={t('remove')}
              title={t('remove')}
              onclick={() => remove(i)}><Icon name="trash" /></button
            >
          </div>{/each}
      </div>
      {#if list.length < 64}<ActionDrop {scope} {identity} {ru} onchange={append} />{/if}
    </div>
    {#if selected}<div class="composite-properties">
        <ActionEditor
          value={selected}
          {device}
          {plugins}
          {locale}
          {openInspector}
          {feedback}
          controller={scope.controller}
          depth={depth + 1}
          inSequence={true}
          onchange={replace}
        />
      </div>{/if}
  </div>
{:else}
  <div class="composite-tabs" role="group" aria-label={t('state')}>
    {#each list as item, i (item.id)}<button
        class:current={selected?.id === item.id}
        aria-pressed={selected?.id === item.id}
        onclick={() => (selectedId = item.id)}
        >{t(value.type === 'switch' ? 'state' : 'action')} {i + 1}</button
      >{/each}
    {#if list.length > 1}<button
        class="quiet"
        title={t('remove')}
        aria-label={t('remove')}
        onclick={() => remove(selectedIndex)}><Icon name="trash" /></button
      >{/if}
  </div>
  {#if selected && value.type === 'switch' && selected.plan?.type === 'sequence'}
    <PlanEditor
      value={selected.plan}
      {device}
      {plugins}
      {locale}
      {openInspector}
      {feedback}
      depth={depth + 1}
      {inSequence}
      onchange={(plan) => replace({ ...selected, plan })}
    />
  {:else if selected}<ActionEditor
      value={selected}
      {device}
      {plugins}
      {locale}
      {openInspector}
      {feedback}
      controller={scope.controller}
      depth={depth + 1}
      {inSequence}
      onchange={replace}
    />{/if}
  {#if list.length < 32}<ActionDrop
      {scope}
      {identity}
      {ru}
      label={ru ? 'Добавить состояние / действие' : 'Add state / action'}
      onchange={append}
    />{/if}
{/if}
