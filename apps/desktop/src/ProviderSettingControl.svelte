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

<label class="setting-control" for={'provider-' + setting.id}>
  <span
    >{label}{#if setting.presentation === 'slider'}<output
        >{number ?? '—'}{setting.max !== undefined ? ' / ' + setting.max : ''}</output
      >{/if}</span
  >
  {#if setting.type === 'boolean'}
    <select
      id={'provider-' + setting.id}
      {disabled}
      value={typeof value === 'boolean' ? String(value) : ''}
      onchange={(e) => onchange(e.currentTarget.value === 'true')}
    >
      <option value="" disabled>{ru ? 'Не считано' : 'Not read'}</option><option value="true"
        >{ru ? 'Включено' : 'On'}</option
      ><option value="false">{ru ? 'Выключено' : 'Off'}</option>
    </select>
  {:else if setting.type === 'number'}
    <input
      id={'provider-' + setting.id}
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
  {:else}
    <select
      id={'provider-' + setting.id}
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
</label>

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
    color: #b7c1db;
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
    accent-color: #a1aaff;
  }
</style>
