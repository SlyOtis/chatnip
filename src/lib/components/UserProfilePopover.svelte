<script lang="ts">
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { anchored, pointBox } from '$lib/popover'
  import type { User } from '$lib/tauri/types'
  import { presenceStore, requestPresence, type PresenceStatus } from '$lib/tauri/presence'
  import PresenceDot from '$lib/components/PresenceDot.svelte'
  import { conversationStore, peopleStore, loadPeople, tagsStore, loadTags, objectTagsStore, myUserIds, providersStore } from '$lib/tauri'
  import { modalsShow } from '$lib/modals'
  import { Avatar, TagMark } from '$lib/components/ui'
  import { openPerson, personOf, personProfileHref } from '$lib/tauri/people'
  import { chatsStore } from '$lib/tauri/chats'
  import { userAvatar } from '$lib/avatar'
  import { providerIcon, providerLabel } from '$lib/providerIcons'
  import { t } from '$lib/i18n.svelte'
  import EmojiGlyph from '$lib/components/shared/EmojiGlyph.svelte'

  interface Props {
    user: User
    x: number
    y: number
    visible: boolean
    onclose: () => void
  }

  let { user, x, y, visible, onclose }: Props = $props()


  let presence = $derived($presenceStore.get(user.id))
  $effect(() => {
    requestPresence([user.id])
  })
  let status = $derived<PresenceStatus>(presence?.status ?? 'offline')
  let isOwnProfile = $derived($myUserIds.has(user.id))
  let ownAccount = $derived(($providersStore.data ?? []).find((provider) => provider.account?.user_id === user.id && provider.capabilities.supports_presence) ?? null)
  let person = $derived(personOf($peopleStore, { user_id: user.id }))
  let personTags = $derived(person ? [...new Set($objectTagsStore.filter((row) => (row.object_type === 'person' && row.object_id === person.id) || (row.object_type === 'identity' && person.identities.some((identity) => identity.id === row.object_id))).map((row) => row.tag_id))] : [])
  let tags = $derived($tagsStore.filter((tag) => personTags.includes(tag.id)))

  $effect(() => {
    if (!visible) return
    if (!$peopleStore.length) loadPeople().catch(() => {})
    if (!$tagsStore.length) loadTags().catch(() => {})
  })

  function openProfile() {
    if (!person) return
    onclose()
    goto(personProfileHref(person.id))
  }

  function getStatusLabel(s: PresenceStatus): string {
    switch (s) {
      case 'online': return t('app.presence.online')
      case 'away': return t('app.presence.away')
      case 'dnd': return t('app.presence.dnd')
      case 'offline': return t('app.presence.offline')
    }
  }

  function handleMessage() {
    onclose()
    const conversation = $conversationStore.data
    if (conversation?.type === 'direct_message' && (conversation.members || []).some((m) => m.id === user.id)) return
    if (person) openPerson(person, $chatsStore, goto).catch(() => {})
  }


  let providerInfo = $derived.by(() => {
    const conversation = $conversationStore.data
    if (!conversation?.providers?.length) return null
    return conversation.providers[0]
  })

  const facts = $derived(
    [
      { icon: 'lucide:mail', value: user.email },
      { icon: 'lucide:phone', value: user.phone },
      { icon: 'lucide:users', value: user.department },
      { icon: 'lucide:building-2', value: user.company },
      { icon: 'lucide:map-pin', value: user.office_location },
    ].filter((f): f is { icon: string; value: string } => !!f.value)
  )
</script>

