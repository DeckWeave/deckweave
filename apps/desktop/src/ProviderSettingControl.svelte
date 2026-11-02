<script lang="ts">
  import type { ProviderSetting } from './types';
  let {
    setting,
    value,
    disabled = false,
    ru = false,
    onchange,
  }: {
    setting: ProviderSetting;
    value: unknown;
    disabled?: boolean;
    ru?: boolean;
    onchange: (value: unknown) => void;
  } = $props();
  let draft = $state<number | null>(null);
  const label = $derived(setting.label?.[ru ? 'ru' : 'en'] ?? setting.label?.en ?? setting.id);
  const number = $derived(draft ?? (typeof value === 'number' ? value : undefined));
</script>

<div class="setting-control">
  <span
    >{label}{#if setting.presentation === 'slider'}<output
        >{number ?? '—'}{setting.max !== undefined ? ' / ' + setting.max : ''}</output
      >{/if}</span
  >
  {#if setting.type === 'boolean'}
    <div class="segmented" role="group" aria-label={label}>
      {#each [true, false] as option}<button
          {disabled}
          class:current={value === option}
          aria-pressed={value === option}
          onclick={() => {
            if (value !== option) onchange(option);
          }}>{option ? (ru ? 'Вкл' : 'On') : ru ? 'Выкл' : 'Off'}</button
        >{/each}{#if typeof value !== 'boolean'}<span>{ru ? 'Не считано' : 'Not read'}</span>{/if}
    </div>
  {:else if setting.type === 'number'}
    <input
      id={'provider-' + setting.id}
      aria-label={label}
      type={setting.presentation === 'slider' ? 'range' : 'number'}
      min={setting.min}
      max={setting.max}
      step={setting.step ?? 1}
      value={number}
      {disabled}
      oninput={(e) => {
        if (setting.presentation === 'slider') draft = e.currentTarget.valueAsNumber;
      }}
      onchange={(e) => {
        if (e.currentTarget.value !== '' && e.currentTarget.validity.valid) {
          draft = null;
          onchange(e.currentTarget.valueAsNumber);
        }
      }}
    />
  {:else if setting.choices && setting.choices.length <= 5}
    <div class="segmented" role="group" aria-label={label}>
      {#each setting.choices as choice}{@const val =
          typeof choice === 'object' ? choice.value : choice}<button
          {disabled}
          class:current={JSON.stringify(value) === JSON.stringify(val)}
          aria-pressed={JSON.stringify(value) === JSON.stringify(val)}
          onclick={() => {
            if (JSON.stringify(value) !== JSON.stringify(val)) onchange(val);
          }}
          >{typeof choice === 'object'
            ? (choice.label?.[ru ? 'ru' : 'en'] ?? choice.label?.en ?? String(val))
            : String(choice)}</button
        >{/each}
    </div>
  {:else}
    <select
      id={'provider-' + setting.id}
      aria-label={label}
      {disabled}
      value={JSON.stringify(value) ?? ''}
      onchange={(e) => onchange(JSON.parse(e.currentTarget.value))}
    >
      <option value="" disabled>{ru ? 'Не считано' : 'Not read'}</option>
      {#each setting.choices ?? [] as choice}{@const val =
          typeof choice === 'object' ? choice.value : choice}<option value={JSON.stringify(val)}
          >{typeof choice === 'object'
            ? (choice.label?.[ru ? 'ru' : 'en'] ?? choice.label?.en ?? String(val))
            : String(choice)}</option
        >{/each}
    </select>
  {/if}
</div>

<style>
  .setting-control {
    display: grid;
    align-content: start;
    gap: 10px;
    min-width: 0;
    font-size: 13px;
  }
  .setting-control > span {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }
  output {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  select,
  input {
    width: 100%;
    min-width: 0;
  }
  input[type='range'] {
    height: 30px;
    padding: 0;
    accent-color: var(--focus);
  }
</style>
