<script lang="ts">
  let {
    reading,
    inline = false,
  }: { reading: { um: number; fraction: number } | null; inline?: boolean } = $props();
</script>

{#if reading}
  <span class="depth-ring" class:inline aria-hidden="true">
    <svg viewBox="0 0 36 36">
      <circle class="track" cx="18" cy="18" r="15" />
      <circle
        class="fill"
        cx="18"
        cy="18"
        r="15"
        pathLength="100"
        stroke-dasharray={`${reading.fraction * 100} 100`}
      />
    </svg>
  </span>
{/if}

<style>
  .depth-ring {
    position: absolute;
    inset: var(--depth-ring-inset, 2px);
    display: flex;
    justify-content: flex-end;
    align-items: flex-end;
    pointer-events: none;
  }
  svg {
    width: var(--depth-ring-size, clamp(12px, 38%, 24px));
    height: auto;
    aspect-ratio: 1;
    flex: none;
    overflow: visible;
  }
  .depth-ring.inline {
    position: static;
    width: 26px;
    height: 26px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .inline svg {
    width: 26px;
    height: 26px;
  }
  circle {
    fill: none;
    stroke-width: 3.5;
  }
  .track {
    stroke: color-mix(in srgb, var(--text) 25%, transparent);
  }
  .fill {
    stroke: var(--depth);
    stroke-linecap: round;
    transform: rotate(-90deg);
    transform-origin: center;
  }
</style>
