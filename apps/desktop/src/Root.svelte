<script lang="ts">
  import PanelWindow from './PanelWindow.svelte';
  const panelId = new URLSearchParams(window.location.search).get('panel');
  const editor = panelId ? null : import('./App.svelte');
</script>

{#if panelId}<PanelWindow id={panelId} />{:else if editor}{#await editor then module}<module.default
    />{/await}{/if}
