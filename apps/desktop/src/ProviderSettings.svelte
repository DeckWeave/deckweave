<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import type { Client } from './client.svelte';
  import ProviderRecovery from './ProviderRecovery.svelte';
  import DeviceDecorations from './DeviceDecorations.svelte';
  import DepthRing from './DepthRing.svelte';
  import { observedStaticColors, unsupportedStaticColors } from './static-color-coverage';
  import type { Device, ProviderSetting } from './types';
  import type { ProviderInventory, ProviderSessionState } from './provider-ui';
  let {
    client,
    device,
    lighting = false,
  }: { client: Client; device: Device; lighting?: boolean } = $props();
  let status = $state<ProviderSessionState | null>(null);
  let loading = $state(false);
  let error = $state('');
  let presetName = $state('');
  let deletingPreset = $state<string | null>(null);
  const lightingState = $derived(client.snapshot?.lighting?.[device.id]);
  let chosen = $state<string[]>([]);
  let color = $state('#ffffff');
  const connected = $derived(status?.state === 'connected' && status.recovery?.state !== 'pending');
  const ru = $derived(client.snapshot?.configuration.locale === 'ru');
  const locale = $derived(ru ? 'ru' : 'en');
  const definition = $derived(device.provider_descriptor?.definition);
  const groups = $derived(definition?.lighting ?? []);
  const settings = $derived(
    (definition?.settings ?? []).filter(
      (s) =>
        lighting === (s.group === 'lighting' || groups.some((g) => g.settings?.includes(s.id))),
    ),
  );
  const keyboardStatusOnly = $derived(!lighting && !!definition?.keyboard && settings.length === 0);
  const geometry = $derived(device.provider_descriptor?.geometry);
  const travel = $derived(definition?.observation?.keyTravel);
  const staticGroup = $derived(groups.find((g) => g.staticColors?.address === 'controlId'));
  const staticControlIds = $derived(
    (geometry?.controls ?? [])
      .filter((c) => c.kind === 'key' && staticGroup?.controls?.includes(c.id))
      .map((c) => c.id),
  );
  const colorCoverage = $derived(
    observedStaticColors(
      staticGroup?.staticColors
        ? status?.observed?.[staticGroup.staticColors.settingId]
        : undefined,
      staticControlIds,
    ),
  );
  const writableStaticIds = $derived(Object.keys(colorCoverage));
  const selectedColorsWritable = $derived(
    connected && chosen.length > 0 && chosen.every((id) => Object.hasOwn(colorCoverage, id)),
  );
  $effect(() => {
    const available = new Set(connected ? writableStaticIds : []);
    if (chosen.some((id) => !available.has(id))) chosen = chosen.filter((id) => available.has(id));
  });
  function unsupportedPreset(settings: Record<string, unknown>) {
    return staticGroup?.staticColors
      ? unsupportedStaticColors(settings[staticGroup.staticColors.settingId], colorCoverage)
      : false;
  }
  function keyLabel(id: string, number: number) {
    const value = definition?.controls?.find((c) => c.id === id)?.label;
    return typeof value === 'string' ? value : (value?.[locale] ?? value?.en ?? String(number));
  }
  function keyColor(id: string) {
    const rgb = colorCoverage[id];
    if (!rgb) return null;
    return {
      background: 'rgb(' + [rgb.r, rgb.g, rgb.b].join(',') + ')',
      foreground: rgb.r * 0.2126 + rgb.g * 0.7152 + rgb.b * 0.0722 > 140 ? '#111' : '#fff',
    };
  }
  function label(s: { label?: { en?: string; ru?: string }; id: string }) {
    return s.label?.[locale] ?? s.label?.en ?? s.id;
  }
  async function refresh(read = false) {
    if (!client.available) return;
    const requestDevice = device.id;
    loading = true;
    try {
      if (read) await invoke('refresh_provider_state', { deviceId: device.id });
      const inventory = await invoke<ProviderInventory>('device_provider_status');
      if (device.id !== requestDevice) return;
      status = inventory.sessions[requestDevice] ?? null;
      error = '';
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
  onMount(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), 1000);
    return () => {
      clearInterval(timer);
      if (lighting)
        void client.send(() => ({ type: 'release_lighting_edit', device_id: device.id }));
    };
  });
  function current(s: ProviderSetting): unknown {
    return status?.observed?.[s.id] ?? device.provider_settings[s.id];
  }
  async function set(id: string, value: unknown) {
    if (
      await client.send((revision) => ({
        type: 'set_provider_settings',
        device_id: device.id,
        changes: { [id]: value },
        expected_revision: revision,
      }))
    )
      await refresh();
  }
  function lightingSetting(id: string) {
    return (
      definition?.settings.some(
        (s) =>
          s.id === id && (s.group === 'lighting' || groups.some((g) => g.settings?.includes(id))),
      ) || groups.some((g) => g.staticColors?.settingId === id)
    );
  }
  async function savePreset() {
    const settings = Object.fromEntries(
      Object.entries({ ...status?.observed, ...device.provider_settings }).filter(([id]) =>
        lightingSetting(id),
      ),
    );
    if (staticGroup?.staticColors) {
      const id = staticGroup.staticColors.settingId;
      const requested = observedStaticColors(device.provider_settings[id], writableStaticIds);
      const colors = { ...colorCoverage, ...requested };
      if (Object.keys(colors).length) settings[id] = colors;
      else delete settings[id];
    }
    if (
      await client.send((revision) => ({
        type: 'save_lighting_preset',
        device_id: device.id,
        preset: { id: crypto.randomUUID().replaceAll('-', ''), name: presetName.trim(), settings },
        expected_revision: revision,
      }))
    )
      presetName = '';
  }
  function choose(id: string) {
    if (!connected || !Object.hasOwn(colorCoverage, id)) return;
    chosen = chosen.includes(id) ? chosen.filter((k) => k !== id) : [...chosen, id];
  }
  async function setColor() {
    if (!staticGroup?.staticColors || !selectedColorsWritable) return;
    const value = {
      r: parseInt(color.slice(1, 3), 16),
      g: parseInt(color.slice(3, 5), 16),
      b: parseInt(color.slice(5, 7), 16),
    };
    await set(
      staticGroup.staticColors.settingId,
      Object.fromEntries(chosen.map((id) => [id, value])),
    );
  }
