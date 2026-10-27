<script lang="ts">
  import Icon from '@iconify/svelte'
  import { api, providersStore, tagRecordsStore, groupsStore, peopleStore } from '$lib/tauri'
  import type { Inbox, InboxMember, InboxMemberKind } from '$lib/tauri/types'
  import { Button, Field, IconPicker, Select, Sheet, Switch, TagField, TagMark } from '$lib/components/ui'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { loadInboxes } from '$lib/inboxes'
  import { layoutSettings, putLayoutSetting, sortKey, countKey, SORT_CHOICES } from '$lib/layoutSettings'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    inbox: Inbox | null
    onclose: () => void
    onnewgroup?: (inbox: Inbox) => void
    ondeleted?: (inbox: Inbox) => void
  }

  let { inbox, onclose, onnewgroup, ondeleted }: Props = $props()

  let name = $state(inbox?.name ?? '')
  let icon = $state<string | null>(inbox?.icon ?? null)
  let members = $state<InboxMember[]>(inbox ? [...inbox.members] : [])
  let peopleQuery = $state('')
  let picking = $state(false)
  let busy = $state(false)
  let sort = $state(inbox ? ($layoutSettings[sortKey(inbox.id)] ?? 'manual') : 'manual')
  let counting = $state(inbox ? $layoutSettings[countKey(inbox.id)] !== 'off' : true)
  let confirming = $state(false)
  let deleting = $state(false)
  let failure = $state<string | null>(null)

  const people = $derived($peopleStore.filter((p) => p.is_human))
  const shownPeople = $derived.by(() => {
    const q = peopleQuery.trim().toLowerCase()
    const chosen = people.filter((p) => has('person', p.id))
    if (!q) return chosen
    return [...chosen, ...people.filter((p) => !has('person', p.id) && (p.display_name.toLowerCase().includes(q) || (p.primary_email ?? '').toLowerCase().includes(q))).slice(0, 8)]
  })

  function has(kind: InboxMemberKind, id: string): boolean {
    return members.some((m) => m.kind === kind && m.member_id === id)
  }

  function toggle(kind: InboxMemberKind, id: string) {
    members = has(kind, id) ? members.filter((m) => !(m.kind === kind && m.member_id === id)) : [...members, { kind, member_id: id }]
  }

  async function save() {
    if (!name.trim()) return
    busy = true
    failure = null
    try {
      if (inbox) {
        await api.putInbox({ inbox_id: inbox.id, name, icon: icon ?? '' })
        if (!inbox.is_default) {
          await api.putInboxMembers({ inbox_id: inbox.id, replace: members })
          await putLayoutSetting(sortKey(inbox.id), sort === 'manual' ? null : sort)
          await putLayoutSetting(countKey(inbox.id), counting ? null : 'off')
        }
      } else {
        await api.postInbox({ name, icon, members })
      }
      await loadInboxes()
      onclose()
    } catch (e) {
      failure = String(e)
    } finally {
      busy = false
    }
  }

  async function remove() {
    if (!inbox || deleting) return
    deleting = true
    failure = null
    try {
      await api.deleteInbox(inbox.id)
      await loadInboxes()
      ondeleted?.(inbox)
      onclose()
    } catch (e) {
      failure = String(e)
      confirming = false
    } finally {
      deleting = false
    }
  }

  const check ='flex h-8 w-full items-center gap-2.5 rounded-[0.5rem] px-2 text-left text-[13.5px] text-ink-2 hover:bg-paper-3 hover:text-ink'
</script>

