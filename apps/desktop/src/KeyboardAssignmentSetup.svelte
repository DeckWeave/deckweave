<script lang="ts">
  import type { Binding, KeyboardAlias } from './types';
  import type { OnboardAssignment } from './keyboard-assignments';
  import { keyboardActionError } from './keyboard-action-errors';
  let {
    native,
    owner,
    binding,
    route,
    connected,
    canReconcile,
    busy,
    ru,
    profileName,
    candidate = null,
    previewNative = null,
    previewCurrent = true,
    affectedProfiles = [],
    onconfirm,
    oncancel,
    onsetup,
    onrestore,
    onreconcile,
  }: {
    native: OnboardAssignment;
    owner: KeyboardAlias | undefined;
    binding: Binding | undefined;
    route: { state: string; error?: string | null; source?: string } | null;
    connected: boolean;
    canReconcile: boolean;
    busy: boolean;
    ru: boolean;
    profileName: string;
    candidate?: Binding | null;
    previewNative?: OnboardAssignment | null;
    previewCurrent?: boolean;
    affectedProfiles?: string[];
    onconfirm: () => void;
    oncancel: () => void;
    onsetup: () => void;
    onrestore: () => void;
    onreconcile: () => void;
  } = $props();
  let restoring = $state(false);
  const verified = $derived(
    owner?.phase === 'verified' &&
      native.record !== null &&
      native.record.every((n, i) => n === owner?.installed[i]),
  );
  const phase = $derived.by(() => {
    if (owner?.phase === 'pending')
      return busy
        ? ru
          ? 'Запись и проверка назначения…'
          : 'Writing and verifying assignment…'
        : ru
          ? 'Запись не подтверждена; проверьте клавиатуру'
          : 'Write not confirmed; check the keyboard';
    if (owner?.phase === 'restoring')
      return busy
        ? ru
          ? 'Восстановление назначения…'
          : 'Restoring assignment…'
        : ru
          ? 'Проверьте завершение восстановления назначения'
          : 'Check whether assignment restoration completed';
    if (['uncertain', 'conflict'].includes(owner?.phase ?? ''))
      return ru ? 'Нужно проверить состояние клавиатуры' : 'Keyboard state needs reconciliation';
    if (owner?.phase === 'legacy_unverified')
      return ru
        ? 'Сохранённая привязка требует проверки происхождения'
        : 'Saved trigger ownership needs reconciliation';
    if (!connected)
      return canReconcile
        ? ru
          ? 'Подключение требует проверки; аппаратные изменения недоступны'
          : 'Connection needs reconciliation; keyboard writes unavailable'
        : ru
          ? 'Клавиатура не подключена'
          : 'Keyboard disconnected';
    if (!binding && verified)
      return ru
        ? 'Служебное назначение сохранено; в этом профиле действия нет'
        : 'Keyboard trigger retained; no action in this profile';
    if (!binding)
      return native.observed
        ? ru
          ? 'Выполняется клавиатурой'
          : 'Runs on keyboard'
        : ru
          ? 'Ожидаем чтение клавиатуры'
          : 'Waiting for keyboard readback';
    if (!verified)
      return ru
        ? 'Действие сохранено; аппаратная привязка не подтверждена'
        : 'Action saved; keyboard mapping not verified';
    if (route?.state === 'active' || route?.state === 'available')
      return ru ? 'Источник ввода подключён' : 'Input source connected';
    return ru
      ? 'Аппаратная привязка подтверждена; источник ввода недоступен'
      : 'Keyboard mapping verified; input source unavailable';
  });
  const legacySetup = $derived(
    owner?.provenance === 'legacy' && owner.phase === 'legacy_unverified',
  );
  const canReplace = $derived(!owner || owner.phase === 'released' || legacySetup);
  const canRestore = $derived(
    owner?.provenance === 'deckweave' &&
      owner.phase !== 'released' &&
      !!owner.original &&
      native.record !== null &&
      (native.record.every((n, i) => n === owner.installed[i]) ||
        native.record.every((n, i) => n === owner.original?.[i])) &&
      connected &&
      !busy,
  );
</script>

<section
  class="keyboard-assignment"
  aria-label={ru ? 'Назначение клавиатуры' : 'Keyboard assignment'}
