<script lang="ts">
  import { getContext } from 'svelte';
  import type { Snippet } from 'svelte';
  import type { Binding } from './types';
  import {
    actionDropContext,
    acceptsNested,
    registerDrop,
    type ActionLibraryBridge,
    type DropScope,
    type LibraryAction,
  } from './action-drop';
  import Icon from './Icon.svelte';
  let {
    scope,
    identity,
    label,
    ru,
    onchange,
    children,
  }: {
    scope: DropScope;
    identity: string;
    label?: string;
    ru: boolean;
    onchange: (binding: Binding) => void;
    children?: Snippet;
  } = $props();
  const library = getContext<ActionLibraryBridge>(actionDropContext);
  function accepts(item: LibraryAction) {
    return !!library?.enabled(item) && acceptsNested(item, scope);
  }
  function drop(item: LibraryAction) {
    if (!accepts(item)) return;
    const next = library.make(item, scope.controller);
    if (next) onchange(next);
  }
  function setup(node: HTMLElement) {
    return registerDrop(node, { accepts, identity: () => identity, drop });
  }
</script>

<div
  class="action-drop"
  data-action-drop
  use:setup
  title={ru ? 'Перетащите действие из списка справа' : 'Drag an action from the library'}
>
  {#if children}{@render children()}{:else}
    <Icon name="plus" />
    <span>{label ?? (ru ? 'Перетащите действие сюда' : 'Drag an action here')}</span>
  {/if}
  {#if library?.selected()}<button
      class="quiet"
      disabled={!accepts(library.selected()!)}
      onclick={() => {
        const item = library.selected();
        if (item) drop(item);
      }}
      >{ru
        ? label
          ? 'Заменить выбранным'
          : 'Добавить выбранное'
        : label
          ? 'Replace with selected'
          : 'Add selected'}</button
    >{/if}
</div>
