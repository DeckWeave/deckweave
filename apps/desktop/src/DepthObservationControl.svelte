<script lang="ts">
  import type { Client } from './client.svelte';
  import type { Locale } from './types';
  import Icon from './Icon.svelte';
  let {
    client,
    locale,
    normalTyping,
    connected,
  }: { client: Client; locale: Locale; normalTyping: boolean; connected: boolean } = $props();
  const ru = $derived(locale === 'ru');
  const active = $derived(client.observation.enabled);
  const failed = $derived(client.observation.state === 'error');
  const title = $derived(
    client.observation.error ||
      (!connected && !active
        ? ru
          ? 'Подключите сохранённое устройство через Настройки → Устройства → Обнаруженное оборудование.'
          : 'Connect the saved device through Settings → Devices → Discovered hardware.'
        : !normalTyping
          ? ru
            ? 'Драйвер не подтверждает наблюдение без перехвата печати.'
            : 'The driver does not declare observation without taking over typing.'
          : ru
            ? `Наблюдать глубину нажатия в этом редакторе. Обычная печать сохраняется. Шкала круга — ${client.depthScaleUm / 1000} мм; фактическое значение в мм не ограничивается шкалой. Нет свежих данных — нет индикатора. Полнота данных зависит от драйвера.`
            : `Observe key travel in this editor. Normal typing continues. Ring scale is ${client.depthScaleUm / 1000} mm; actual millimetre readings are not clipped. No fresh sample means no indicator. Coverage depends on the driver.`),
  );
</script>

<div class="depth-tools">
  <button
    class:active
    class:failed
    aria-pressed={active}
    {title}
    disabled={client.observationBusy || ((!normalTyping || !connected) && !active)}
    onclick={() => void client.setObservation(!active)}
  >
    <Icon name="dial" />
    {ru ? 'Глубина' : 'Key travel'}{failed ? ' !' : active ? ' ●' : ''}
  </button>
  <select
    aria-label={ru ? 'Шкала индикатора глубины' : 'Key travel display scale'}
    title={ru
      ? 'Меняет только шкалу кругового индикатора. Показания в миллиметрах остаются фактическими.'
      : 'Changes only the ring display scale. Millimetre readings remain unchanged.'}
    value={client.depthScaleUm}
    onchange={(e) => {
      client.depthScaleUm = Number(e.currentTarget.value) === 4000 ? 4000 : 3200;
    }}
    ><option value={3200}>{ru ? 'Шкала 3,2 мм' : 'Scale 3.2 mm'}</option><option value={4000}
      >{ru ? 'Шкала 4,0 мм' : 'Scale 4.0 mm'}</option
    ></select
  >
</div>

<style>
  .depth-tools {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  select {
    padding: 5px 7px;
    font-size: 11px;
    max-width: 130px;
  }

  button {
    padding: 5px 9px;
    font-size: 12px;
    white-space: nowrap;
  }
  button.active {
    border-color: var(--focus);
    background: var(--selection);
  }
  button.failed {
    border-color: #d79d58;
  }
</style>
