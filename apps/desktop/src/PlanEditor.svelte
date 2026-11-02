<script lang="ts">
  import type { Plan, Device, PluginPackage, Locale, Snapshot } from './types';
  import { action } from './action-factory';
  import ActionEditor from './ActionEditor.svelte';
  import { text } from './i18n';
  import { pageLabel } from './page-label';
  let {
    value,
    device,
    plugins,
    locale,
    onchange,
    depth = 0,
    openInspector,
    feedback = [],
  }: {
    value: Plan;
    device: Device;
    plugins: PluginPackage[];
    locale: Locale;
    onchange: (value: Plan) => void;
    depth?: number;
    openInspector: (context: string) => void;
    feedback?: Snapshot['feedback'];
  } = $props();
  const t = $derived((key: Parameters<typeof text>[1]) => text(locale, key));
  function shift<T>(list: T[], i: number, delta: number) {
    const next = [...list];
    [next[i], next[i + delta]] = [next[i + delta], next[i]];
    return next;
  }
</script>

<div class="plan-fields">
  {#if value.type === 'lighting_preset'}
    {@const lighting = value}
    <label
      >{t('lighting_preset')}<select
        value={lighting.preset_id}
        onchange={(e) => onchange({ ...lighting, preset_id: e.currentTarget.value })}
      >
        {#if !device.lighting_presets.some((p) => p.id === lighting.preset_id)}<option
            value={lighting.preset_id}
            disabled>{locale === 'ru' ? 'Пресет отсутствует' : 'Preset missing'}</option
          >{/if}
        {#each device.lighting_presets as preset}<option value={preset.id}>{preset.name}</option
          >{/each}
      </select></label
    >
    <label
      >{locale === 'ru'
        ? 'Вернуть ручной режим через (мс; пусто — постоянно)'
        : 'Return to manual after (ms; blank = permanent)'}<input
        type="number"
        min="1"
        max="86400000"
        value={lighting.temporary_ms ?? undefined}
        onchange={(e) =>
          onchange({
            ...lighting,
            temporary_ms: e.currentTarget.value === '' ? null : e.currentTarget.valueAsNumber,
          })}
      /></label
    >
    <p>
      {locale === 'ru'
        ? 'Временный режим доступен только для проверенного временного вывода. Он не включает перезапись постоянных настроек клавиатуры.'
        : 'Temporary mode requires verified volatile output. It does not rewrite persistent keyboard settings temporarily.'}
    </p>
  {/if}
  {#if value.type === 'dial_bindings'}
    {@const dial = value}
    {#each ['left', 'right', 'press', 'tap'] as branch}{@const key = branch as
        'left' | 'right' | 'press' | 'tap'}
      <div class="action-step">
        <div class="step-toolbar">
          <strong
            >{t(
              key === 'left'
                ? 'rotateLeft'
                : key === 'right'
                  ? 'rotateRight'
                  : key === 'press'
                    ? 'dialPress'
                    : 'panelTap',
            )}</strong
          >
          {#if dial[key]}<button
              aria-label={t('remove')}
              onclick={() => onchange({ ...dial, [key]: null })}>×</button
            >{:else}<button
              onclick={() => onchange({ ...dial, [key]: action('counter', t('counter')) })}
              >＋ {t('addAction')}</button
            >{/if}
        </div>
        {#if dial[key]}<ActionEditor
            {feedback}
            {openInspector}
            value={dial[key]!}
            {device}
            {plugins}
            {locale}
            {depth}
            onchange={(next) => onchange({ ...dial, [key]: next })}
          />{/if}
      </div>
    {/each}
  {/if}
  {#if value.type === 'page_cycle'}<label
      >{t('pageDirection')}<select
        value={value.direction}
        onchange={(e) =>
          onchange({
            type: 'page_cycle',
            direction: e.currentTarget.value as 'previous' | 'next' | 'dial',
          })}
      >
        <option value="dial">{t('dialPageDirection')}</option><option value="previous"
          >{t('previousPage')}</option
        ><option value="next">{t('nextPage')}</option>
      </select></label
    >
  {:else if value.type === 'page'}<label
      >{t('page')}<select
        value={value.page_id}
        onchange={(e) => onchange({ type: 'page', page_id: e.currentTarget.value })}
        >{#each device.profile.pages as p}<option value={p.id}
            >{pageLabel(device.profile, p)}</option
          >{/each}</select
      ></label
    >
  {:else if value.type === 'profile'}<label
      >{t('profile')}<select
        value={value.profile_id}
        onchange={(e) => onchange({ type: 'profile', profile_id: e.currentTarget.value })}
        >{#each [device.profile, ...device.profiles] as p}<option value={p.id}>{p.name}</option
          >{/each}</select
      ></label
    >
  {:else if value.type === 'launch'}
    <label
      >{t('programPath')}<input
        value={value.program}
        onchange={(e) =>
          onchange({
            ...(value as Extract<Plan, { type: 'launch' }>),
            program: e.currentTarget.value,
          })}
      /></label
    >
    <label
      >{t('arguments')}<textarea
        rows="2"
        value={value.args.join('\n')}
        onchange={(e) =>
          onchange({
            ...(value as Extract<Plan, { type: 'launch' }>),
            args: e.currentTarget.value.split('\n').filter(Boolean),
          })}></textarea></label
    >
  {:else if value.type === 'hotkey'}<label
      >{t('hotkey')}<input
        value={value.keys.join('+')}
        placeholder="Ctrl+Shift+A"
        onchange={(e) =>
          onchange({
            type: 'hotkey',
            keys: e.currentTarget.value
              .split('+')
              .map((s) => s.trim())
              .filter(Boolean),
          })}
      /></label
    >
  {:else if value.type === 'sequence'}
    {@const sequence = value}
    {#each sequence.steps as step, i (step.action.id)}<div class="action-step">
        <div class="step-toolbar">
          <strong>{i + 1}</strong><label
            >{t('delay')}<input
              type="number"
              min="0"
              max="3600000"
              value={step.delay_ms}
              onchange={(e) =>
                onchange({
                  ...sequence,
                  steps: sequence.steps.map((s, n) =>
                    n === i ? { ...s, delay_ms: +e.currentTarget.value } : s,
                  ),
                })}
            /> ms</label
          ><button
            disabled={i === 0}
            onclick={() => onchange({ ...sequence, steps: shift(sequence.steps, i, -1) })}
            aria-label={t('moveUp')}>↑</button
          ><button
            disabled={i === sequence.steps.length - 1}
            onclick={() => onchange({ ...sequence, steps: shift(sequence.steps, i, 1) })}
            aria-label={t('moveDown')}>↓</button
          ><button
            disabled={sequence.steps.length === 1}
            onclick={() =>
              onchange({ ...sequence, steps: sequence.steps.filter((_, n) => n !== i) })}
            aria-label={t('remove')}>×</button
          >
        </div>
        <ActionEditor
          {feedback}
          {openInspector}
          value={step.action}
          {device}
          {plugins}
          {locale}
          {depth}
          onchange={(next) =>
            onchange({
              ...sequence,
              steps: sequence.steps.map((s, n) => (n === i ? { ...s, action: next } : s)),
            })}
        />
      </div>{/each}
    <button
      onclick={() =>
        onchange({
          ...sequence,
          steps: [...sequence.steps, { delay_ms: 200, action: action('counter', t('counter')) }],
        })}>＋ {t('addAction')}</button
    >
  {:else if value.type === 'switch' || value.type === 'dial_stack' || value.type === 'action_wheel'}
    {@const container = value}{@const list = value.type === 'switch' ? value.states : value.actions}
    {#each list as item, i (item.id)}<div class="action-step">
        <div class="step-toolbar">
          <strong>{t(container.type === 'switch' ? 'state' : 'action')} {i + 1}</strong><button
            disabled={i === 0}
            onclick={() =>
              onchange(
                container.type === 'switch'
                  ? { ...container, states: shift(list, i, -1) }
                  : { ...container, actions: shift(list, i, -1) },
              )}
            aria-label={t('moveUp')}>↑</button
          ><button
            disabled={list.length === 1}
            onclick={() =>
              onchange(
                container.type === 'switch'
                  ? { ...container, states: list.filter((_, n) => n !== i) }
                  : { ...container, actions: list.filter((_, n) => n !== i) },
              )}
            aria-label={t('remove')}>×</button
          >
        </div>
        <ActionEditor
          {feedback}
          {openInspector}
          value={item}
          controller={container.type === 'dial_stack' ? 'Encoder' : 'Keypad'}
          {device}
          {plugins}
          {locale}
          {depth}
          onchange={(next) =>
            onchange(
              container.type === 'switch'
                ? { ...container, states: list.map((s, n) => (n === i ? next : s)) }
                : { ...container, actions: list.map((s, n) => (n === i ? next : s)) },
            )}
        />
      </div>{/each}
    <button
      onclick={() => {
        const next = [...list, action('counter', t('counter'))];
        onchange(
          container.type === 'switch'
            ? { ...container, states: next }
            : { ...container, actions: next },
        );
      }}>＋ {t('addAction')}</button
    >
  {:else if value.type === 'key_logic'}
    {@const logic = value}
    <div class="gesture-thresholds">
      <label
        >{t('doubleWindow')}<input
          type="number"
          min="100"
          max="1000"
          value={logic.double_ms}
          onchange={(e) => onchange({ ...logic, double_ms: +e.currentTarget.value })}
        /> ms</label
      ><label
        >{t('holdThreshold')}<input
          type="number"
          min="200"
          max="5000"
          value={logic.hold_ms}
          onchange={(e) => onchange({ ...logic, hold_ms: +e.currentTarget.value })}
        /> ms</label
      >
    </div>
    {#each ['single', 'double', 'hold'] as branch}{@const key = branch as
        'single' | 'double' | 'hold'}
      <div class="action-step">
        <div class="step-toolbar">
          <strong>{t(key)}</strong>{#if logic[key]}<button
              onclick={() => onchange({ ...logic, [key]: null })}
              aria-label={t('remove')}>×</button
            >{:else}<button onclick={() => onchange({ ...logic, [key]: action('counter', t(key)) })}
              >＋ {t('addAction')}</button
            >{/if}
        </div>
        {#if logic[key]}<ActionEditor
            {feedback}
            {openInspector}
            value={logic[key]!}
            {device}
            {plugins}
            {locale}
            {depth}
            onchange={(next) => onchange({ ...logic, [key]: next })}
          />{/if}
      </div>{/each}
  {/if}
</div>