>
  {#if !candidate}<dl>
      <dt>{ru ? 'В клавиатуре' : 'On keyboard'}</dt>
      <dd>
        {native.label}<span>{ru ? 'Общее для всех профилей' : 'Shared by all profiles'}</span>
      </dd>
      {#if binding}<dt>DeckWeave</dt>
        <dd>{binding.label}<span>{ru ? 'Профиль' : 'Profile'}: {profileName}</span></dd>{/if}
    </dl>
    <p class="assignment-status" role="status">{phase}</p>
    {#if !native.replaceable && native.observed && !owner}<p class="assignment-hint">
        {ru
          ? 'Это аппаратное назначение пока доступно только для просмотра.'
          : 'This onboard assignment is currently read-only.'}
      </p>{/if}{/if}
  {#if candidate}<div class="assignment-preview replacement-preview">
      <strong title={`${previewNative?.label ?? native.label} → ${candidate.label}`}
        >{previewNative?.label ?? native.label} → {candidate.label}</strong
      >
      <div class="assignment-buttons">
        <button
          class="primary"
          disabled={busy ||
            !connected ||
            !(previewNative ?? native).replaceable ||
            !previewCurrent ||
            !canReplace}
          onclick={onconfirm}
          >{busy
            ? ru
              ? 'Запись и проверка…'
              : 'Writing and verifying…'
            : ru
              ? 'Использовать DeckWeave'
              : 'Use DeckWeave'}</button
        ><button disabled={busy} onclick={oncancel}>{ru ? 'Отмена' : 'Cancel'}</button>
      </div>
      <p>
        {ru
          ? `Назначение клавиши изменится для всех профилей; действие сохранится в профиле «${profileName}». После полного выхода из DeckWeave клавиша отправит служебный F-код без запуска действия.`
          : `The keyboard assignment changes for all profiles; the action is saved in “${profileName}”. After DeckWeave exits, the key emits its internal F-key without running the action.`}
      </p>
      {#if !previewCurrent}<p class="assignment-error">
          {ru
            ? 'Состояние клавиатуры изменилось. Отмените предложение и выберите действие заново.'
            : 'The keyboard state changed. Cancel this proposal and select the action again.'}
        </p>{/if}
    </div>
  {:else if binding && !verified && canReplace}<button
      disabled={busy || !connected || !native.replaceable}
      onclick={onsetup}>{ru ? 'Завершить настройку привязки' : 'Complete keyboard setup'}</button
    >{/if}
  {#if owner && owner.phase !== 'released' && (!verified || route?.state !== 'active')}<button
      disabled={busy || !canReconcile}
      onclick={onreconcile}
      >{ru ? 'Проверить привязку клавиатуры' : 'Check keyboard connection'}</button
    >{/if}
  {#if owner && owner.phase !== 'released'}<details class="assignment-details">
      <summary>{ru ? 'Аппаратная привязка' : 'Keyboard trigger'}</summary>
      <p>F{owner.alias} · {owner.outcome}</p>
      {#if !owner.original}<p>
          {ru
            ? 'Исходное назначение неизвестно. Автоматическое восстановление недоступно.'
            : 'The original assignment is unknown. Automatic restoration is unavailable.'}
        </p>{/if}
      {#if restoring}<div class="assignment-preview">
          <p>
            {ru
              ? 'Действия следующих профилей сохранятся, но перестанут запускаться этой клавишей:'
              : 'Actions in these profiles remain saved, but this key will no longer launch them:'}
            {affectedProfiles.join(', ') || '—'}
          </p>
          <div class="assignment-buttons">
            <button
              disabled={!canRestore}
              onclick={() => {
                restoring = false;
                onrestore();
              }}>{ru ? 'Восстановить назначение клавиатуры' : 'Restore keyboard assignment'}</button
            ><button disabled={busy} onclick={() => (restoring = false)}
              >{ru ? 'Отмена' : 'Cancel'}</button
            >
          </div>
        </div>
      {:else}<button disabled={!canRestore} onclick={() => (restoring = true)}
          >{ru ? 'Восстановить назначение клавиатуры…' : 'Restore keyboard assignment…'}</button
        >{/if}
    </details>{/if}
  {#if route?.error}<p class="assignment-error" role="alert">
      {keyboardActionError(route.error, ru)}
    </p>{/if}
</section>

<style>
  .keyboard-assignment {
    grid-column: 1/-1;
    min-width: 0;
    padding: 10px 0;
    border-bottom: 1px solid #41454e;
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 6px 14px;
    margin: 0;
    font-size: 12px;
  }
  dt {
    color: #b4bdcb;
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  dd span {
    color: #b4bdcb;
    margin-left: 10px;
  }
  p {
    font-size: 12px;
    line-height: 1.45;
    margin: 8px 0;
  }
  .assignment-status {
    color: #e6e8ed;
  }
  .assignment-hint,
  .assignment-details {
    color: #b4bdcb;
  }
  .assignment-details {
    margin-top: 8px;
    font-size: 12px;
  }
  .assignment-details summary {
    cursor: pointer;
  }
  .assignment-preview {
    padding: 10px;
    background: #292d35;
    border: 1px solid #41454e;
    border-radius: 7px;
    margin: 8px 0;
  }
  .assignment-preview strong {
    font-size: 13px;
    overflow-wrap: anywhere;
  }
  .replacement-preview {
    margin-top: 0;
    padding: 8px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px 12px;
    align-items: center;
  }
  .replacement-preview strong {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    line-height: 1.3;
  }
  .replacement-preview p {
    grid-column: 1/-1;
    margin: 0;
    line-height: 1.35;
  }
  @media (max-width: 680px) {
    .replacement-preview {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .assignment-buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .assignment-error {
    color: #ffb0aa;
  }
</style>