</script>

<div class="provider-settings-workspace" style:padding={keyboardStatusOnly ? '0' : undefined}>
  {#if !keyboardStatusOnly}<header>
      <div>
        <h2>
          {lighting
            ? ru
              ? 'Освещение'
              : 'Lighting'
            : ru
              ? 'Настройки устройства'
              : 'Device settings'}
        </h2>
        <p>{device.name}</p>
      </div>
      <button disabled={loading || client.busy} onclick={() => void refresh(true)}
        >{ru ? 'Прочитать с устройства' : 'Read device state'}</button
      >
    </header>{/if}
  <ProviderRecovery {client} {device} recovery={status?.recovery} oncomplete={() => refresh()} />
  {#if error || status?.error}<p class="connection-error" role="alert">
      {error || status?.error}
    </p>{/if}
  {#if !keyboardStatusOnly}
    {#if !status}<p>
        {ru
          ? 'Нет активного соединения. Сохранённые значения не подтверждают состояние устройства.'
          : 'No active connection. Saved values do not confirm device state.'}
      </p>{/if}
    {#if lighting}
      <div class="lighting-owner" aria-live="polite">
        <strong
          >{ru ? 'Управление освещением' : 'Lighting source'}: {lightingState?.source === 'action'
            ? ru
              ? 'Действие'
              : 'Action'
            : lightingState?.source === 'profile'
              ? ru
                ? 'Профиль'
                : 'Profile'
              : ru
                ? 'Ручное'
                : 'Manual'}</strong
        >
        {#if lightingState?.manual_editing}<span
            >{ru
              ? 'Редактирование защищено от автоматики'
              : 'Manual editing owns this output'}</span
          ><button
            onclick={() =>
              void client.send(() => ({ type: 'release_lighting_edit', device_id: device.id }))}
            >{ru ? 'Завершить редактирование' : 'Finish editing'}</button
          >{/if}
        <button
          onclick={() =>
            void client.send(() => ({ type: 'return_to_manual_lighting', device_id: device.id }))}
          >{ru ? 'Вернуться к ручному' : 'Return to manual'}</button
        >
      </div>
      {#if lightingState?.error}<p class="connection-error" role="status">
          {lightingState.error}
        </p>{/if}
      <div class="lighting-presets">
        <h3>{ru ? 'Пресеты этого устройства' : 'Device lighting presets'}</h3>
        <div class="preset-save">
          <input
            maxlength="80"
            bind:value={presetName}
            placeholder={ru ? 'Название нового пресета' : 'New preset name'}
            aria-label={ru ? 'Название пресета' : 'Preset name'}
          /><button disabled={!presetName.trim() || client.busy} onclick={() => void savePreset()}
            >{ru ? 'Сохранить текущие настройки' : 'Save current settings'}</button
          >
        </div>
        {#each device.lighting_presets as preset (preset.id)}
          <div class="preset-row">
            <input
              maxlength="80"
              aria-label={ru ? 'Название пресета' : 'Preset name'}
              value={preset.name}
              onchange={(e) => {
                const name = e.currentTarget.value;
                void client.send((revision) => ({
                  type: 'save_lighting_preset',
                  device_id: device.id,
                  preset: { ...preset, name },
                  expected_revision: revision,
                }));
              }}
            /><button
              class:current={device.manual_lighting_preset === preset.id}
              disabled={!connected || client.busy || unsupportedPreset(preset.settings)}
              onclick={() =>
                void client.send((revision) => ({
                  type: 'select_lighting_preset',
                  device_id: device.id,
                  preset_id: preset.id,
                  expected_revision: revision,
                }))}>{ru ? 'Выбрать' : 'Select'}</button
            ><button onclick={() => (deletingPreset = preset.id)}
              >{ru ? 'Удалить…' : 'Delete…'}</button
            >
            {#if connected && unsupportedPreset(preset.settings)}<small class="connection-error"
                >{ru
                  ? 'Пресет содержит индивидуальные цвета клавиш с неподтверждёнными адресами. Выбор заблокирован.'
                  : 'This preset includes individual colors for unconfirmed key addresses. Selection is blocked.'}</small
              >{/if}
            {#if deletingPreset === preset.id}<p>
                {ru
                  ? 'Удалить пресет? Ссылки в действиях и профилях останутся видны как отсутствующий пресет.'
                  : 'Delete this preset? Actions and profiles will retain a visible missing-preset reference.'}
              </p>
              <button onclick={() => (deletingPreset = null)}>{ru ? 'Отмена' : 'Cancel'}</button
              ><button
                onclick={() => {
                  const id = preset.id;
                  void client
                    .send((revision) => ({
                      type: 'delete_lighting_preset',
                      device_id: device.id,
                      preset_id: id,
                      expected_revision: revision,
                    }))
                    .then((ok) => {
                      if (ok) deletingPreset = null;
                    });
                }}>{ru ? 'Удалить пресет' : 'Delete preset'}</button
              >{/if}
          </div>
        {/each}
        <label class="profile-lighting"
          >{ru ? 'Освещение профиля' : 'Profile lighting'}: {device.profile.name}<select
            value={device.profile.lighting_preset ?? ''}
            onchange={(e) => {
              const preset_id = e.currentTarget.value || null;
              void client.send((revision) => ({
                type: 'profile_lighting_preset',
                device_id: device.id,
                preset_id,
                expected_revision: revision,
              }));
            }}
            ><option value="">{ru ? 'Не менять освещение' : 'Keep current lighting'}</option
            >{#if device.profile.lighting_preset && !device.lighting_presets.some((p) => p.id === device.profile.lighting_preset)}<option
                disabled
                value={device.profile.lighting_preset}
                >{ru ? 'Пресет отсутствует' : 'Preset missing'}</option
              >{/if}{#each device.lighting_presets as preset}<option
                value={preset.id}
                disabled={connected && unsupportedPreset(preset.settings)}>{preset.name}</option
              >{/each}</select
          ></label
        >
        <small
          >{ru
            ? 'Привязанный пресет запрашивается при переходе на этот профиль. Во время ручного редактирования такие запросы отклоняются и не запускаются позже.'
            : 'An associated preset is requested on switching to this profile. Manual editing rejects competing requests without later replay.'}</small
        >
      </div>
    {/if}
    {#if lighting && geometry}
      <p>
        {ru
          ? 'Режимы ниже управляют освещением устройства целиком. Превью показывает расположение клавиш; фактические цвета проверяются на устройстве.'
          : 'The modes below control the whole device. The preview shows key positions; actual colors must be checked on hardware.'}
      </p>
      <div
        class="keyboard-lighting-preview"
        style:aspect-ratio={geometry.width + '/' + geometry.height}
      >
        <DeviceDecorations {definition} {geometry} locale={ru ? 'ru' : 'en'} />
        {#each geometry.controls.filter((c) => c.kind === 'key') as control (control.id)}
          {@const keyRgb = keyColor(control.id)}
          {@const depth = travel ? client.depth(control.id, travel.maxUm) : null}
          <button
            class:selected={chosen.includes(control.id)}
            class:depth-pressed={!!depth && depth.um >= 300}
            disabled={!connected || !Object.hasOwn(colorCoverage, control.id)}
            title={keyLabel(control.id, control.number) +
              (depth ? ' · ' + (depth.um / 1000).toFixed(2) + (ru ? ' мм' : ' mm') : '')}
            aria-label={keyLabel(control.id, control.number)}
            style:background={keyRgb?.background}
            style:color={keyRgb?.foreground}
            aria-pressed={chosen.includes(control.id)}
            style:left={(control.rect[0] / geometry.width) * 100 + '%'}
            style:top={(control.rect[1] / geometry.height) * 100 + '%'}
            style:width={(control.rect[2] / geometry.width) * 100 + '%'}
            style:height={(control.rect[3] / geometry.height) * 100 + '%'}
            onclick={() => choose(control.id)}
            ><DepthRing reading={depth} />{keyLabel(control.id, control.number)}</button
          >
        {/each}
      </div>
      {#if staticGroup}<p class="static-coverage" role="status">
          {ru ? 'Считанные адреса индивидуального цвета' : 'Individual color addresses read'}: {writableStaticIds.length}
          / {staticControlIds.length}. {ru
            ? 'Для остальных клавиш адреса не подтверждены; они недоступны для индивидуального цвета. Общие эффекты и яркость всей клавиатуры доступны ниже.'
            : 'Other key addresses are unconfirmed and unavailable for individual colors. Whole-keyboard effects and brightness remain available below.'}
        </p>
        <div class="static-key-tools">
          <button
            onclick={() => (chosen = [...writableStaticIds])}
            disabled={!connected || !writableStaticIds.length}
            >{ru ? 'Выбрать доступные клавиши' : 'Select available keys'}</button
          ><button onclick={() => (chosen = [])}
            >{ru ? 'Снять выделение' : 'Clear selection'}</button
          ><label
            >{ru ? 'Цвет группы' : 'Group color'} <input type="color" bind:value={color} /></label
          ><button disabled={!selectedColorsWritable || client.busy} onclick={() => void setColor()}
            >{ru ? 'Задать цвет выбранным' : 'Set selected key color'} ({chosen.length})</button
          >
        </div>{/if}
    {/if}
    <div class="provider-settings-grid">
      {#each settings as setting (setting.id)}
        <div class="provider-setting">
          <label for={'provider-' + setting.id}
            >{label(setting)}<small
              >{setting.persistence === 'onboard'
                ? ru
                  ? 'Сохраняется в устройстве'
                  : 'Stored on device'
                : ru
                  ? 'Временная настройка'
                  : 'Volatile setting'}</small
            ></label
          >
          {#if setting.type === 'boolean'}<select
              id={'provider-' + setting.id}
              disabled={client.busy || !connected}
              value={typeof current(setting) === 'boolean' ? String(current(setting)) : ''}
              onchange={(e) => void set(setting.id, e.currentTarget.value === 'true')}
              ><option value="" disabled>{ru ? 'Неизвестно' : 'Unknown'}</option><option
                value="true">{ru ? 'Включено' : 'On'}</option
              ><option value="false">{ru ? 'Выключено' : 'Off'}</option></select
            >
          {:else if setting.type === 'number'}<input
              id={'provider-' + setting.id}
              type="number"
              min={setting.min}
              max={setting.max}
              step={setting.step ?? 1}
              value={typeof current(setting) === 'number' ? Number(current(setting)) : undefined}
              placeholder={ru ? 'Неизвестно' : 'Unknown'}
              disabled={client.busy || !connected}
              onchange={(e) => {
                if (e.currentTarget.value !== '' && e.currentTarget.validity.valid)
                  void set(setting.id, e.currentTarget.valueAsNumber);
              }}
            />
          {:else}<select
              id={'provider-' + setting.id}
              disabled={client.busy || !connected}
              value={JSON.stringify(current(setting)) ?? ''}
              onchange={(e) => void set(setting.id, JSON.parse(e.currentTarget.value))}
              ><option value="" disabled>{ru ? 'Неизвестно' : 'Unknown'}</option
              >{#each setting.choices ?? [] as choice}{@const value =
                  typeof choice === 'object' ? choice.value : choice}<option
                  value={JSON.stringify(value)}
                  >{typeof choice === 'object'
                    ? (choice.label?.[locale] ?? choice.label?.en ?? String(value))
                    : String(choice)}</option
                >{/each}</select
            >{/if}
          <small class="setting-evidence"
            >{status && Object.hasOwn(status.observed ?? {}, setting.id)
              ? ru
                ? 'Считано с устройства'
                : 'Read from device'
              : Object.hasOwn(device.provider_settings, setting.id)
                ? ru
                  ? 'Сохранено; чтение не подтверждено'
                  : 'Saved; readback unconfirmed'
                : ru
                  ? 'Ещё не запрашивалось'
                  : 'Not requested'}</small
          >
        </div>
      {/each}
    </div>
    {#if !settings.length}<p>
        {ru
          ? 'Поддерживаемые настройки не заявлены устройством.'
          : 'This device declares no supported settings here.'}
      </p>{/if}
    <p class="provider-evidence">
      {ru
        ? 'Отправка команды и обратное чтение не подтверждают видимый результат. Открытие этой вкладки не меняет настройки.'
        : 'Transport and readback do not confirm visible hardware behavior. Opening this tab does not change settings.'}
    </p>
  {/if}
</div>

<style>
  .keyboard-lighting-preview button.depth-pressed {
    opacity: 1;
    border-color: #77e2ff;
    box-shadow: inset 0 0 9px #77e2ff40;
  }
  .lighting-owner,
  .preset-save,
  .preset-row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    margin: 10px 0;
  }
  .lighting-presets {
    padding: 12px;
    border: 1px solid #ffffff20;
    border-radius: 8px;
    margin: 14px 0;
  }
  .lighting-presets small {
    opacity: 0.7;
  }
  .preset-row p {
    width: 100%;
    font-size: 12px;
  }
  .profile-lighting {
    display: flex;
    gap: 12px;
    align-items: center;
    margin: 14px 0;
  }
  .preset-save input,
  .preset-row input {
    flex: 1;
    min-width: 160px;
  }
  .lighting-owner span {
    font-size: 12px;
    opacity: 0.7;
  }
  .provider-settings-workspace {
    padding: 24px;
    overflow: auto;
    min-height: 0;
  }
  .provider-settings-workspace header {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    align-items: center;
    margin-bottom: 20px;
  }
  .provider-settings-workspace h2 {
    margin: 0;
  }
  .provider-settings-workspace p {
    line-height: 1.5;
  }
  .provider-settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 14px;
  }
  .provider-setting {
    display: grid;
    grid-template-columns: minmax(100px, 1fr) minmax(90px, 120px);
    gap: 8px;
    padding: 12px;
    border: 1px solid #ffffff20;
    border-radius: 8px;
  }
  .provider-setting label small {
    display: block;
    opacity: 0.65;
    margin-top: 4px;
  }
  .setting-evidence {
    grid-column: 1/-1;
    opacity: 0.7;
  }
  .provider-evidence {
    opacity: 0.65;
    font-size: 12px;
  }
  .keyboard-lighting-preview {
    position: relative;
    max-width: 1000px;
    width: 100%;
    margin: 24px 0;
  }
  .keyboard-lighting-preview button {
    position: absolute;
    border: 2px solid #222;
    background: #383c46;
    color: #ddd;
    border-radius: 6px;
    padding: 2px;
    font-size: 11px;
    min-height: 0;
  }
  .keyboard-lighting-preview button.selected {
    background: #346ca6;
    border-color: #8ec8ff;
  }
  .keyboard-lighting-preview button:disabled {
    opacity: 0.75;
  }
  .static-coverage {
    font-size: 12px;
    line-height: 1.5;
  }
  .static-key-tools {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    align-items: center;
    margin-bottom: 20px;
  }
  .static-key-tools label {
    display: flex;
    gap: 8px;
    align-items: center;
  }
</style>
