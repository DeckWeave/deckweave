<script lang="ts">
  import type { Client } from './client.svelte';
  import type { Locale } from './types';
  let {
    client,
    locale,
    maxUm,
    normalTyping,
    connected,
  }: { client: Client; locale: Locale; maxUm: number; normalTyping: boolean; connected: boolean } =
    $props();
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
            ? `Наблюдать глубину нажатия в этом редакторе. Обычная печать сохраняется. Круг показывает номинальный ход 0–${maxUm / 1000} мм; нет свежих данных — нет индикатора. Полнота данных зависит от драйвера.`
            : `Observe key travel in this editor. Normal typing continues. Rings show nominal 0–${maxUm / 1000} mm travel; no fresh sample means no indicator. Coverage depends on the driver.`),
  );
</script>

<button
  class:active
  class:failed
  aria-pressed={active}
  {title}
  disabled={client.observationBusy || ((!normalTyping || !connected) && !active)}
  onclick={() => void client.setObservation(!active)}
>
  <span aria-hidden="true">◎</span>
  {ru ? 'Глубина' : 'Key travel'}{failed ? ' !' : active ? ' ●' : ''}
</button>

<style>
  button {
    padding: 5px 9px;
    font-size: 12px;
    white-space: nowrap;
  }
  button.active {
    border-color: #73c8ea;
    background: #214450;
  }
  button.failed {
    border-color: #d79d58;
  }
</style>
