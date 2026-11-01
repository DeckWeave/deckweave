<script lang="ts">
  import type { Binding, Device, PluginPackage, Locale, Snapshot } from './types';
  import { action, plan, planTypes } from './action-factory';
  import PlanEditor from './PlanEditor.svelte';
  import { text } from './i18n';
  let {
    value,
    device,
    plugins,
    locale,
    onchange,
    depth = 0,
    openInspector,
    controller = 'Keypad',
    feedback = [],
  }: {
    value: Binding;
    device: Device;
    plugins: PluginPackage[];
    locale: Locale;
    onchange: (value: Binding) => void;
    depth?: number;
    openInspector: (context: string) => void;
    controller?: 'Keypad' | 'Encoder';
    feedback?: Snapshot['feedback'];
  } = $props();
  const t = $derived((key: Parameters<typeof text>[1]) => text(locale, key));
  const result = $derived(
    feedback.find((f) => f.device_id === device.id && f.binding_id === value.id),
  );
  const choice = $derived(
    value.plugin
      ? `plugin:${value.plugin.package}:${value.plugin.action}`
      : value.plan
        ? `program:${value.plan.type}`
        : value.kind,
  );
  function choose(kind: string) {
    if (kind.startsWith('program:')) {
      const type = kind.slice(8) as Parameters<typeof plan>[0];
      onchange({ ...action('program', t(type)), plan: plan(type, device) });
    } else if (kind.startsWith('plugin:')) {
      const [, pack, id] = kind.split(':');
      const item = plugins.find((p) => p.id === pack)?.actions.find((a) => a.id === id);
      onchange({
        ...action('plugin', item?.name ?? id),
        plugin: { package: pack, action: id, settings: {}, controller },
      });
    } else {
      onchange(action(kind, t(kind as 'counter' | 'toggle')));
    }
  }
</script>

<div class="nested-action">
  <select aria-label={t('action')} value={choice} onchange={(e) => choose(e.currentTarget.value)}>
    <option value="counter">{t('counter')}</option><option value="toggle">{t('toggle')}</option>
    {#if depth < 6}{#each planTypes.filter((p) => !['key_logic', 'dial_bindings', 'dial_stack', 'action_wheel'].includes(p)) as p}<option
          value={`program:${p}`}>{t(p)}</option
        >{/each}{/if}
    {#each plugins.filter((p) => !p.error) as pack}<optgroup label={pack.name}
        >{#each pack.actions.filter((a) => a.controllers.includes(controller)) as a}<option
            value={`plugin:${pack.id}:${a.id}`}>{a.name}</option
          >{/each}</optgroup
      >{/each}
  </select>
  <input
    aria-label={t('title')}
    maxlength="80"
    value={value.label}
    onchange={(e) => onchange({ ...value, label: e.currentTarget.value })}
  />
  {#if value.plan}<PlanEditor
      {feedback}
      {openInspector}
      value={value.plan}
      {device}
      {plugins}
      {locale}
      depth={depth + 1}
      onchange={(next) => onchange({ ...value, plan: next })}
    />{/if}
  {#if value.plugin && plugins
      .find((p) => p.id === value.plugin!.package)
      ?.actions.find((a) => a.id === value.plugin!.action)?.inspector}<button
      onclick={() => openInspector(value.id)}>{t('pluginSettings')}</button
    >
  {:else if value.plugin}<label
      >{t('settingsJson')}<textarea
        rows="2"
        value={JSON.stringify(value.plugin.settings)}
        onchange={(e) => {
          try {
            const settings = JSON.parse(e.currentTarget.value);
            if (settings && typeof settings === 'object' && !Array.isArray(settings))
              onchange({ ...value, plugin: { ...value.plugin!, settings } });
          } catch {
            e.currentTarget.setCustomValidity('Invalid JSON');
            e.currentTarget.reportValidity();
          }
        }}></textarea></label
    >{/if}
  <output class="nested-feedback" class:connection-error={!!result?.error}
    >{result?.error ??
      (result?.running
        ? t('running')
        : value.kind === 'counter'
          ? String(result?.value ?? 0)
          : value.kind === 'toggle'
            ? t(result?.active ? 'on' : 'off')
            : (result?.display ?? result?.title ?? t('noActionYet')))}</output
  >
</div>
