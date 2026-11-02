<script lang="ts">
  import type { PanelLayout, PanelRegion, Control, Locale, Binding } from './types';
  import { text } from './i18n';
  import { freeRegion, splitRegion, validRegion } from './panel-layout';
  import Icon from './Icon.svelte';
  let {
    value,
    context,
    controls,
    locale,
    pixels,
    bindings,
    selectedId,
    onselect,
    onaction,
    onchange,
  }: {
    value: PanelLayout | null;
    context: string;
    controls: Control[];
    locale: Locale;
    pixels: [number, number];
    bindings: Record<string, Binding>;
    selectedId: string | null;
    onselect: (id: string) => void;
    onaction: (id: string) => void;
    onchange: (layout: PanelLayout | null) => void;
  } = $props();
  const ru = $derived(locale === 'ru');
  const t = $derived((key: Parameters<typeof text>[1]) => text(locale, key));
  const dials = $derived(controls.filter((c) => c.kind === 'dial'));
  const selected = $derived(
    value?.regions.find((r) => 'panel-' + r.id === selectedId) ?? value?.regions[0],
  );
  const empty = $derived(value ? freeRegion(value.regions) : null);
  let replacement = $state<{ kind: string; before: string; context: string } | null>(null);
  let removal = $state<{ id: string; before: string; context: string } | null>(null);
  let geometryError = $state('');
  const currentKind = $derived(
    !value
      ? 'legacy'
      : value.regions.length === 1 && value.regions[0].rect.every((v, i) => v === [0, 0, 1, 1][i])
        ? 'full'
        : value.regions.length === dials.length &&
            value.regions.every(
              (r, i) =>
                r.dials.length === 1 &&
                r.dials[0] === dials[i]?.id &&
                r.rect.every(
                  (v, n) => Math.abs(v - [i / dials.length, 0, 1 / dials.length, 1][n]) < 1e-8,
                ),
            )
          ? 'dials'
          : [3, 4, 5].some(
                (n) =>
                  value.regions.length === n &&
                  value.regions.every((r, i) =>
                    r.rect.every((v, j) => Math.abs(v - [i / n, 0, 1 / n, 1][j]) < 1e-8),
                  ),
              )
            ? 'columns-' + value.regions.length
            : 'custom',
  );
  function region(name: string, rect: PanelRegion['rect'], dials: string[] = []): PanelRegion {
    return { id: crypto.randomUUID().replaceAll('-', ''), name, rect, dials };
  }
  function propose(kind: string) {
    if (kind === currentKind) return;
    replacement = { kind, before: JSON.stringify(value), context };
    removal = null;
  }
  function preset(kind: string) {
    if (kind === 'legacy') {
      onchange(null);
      return;
    }
    const regions =
      kind === 'full'
        ? [region(t('wholePanel'), [0, 0, 1, 1])]
        : kind.startsWith('columns-')
          ? Array.from({ length: Number(kind.slice(8)) }, (_, i) =>
              region(`${t('region')} ${i + 1}`, [
                i / Number(kind.slice(8)),
                0,
                1 / Number(kind.slice(8)),
                1,
              ]),
            )
          : dials.map((d, i) =>
              region(`D${d.number}`, [i / dials.length, 0, 1 / dials.length, 1], [d.id]),
            );
    onchange({ regions, swipe_pages: value?.swipe_pages ?? true });
    onselect('panel-' + regions[0].id);
  }
  function update(id: string, patch: Partial<PanelRegion>) {
    if (value)
      onchange({
        ...value,
        regions: value.regions.map((r) => (r.id === id ? { ...r, ...patch } : r)),
      });
  }
  function add() {
    if (!value) return;
    let regions = value.regions;
    let rect = empty;
    if (!rect && selected) {
      const split = splitRegion(selected);
      if (!split) return;
      regions = regions.map((r) => (r.id === selected.id ? { ...r, rect: split[0] } : r));
      rect = split[1];
    }
    if (!rect) return;
    const next = region(`${t('region')} ${value.regions.length + 1}`, rect);
    onchange({ ...value, regions: [...regions, next] });
    onselect('panel-' + next.id);
  }
  function dimension(event: Event, r: PanelRegion, index: number) {
    const input = event.currentTarget as HTMLInputElement;
    const rect = [...r.rect] as PanelRegion['rect'];
    rect[index] = input.valueAsNumber / pixels[index % 2];
    if (!validRegion(rect, value!.regions, r.id)) {
      input.value = String(+(r.rect[index] * pixels[index % 2]).toFixed(2));
      geometryError = ru
        ? 'Размер не изменён: область выходит за панель, слишком мала или пересекается с соседней.'
        : 'Size unchanged: the region is outside the panel, too small or overlaps another region.';
      return;
    }
    geometryError = '';
    input.setCustomValidity('');
    update(r.id, { rect });
  }
