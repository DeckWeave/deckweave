<script lang="ts">
  import type { PanelLayout, PanelRegion, Control, Locale } from './types';
  import { text } from './i18n';
  let {
    value,
    controls,
    locale,
    onchange,
  }: {
    value: PanelLayout | null;
    controls: Control[];
    locale: Locale;
    onchange: (v: PanelLayout | null) => void;
  } = $props();
  const t = $derived((key: Parameters<typeof text>[1]) => text(locale, key));
  const dials = $derived(controls.filter((c) => c.kind === 'dial'));
  function region(name: string, rect: PanelRegion['rect'], dials: string[] = []): PanelRegion {
    return { id: crypto.randomUUID().replaceAll('-', ''), name, rect, dials };
  }
  function preset(kind: string) {
    if (kind === 'legacy') {
      onchange(null);
      return;
    }
    const regions =
      kind === 'full'
        ? [region(t('wholePanel'), [0, 0, 1, 1])]
        : dials.map((d, i) =>
            region(`D${d.number}`, [i / dials.length, 0, 1 / dials.length, 1], [d.id]),
          );
    onchange({ regions, swipe_pages: value?.swipe_pages ?? true });
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
    for (let y = 0; y < 2; y++)
      for (let x = 0; x < 8; x++) {
        const rect: PanelRegion['rect'] = [x / 8, y / 2, 1 / 8, 1 / 2];
        if (
          value.regions.every(
            (r) =>
              rect[0] >= r.rect[0] + r.rect[2] - 1e-9 ||
              rect[0] + rect[2] <= r.rect[0] + 1e-9 ||
              rect[1] >= r.rect[1] + r.rect[3] - 1e-9 ||
              rect[1] + rect[3] <= r.rect[1] + 1e-9,
          )
        ) {
          onchange({
            ...value,
            regions: [...value.regions, region(`${t('region')} ${value.regions.length + 1}`, rect)],
          });
          return;
        }
      }
  }
</script>

<details class="panel-settings">
  <summary>{t('panelLayout')}</summary>
  <p>{t('panelLayoutHint')}</p>
  <div class="dial-tests">
    <button onclick={() => preset('dials')}>{t('dialRegions')}</button><button
      onclick={() => preset('full')}>{t('wholePanel')}</button
    ><button onclick={() => preset('legacy')}>{t('legacyPanel')}</button>
  </div>
  {#if value}
    <label
      ><input
        type="checkbox"
        checked={value.swipe_pages}
        onchange={(e) => onchange({ ...value!, swipe_pages: e.currentTarget.checked })}
      />{t('swipePages')}</label
    >
    {#each value.regions as r (r.id)}
      <div class="panel-region-fields">
        <div class="step-toolbar">
          <input
            aria-label={t('regionName')}
            value={r.name}
            maxlength="80"
            onchange={(e) => update(r.id, { name: e.currentTarget.value })}
          /><button
            aria-label={t('remove')}
            onclick={() =>
              onchange({ ...value!, regions: value!.regions.filter((next) => next.id !== r.id) })}
            >×</button
          >
        </div>
        <div class="region-rect">
          {#each ['X', 'Y', t('width'), t('height')] as label, i}<label
              >{label} %<input
                type="number"
                min="0"
                max="100"
                step="0.1"
                value={+(r.rect[i] * 100).toFixed(2)}
                onchange={(e) => {
                  const rect = [...r.rect] as PanelRegion['rect'];
                  rect[i] = +e.currentTarget.value / 100;
                  update(r.id, { rect });
                }}
              /></label
            >{/each}
        </div>
        <div class="dial-tests">
          {#each dials as d}<label
              ><input
                type="checkbox"
                checked={r.dials.includes(d.id)}
                disabled={!r.dials.includes(d.id) &&
                  value.regions.some((other) => other.dials.includes(d.id))}
                onchange={(e) =>
                  update(r.id, {
                    dials: e.currentTarget.checked
                      ? [...r.dials, d.id]
                      : r.dials.filter((id) => id !== d.id),
                  })}
              />D{d.number}</label
            >{/each}
        </div>
        {#if r.rect[1] !== 0 || r.rect[3] !== 1}<small>{t('exactYHint')}</small>{/if}
      </div>
    {/each}
    <button onclick={add} disabled={value.regions.length >= 64}>＋ {t('addRegion')}</button>
  {/if}
</details>
