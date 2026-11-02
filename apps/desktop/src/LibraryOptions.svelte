<script lang="ts">
  import type { Client } from './client.svelte';
  import type { LibraryPreferences } from './types';
  import { text } from './i18n';
  import { placed, moved, libraryDefaults } from './library';
  import { actionIcon } from './icons';
  import Icon from './Icon.svelte';
  let {
    client,
    groups,
    search = '',
  }: {
    client: Client;
    groups: { id: string; name: string; actions: { id: string; name: string }[] }[];
    search?: string;
  } = $props();
  const prefs = $derived(client.snapshot!.configuration.preferences.library);
  const ru = $derived(client.snapshot!.configuration.locale === 'ru');
  const t = $derived((key: Parameters<typeof text>[1]) =>
    text(client.snapshot!.configuration.locale, key),
  );
  let expanded = $state<string[]>([]);
  let drag = $state<{
    pointer: number;
    id: string;
    scope: string;
    siblings: string[];
    x: number;
    y: number;
  } | null>(null);
  let destination = $state<{ id: string; after: boolean } | null>(null);
  const shown = $derived(
    groups.filter((g) =>
      (g.name + ' ' + g.actions.map((a) => a.name).join(' '))
        .toLocaleLowerCase()
        .includes(search.toLocaleLowerCase()),
    ),
  );
  function update(make: (old: LibraryPreferences) => LibraryPreferences) {
    void client.send((revision) => ({
      type: 'set_preferences',
      expected_revision: revision,
      preferences: {
        ...client.snapshot!.configuration.preferences,
        library: make(client.snapshot!.configuration.preferences.library),
      },
    }));
  }
  function show(id: string, visible: boolean) {
    update((old) => ({
      ...old,
      hidden: visible ? old.hidden.filter((key) => key !== id) : [...new Set([...old.hidden, id])],
    }));
  }
  function handle(id: string, scope: string, siblings: string[]) {
    return {
      onpointerdown: (event: PointerEvent) => {
        if (event.button !== 0 || client.busy) return;
        (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
        drag = {
          pointer: event.pointerId,
          id,
          scope,
          siblings,
          x: event.clientX,
          y: event.clientY,
        };
      },
      onpointermove: (event: PointerEvent) => {
        if (
          !drag ||
          drag.pointer !== event.pointerId ||
          Math.hypot(event.clientX - drag.x, event.clientY - drag.y) < 6
        )
          return;
        event.preventDefault();
        const row = document
          .elementFromPoint(event.clientX, event.clientY)
          ?.closest<HTMLElement>('[data-order-id]');
        destination =
          row?.dataset.orderScope === drag.scope && row.dataset.orderId !== drag.id
            ? {
                id: row.dataset.orderId!,
                after: event.clientY > row.getBoundingClientRect().top + row.offsetHeight / 2,
              }
            : null;
        row?.scrollIntoView({ block: 'nearest' });
      },
      onpointerup: (event: PointerEvent) => {
        if (drag?.pointer !== event.pointerId) return;
        if (destination) {
          const current = drag,
            target = destination;
          update((old) => ({
            ...old,
            order: placed(old.order, current.siblings, current.id, target.id, target.after),
          }));
        }
        drag = null;
        destination = null;
      },
      onpointercancel: () => {
        drag = null;
        destination = null;
      },
      onlostpointercapture: () => {
        drag = null;
        destination = null;
      },
      onkeydown: (event: KeyboardEvent) => {
        if (!event.altKey || !['ArrowUp', 'ArrowDown'].includes(event.key)) return;
        event.preventDefault();
        update((old) => ({
          ...old,
          order: moved(old.order, siblings, id, event.key === 'ArrowUp' ? -1 : 1),
        }));
      },
    };
  }
</script>

<section class="library-options" aria-label={t('libraryDisplay')}>
  <div class="library-options-help">
    <span
      >{ru
        ? 'Порядок — за маркер. Видимость — переключателем.'
        : 'Drag a handle to reorder. Toggle library visibility.'}</span
    ><button
      class="quiet icon-button"
      title={t('restoreLibrary')}
      aria-label={t('restoreLibrary')}
      onclick={() => update(libraryDefaults)}><Icon name="refresh" /></button
    >
  </div>
  <label class="library-check"
    ><input
      type="checkbox"
      checked={prefs.show_development}
      disabled={client.busy}
      onchange={(e) => {
        const checked = e.currentTarget.checked;
        update((old) => ({ ...old, show_development: checked }));
      }}
    />{t('showDevelopment')}</label
  >
  {#each shown as group (group.id)}
    <div class="library-option-group" class:option-hidden={prefs.hidden.includes(group.id)}>
      <div
        class="library-option-row"
        data-order-id={group.id}
        data-order-scope="groups"
        class:order-before={destination?.id === group.id && !destination.after}
        class:order-after={destination?.id === group.id && destination.after}
        class:order-dragging={drag?.id === group.id}
      >
        <button
          class="reorder-handle icon-button quiet"
          aria-label={(ru ? 'Переместить: ' : 'Reorder: ') + group.name}
          title={ru ? 'Перетащить · Alt+↑/↓' : 'Drag · Alt+↑/↓'}
          {...handle(
            group.id,
            'groups',
            groups.map((g) => g.id),
          )}><Icon name="grip" /></button
        >
        <button
          class="library-expand quiet"
          aria-expanded={expanded.includes(group.id)}
          onclick={() =>
            (expanded = expanded.includes(group.id)
              ? expanded.filter((id) => id !== group.id)
              : [...expanded, group.id])}
          ><span class:expanded={expanded.includes(group.id)}
            ><Icon name="chevron" size={12} /></span
          ><strong>{group.name}</strong><small>{group.actions.length}</small></button
        >
        <input
          class="visibility-toggle"
          type="checkbox"
          role="switch"
          aria-label={(ru ? 'Показывать в библиотеке: ' : 'Show in library: ') + group.name}
          title={ru
            ? 'Скрытие из списка сохраняет все назначения'
            : 'Hiding preserves all assignments'}
          checked={!prefs.hidden.includes(group.id)}
          disabled={client.busy}
          onchange={(e) => show(group.id, e.currentTarget.checked)}
        />
      </div>
      {#if expanded.includes(group.id) || search}
        {#each group.actions.filter((a) => !search || (group.name + ' ' + a.name)
              .toLocaleLowerCase()
              .includes(search.toLocaleLowerCase())) as action (action.id)}
          <div
            class="library-option-row library-option-action"
            data-order-id={action.id}
            data-order-scope={group.id}
            class:order-before={destination?.id === action.id && !destination.after}
            class:order-after={destination?.id === action.id && destination.after}
            class:order-dragging={drag?.id === action.id}
          >
            <button
              class="reorder-handle icon-button quiet"
              aria-label={(ru ? 'Переместить: ' : 'Reorder: ') + action.name}
              title={ru ? 'Перетащить · Alt+↑/↓' : 'Drag · Alt+↑/↓'}
              {...handle(
                action.id,
                group.id,
                group.actions.map((a) => a.id),
              )}><Icon name="grip" /></button
            ><Icon name={actionIcon(action.id)} /><span>{action.name}</span><input
              type="checkbox"
              aria-label={(ru ? 'Показывать: ' : 'Show: ') + action.name}
              checked={!prefs.hidden.includes(action.id)}
              disabled={client.busy}
              onchange={(e) => show(action.id, e.currentTarget.checked)}
            />
          </div>
        {/each}
      {/if}
    </div>
  {/each}
  {#if !shown.length}<p class="library-empty">{t('noResults')}</p>{/if}
</section>
