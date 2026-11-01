<script lang="ts">
  let {
    orientation,
    label,
    value,
    min,
    max,
    onchange,
  }: {
    orientation: 'horizontal' | 'vertical';
    label: string;
    value: number;
    min: number;
    max: number;
    onchange: (value: number) => void;
  } = $props();
  let drag = $state<{ id: number; position: number; value: number } | null>(null);
  function update(next: number) {
    onchange(Math.round(Math.max(min, Math.min(max, next))));
  }
</script>

<!-- A focusable separator implements the keyboard-operable window splitter pattern. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="pane-splitter"
  class:vertical={orientation === 'vertical'}
  role="separator"
  tabindex="0"
  aria-label={label}
  aria-orientation={orientation}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={Math.round(value)}
  onpointerdown={(e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag = { id: e.pointerId, position: orientation === 'vertical' ? e.clientX : e.clientY, value };
  }}
  onpointermove={(e) => {
    if (drag?.id !== e.pointerId) return;
    update(drag.value + drag.position - (orientation === 'vertical' ? e.clientX : e.clientY));
  }}
  onpointerup={() => {
    drag = null;
  }}
  onlostpointercapture={() => {
    drag = null;
  }}
  onkeydown={(e) => {
    const negative = orientation === 'vertical' ? 'ArrowRight' : 'ArrowDown';
    const positive = orientation === 'vertical' ? 'ArrowLeft' : 'ArrowUp';
    if (![negative, positive, 'Home', 'End'].includes(e.key)) return;
    e.preventDefault();
    update(
      e.key === 'Home' ? min : e.key === 'End' ? max : value + (e.key === positive ? 20 : -20),
    );
  }}
>
  <span></span>
</div>
