<script lang="ts">
  let { reading }: { reading: { um: number; fraction: number } | null } = $props();
</script>

{#if reading}
  <span class="depth-ring" aria-hidden="true">
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
    inset: 2px;
    display: flex;
    justify-content: flex-end;
    align-items: flex-end;
    pointer-events: none;
  }
  svg {
    width: min(42%, 25px);
    height: min(72%, 25px);
    overflow: visible;
  }
  circle {
    fill: none;
    stroke-width: 3.5;
  }
  .track {
    stroke: #ffffff24;
  }
  .fill {
    stroke: #77e2ff;
    stroke-linecap: round;
    transform: rotate(-90deg);
    transform-origin: center;
  }
</style>
