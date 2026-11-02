<script lang="ts">
  import type { Plan, Device, PluginPackage, Locale, Snapshot } from './types';
  import CompositionEditor from './CompositionEditor.svelte';
  import { text } from './i18n';
  import Icon from './Icon.svelte';
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
    controller = 'Keypad',
    inSequence = false,
  }: {
    value: Plan;
    device: Device;
    plugins: PluginPackage[];
    locale: Locale;
    onchange: (value: Plan) => void;
    depth?: number;
    openInspector: (context: string) => void;
    feedback?: Snapshot['feedback'];
    controller?: 'Keypad' | 'Encoder';
    inSequence?: boolean;
  } = $props();
  const t = $derived((key: Parameters<typeof text>[1]) => text(locale, key));
</script>

<div
  class="plan-fields"
  class:composite-plan={[
    'sequence',
    'switch',
    'dial_stack',
    'action_wheel',
    'key_logic',
    'dial_bindings',
  ].includes(value.type)}
>
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
  {#if value.type === 'page_cycle'}<div class="direction-choice">
      <span>{t('pageDirection')}</span>
      <div class="segmented" role="group" aria-label={t('pageDirection')}>
        {#each ['dial', 'previous', 'next'] as direction}<button
            class:current={value.direction === direction}
            aria-pressed={value.direction === direction}
            onclick={() =>
              onchange({
                type: 'page_cycle',
                direction: direction as 'dial' | 'previous' | 'next',
              })}
            >{t(
              direction === 'dial'
                ? 'dialPageDirection'
                : direction === 'previous'
                  ? 'previousPage'
                  : 'nextPage',
            )}</button
          >{/each}
      </div>
    </div>
  {:else if value.type === 'page'}
    <div class="direct-options" role="group" aria-label={t('page')}>
      {#each device.profile.pages as page}<button
          class:current={value.page_id === page.id}
          aria-pressed={value.page_id === page.id}
          onclick={() => onchange({ type: 'page', page_id: page.id })}
          >{pageLabel(device.profile, page)}</button
        >{/each}
    </div>
  {:else if value.type === 'profile'}
    <div class="direct-options" role="group" aria-label={t('profile')}>
      {#each [device.profile, ...device.profiles] as profile}<button
          class:current={value.profile_id === profile.id}
          aria-pressed={value.profile_id === profile.id}
          onclick={() => onchange({ type: 'profile', profile_id: profile.id })}
          >{profile.name}</button
        >{/each}
    </div>
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
  {:else if ['sequence', 'switch', 'dial_stack', 'action_wheel', 'key_logic', 'dial_bindings'].includes(value.type)}
    <CompositionEditor
      {value}
      {device}
      {plugins}
      {locale}
      {onchange}
      {depth}
      {controller}
      {inSequence}
      {openInspector}
      {feedback}
    />
  {/if}
</div>
