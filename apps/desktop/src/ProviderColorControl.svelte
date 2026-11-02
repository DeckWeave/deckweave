<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
  import { hexHsv, hsvHex } from './lighting-ui';
  let {
    label,
    value,
    disabled = false,
    ru = false,
    onpreview,
    onchange,
  }: {
    label: string;
    value: string | null;
    disabled?: boolean;
    ru?: boolean;
    onpreview?: (value: string | null) => void;
    onchange: (value: string) => void;
  } = $props();
  let draft = $state<string | null>(null);
  const shown = $derived(draft ?? value ?? '#ffffff');
  const hsv = $derived(hexHsv(shown));
  let hue = $state<number | null>(null);
  let lastHue = $state(0);
  let pointer = $state<number | null>(null);
  const currentHue = $derived(hue ?? (hsv.s > 0 ? hsv.h : lastHue));
  function cancel() {
    draft = null;
    hue = null;
    pointer = null;
    onpreview?.(null);
  }
  function point(e: PointerEvent) {
    if (disabled) return;
    const rect =
      e.currentTarget instanceof HTMLElement ? e.currentTarget.getBoundingClientRect() : null;
    if (!rect) return;
    const saturation = Math.max(0, Math.min(100, ((e.clientX - rect.left) / rect.width) * 100));
    const brightness = Math.max(
      0,
      Math.min(100, 100 - ((e.clientY - rect.top) / rect.height) * 100),
    );
    preview(hsvHex(currentHue, saturation, brightness));
  }
  function key(e: KeyboardEvent) {
    if (disabled) return;
    if (e.key === 'Escape') {
      cancel();
      return;
    }
    const step = e.shiftKey ? 10 : 1;
    let s = hsv.s,
      v = hsv.v;
    if (e.key === 'ArrowLeft') s -= step;
    else if (e.key === 'ArrowRight') s += step;
    else if (e.key === 'ArrowUp') v += step;
    else if (e.key === 'ArrowDown') v -= step;
    else return;
    e.preventDefault();
    preview(hsvHex(currentHue, s, v));
  }
  onDestroy(() => onpreview?.(null));
  const swatches = [
    '#ffffff',
    '#ff5050',
    '#ffb340',
    '#ffe45c',
    '#60df87',
    '#42d9e8',
    '#588bff',
    '#b17aff',
    '#ed72c4',
  ];
  function preview(hex: string) {
    draft = hex;
    onpreview?.(hex);
  }
  function commit(hex: string) {
    if (!/^#[0-9a-f]{6}$/i.test(hex)) return;
    lastHue = currentHue;
    draft = null;
    hue = null;
    onpreview?.(null);
    onchange(hex.toLowerCase());
  }
  $effect(() => {
    if (disabled) {
      untrack(cancel);
    }
  });
</script>

<fieldset {disabled} class="color-control">
  <legend>{label}</legend>
  <div
    class="saturation"
    role="slider"
    tabindex={disabled ? -1 : 0}
    aria-label={label + (ru ? ': насыщенность и яркость' : ': saturation and value')}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={Math.round(hsv.s)}
    aria-valuetext={Math.round(hsv.s) + '% / ' + Math.round(hsv.v) + '%'}
    aria-disabled={disabled}
    style:background-color={hsvHex(currentHue, 100, 100)}
    onpointerdown={(e) => {
      if (disabled) return;
      hue = currentHue;
      pointer = e.pointerId;
      e.currentTarget.setPointerCapture(e.pointerId);
      point(e);
    }}
    onpointermove={(e) => {
      if (pointer === e.pointerId) point(e);
    }}
    onpointerup={(e) => {
      if (pointer !== e.pointerId) return;
      point(e);
      pointer = null;
      commit(draft ?? shown);
    }}
    onpointercancel={cancel}
    onkeydown={key}
    onkeyup={(e) => {
      if (e.key.startsWith('Arrow') && draft) commit(draft);
    }}
    onblur={cancel}
    title={ru
      ? 'Стрелки: насыщенность и яркость. Shift: шаг 10%. Escape: отменить.'
      : 'Arrows: saturation and value. Shift: 10% steps. Escape: cancel.'}
  >
    <span class="color-point" style:left={hsv.s + '%'} style:top={100 - hsv.v + '%'}></span>
  </div>
  <input
    class="hue"
    type="range"
    min="0"
    max="360"
    step="1"
    aria-label={label + (ru ? ': оттенок' : ': hue')}
    value={currentHue}
    oninput={(e) => {
      hue = e.currentTarget.valueAsNumber;
      preview(hsvHex(hue, hsv.s, hsv.v));
    }}
    onchange={(e) => commit(hsvHex(e.currentTarget.valueAsNumber, hsv.s, hsv.v))}
    onblur={cancel}
  />
  <div class="color-value">
    <span class="color-chip" style:background={shown}></span>
    <input
      class="hex"
      aria-label={label + ' HEX'}
      type="text"
      maxlength="7"
      pattern={'#[0-9a-fA-F]{6}'}
      value={shown.toUpperCase()}
      onchange={(e) => {
        if (e.currentTarget.validity.valid) commit(e.currentTarget.value);
      }}
      onkeydown={(e) => {
        if (e.key === 'Enter' && e.currentTarget.validity.valid) {
          e.preventDefault();
          commit(e.currentTarget.value);
        }
        if (e.key === 'Escape') {
          cancel();
          e.currentTarget.value = (value ?? '#ffffff').toUpperCase();
        }
      }}
    />
    {#if !value}<small>{ru ? 'Не считан' : 'Not read'}</small>{/if}
  </div>
  <div class="swatches" aria-label={ru ? 'Палитра' : 'Palette'}>
    {#each swatches as hex}<button
        type="button"
        aria-label={label + ' ' + hex}
        title={hex.toUpperCase()}
        style:background={hex}
        class:active={shown.toLowerCase() === hex}
        onclick={() => commit(hex)}
      ></button>{/each}
  </div>
</fieldset>

<style>
  .color-control {
    border: 0;
    padding: 0;
    margin: 0;
    min-width: 0;
  }
  legend {
    font-size: 13px;
    margin-bottom: 10px;
  }
  .color-value {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .saturation {
    position: relative;
    width: 100%;
    height: 142px;
    border-radius: 6px;
    margin-bottom: 9px;
    background-image:
      linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, transparent);
    cursor: crosshair;
    touch-action: none;
  }
  .saturation:focus-visible {
    outline: 2px solid #b2baff;
    outline-offset: 2px;
  }
  .color-point {
    position: absolute;
    width: 12px;
    height: 12px;
    border: 2px solid #fff;
    border-radius: 50%;
    box-shadow: 0 0 0 1px #000;
    transform: translate(-50%, -50%);
    pointer-events: none;
  }
  .hue {
    width: 100%;
    height: 14px;
    padding: 0;
    margin: 2px 0 14px;
    appearance: none;
    border-radius: 8px;
    background: linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00);
  }
  .hue::-webkit-slider-thumb {
    appearance: none;
    width: 13px;
    height: 20px;
    border: 2px solid white;
    border-radius: 6px;
    background: #ffffff50;
    box-shadow: 0 0 2px #000;
  }
  .color-chip {
    width: 38px;
    height: 32px;
    border: 1px solid #ffffff30;
    border-radius: 5px;
  }
  .hex {
    width: 104px;
    font-family: monospace;
  }
  small {
    color: #aaa;
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 7px;
    margin-top: 12px;
  }
  .swatches button {
    width: 23px;
    height: 23px;
    min-height: 0;
    border-radius: 50%;
    padding: 0;
    border: 2px solid #ffffff30;
  }
  .swatches button.active {
    outline: 2px solid #b2baff;
    outline-offset: 2px;
  }
  fieldset:disabled {
    opacity: 0.55;
  }
</style>
