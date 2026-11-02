<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import type { Client } from './client.svelte';
  import type { Device } from './types';
  import type { ProviderInventory, ProviderSessionState } from './provider-ui';
  import DeviceDecorations from './DeviceDecorations.svelte';
  import DepthRing from './DepthRing.svelte';
  import { recoveryErrorText } from './provider-recovery';
  let { client, device }: { client: Client; device: Device } = $props();
  let status = $state<ProviderSessionState | null>(null);
  let selected = $state<string[]>([]);
  let layer = $state('main');
  let error = $state('');
  let reading = $state(false);
  const ru = $derived(client.snapshot?.configuration.locale === 'ru');
  const definition = $derived(device.provider_descriptor?.definition);
  const keyboard = $derived(definition?.keyboard);
  const geometry = $derived(device.provider_descriptor!.geometry);
  const mapping = $derived(keyboard?.onboardMappings);
  const actuation = $derived(keyboard?.actuation);
  const travelCapability = $derived(definition?.observation?.keyTravel);
  const connected = $derived(status?.state === 'connected' && status.recovery?.state !== 'pending');
  const usageList = [
    ...Array.from({ length: 112 }, (_, i) => i + 4),
    ...Array.from({ length: 8 }, (_, i) => i + 224),
  ];
  function usageLabel(n: number) {
    if (n >= 4 && n <= 29) return String.fromCharCode(65 + n - 4);
    if (n >= 30 && n <= 38) return String(n - 29);
    if (n === 39) return '0';
    if (n >= 58 && n <= 69) return 'F' + (n - 57);
    if (n >= 104 && n <= 115) return 'F' + (n - 91);
    const names: Record<number, string> = {
      40: 'Enter',
      41: 'Esc',
      42: 'Backspace',
      43: 'Tab',
      44: ru ? 'Пробел' : 'Space',
      45: '-',
      46: '=',
      47: '[',
      48: ']',
      49: 'Backslash',
      51: ';',
      52: 'Quote',
      53: 'Backtick',
      54: ',',
      55: '.',
      56: '/',
      57: 'Caps Lock',
      70: 'Print Screen',
      71: 'Scroll Lock',
      72: 'Pause',
      73: 'Insert',
      74: 'Home',
      75: 'Page Up',
      76: 'Delete',
      77: 'End',
      78: 'Page Down',
      79: '→',
      80: '←',
      81: '↓',
      82: '↑',
      83: 'Num Lock',
      84: 'Num /',
      85: 'Num *',
      86: 'Num -',
      87: 'Num +',
      88: 'Num Enter',
      89: 'Num 1',
      90: 'Num 2',
      91: 'Num 3',
      92: 'Num 4',
      93: 'Num 5',
      94: 'Num 6',
      95: 'Num 7',
      96: 'Num 8',
      97: 'Num 9',
      98: 'Num 0',
      99: 'Num .',
      101: 'Menu',
      224: 'Left Ctrl',
      225: 'Left Shift',
      226: 'Left Alt',
      227: 'Left Win',
      228: 'Right Ctrl',
      229: 'Right Shift',
      230: 'Right Alt',
      231: 'Right Win',
    };
    return names[n] ?? (ru ? 'Клавиша ' : 'Key ') + n;
  }
  function label(id: string) {
    const l = definition?.controls?.find((c) => c.id === id)?.label;
    return typeof l === 'string' ? l : (l?.[ru ? 'ru' : 'en'] ?? l?.en ?? id);
  }
  function record(value: unknown): Record<string, unknown> {
    return value && typeof value === 'object' && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : {};
  }
  function observed(setting: string | undefined, key: string) {
    return record(record(status?.observed?.[setting ?? ''])[key]);
  }
  function mapValue(key: string) {
    return record(observed(mapping?.settingId, key)[layer]);
  }
  const paired = $derived(selected.some((key) => [11, 12].includes(Number(mapValue(key).page))));
  function common(field: string) {
    const values = selected.map((key) => observed(actuation?.settingId, key)[field]);
    return values.length && values.every((v) => v === values[0]) ? values[0] : undefined;
  }
  function usageValue() {
    const values = selected.map((key) => mapValue(key).usage);
    return values.length && values.every((v) => v === values[0]) ? values[0] : undefined;
  }
  function toggle(id: string) {
    selected = selected.includes(id) ? selected.filter((k) => k !== id) : [...selected, id];
  }
  async function refresh(read = false) {
    if (!client.available) return;
    reading = true;
    const id = device.id;
    try {
      if (read) await invoke('refresh_provider_state', { deviceId: id });
      const inventory = await invoke<ProviderInventory>('device_provider_status');
      if (device.id === id) {
        status = inventory.sessions[id] ?? null;
        error = '';
      }
    } catch (e) {
      error = String(e);
    } finally {
      reading = false;
    }
  }
  onMount(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), 1000);
    return () => clearInterval(timer);
  });
  async function send(setting: string, patch: unknown) {
    if (!selected.length) return;
    if (
      await client.send((revision) => ({
        type: 'set_provider_settings',
        device_id: device.id,
        changes: { [setting]: Object.fromEntries(selected.map((id) => [id, patch])) },
        expected_revision: revision,
      }))
    )
      await refresh();
  }
  function travel(field: string, value: number) {
    if (actuation && Number.isFinite(value))
      void send(actuation.settingId, { [field]: Math.round(value * 1000) });
  }
  function rapid(enabled: boolean) {
    if (actuation)
      void send(
        actuation.settingId,
        enabled
          ? {
              rapidTrigger: true,
              pressUm:
                typeof common('pressUm') === 'number' && Number(common('pressUm')) >= 10
                  ? common('pressUm')
                  : 100,
              releaseUm:
                typeof common('releaseUm') === 'number' && Number(common('releaseUm')) >= 10
                  ? common('releaseUm')
                  : 100,
            }
          : { rapidTrigger: false },
      );
  }
  function supports(field: string) {
    return actuation?.fields?.includes(field) ?? false;
  }
