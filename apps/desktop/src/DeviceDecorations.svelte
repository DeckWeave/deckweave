<script lang="ts">
  import type { Locale, ProviderDefinition } from './types';
  let {
    definition,
    geometry,
    locale = 'en',
  }: {
    definition?: ProviderDefinition;
    geometry: { width: number; height: number };
    locale?: Locale;
  } = $props();
  const decorations = $derived(
    (definition?.decorations ?? []).filter(
      (d) =>
        d.kind === 'light-strip' &&
        [d.x, d.y, d.width, d.height].every(Number.isFinite) &&
        d.x >= 0 &&
        d.y >= 0 &&
        d.width > 0 &&
        d.height > 0 &&
        d.x + d.width <= geometry.width &&
        d.y + d.height <= geometry.height,
    ),
  );
  const note = $derived(
    locale === 'ru'
      ? 'Физическая LED-панель. Текущие цвета и прямое управление пока недоступны.'
      : 'Physical LED panel. Current colors and direct control are not available yet.',
  );
</script>

{#each decorations as decoration (decoration.id)}
  <div
    class="device-light-strip"
    role="img"
    aria-label={(decoration.label?.[locale] ?? decoration.label?.en ?? 'LED') + '. ' + note}
    title={note}
    style:left={(decoration.x / geometry.width) * 100 + '%'}
    style:top={(decoration.y / geometry.height) * 100 + '%'}
    style:width={(decoration.width / geometry.width) * 100 + '%'}
    style:height={(decoration.height / geometry.height) * 100 + '%'}
  >
    <span></span>
  </div>
{/each}

<style>
  .device-light-strip {
    position: absolute;
    box-sizing: border-box;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 2px;
    background: var(--inset);
    box-shadow: inset 0 0 3px #0008;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }
  .device-light-strip span {
    width: 78%;
    height: 38%;
    min-height: 2px;
    border-radius: 2px;
    background: repeating-linear-gradient(90deg, var(--text) 0 4%, var(--raised) 4% 7%);
    opacity: 0.7;
  }
</style>
