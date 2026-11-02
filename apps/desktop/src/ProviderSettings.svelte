<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import type { Client } from './client.svelte';
  import ProviderRecovery from './ProviderRecovery.svelte';
  import DeviceDecorations from './DeviceDecorations.svelte';
  import DepthRing from './DepthRing.svelte';
  import ProviderColorControl from './ProviderColorControl.svelte';
  import ProviderSettingControl from './ProviderSettingControl.svelte';
  import {
    conditionMatches,
    colorVisible,
    colorHex,
    colorChanges,
    colorForeground,
    patchMatches,
  } from './lighting-ui';
  import { recoveryErrorText } from './provider-recovery';
  import { observedStaticColors, unsupportedStaticColors } from './static-color-coverage';
  import type { Device, ProviderSetting } from './types';
  import type { ProviderSessionState } from './provider-ui';
  import {
    ProviderEditQueue,
    type ProviderEditState,
    type SettingsPatch,
  } from './provider-edit-queue';
  let {
    client,
    device,
    lighting = false,
  }: { client: Client; device: Device; lighting?: boolean } = $props();
  let status = $state<ProviderSessionState | null>(null);
  let reading = $state(false);
  let disposed = false;
  let statusFlight: Promise<ProviderSessionState | null> | null = null;
  let idleTimer: ReturnType<typeof setTimeout> | undefined;
  let error = $state('');
  let presetName = $state('');
  let deletingPreset = $state<string | null>(null);
  const lightingState = $derived(client.snapshot?.lighting?.[device.id]);
  let chosen = $state<string[]>([]);
  let color = $state('#ffffff');
  let keyPreview = $state<string | null>(null);
  let edits = $state<ProviderEditState>({ pending: false, desired: {}, error: null });
  let writer = new ProviderEditQueue(commitPatch, (next) => {
    edits = next;
  });
  let colorPreview = $state<Record<string, string>>({});
  let colorTarget = $state<'effect' | 'keys'>('effect');
  let selectedKey = $state<string | null>(null);
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
  const observed = $derived(status?.observed ?? {});
  const staticGroup = $derived(groups.find((g) => g.staticColors?.address === 'controlId'));
  const staticModeAvailable = $derived(
    conditionMatches(staticGroup?.staticColors?.visibleWhen, observed),
  );
  const staticModeLabel = $derived.by(() => {
    const condition = staticGroup?.staticColors?.visibleWhen;
    const setting = definition?.settings.find((s) => s.id === condition?.settingId);
    if (!condition || !setting) return '';
    return (setting.choices ?? [])
      .filter((c) => condition.values.includes(typeof c === 'object' ? c.value : c))
      .map((c) =>
        typeof c === 'object' ? (c.label?.[locale] ?? c.label?.en ?? String(c.value)) : String(c),
      )
      .join(', ');
  });
  const colorControls = $derived(groups.flatMap((g) => g.colorChannels ?? []));
  const channelIds = $derived(new Set(colorControls.flatMap((c) => [c.r, c.g, c.b])));
  const visibleSettings = $derived(
    settings.filter((s) => !channelIds.has(s.id) && conditionMatches(s.visibleWhen, observed)),
  );
  const visibleColors = $derived(colorControls.filter((c) => colorVisible(c, settings, observed)));
  const sampleColor = $derived(
    colorTarget === 'effect' && visibleColors.length
      ? (colorPreview[visibleColors[0].id] ??
          colorHex(visibleColors[0], { ...observed, ...edits.desired }))
      : null,
  );
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
    connected &&
      staticModeAvailable &&
      chosen.length > 0 &&
      chosen.every((id) => Object.hasOwn(colorCoverage, id)),
  );
  const structuralPending = $derived(
    edits.pending &&
      Object.keys(edits.desired).some(
        (id) => !channelIds.has(id) && id !== staticGroup?.staticColors?.settingId,
      ),
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
  function readStatus(): Promise<ProviderSessionState | null> {
    if (statusFlight) return statusFlight;
    const requestDevice = device.id;
    statusFlight = invoke<ProviderSessionState | null>('device_provider_session', {
      deviceId: requestDevice,
    })
      .then((next) => {
        if (!disposed && device.id === requestDevice) status = next;
        return next;
      })
      .finally(() => {
        statusFlight = null;
      });
    return statusFlight;
  }
  const pause = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
  async function commitPatch(changes: SettingsPatch, cancelled: () => boolean) {
    const requestDevice = device.id;
    const baseline = await readStatus();
    if (cancelled()) return;
    if (
      !baseline ||
      baseline.state !== 'connected' ||
      baseline.error ||
      baseline.recovery?.state === 'pending'
    )
      throw new Error(
        ru ? 'Устройство недоступно для изменения.' : 'The device is unavailable for editing.',
      );
    if (
      !(await client.send(
        (revision) => ({
          type: 'set_provider_settings',
          device_id: requestDevice,
          changes,
          expected_revision: revision,
        }),
        () => !cancelled() && device.id === requestDevice,
      ))
    )
      throw new Error(
        client.error || (ru ? 'Изменение не принято.' : 'The edit was not accepted.'),
      );
    const deadline = performance.now() + 5500;
    while (!cancelled() && performance.now() < deadline) {
      const next = await readStatus();
      if (cancelled()) return;
      if (!next || next.generation !== baseline.generation)
        throw new Error(
          ru
            ? 'Подключение изменилось. Неотправленные изменения отменены.'
            : 'The connection changed. Unsent edits were cancelled.',
        );
      if (next.error || next.state !== 'connected' || next.recovery?.state === 'pending')
        throw new Error(
          next.error ||
            (ru
              ? 'Запись не подтверждена. Проверьте подключение.'
              : 'The write was not confirmed. Check the connection.'),
        );
      if (
        next.settings_edit !== baseline.settings_edit &&
        patchMatches(changes, next.requested ?? {}) &&
        patchMatches(changes, next.observed ?? {})
      )
        return;
      await pause(50);
    }
    if (!cancelled())
      throw new Error(
        ru
          ? 'Запись ещё не подтверждена. Следующие изменения не отправлены; прочитайте состояние устройства.'
          : 'The write is unconfirmed. Further edits were not sent; read device state.',
      );
  }
  async function refresh(read = false) {
    if (!client.available || disposed || (read && reading)) return;
    const requestDevice = device.id;
    if (!read) {
      try {
        await readStatus();
      } catch (e) {
        if (!disposed) error = String(e);
      }
      return;
    }
    reading = true;
    error = '';
    try {
      const baseline = await readStatus();
      if (disposed || device.id !== requestDevice) return;
      await invoke('refresh_provider_state', { deviceId: requestDevice });
      const deadline = performance.now() + 12000;
      while (!disposed && performance.now() < deadline) {
        const next = await readStatus();
        if (disposed || device.id !== requestDevice) return;
        if (!next || next.generation !== baseline?.generation)
          throw new Error(ru ? 'Подключение изменилось.' : 'The connection changed.');
        if (next.error) throw new Error(next.error);
        if (next.read_sequence !== baseline?.read_sequence) {
          if (next.state === 'connected' && next.recovery?.state !== 'pending') {
            writer.dispose();
            edits = { pending: false, desired: {}, error: null };
            writer = new ProviderEditQueue(commitPatch, (state) => {
              edits = state;
            });
          }
          return;
        }
        await pause(75);
      }
      if (!disposed)
        throw new Error(
          ru ? 'Устройство не подтвердило чтение.' : 'Device read completion was not confirmed.',
        );
    } catch (e) {
      if (!disposed) error = String(e);
    } finally {
      if (!disposed) reading = false;
    }
  }
  onMount(() => {
    const poll = async () => {
      await refresh();
      if (!disposed) idleTimer = setTimeout(() => void poll(), 1000);
    };
    void poll();
    return () => {
      disposed = true;
      clearTimeout(idleTimer);
      writer.dispose();
      if (lighting)
        void client.send(() => ({ type: 'release_lighting_edit', device_id: device.id }));
    };
  });
  function current(s: ProviderSetting): unknown {
    return status ? status.observed?.[s.id] : device.provider_settings[s.id];
  }
  async function setChanges(changes: Record<string, unknown>) {
    if (!connected || reading || disposed) return;
    error = '';
    writer.enqueue(changes);
  }
  async function set(id: string, value: unknown) {
    await setChanges({ [id]: value });
  }
  function lightingSetting(id: string) {
    const setting = settings.find((s) => s.id === id);
    if (setting)
      return (
        conditionMatches(setting.visibleWhen, observed) &&
        (!channelIds.has(id) || visibleColors.some((c) => [c.r, c.g, c.b].includes(id)))
      );
    return groups.some(
      (g) =>
        g.staticColors?.settingId === id && conditionMatches(g.staticColors.visibleWhen, observed),
    );
  }
  async function savePreset() {
    if (edits.pending || edits.error || !connected) return;
    const settings = Object.fromEntries(
      Object.entries(status?.observed ?? {}).filter(([id]) => lightingSetting(id)),
    );
    if (staticGroup?.staticColors && staticModeAvailable) {
      const id = staticGroup.staticColors.settingId;
      const colors = { ...colorCoverage };
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
    selectedKey = id;
    if (colorTarget !== 'keys') return;
    if (!connected || !staticModeAvailable || !Object.hasOwn(colorCoverage, id)) return;
    if (!chosen.length) {
      const rgb = colorCoverage[id];
      color = '#' + [rgb.r, rgb.g, rgb.b].map((v) => v.toString(16).padStart(2, '0')).join('');
    }
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
      <button disabled={reading || edits.pending || !status} onclick={() => void refresh(true)}
        >{reading
          ? ru
            ? 'Чтение…'
            : 'Reading…'
          : ru
            ? 'Прочитать с устройства'
            : 'Read device state'}</button
      >
    </header>{/if}
  <ProviderRecovery
    {client}
    {device}
    recovery={status?.recovery}
    oncomplete={() => refresh(true)}
  />
  {#if error || edits.error || status?.error}<p class="connection-error" role="alert">
      {recoveryErrorText(error || edits.error || status?.error, ru)}
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
            disabled={edits.pending || reading}
            onclick={() =>
              void client.send(() => ({ type: 'release_lighting_edit', device_id: device.id }))}
            >{ru ? 'Завершить редактирование' : 'Finish editing'}</button
          >{/if}
        <button
          disabled={edits.pending || reading}
          onclick={() =>
            void client.send(() => ({ type: 'return_to_manual_lighting', device_id: device.id }))}
          >{ru ? 'Вернуться к ручному' : 'Return to manual'}</button
        >
      </div>
      {#if lightingState?.error}<p class="connection-error" role="status">
          {lightingState.error}
        </p>{/if}
    {/if}
    <div class:lighting-workbench={lighting}>
      {#if lighting && geometry}
        <section class="lighting-canvas">
          <div class="lighting-preview-heading">
            <div
              class="lighting-target"
              role="group"
              aria-label={ru ? 'Область настройки цвета' : 'Color target'}
            >
              <button
                class:current={colorTarget === 'effect'}
                onclick={() => {
                  colorTarget = 'effect';
                  chosen = [];
                }}>{ru ? 'Эффект клавиатуры' : 'Keyboard effect'}</button
              >
              {#if staticGroup}<button
                  class:current={colorTarget === 'keys'}
                  onclick={() => (colorTarget = 'keys')}
                  >{ru ? 'Цвета клавиш' : 'Individual keys'}</button
                >{/if}
            </div>
            <span
              >{selectedKey
                ? keyLabel(selectedKey, 0)
                : ''}{#if selectedKey && travel}{@const reading = client.depth(
                  selectedKey,
                  client.depthScaleUm,
                )} · {reading ? (reading.um / 1000).toFixed(2) : '—'}
                {ru ? 'мм' : 'mm'}{/if}</span
            >
          </div>
          <div class="keyboard-lighting-scroll">
            <div
              class="keyboard-lighting-preview"
              style:aspect-ratio={geometry.width + '/' + geometry.height}
            >
              <DeviceDecorations {definition} {geometry} locale={ru ? 'ru' : 'en'} />
              {#each geometry.controls.filter((c) => c.kind === 'key') as control (control.id)}
                {@const keyRgb = keyColor(control.id)}
                {@const depth = travel ? client.depth(control.id, client.depthScaleUm) : null}
                <button
                  class:selected={chosen.includes(control.id) || selectedKey === control.id}
                  class:depth-pressed={!!depth && depth.um >= 300}
                  aria-disabled={colorTarget === 'keys' &&
                    (!connected ||
                      !staticModeAvailable ||
                      !Object.hasOwn(colorCoverage, control.id))}
                  title={keyLabel(control.id, control.number) +
                    (depth ? ' · ' + (depth.um / 1000).toFixed(2) + (ru ? ' мм' : ' mm') : '')}
                  aria-label={keyLabel(control.id, control.number)}
                  style:background={colorTarget === 'keys' && chosen.includes(control.id)
                    ? (keyPreview ?? color)
                    : (sampleColor ?? keyRgb?.background)}
                  style:color={colorTarget === 'keys' && chosen.includes(control.id)
                    ? colorForeground(keyPreview ?? color)
                    : sampleColor
                      ? colorForeground(sampleColor)
                      : keyRgb?.foreground}
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
          </div>
          <p class="preview-caption">
            {sampleColor
              ? ru
                ? 'Образец выбранного цвета; анимация эффекта воспроизводится на клавиатуре.'
                : 'Selected color sample; the keyboard renders the effect animation.'
              : ru
                ? 'Расположение клавиш и наблюдение глубины. Текущая анимация на превью не воспроизводится.'
                : 'Key layout and live travel. The current effect animation is not simulated.'}
          </p>
        </section>
      {/if}
      <section class:lighting-options={lighting}>
        {#if staticGroup && colorTarget === 'keys'}
          {#if !staticModeAvailable}<p class="static-coverage">
              {ru ? 'Индивидуальные цвета доступны в режиме' : 'Individual colors require mode'}:
              <strong>{staticModeLabel || '—'}</strong>.
              <button onclick={() => (colorTarget = 'effect')}
                >{ru ? 'Выбрать эффект' : 'Choose effect'}</button
              >
            </p>{/if}
          <p class="static-coverage" role="status">
            {ru ? 'Считанные адреса индивидуального цвета' : 'Individual color addresses read'}: {writableStaticIds.length}
            / {staticControlIds.length}. {ru
              ? 'Остальные клавиши пока недоступны для индивидуального цвета. Для общего освещения выберите «Эффект клавиатуры».'
              : 'Other keys are not yet available for individual colors. Choose Keyboard effect for whole-device lighting.'}
          </p>
          <div class="static-key-tools">
            <button
              onclick={() => (chosen = [...writableStaticIds])}
              disabled={!connected || !staticModeAvailable || !writableStaticIds.length}
              >{ru ? 'Выбрать доступные клавиши' : 'Select available keys'}</button
            ><button onclick={() => (chosen = [])}
              >{ru ? 'Снять выделение' : 'Clear selection'}</button
            ><span>{ru ? 'Выбрано' : 'Selected'}: {chosen.length}</span>
          </div>
          <div class="provider-setting static-color-picker">
            <ProviderColorControl
              label={ru ? 'Цвет выбранных клавиш' : 'Selected key color'}
              value={color}
              {ru}
              disabled={!selectedColorsWritable || reading || structuralPending || !!edits.error}
              onpreview={(hex) => (keyPreview = hex)}
              onchange={(hex) => {
                color = hex;
                void setColor();
              }}
            />
          </div>{/if}
        {#if !lighting || colorTarget === 'effect'}
          <div class="provider-settings-grid" class:lighting-controls={lighting}>
            {#each visibleSettings as setting (setting.id)}
              <div class="provider-setting">
                <ProviderSettingControl
                  {setting}
                  value={current(setting)}
                  disabled={client.busy || edits.pending || reading || !!edits.error || !connected}
                  {ru}
                  onchange={(value) => void set(setting.id, value)}
                />
                {#if !lighting}<small class="setting-evidence"
                    >{status && Object.hasOwn(observed, setting.id)
                      ? ru
                        ? 'Считано с устройства'
                        : 'Read from device'
                      : ru
                        ? 'Не подтверждено чтением'
                        : 'Readback unconfirmed'}</small
                  >{/if}
              </div>
            {/each}
            {#each visibleColors as channels (channels.id)}
              <div class="provider-setting color-setting">
                <ProviderColorControl
                  label={channels.label?.[locale] ??
                    channels.label?.en ??
                    (ru ? 'Цвет эффекта' : 'Effect color')}
                  value={colorHex(channels, { ...observed, ...edits.desired })}
                  disabled={reading || structuralPending || !!edits.error || !connected}
                  {ru}
                  onpreview={(hex) => {
                    const next = { ...colorPreview };
                    if (hex) next[channels.id] = hex;
                    else delete next[channels.id];
                    colorPreview = next;
                  }}
                  onchange={(hex) => {
                    const changes = colorChanges(channels, hex);
                    if (changes) void setChanges(changes);
                  }}
                />
              </div>
            {/each}
          </div>
        {/if}
      </section>
    </div>
    {#if lighting}
      <p class="lighting-readback" role="status">
        {edits.pending
          ? ru
            ? 'Сохраняется выбранное состояние… Можно продолжать выбирать цвет.'
            : 'Saving the selected state… You can continue choosing colors.'
          : status?.recovery?.state === 'pending' || status?.error || edits.error
            ? ru
              ? 'Показаны последние считанные настройки. Результат предыдущей записи не подтверждён; изменения заблокированы.'
              : 'Showing last read settings. The previous write is unverified; editing is blocked.'
            : ru
              ? 'Настройки считываются с клавиатуры. Законченное изменение сразу записывается в устройство.'
              : 'Settings are read from the keyboard. Completed edits are written immediately.'}
      </p>
      <details class="lighting-presets">
        <summary>{ru ? 'Пресеты и освещение профиля' : 'Presets and profile lighting'}</summary>
        <h3>{ru ? 'Пресеты этого устройства' : 'Device lighting presets'}</h3>
        <div class="preset-save">
          <input
            maxlength="80"
            bind:value={presetName}
            placeholder={ru ? 'Название нового пресета' : 'New preset name'}
            aria-label={ru ? 'Название пресета' : 'Preset name'}
          /><button
            disabled={!presetName.trim() ||
              client.busy ||
              edits.pending ||
              reading ||
              !!edits.error ||
              !connected}
            onclick={() => void savePreset()}
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
              disabled={!connected ||
                client.busy ||
                edits.pending ||
                !!edits.error ||
                unsupportedPreset(preset.settings)}
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
      </details>
    {/if}

    {#if !settings.length}<p>
        {ru
          ? 'Поддерживаемые настройки не заявлены устройством.'
          : 'This device declares no supported settings here.'}
      </p>{/if}
  {/if}
</div>

<style>
  .provider-settings-workspace {
    padding: 20px 24px;
    overflow: auto;
    min-height: 0;
  }
  header {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    align-items: center;
    margin-bottom: 12px;
  }
  h2 {
    margin: 0;
    font-size: 18px;
  }
  header p {
    margin: 4px 0 0;
    font-size: 12px;
    color: #aab1c0;
  }
  p {
    line-height: 1.5;
  }
  .lighting-owner {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    font-size: 12px;
    color: #b7becd;
    margin: 8px 0 18px;
  }
  .lighting-owner strong {
    font-weight: 500;
    margin-right: auto;
  }
  .lighting-owner span {
    display: none;
  }
  .lighting-owner button {
    font-size: 11px;
    padding: 4px 8px;
    min-height: 26px;
  }
  .lighting-preview-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin: 12px 0;
  }
  .lighting-preview-heading > span {
    font-size: 14px;
    font-variant-numeric: tabular-nums;
  }
  .lighting-target {
    display: flex;
    gap: 4px;
    background: #191b21;
    padding: 3px;
    border: 1px solid #ffffff14;
    border-radius: 7px;
  }
  .lighting-target button {
    border: 0;
    background: transparent;
    min-height: 30px;
    padding: 5px 12px;
    font-size: 12px;
  }
  .lighting-target button.current {
    background: #454b69;
    color: #fff;
  }
  .keyboard-lighting-scroll {
    width: 100%;
    overflow: auto;
    padding: 10px 0;
  }
  .keyboard-lighting-preview {
    position: relative;
    min-width: 600px;
    width: 100%;
  }
  .keyboard-lighting-preview button {
    position: absolute;
    border: 1px solid #747b8b;
    background: #333844;
    color: #eef1f8;
    border-radius: 5px;
    padding: 3px;
    font-size: clamp(11px, 1vw, 15px);
    min-height: 0;
    line-height: 1.1;
    text-shadow: 0 1px 2px #0006;
  }
  .keyboard-lighting-preview button.selected {
    outline: 2px solid #c2caff;
    outline-offset: 1px;
    z-index: 1;
  }
  .keyboard-lighting-preview button[aria-disabled='true'] {
    cursor: default;
  }
  .keyboard-lighting-preview button.depth-pressed {
    border-color: #77e2ff;
    box-shadow: inset 0 0 9px #77e2ff40;
  }
  .preview-caption {
    color: #aab1c0;
    font-size: 12px;
    margin: 3px 0 20px;
  }
  .provider-settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
    gap: 14px;
    margin: 12px 0;
  }
  .provider-setting {
    border: 1px solid #ffffff16;
    background: #24272f;
    border-radius: 8px;
    padding: 16px;
    min-width: 0;
  }
  .lighting-workbench {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 330px;
    gap: 22px;
    align-items: start;
  }
  .lighting-canvas {
    min-width: 0;
  }
  .lighting-options {
    min-width: 0;
    padding-top: 12px;
  }
  .lighting-controls {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    margin: 0;
  }
  .lighting-controls .provider-setting {
    padding: 13px;
  }
  .lighting-controls .provider-setting:first-child {
    grid-column: 1/-1;
  }
  .lighting-options .color-setting {
    grid-column: 1/-1;
  }
  @media (max-width: 950px) {
    .lighting-workbench {
      grid-template-columns: minmax(0, 1fr);
    }
    .lighting-controls {
      grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    }
    .lighting-controls .provider-setting:first-child {
      grid-column: auto;
    }
  }
  .color-setting {
    grid-column: span 2;
  }
  .setting-evidence {
    display: block;
    margin-top: 8px;
    font-size: 11px;
    color: #aab1c0;
  }
  .lighting-readback {
    font-size: 12px;
    color: #acb7cd;
    margin: 12px 0 20px;
  }
  .lighting-presets {
    border-top: 1px solid #ffffff20;
    padding-top: 16px;
    margin: 20px 0 10px;
  }
  .lighting-presets summary {
    cursor: pointer;
    font-size: 13px;
    color: #c5cee5;
  }
  .lighting-presets h3 {
    font-size: 13px;
    margin: 18px 0 10px;
  }
  .lighting-presets small {
    color: #aab1c0;
    font-size: 12px;
  }
  .preset-save,
  .preset-row,
  .static-key-tools {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    margin: 10px 0;
  }
  .preset-save input,
  .preset-row input {
    flex: 1;
    min-width: 160px;
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
    font-size: 13px;
  }
  .static-coverage {
    font-size: 12px;
    color: #b5bece;
  }
  .static-color-picker {
    max-width: 400px;
  }
  @media (max-width: 850px) {
    .provider-settings-workspace {
      padding: 16px;
    }
    .color-setting {
      grid-column: auto;
    }
    .lighting-preview-heading {
      flex-wrap: wrap;
    }
  }
</style>
