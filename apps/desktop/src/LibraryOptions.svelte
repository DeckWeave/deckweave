<script lang="ts">
  import type { Client } from './client.svelte';
  import type { LibraryPreferences } from './types';
  import { text } from './i18n';
  import { moved, libraryDefaults } from './library';
  let {
    client,
    groups,
  }: {
    client: Client;
    groups: { id: string; name: string; actions: { id: string; name: string }[] }[];
  } = $props();
  const prefs = $derived(client.snapshot!.configuration.preferences.library);
  const t = $derived((key: Parameters<typeof text>[1]) =>
    text(client.snapshot!.configuration.locale, key),
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
  function move(ids: string[], id: string, direction: -1 | 1) {
    update((old) => ({ ...old, order: moved(old.order, ids, id, direction) }));
  }
  function development(checked: boolean) {
    update((old) => ({ ...old, show_development: checked }));
  }
</script>

<section class="library-options" aria-label={t('libraryDisplay')}>
  <p>{t('libraryDisplayHint')}</p>
  <label class="library-check"
    ><input
      type="checkbox"
      checked={prefs.show_development}
      onchange={(e) => development(e.currentTarget.checked)}
    />{t('showDevelopment')}</label
  >
  {#each groups as group, i (group.id)}
    <div class="library-option-group">
      <div class="library-option-row">
        <label
          ><input
            type="checkbox"
            checked={!prefs.hidden.includes(group.id)}
            onchange={(e) => show(group.id, e.currentTarget.checked)}
          />{group.name}</label
        >
        <button
          class="small"
          disabled={i === 0}
          aria-label={`${t('moveUp')}: ${group.name}`}
          onclick={() =>
            move(
              groups.map((g) => g.id),
              group.id,
              -1,
            )}>↑</button
        >
        <button
          class="small"
          disabled={i === groups.length - 1}
          aria-label={`${t('moveDown')}: ${group.name}`}
          onclick={() =>
            move(
              groups.map((g) => g.id),
              group.id,
              1,
            )}>↓</button
        >
      </div>
      {#each group.actions as action, j (action.id)}
        <div class="library-option-row library-option-action">
          <label
            ><input
              type="checkbox"
              checked={!prefs.hidden.includes(action.id)}
              onchange={(e) => show(action.id, e.currentTarget.checked)}
            />{action.name}</label
          >
          <button
            class="small"
            disabled={j === 0}
            aria-label={`${t('moveUp')}: ${action.name}`}
            onclick={() =>
              move(
                group.actions.map((a) => a.id),
                action.id,
                -1,
              )}>↑</button
          >
          <button
            class="small"
            disabled={j === group.actions.length - 1}
            aria-label={`${t('moveDown')}: ${action.name}`}
            onclick={() =>
              move(
                group.actions.map((a) => a.id),
                action.id,
                1,
              )}>↓</button
          >
        </div>
      {/each}
    </div>
  {/each}
  <button class="quiet" onclick={() => update(libraryDefaults)}>{t('restoreLibrary')}</button>
</section>
