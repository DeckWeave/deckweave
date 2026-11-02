<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { keyboardActionError } from './keyboard-action-errors';
  import type { Client } from './client.svelte';
  import Icon from './Icon.svelte';
  import type { Binding, Device } from './types';
  let { client, device, binding }: { client: Client; device: Device; binding: Binding } = $props();
  const ru = $derived(client.snapshot?.configuration.locale === 'ru');
  type Alert = {
    enabled: boolean;
    deviceId: string;
    controlId: string;
    color: [number, number, number];
    periodMs: number;
    durationMs: number;
  };
  const targets = $derived(
    client.snapshot?.configuration.devices.filter(
      (d) => d.provider_descriptor?.definition.keyLighting,
    ) ?? [],
  );
  const config = $derived(
    (binding.plugin?.settings.deckweaveKeyLightingAlert ?? {
      enabled: false,
      deviceId: targets[0]?.id ?? '',
      controlId: 'key-37',
      color: [255, 0, 0],
      periodMs: 1000,
      durationMs: 5000,
    }) as Alert,
  );
  const target = $derived(targets.find((d) => d.id === config.deviceId));
  const keys = $derived(
    target?.provider_descriptor?.definition.controls?.filter((c) =>
      target.provider_descriptor?.geometry.controls.some(
        (key) => key.id === c.id && key.kind === 'key',
      ),
    ) ?? [],
  );
  let statuses = $state<Record<string, { active: boolean; error: string | null }>>({});
  let error = $state('');
  let disposed = false;
  let timer: ReturnType<typeof setTimeout>;
  const colorHex = $derived(
    '#' + config.color.map((n) => n.toString(16).padStart(2, '0')).join(''),
  );
  function save(patch: Partial<Alert>) {
    const next = { ...config, ...patch };
    void client.send((revision) => ({
      type: 'plugin_settings',
      device_id: device.id,
      binding_id: binding.id,
      settings: { ...binding.plugin!.settings, deckweaveKeyLightingAlert: next },
      expected_revision: revision,
    }));
  }
  async function poll() {
    try {
      const next = await invoke<typeof statuses>('key_lighting_status');
      if (!disposed) statuses = next;
    } catch (e) {
      if (!disposed) error = String(e);
    }
  }
  onMount(() => {
    const run = async () => {
      await poll();
      if (!disposed) timer = setTimeout(() => void run(), 500);
    };
    void run();
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  });
  async function test() {
    error = '';
    try {
      await invoke('key_lighting_test', { alert: config });
      await poll();
    } catch (e) {
      error = String(e);
    }
  }
  async function stop() {
    error = '';
    try {
      await invoke('key_lighting_stop', { deviceId: config.deviceId });
      await poll();
    } catch (e) {
      error = String(e);
    }
  }
  function keyLabel(key: (typeof keys)[number]) {
    const l = key.label;
    return typeof l === 'string' ? l : (l?.[ru ? 'ru' : 'en'] ?? key.id);
  }
</script>

<details class="key-light-alert">
  <summary>{ru ? 'Подсветка клавиши по завершении' : 'Key lighting on completion'}</summary>
  <p>
    {ru
      ? 'Экспериментальный временный выход IO84. Требует статичного одноцветного эффекта и яркости от 2. Действующий эффект не переключается автоматически. Физический цвет ещё требует проверки.'
      : 'Experimental IO84 temporary output. Requires Static, Single colour and brightness at least 2. The current effect is not changed automatically. Physical colour still needs verification.'}
  </p>
  {#if !targets.length}<p>
      {ru
        ? 'Добавьте устройство с поддержкой временной подсветки клавиш.'
        : 'Add a device supporting temporary key lighting.'}
    </p>{:else}
    <label class="enable"
      ><input
        type="checkbox"
        checked={config.enabled}
        onchange={(e) => save({ enabled: e.currentTarget.checked })}
      />{ru
        ? 'Включить подсветку по завершении таймера'
        : 'Enable lighting when the timer finishes'}</label
    >
    <div class="alert-fields">
      <label
        >{ru ? 'Устройство' : 'Device'}<select
          value={config.deviceId}
          onchange={(e) => save({ deviceId: e.currentTarget.value })}
          >{#each targets as d}<option value={d.id}>{d.name}</option>{/each}</select
        ></label
      >
      <label
        >{ru ? 'Клавиша' : 'Key'}<select
          value={config.controlId}
          onchange={(e) => save({ controlId: e.currentTarget.value })}
          >{#each keys as key}<option value={key.id}>{keyLabel(key)}</option>{/each}</select
        ></label
      >
      <label
        >{ru ? 'Цвет' : 'Colour'}<input
          type="color"
          value={colorHex}
          onchange={(e) => {
            const v = e.currentTarget.value;
            save({
              color: [
                parseInt(v.slice(1, 3), 16),
                parseInt(v.slice(3, 5), 16),
                parseInt(v.slice(5, 7), 16),
              ],
            });
          }}
        /></label
      >
      <label
        >{ru ? 'Период, мс' : 'Period, ms'}<input
          type="number"
          min="300"
          max="5000"
          step="100"
          value={config.periodMs}
          onchange={(e) => save({ periodMs: e.currentTarget.valueAsNumber })}
        /></label
      >
      <label
        >{ru ? 'Длительность, с' : 'Duration, s'}<input
          type="number"
          min="1"
          max="60"
          step="1"
          value={config.durationMs / 1000}
          onchange={(e) => save({ durationMs: e.currentTarget.valueAsNumber * 1000 })}
        /></label
      >
      <div class="alert-buttons">
        <button disabled={!config.enabled || !target} onclick={() => void test()}
          ><Icon name="play" />{ru ? 'Проверить' : 'Test'}</button
        ><button onclick={() => void stop()}>{ru ? 'Остановить' : 'Stop'}</button>
      </div>
    </div>
    {#if statuses[config.deviceId]?.active}<p role="status">
        {ru
          ? 'Временная подсветка выполняется. Затем возвращается исходный цвет.'
          : 'Temporary lighting is running. The original colour is restored afterwards.'}
      </p>{/if}
    {#if error || statuses[config.deviceId]?.error}<p role="alert" class="connection-error">
        {keyboardActionError(error || statuses[config.deviceId]?.error, ru)}
      </p>{/if}
  {/if}
</details>

<style>
  .key-light-alert {
    grid-column: 1/-1;
    border: 1px solid var(--border);
    padding: 9px;
    border-radius: 7px;
  }
  .key-light-alert summary {
    cursor: pointer;
  }
  .key-light-alert p {
    font-size: 12px;
    line-height: 1.4;
    color: var(--muted);
  }
  .alert-fields {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: 9px;
    margin-top: 10px;
  }
  .alert-fields label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
  }
  .alert-fields input,
  .alert-fields select {
    min-width: 0;
    width: 100%;
    box-sizing: border-box;
  }
  .alert-buttons {
    display: flex;
    gap: 6px;
    align-items: end;
  }
  .enable {
    display: flex;
    gap: 7px;
    align-items: center;
  }
  .key-light-alert .connection-error {
    color: var(--danger);
  }
</style>