</script>

<div class="panel-layout-editor">
  <div class="panel-layout-toolbar">
    <div class="segmented" role="group" aria-label={t('panelLayout')}>
      {#each [3, 4, 5] as count}<button
          class:current={currentKind === 'columns-' + count}
          aria-pressed={currentKind === 'columns-' + count}
          title={ru
            ? `${count} ${count === 5 ? 'равных областей' : 'равные области'} без связей с крутилками`
            : `${count} equal regions, no dial links`}
          aria-label={ru ? `${count} ${count === 5 ? 'областей' : 'области'}` : `${count} regions`}
          onclick={() => propose('columns-' + count)}>{count}</button
        >{/each}
      <button
        class:current={currentKind === 'dials'}
        aria-pressed={currentKind === 'dials'}
        disabled={!dials.length}
        onclick={() => propose('dials')}
        ><Icon name="dial" />{ru ? 'По крутилкам' : 'Match dials'}</button
      >
      <button
        class:current={currentKind === 'full'}
        aria-pressed={currentKind === 'full'}
        onclick={() => propose('full')}><Icon name="device" />{t('wholePanel')}</button
      >
      <button
        class:current={currentKind === 'legacy'}
        aria-pressed={currentKind === 'legacy'}
        onclick={() => propose('legacy')}
        ><Icon name="device" />{ru ? 'Штатная' : 'Original'}</button
      >
    </div>
    <span class="panel-resolution">{pixels[0]} × {pixels[1]} px</span>
    {#if value}
      <button
        class="panel-add-area"
        disabled={value.regions.length >= 64 || (!empty && (!selected || !splitRegion(selected)))}
        title={ru
          ? empty
            ? 'Добавить область в свободное место'
            : 'Разделить выбранную область пополам; её действие останется в левой половине'
          : empty
            ? 'Add a region in free space'
            : 'Split the selected region; its action stays in the left half'}
        onclick={add}
        ><Icon name="plus" />{empty
          ? ru
            ? 'Область'
            : 'Region'
          : ru
            ? 'Разделить'
            : 'Split'}</button
      >
    {/if}
    {#if value}<label class="panel-swipe"
        ><input
          type="checkbox"
          checked={value.swipe_pages}
          onchange={(e) => onchange({ ...value!, swipe_pages: e.currentTarget.checked })}
        />{t('swipePages')}</label
      >{/if}
  </div>
  {#if replacement}<div class="action-replacement" role="status">
      <span
        >{ru
          ? 'Заменить раскладку? Назначения прежних областей будут удалены.'
          : 'Replace layout? Assignments of the previous regions will be removed.'}</span
      >
      <button
        disabled={replacement.context !== context || replacement.before !== JSON.stringify(value)}
        onclick={() => {
          if (replacement!.context !== context || replacement!.before !== JSON.stringify(value))
            return;
          const kind = replacement!.kind;
          replacement = null;
          preset(kind);
        }}>{ru ? 'Заменить' : 'Replace'}</button
      >
      <button class="quiet" onclick={() => (replacement = null)}>{ru ? 'Отмена' : 'Cancel'}</button>
    </div>{/if}
  {#if value}
    <div class="panel-area-workspace">
      <div
        class="panel-area-list"
        role="group"
        aria-label={ru ? 'Области панели' : 'Panel regions'}
      >
        {#each value.regions as r (r.id)}<button
            class:current={selected?.id === r.id}
            aria-pressed={selected?.id === r.id}
            onclick={() => onselect('panel-' + r.id)}
          >
            <span>{r.name}</span><small
              >{Math.round(r.rect[2] * pixels[0])} × {Math.round(r.rect[3] * pixels[1])}</small
            >
          </button>{/each}
      </div>
      {#if selected}{@const r = selected}
        <div class="panel-area-properties">
          <div class="panel-area-heading">
            <input
              aria-label={t('regionName')}
              value={r.name}
              maxlength="80"
              onchange={(e) => update(r.id, { name: e.currentTarget.value })}
            />
            <button
              onclick={() => onaction('panel-' + r.id)}
              title={bindings['panel-' + r.id]?.label ?? t('unassigned')}
              ><Icon name="sliders" />{ru ? 'Действие области' : 'Region action'}</button
            >
            <button
              class="quiet"
              aria-label={ru ? 'Удалить область' : 'Remove region'}
              title={ru ? 'Удалить область' : 'Remove region'}
              onclick={() => (removal = { id: r.id, before: JSON.stringify(value), context })}
              ><Icon name="trash" /></button
            >
          </div>
          <div class="panel-area-dimensions">
            {#each ['X', 'Y', ru ? 'Ширина' : 'Width', ru ? 'Высота' : 'Height'] as label, i}<label
                >{label}<input
                  type="number"
                  min={i < 2 ? 0 : 1}
                  max={pixels[i % 2]}
                  step="0.1"
                  value={+(r.rect[i] * pixels[i % 2]).toFixed(2)}
                  aria-label={label + ' px'}
                  oninput={(e) => e.currentTarget.setCustomValidity('')}
                  onchange={(e) => dimension(e, r, i)}
                /><span>px</span></label
              >{/each}
          </div>
          {#if geometryError}<p class="connection-error" role="alert">{geometryError}</p>{/if}
          <div class="panel-dial-links">
            <span>{ru ? 'Крутилки' : 'Dials'}</span>
            <div
              class="segmented"
              role="group"
              aria-label={ru ? 'Связанные крутилки' : 'Linked dials'}
            >
              {#each dials as dial}{@const other = value.regions.find(
                  (other) => other.id !== r.id && other.dials.includes(dial.id),
                )}<button
                  class:current={r.dials.includes(dial.id)}
                  aria-pressed={r.dials.includes(dial.id)}
                  disabled={!!other}
                  title={other
                    ? ru
                      ? `Используется областью ${other.name}`
                      : `Used by ${other.name}`
                    : ru
                      ? 'Совместное действие области и крутилки'
                      : 'Shared action for this region and dial'}
                  onclick={() =>
                    update(r.id, {
                      dials: r.dials.includes(dial.id)
                        ? r.dials.filter((id) => id !== dial.id)
                        : [...r.dials, dial.id],
                    })}>D{dial.number}</button
                >{/each}
            </div>
            <span class="panel-assignment-name"
              >{bindings['panel-' + r.id]?.label ?? t('unassigned')}</span
            >
          </div>
          {#if r.rect[1] !== 0 || r.rect[3] !== 1}<small class="panel-y-hint"
              >{t('exactYHint')}</small
            >{/if}
          {#if removal?.id === r.id}<div class="action-replacement" role="status">
              <span
                >{ru
                  ? 'Удалить область и её назначение?'
                  : 'Remove the region and its assignment?'}</span
              ><button
                disabled={removal.context !== context || removal.before !== JSON.stringify(value)}
                onclick={() => {
                  if (removal!.context !== context || removal!.before !== JSON.stringify(value))
                    return;
                  onchange({
                    ...value!,
                    regions: value!.regions.filter((r) => r.id !== removal!.id),
                  });
                  removal = null;
                }}>{ru ? 'Удалить область' : 'Remove region'}</button
              ><button class="quiet" onclick={() => (removal = null)}
                >{ru ? 'Отмена' : 'Cancel'}</button
              >
            </div>{/if}
        </div>
      {/if}
    </div>
  {:else}
    <div
      class="panel-legacy-actions"
      role="group"
      aria-label={ru ? 'Кнопки панели' : 'Panel buttons'}
    >
      {#each controls.filter((c) => c.kind === 'touch') as control}<button
          onclick={() => onaction(control.id)}
          ><Icon name="sliders" />T{control.number} · {bindings[control.id]?.label ??
            t('unassigned')}</button
        >{/each}
    </div>
  {/if}
</div>