</script>

<section class="keyboard-settings">
  <header>
    <div>
      <h3>{ru ? 'Настройки клавиатуры' : 'Keyboard settings'}</h3>
      <p>
        {ru
          ? 'Выберите одну или несколько клавиш. Завершённое изменение сразу отправляется устройству.'
          : 'Select one or more keys. Each completed edit is sent immediately.'}
      </p>
    </div>
    <button disabled={reading} onclick={() => void refresh(true)}
      >{ru ? 'Прочитать с устройства' : 'Read device state'}</button
    >
  </header>
  {#if error || status?.error}<p class="connection-error" role="alert">
      {recoveryErrorText(error || status?.error, ru)}
    </p>{/if}
  <div class="keyboard-selection" style:aspect-ratio={geometry.width + '/' + geometry.height}>
    <DeviceDecorations {definition} {geometry} locale={ru ? 'ru' : 'en'} />
    {#each geometry.controls.filter((c) => c.kind === 'key') as control (control.id)}{@const depth =
        travelCapability ? client.depth(control.id, client.depthScaleUm) : null}<button
        class:selected={selected.includes(control.id)}
        class:depth-pressed={!!depth && depth.um >= 300}
        aria-pressed={selected.includes(control.id)}
        title={label(control.id) +
          (depth ? ' · ' + (depth.um / 1000).toFixed(2) + (ru ? ' мм' : ' mm') : '')}
        style:left={(control.rect[0] / geometry.width) * 100 + '%'}
        style:top={(control.rect[1] / geometry.height) * 100 + '%'}
        style:width={(control.rect[2] / geometry.width) * 100 + '%'}
        style:height={(control.rect[3] / geometry.height) * 100 + '%'}
        onclick={() => toggle(control.id)}><DepthRing reading={depth} />{label(control.id)}</button
      >{/each}
  </div>
  <div class="key-selection-tools">
    <button
      onclick={() =>
        (selected = geometry.controls.filter((c) => c.kind === 'key').map((c) => c.id))}
      >{ru ? 'Выбрать все' : 'Select all'}</button
    ><button onclick={() => (selected = [])}>{ru ? 'Снять выделение' : 'Clear selection'}</button
    ><span>{ru ? 'Выбрано' : 'Selected'}: {selected.length}</span>
    {#if travelCapability && selected.length === 1}{@const depth = client.depth(
        selected[0],
        client.depthScaleUm,
      )}<span
        >{ru ? 'Глубина' : 'Key travel'}: {depth
          ? (depth.um / 1000).toFixed(2) + (ru ? ' мм' : ' mm')
          : '—'}</span
      >{/if}
  </div>
  {#if !connected}<p>
      {ru
        ? status?.recovery?.state === 'pending'
          ? 'Сначала завершите восстановление настроек выше.'
          : 'Для настройки требуется подключённое устройство.'
        : status?.recovery?.state === 'pending'
          ? 'Complete settings recovery above before editing.'
          : 'Connect the device to change onboard settings.'}
    </p>{/if}
  {#if selected.length}
    <div class="keyboard-setting-groups">
      {#if mapping?.bindingFormat === 'keyboard-usage'}<div>
          <h4>{ru ? 'Клавиша на устройстве' : 'Onboard assignment'}</h4>
          <label
            >{ru ? 'Слой' : 'Layer'}<select bind:value={layer}
              >{#each mapping.layers as item}<option value={item}
                  >{item === 'main'
                    ? ru
                      ? 'Основной'
                      : 'Main'
                    : item === 'fn'
                      ? 'Fn'
                      : item}</option
                >{/each}</select
            ></label
          ><label
            >{ru ? 'Выполняемая клавиша' : 'Assigned key'}<select
              disabled={!connected || client.busy || paired}
              value={typeof usageValue() === 'number' ? String(usageValue()) : ''}
              onchange={(e) =>
                void send(mapping.settingId, { [layer]: { usage: Number(e.currentTarget.value) } })}
              ><option value="" disabled
                >{ru ? 'Разные / специальное назначение' : 'Mixed / special assignment'}</option
              >{#each usageList as usage}<option value={String(usage)}>{usageLabel(usage)}</option
                >{/each}</select
            ></label
          >{#if paired}<p class="connection-error">
              {ru
                ? 'В выделении есть парное назначение. Оно сохраняется; этот редактор не меняет пары.'
                : 'The selection includes a paired assignment. This editor preserves it and cannot edit pairs.'}
            </p>{/if}
          <p>
            {ru
              ? 'Это обычное назначение в прошивке, отдельное от действий DeckWeave.'
              : 'This is the normal onboard key mapping, separate from DeckWeave actions.'}
          </p>
        </div>{/if}
      {#if actuation}<div>
          <h4>{ru ? 'Срабатывание и Rapid Trigger' : 'Actuation and Rapid Trigger'}</h4>
          <label
            >{ru ? 'Срабатывание, мм' : 'Actuation, mm'}<input
              type="number"
              min={actuation.minUm / 1000}
              max={actuation.maxUm / 1000}
              step={actuation.stepUm / 1000}
              value={typeof common('triggerUm') === 'number'
                ? Number(common('triggerUm')) / 1000
                : undefined}
              placeholder={ru ? 'Разные значения' : 'Mixed values'}
              disabled={!connected || client.busy}
              onchange={(e) => {
                if (e.currentTarget.value !== '' && e.currentTarget.validity.valid)
                  travel('triggerUm', e.currentTarget.valueAsNumber);
              }}
            /></label
          >
          {#if supports('rapidTrigger')}<label
              ><input
                type="checkbox"
                checked={common('rapidTrigger') === true}
                disabled={!connected || client.busy}
                onchange={(e) => rapid(e.currentTarget.checked)}
              />Rapid Trigger</label
            >{/if}
          {#if common('rapidTrigger') === true}{#each ['pressUm', 'releaseUm'] as field}<label
                >{field === 'pressUm'
                  ? ru
                    ? 'Повторное нажатие, мм'
                    : 'Re-press travel, mm'
                  : ru
                    ? 'Отпускание, мм'
                    : 'Release travel, mm'}<input
                  type="number"
                  min={(actuation.rapidMinUm ?? 10) / 1000}
                  max={actuation.maxUm / 1000}
                  step={actuation.stepUm / 1000}
                  value={typeof common(field) === 'number'
                    ? Number(common(field)) / 1000
                    : undefined}
                  disabled={!connected || client.busy}
                  onchange={(e) => {
                    if (e.currentTarget.value !== '' && e.currentTarget.validity.valid)
                      travel(field, e.currentTarget.valueAsNumber);
                  }}
                /></label
              >{/each}{/if}
          {#each ['wholeTravel', 'rampage'] as field}{#if supports(field)}<label
                ><input
                  type="checkbox"
                  checked={common(field) === true}
                  disabled={!connected || client.busy}
                  onchange={(e) =>
                    void send(actuation.settingId, { [field]: e.currentTarget.checked })}
                />{field === 'wholeTravel' ? (ru ? 'Весь ход' : 'Whole travel') : 'Rampage'}</label
              >{/if}{/each}
        </div>{/if}
    </div>
    <details>
      <summary
        >{ru ? 'Считанные значения выбранных клавиш' : 'Last read selected key values'}</summary
      >{#each selected as id}<p>
          <strong>{label(id)}</strong> · {ru ? 'Основной' : 'Main'}: {typeof record(
            observed(mapping?.settingId, id).main,
          ).usage === 'number'
            ? usageLabel(Number(record(observed(mapping?.settingId, id).main).usage))
            : ru
              ? 'Специальное назначение'
              : 'Special assignment'} · Fn: {typeof record(observed(mapping?.settingId, id).fn)
            .usage === 'number'
            ? usageLabel(Number(record(observed(mapping?.settingId, id).fn).usage))
            : ru
              ? 'Специальное назначение'
              : 'Special assignment'} · {ru ? 'Срабатывание' : 'Actuation'}: {typeof observed(
            actuation?.settingId,
            id,
          ).triggerUm === 'number'
            ? Number(observed(actuation?.settingId, id).triggerUm) / 1000 + ' mm'
            : ru
              ? 'Неизвестно'
              : 'Unknown'}
        </p>{/each}
    </details>
  {/if}
</section>

<style>
  .keyboard-selection button.depth-pressed {
    border-color: #77e2ff;
    box-shadow: inset 0 0 9px #77e2ff40;
  }
  .keyboard-settings {
    margin-top: 24px;
    border-top: 1px solid #ffffff20;
    padding-top: 20px;
  }
  .keyboard-settings header {
    display: flex;
    gap: 16px;
    align-items: center;
    justify-content: space-between;
  }
  .keyboard-settings p {
    font-size: 12px;
    line-height: 1.5;
  }
  .keyboard-selection {
    position: relative;
    width: 100%;
    max-width: 1100px;
    margin: 20px 0;
  }
  .keyboard-selection button {
    position: absolute;
    border: 2px solid #222;
    border-radius: 5px;
    background: #383c46;
    color: #ddd;
    padding: 2px;
    font-size: 11px;
    min-height: 0;
  }
  .keyboard-selection button.selected {
    background: #356d9f;
    border-color: #9dd6ff;
  }
  .key-selection-tools {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .keyboard-setting-groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 20px;
    margin: 20px 0;
  }
  .keyboard-setting-groups > div {
    border: 1px solid #ffffff20;
    padding: 16px;
    border-radius: 8px;
  }
  .keyboard-setting-groups label {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: center;
    margin: 12px 0;
  }
  .keyboard-setting-groups select {
    max-width: 220px;
  }
  .keyboard-setting-groups input[type='number'] {
    width: 120px;
  }
</style>
