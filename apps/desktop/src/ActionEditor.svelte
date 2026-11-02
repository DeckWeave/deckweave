<script lang="ts">
  import type { Binding, Device, PluginPackage, Locale, Snapshot } from './types';
  import ActionDrop from './ActionDrop.svelte';
  import PlanEditor from './PlanEditor.svelte';
  import { text } from './i18n';
  import Icon from './Icon.svelte';
  import { actionIcon } from './icons';
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
    inSequence = false,
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
    inSequence?: boolean;
  } = $props();
  const t = $derived((key: Parameters<typeof text>[1]) => text(locale, key));
  const result = $derived(
    feedback.find((f) => f.device_id === device.id && f.binding_id === value.id),
  );
  const packageInfo = $derived(
    value.plugin ? plugins.find((p) => p.id === value.plugin!.package) : undefined,
  );
  const pluginAction = $derived(
    value.plugin ? packageInfo?.actions.find((a) => a.id === value.plugin!.action) : undefined,
  );
  const unavailable = $derived(
    !!value.plugin && (!packageInfo || !!packageInfo.error || !pluginAction),
  );
</script>

<div class="nested-action">
  <ActionDrop
    scope={{ controller, depth, inSequence }}
    identity={JSON.stringify(value)}
    ru={locale === 'ru'}
    label={value.label}
    {onchange}
  >
    <Icon name={actionIcon(value.plan?.type ?? value.plugin?.action ?? value.kind)} />
    <input
      aria-label={t('title')}
      maxlength="80"
      value={value.label}
      onchange={(e) => onchange({ ...value, label: e.currentTarget.value })}
    />
  </ActionDrop>
  {#if value.plan}<PlanEditor
      {feedback}
      {openInspector}
      {controller}
      {inSequence}
      value={value.plan}
      {device}
      {plugins}
      {locale}
      depth={depth + 1}
      onchange={(next) => onchange({ ...value, plan: next })}
    />{/if}
  {#if unavailable}<p class="connection-error" role="status">
      {packageInfo?.error ??
        (locale === 'ru'
          ? 'Плагин недоступен. Назначение и настройки сохранены; установите или подключите его в разделе расширений.'
          : 'Plugin unavailable. Assignment and settings are retained; install or reconnect it in Extensions.')}
    </p>{/if}
  {#if !unavailable && value.plugin && plugins
      .find((p) => p.id === value.plugin!.package)
      ?.actions.find((a) => a.id === value.plugin!.action)?.inspector}<button
      onclick={() => openInspector(value.id)}>{t('pluginSettings')}</button
    >
  {:else if value.plugin && !unavailable}<label
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
  {#if result}<output class="nested-feedback" class:connection-error={!!result?.error}
      >{result?.error ??
        (result?.running
          ? t('running')
          : value.kind === 'counter'
            ? String(result?.value ?? 0)
            : value.kind === 'toggle'
              ? t(result?.active ? 'on' : 'off')
              : (result?.display ?? result?.title ?? t('noActionYet')))}</output
    >{/if}
</div>