<Sheet label={inbox ? t('settings.inbox.edit', { name: inbox.name }) : t('settings.inbox.new')} width="34rem" testid="inbox-editor" {onclose}>
  <form class="flex flex-col gap-4 p-5" onsubmit={(e) => { e.preventDefault(); save() }}>
    <Field data-testid="inbox-editor-name" bind:value={name} icon={icon ?? 'lucide:inbox'} onicon={() => (picking = true)} placeholder={t('settings.inbox.namePlaceholder')} aria-label={t('settings.inbox.name')} />

    {#if inbox && !inbox.is_default}
      <TagField objectType="inbox" objectId={inbox.id} />

      <div class="flex items-end gap-4">
        <label class="flex min-w-0 flex-1 flex-col gap-1 text-[13px] text-ink-2">
          {t('app.rail.sortOrder')}
          <Select testid="inbox-editor-sort" bind:value={sort} options={SORT_CHOICES.map((entry) => ({ value: entry.value, label: t(entry.label) }))} />
        </label>
        <span class="flex h-8 items-center gap-2 text-[13px] text-ink">
          {t('app.rail.showCount')}
          <Switch testid="inbox-editor-count" label={t('app.rail.showCount')} bind:checked={counting} />
        </span>
      </div>
    {/if}

    {#if inbox?.is_default}
      <p class="text-[13px] text-ink-3">{t('settings.inbox.catchAll')}</p>
    {:else}
      <p class="text-[13px] text-ink-3">{t('settings.inbox.explain')}</p>

      <section class="flex flex-col gap-1">
        <h3 class="text-[12.5px] font-medium text-ink-3">{t('settings.inbox.accounts')}</h3>
        {#each $providersStore.data as provider (provider.id)}
          <button type="button" class={check} aria-pressed={has('provider', provider.id)} onclick={() => toggle('provider', provider.id)}>
            <Icon icon={has('provider', provider.id) ? 'lucide:square-check' : 'lucide:square'} class="shrink-0 text-[14px] {has('provider', provider.id) ? 'text-accent' : 'text-ink-3'}" />
            <ProviderIcon providers={[provider]} size="sm" max={1} />
            <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{provider.name}</span>
          </button>
        {/each}
      </section>

      <section class="flex flex-col gap-1">
        <h3 class="text-[12.5px] font-medium text-ink-3">{t('settings.inbox.people')}</h3>
        <Field bind:value={peopleQuery} size="sm" placeholder={t('settings.inbox.findPerson')} aria-label={t('settings.inbox.findPerson')} />
        {#each shownPeople as person (person.id)}
          <button type="button" class={check} aria-pressed={has('person', person.id)} onclick={() => toggle('person', person.id)}>
            <Icon icon={has('person', person.id) ? 'lucide:square-check' : 'lucide:square'} class="shrink-0 text-[14px] {has('person', person.id) ? 'text-accent' : 'text-ink-3'}" />
            <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{person.display_name}</span>
            <span class="font-mono text-[11px] text-ink-3">{person.primary_email ?? ''}</span>
          </button>
        {/each}
      </section>

      {#if $tagRecordsStore.length}
        <section class="flex flex-col gap-1">
          <h3 class="text-[12.5px] font-medium text-ink-3">{t('settings.inbox.tags')}</h3>
          {#each $tagRecordsStore as label (label.id)}
            <button type="button" class={check} aria-pressed={has('tag', label.id)} onclick={() => toggle('tag', label.id)}>
              <Icon icon={has('tag', label.id) ? 'lucide:square-check' : 'lucide:square'} class="shrink-0 text-[14px] {has('tag', label.id) ? 'text-accent' : 'text-ink-3'}" />
              <span class="flex w-4 shrink-0 justify-center"><TagMark tag={label} /></span>
              <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{label.name}</span>
            </button>
          {/each}
        </section>
      {/if}

      {#if $groupsStore.length}
        <section class="flex flex-col gap-1">
          <h3 class="text-[12.5px] font-medium text-ink-3">{t('settings.inbox.groups')}</h3>
          {#each $groupsStore as group (group.id)}
            <button type="button" class={check} aria-pressed={has('group', group.id)} onclick={() => toggle('group', group.id)}>
              <Icon icon={has('group', group.id) ? 'lucide:square-check' : 'lucide:square'} class="shrink-0 text-[14px] {has('group', group.id) ? 'text-accent' : 'text-ink-3'}" />
              <Icon icon={group.icon || 'lucide:folder'} class="shrink-0 text-[13px] text-ink-3" />
              <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{group.name}</span>
            </button>
          {/each}
        </section>
      {/if}
    {/if}

    {#if inbox && !inbox.is_default && onnewgroup}
      {@const current = inbox}
      <div class="flex flex-wrap items-center gap-2">
        <Button size="sm" icon="lucide:folder-plus" testid="inbox-editor-new-group" onclick={() => onnewgroup(current)}>{t('app.rail.newGroupInside')}</Button>
      </div>
    {/if}

    {#if failure}
      <p class="text-[13px] text-error-600">{failure}</p>
    {/if}

    {#if inbox && !inbox.is_default}
      <div class="flex min-h-8 items-center gap-2 border-t border-line pt-4">
        {#if confirming}
          <p class="min-w-0 flex-1 text-[13px] text-ink-2">{t('app.rail.confirmDeleteInbox', { name: inbox.name })}</p>
          <Button size="sm" onclick={() => (confirming = false)}>{t('common.cancel')}</Button>
          <Button size="sm" variant="danger" icon="lucide:trash-2" testid="inbox-editor-delete-confirm" busy={deleting} onclick={remove}>{t('common.delete')}</Button>
        {:else}
          <Button size="sm" variant="danger" icon="lucide:trash-2" testid="inbox-editor-delete" onclick={() => (confirming = true)}>{t('app.rail.deleteInbox')}</Button>
        {/if}
      </div>
    {/if}
  </form>
  {#snippet footer()}
    <span class="text-[12px] text-ink-3">{t('settings.inbox.links', { count: members.length })}</span>
    <span class="ml-auto"></span>
    <Button size="sm" onclick={onclose}>{t('common.cancel')}</Button>
    <Button size="sm" variant="primary" testid="inbox-editor-save" keys="↵" {busy} disabled={!name.trim()} onclick={save}>{inbox ? t('common.save') : t('settings.inbox.create')}</Button>
  {/snippet}
</Sheet>

{#if picking}
  <IconPicker value={icon} fallback="lucide:inbox" onpick={(next) => (icon = next)} onclose={() => (picking = false)} />
{/if}