{#if visible}
  <div
    use:anchored={{ at: pointBox(x, y), align: 'start', onclose }}
    class="min-w-[17.5rem] max-w-[20rem] rounded-[var(--radius-sheet)] p-4 glass shadow-[0_12px_32px_rgb(0_0_0/0.28)]"
    role="dialog"
    aria-label={user.display_name || t('app.rail.profile')}
  >
    <div class="flex items-start gap-3">
      <div class="relative shrink-0">
        <Avatar source={userAvatar(user, $peopleStore)} size="xl" label={t('app.profilePopover.openProfile')} onclick={person ? openProfile : undefined} />
        <PresenceDot user_id={user.id} size="xl" />
      </div>
      <div class="min-w-0 flex-1">
        {#if person}
          <button type="button" class="block max-w-full truncate text-left text-[14px] font-semibold text-ink hover:underline" onclick={handleMessage}>{user.display_name || t('common.unknown')}</button>
        {:else}
          <h3 class="truncate text-[14px] font-semibold text-ink">{user.display_name || t('common.unknown')}</h3>
        {/if}
        {#if user.job_title}
          <p class="truncate text-[12px] text-ink-2">{user.job_title}</p>
        {/if}
        <p class="mt-0.5 font-mono text-[11px] text-ink-3">{getStatusLabel(status)}</p>
      </div>
    </div>

    {#if presence?.status_emoji || presence?.status_text}
      <div class="mt-3 flex items-center gap-1.5 rounded-[var(--radius-row)] bg-paper-2 px-2.5 py-1.5">
        {#if presence.status_emoji}
          <span class="text-[13px]"><EmojiGlyph emoji={presence.status_emoji} /></span>
        {/if}
        {#if presence.status_text}
          <span class="truncate text-[12px] text-ink-2">{presence.status_text}</span>
        {/if}
      </div>
    {/if}

    {#if tags.length}
      <div class="mt-3 flex flex-wrap gap-1.5">
        {#each tags as tag (tag.id)}
          <span class="flex h-5 items-center gap-1.5 rounded-full bg-paper-2 px-2 text-[11.5px] text-ink-2">
            <TagMark {tag} size="xs" label ink="ink-2" />
          </span>
        {/each}
      </div>
    {/if}

    {#if facts.length > 0}
      <div class="mt-3 flex flex-col gap-1.5 border-t border-line pt-3">
        {#each facts as fact (fact.icon)}
          <div class="flex items-center gap-2.5">
            <Icon icon={fact.icon} class="shrink-0 text-[14px] text-ink-3" />
            <span class="selectable truncate text-[12px] text-ink-2">{fact.value}</span>
          </div>
        {/each}
      </div>
    {/if}

    {#if providerInfo}
      <div class="mt-3 flex items-center gap-2 border-t border-line pt-3">
        <Icon icon={providerIcon(providerInfo, $conversationStore.data?.type)} width={14} height={14} />
        <span class="font-mono text-[10px] text-ink-3">{providerLabel(providerInfo)}</span>
      </div>
    {/if}

    <div class="mt-3 flex gap-2 border-t border-line pt-3">
      <button
        onclick={handleMessage}
        class="flex flex-1 items-center justify-center gap-1.5 rounded-[var(--radius-row)] bg-accent px-3 py-1.5 text-[12px] font-semibold text-accent-ink hover:brightness-110"
      >
        <Icon icon="lucide:message-circle" class="text-[14px]" />
        {t('app.profilePopover.message')}
      </button>
      {#if person}
        <button
          onclick={openProfile}
          class="flex items-center justify-center gap-1.5 rounded-[var(--radius-row)] bg-paper-2 px-3 py-1.5 text-[12px] font-medium text-ink-2 hairline hover:bg-paper-3 hover:text-ink"
        >
          <Icon icon="lucide:id-card" class="text-[14px]" />
          {t('app.profilePopover.openProfile')}
        </button>
      {/if}
      {#if isOwnProfile && ownAccount}
        <button class="flex items-center justify-center gap-1.5 rounded-[var(--radius-row)] bg-paper-2 px-3 py-1.5 text-[12px] font-medium text-ink-2 hairline hover:bg-paper-3 hover:text-ink" onclick={() => { if (ownAccount) { onclose(); modalsShow({ initial_text: presence?.status_text ?? '', initial_emoji: presence?.status_emoji ?? '', provider_id: ownAccount.id }) } }}>
          <Icon icon="lucide:pencil" class="text-[14px]" />
          {t('app.profilePopover.setStatus')}
        </button>
      {/if}
    </div>
  </div>
{/if}
