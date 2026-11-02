<script lang="ts">
  import type { Client } from './client.svelte';
  import type { Device } from './types';
  import Icon from './Icon.svelte';
  let { client, device }: { client: Client; device: Device } = $props();
  const ru = $derived(client.snapshot?.configuration.locale === 'ru');
  let name = $state('');
  let deleting = $state<string | null>(null);
  $effect(() => {
    device.profile.id;
    deleting = null;
  });
  function named(type: 'add_profile' | 'rename_profile' | 'duplicate_profile', value: string) {
    if (!value.trim()) return Promise.resolve(false);
    const deviceId = device.id;
    const profileId = device.profile.id;
    return client.send((revision) => ({
      type,
      device_id: deviceId,
      profile_id: profileId,
      name: value.trim(),
      expected_revision: revision,
    }));
  }
</script>

<section class="profile-editor" aria-label={ru ? 'Профили' : 'Profiles'}>
  <div class="profile-list" role="group" aria-label={ru ? 'Наборы страниц' : 'Page sets'}>
    {#each [device.profile, ...device.profiles] as profile (profile.id)}
      <button
        class:current={profile.id === device.profile.id}
        aria-pressed={profile.id === device.profile.id}
        disabled={client.busy}
        onclick={() =>
          void client.send((revision) => ({
            type: 'select_profile',
            device_id: device.id,
            profile_id: profile.id,
            expected_revision: revision,
          }))}
      >
        <Icon name="profile" /><strong>{profile.name}</strong><span
          >{profile.pages.filter((p) => !p.parent).length} {ru ? 'стр.' : 'pages'}</span
        >
      </button>
    {/each}
  </div>
  <div class="profile-fields">
    <div class="profile-row">
      <input
        aria-label={ru ? 'Имя профиля' : 'Profile name'}
        maxlength="80"
        value={device.profile.name}
        disabled={client.busy}
        onchange={async (e) => {
          const input = e.currentTarget;
          if (!(await named('rename_profile', input.value))) input.value = device.profile.name;
        }}
      />
      <button
        disabled={client.busy || device.profiles.length >= 31}
        onclick={() =>
          named(
            'duplicate_profile',
            device.profile.name.slice(0, 65) + (ru ? ' — копия' : ' — copy'),
          )}>{ru ? 'Дублировать' : 'Duplicate'}</button
      >
      <button
        title={ru ? 'Удалить выбранный профиль' : 'Delete selected profile'}
        aria-label={ru ? 'Удалить профиль' : 'Delete profile'}
        disabled={client.busy || !device.profiles.length}
        onclick={() => (deleting = device.profile.id)}><Icon name="trash" /></button
      >
    </div>
    <label class="profile-row"
      ><span>{ru ? 'Приложение' : 'Application'}</span><input
        aria-label={ru ? 'Приложение Smart Profile' : 'Smart Profile application'}
        value={device.profile.application ?? ''}
        placeholder="obs64.exe"
        disabled={client.busy}
        onchange={(e) => {
          const application = e.currentTarget.value;
          void client.send((revision) => ({
            type: 'profile_application',
            device_id: device.id,
            application: application.trim() || null,
            expected_revision: revision,
          }));
        }}
      /></label
    >
    <form
      class="profile-row"
      onsubmit={async (e) => {
        e.preventDefault();
        if (await named('add_profile', name)) name = '';
      }}
    >
      <input
        aria-label={ru ? 'Имя нового профиля' : 'New profile name'}
        placeholder={ru ? 'Новый набор страниц' : 'New page set'}
        maxlength="80"
        bind:value={name}
      />
      <button type="submit" disabled={client.busy || !name.trim() || device.profiles.length >= 31}
        ><Icon name="plus" />{ru ? 'Создать' : 'Create'}</button
      >
    </form>
    {#if deleting === device.profile.id}<div class="profile-delete" role="alert">
        <span
          >{ru
            ? `Удалить «${device.profile.name}» со всеми его страницами и действиями?`
            : `Delete “${device.profile.name}” and all its pages and actions?`}</span
        >
        <button
          disabled={client.busy}
          onclick={async () => {
            const deviceId = device.id;
            const profileId = device.profile.id;
            if (
              await client.send((revision) => ({
                type: 'delete_profile',
                device_id: deviceId,
                profile_id: profileId,
                expected_revision: revision,
              }))
            )
              deleting = null;
          }}>{ru ? 'Удалить профиль' : 'Delete profile'}</button
        >
        <button onclick={() => (deleting = null)}>{ru ? 'Отмена' : 'Cancel'}</button>
      </div>{/if}
  </div>
</section>

<style>
  .profile-editor {
    display: grid;
    grid-template-columns: minmax(140px, 1fr) minmax(260px, 2fr);
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
  }
  .profile-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 144px;
    overflow: auto;
  }
  .profile-list button {
    display: flex;
    align-items: center;
    gap: 6px;
    text-align: left;
  }
  .profile-list strong {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .profile-list span {
    color: var(--muted);
    white-space: nowrap;
  }
  .profile-fields {
    display: grid;
    align-content: start;
    gap: 6px;
    min-width: 0;
  }
  .profile-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .profile-row input {
    flex: 1;
    min-width: 0;
    width: 0;
  }
  .profile-delete {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .profile-delete span {
    flex-basis: 100%;
  }
  @media (max-width: 720px) {
    .profile-editor {
      grid-template-columns: 1fr;
    }
    .profile-list {
      max-height: 100px;
    }
  }
</style>
