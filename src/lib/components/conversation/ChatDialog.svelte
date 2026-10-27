<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { api, conversationStore, conversationsStore, chatDetailsStore, closeChatDetails, putChatMode, readStore, peopleStore, myUserIds } from '$lib/tauri'
  import { putDetailsOpen } from '$lib/tauri/profile'
  import { CHAT_MODES, NOTIFY_LEVELS } from '$lib/tauri/chats'
  import type { Conversation, LocalField, Tag } from '$lib/tauri/types'
  import { chatAvatar } from '$lib/avatar'
  import { mergedItems, openTag, rowModel } from '$lib/inbox'
  import { objectTagsStore, tagIdsOn, tagRecordsStore, toggleObjectTag } from '$lib/tauri/tags'
  import { boxOf, type Box } from '$lib/popover'
  import { modalsShow } from '$lib/modals'
  import { Avatar, Button, ColourSwatch, Field, IconButton, Segmented, Sheet, Switch, TagMark, TagPicker } from '$lib/components/ui'
  import ConversationInfo from './details/ConversationInfo.svelte'
  import Shared from './details/Shared.svelte'
  import Appearance from '$lib/components/settings/Appearance.svelte'
  import { conversationKey } from '$lib/appearance'
  import { t } from '$lib/i18n.svelte'

  const conversation = $derived<Conversation | null>($chatDetailsStore ? (($conversationStore.data?.id === $chatDetailsStore ? $conversationStore.data : null) ?? $conversationsStore.data.find((c) => c.id === $chatDetailsStore) ?? null) : null)
  const local = $derived(conversation?.overrides ?? {})
  const group = $derived($chatDetailsStore ? ($mergedItems.get($chatDetailsStore) ?? null) : null)
  const shown = $derived(group ? rowModel(group, $peopleStore, $myUserIds) : null)
  const avatar = $derived(conversation ? (shown?.avatar ?? chatAvatar(conversation, $peopleStore, $myUserIds)) : null)
  const title = $derived(shown?.title ?? conversation?.name ?? '')
  const provider = $derived(conversation?.providers[0] ?? null)
  const renamable = $derived(!group && !!conversation && !!provider && (conversation.type === 'group_message' || conversation.type === 'channel'))
  const reading = $derived(!!conversation && $readStore.has(conversation.id))
  const tagsOf = (ids: string[]): Tag[] => [...new Set(ids)].map((id) => $tagRecordsStore.find((tag) => tag.id === id)).filter((tag): tag is Tag => !!tag)
  const groupTags = $derived(group ? tagsOf(tagIdsOn($objectTagsStore, 'merge_group', group.groupId)) : [])
  const memberTags = $derived(group ? tagsOf(group.conversations.flatMap((c) => c.tags.map((tag) => tag.tag_id))) : [])

  let tagAt = $state<Box | null>(null)

  let name = $state('')
  let description = $state('')
  let alsoProvider = $state(false)
  let busy = $state(false)
  let failure = $state<string | null>(null)
  let note = $state<string | null>(null)
  let seeded = $state<string | null>(null)

  $effect(() => {
    if (!conversation || seeded === conversation.id) return
    seeded = conversation.id
    name = title
    description = conversation.overrides.description ?? ''
    alsoProvider = false
    failure = null
    note = null
  })

  $effect(() => {
    if (!$chatDetailsStore) seeded = null
  })

  async function refresh(id: string) {
    const [list] = await Promise.all([api.getConversations(), $conversationStore.data?.id === id ? api.getCurrentConversation().then((current) => conversationStore.set(current)) : Promise.resolve()])
    conversationsStore.set(list)
  }

  async function run(work: () => Promise<unknown>) {
    if (!conversation) return
    const id = conversation.id
    busy = true
    failure = null
    try {
      await work()
      await refresh(id)
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }

  function putLocal(field: LocalField, value: string | null) {
    if (!conversation) return Promise.resolve()
    const target = { object_type: 'conversation' as const, object_id: conversation.id, field }
    return value?.trim() ? api.putObjectOverride({ ...target, value }) : api.deleteObjectOverride(target)
  }

  function rename() {
    const current = conversation
    const next = name.trim()
    if (!current || !next || (next === title && !alsoProvider)) return
    run(async () => {
      if (next === current.provider_name && !alsoProvider) {
        await putLocal('name', null)
        return
      }
      const before = current.provider_name
      await api.putConversation({ conversation_id: current.id, name: next, rename_on_provider: alsoProvider && renamable })
      if (alsoProvider && renamable) {
        const fresh = await api.getConversations()
        const after = fresh.data.find((c) => c.id === current.id)
        note = after && after.provider_name !== before && !after.overrides.name ? t('chats.details.renamedOn', { provider: provider?.name ?? '' }) : t('chats.details.keptOn', { provider: provider?.name ?? '' })
      }
      alsoProvider = false
    })
  }

  function resetName() {
    const current = conversation
    if (!current) return
    name = current.provider_name
    run(() => putLocal('name', null))
  }

  function saveDescription() {
    const next = description.trim()
    if (next === (local.description ?? '')) return
    run(() => putLocal('description', next || null))
  }

  function pickPicture() {
    const current = conversation
    if (!current) return
    modalsShow({ currentImage: current.image, title: t('chats.details.picture'), library: true, resettable: !!current.overrides.avatar_data, onpick: (image) => run(() => putLocal('avatar_data', image)) })
  }

  function setLevel(level: string) {
    const current = conversation
    if (!current) return
    run(() => api.putConversationNotification({ conversation_id: current.id, notification_level: level }))
  }

  const leavable = $derived(!!conversation && !!provider && ((conversation.type === 'channel' && provider.capabilities.supports_channel_join) || (conversation.type === 'group_message' && provider.capabilities.can_add_members)))
  const archivable = $derived(!!conversation && !!provider && conversation.type === 'channel' && provider.capabilities.supports_channel_archive)
  let confirming = $state<'leave' | 'archive' | null>(null)

  function confirmOr(kind: 'leave' | 'archive', act: () => void) {
    if (confirming === kind) {
      confirming = null
      act()
    } else confirming = kind
  }

  function leave() {
    const current = conversation
    if (!current) return
    run(async () => {
      const left = await api.deleteConversationMembership(current.id)
      if (!left) throw new Error(t('modals.channel.notLeft', { provider: provider?.name ?? '' }))
      closeChatDetails()
    })
  }

  function archive() {
    const current = conversation
    if (!current) return
    run(async () => {
      const archived = await api.putConversationState(current.id, true)
      if (!archived) throw new Error(t('modals.channel.notArchived', { provider: provider?.name ?? '' }))
      note = t('modals.channel.archived', { provider: provider?.name ?? '' })
    })
  }

  function openDetails() {
    const id = conversation?.id
    if (id && $conversationStore.data?.id !== id) goto(conversationHref(id))
    putDetailsOpen(true)
    closeChatDetails()
  }
</script>

{#snippet localMark(field: LocalField, reset: () => void)}
  {#if local[field]}
    <span class="flex shrink-0 items-center gap-1 text-[11.5px] text-ink-3">
      <Icon icon="lucide:hard-drive" class="text-[11px]" />{t('people.local.badge')}
      <IconButton testid={`chat-dialog-reset-${field}`} size="sm" icon="lucide:rotate-ccw" label={t('people.local.reset')} disabled={busy} onclick={reset} />
    </span>
  {/if}
{/snippet}

{#if conversation && avatar}
  <Sheet testid="chat-dialog" label={t('chats.details.title')} width="34rem" movable onclose={closeChatDetails}>
    <header class="glass sticky top-0 z-10 grid grid-cols-[auto_minmax(0,1fr)] items-center gap-4 border-b border-line px-5 pb-3 pt-2">
      <Avatar testid="chat-dialog-avatar" source={avatar} size="xl" label={t('chats.details.changePicture')} hint={t('chats.details.changePicture')} onclick={pickPicture} />
      <div class="flex min-w-0 flex-col gap-1">
        <h2 class="truncate text-[17px] font-semibold tracking-tight text-ink">{title}</h2>
        <p class="truncate text-[12.5px] text-ink-3">{group ? t('conversation.overview.chatsTogether', { count: group.conversations.length }) : conversation.name !== conversation.provider_name ? t('chats.details.providerName', { name: conversation.provider_name }) : provider ? provider.name : t('chats.type.local')}</p>
      </div>
    </header>
    <div class="flex flex-col gap-5 px-5 pb-5 pt-4">

      <section class="flex flex-col gap-2">
        <div class="flex h-7 items-center justify-between gap-2">
          <p class="text-[13px] text-ink-3">{t('common.name')}</p>
          {@render localMark('name', resetName)}
        </div>
        <form data-testid="chat-dialog-name-form" onsubmit={(e) => { e.preventDefault(); rename() }}>
          <Field data-testid="chat-dialog-name" bind:value={name} aria-label={t('common.name')} onicon={pickPicture} onblur={() => !alsoProvider && rename()}>
            {#snippet glyph()}<Avatar source={avatar} size="sm" />{/snippet}
          </Field>
        </form>
        {#if renamable && provider}
          <div class="flex items-center gap-3">
            <Switch testid="chat-dialog-also-provider" label={t('chats.details.alsoRename', { provider: provider.name })} bind:checked={alsoProvider} />
            <p class="min-w-0 flex-1 text-[12.5px] text-ink-2">{t('chats.details.alsoRename', { provider: provider.name })}</p>
            {#if alsoProvider}<Button testid="chat-dialog-rename" size="sm" variant="primary" {busy} onclick={rename}>{t('chats.details.rename')}</Button>{/if}
          </div>
        {/if}
        <p data-testid="chat-dialog-note" data-noted={!!note} class="text-[12px] text-ink-3">{note ?? t('chats.details.localHint')}</p>
      </section>

      <section class="flex flex-col gap-2">
        <div class="flex h-7 items-center justify-between gap-2">
          <p class="text-[13px] text-ink-3">{t('chats.details.picture')}</p>
          {@render localMark('avatar_data', () => run(() => putLocal('avatar_data', null)))}
        </div>
        <div class="flex flex-wrap items-center gap-x-5 gap-y-2">
          <div class="flex min-w-0 items-center gap-2">
            <Button testid="chat-dialog-picture" size="sm" icon="lucide:image-up" onclick={pickPicture}>{t('chats.details.choosePicture')}</Button>
            <span data-testid="chat-dialog-picture-status" class="text-[12px] text-ink-3">{conversation.image ? (local.avatar_data ? t('chats.details.pictureLocal') : t('chats.details.pictureProvider')) : t('chats.details.pictureGenerated')}</span>
          </div>
          <div class="flex items-center gap-1.5">
            <ColourSwatch size="sm" value={local.color ?? null} clearable label={t('chats.details.colour')} onpick={(color) => run(() => putLocal('color', color))} />
            <span class="text-[12.5px] text-ink-2">{t('chats.details.colour')}</span>
          </div>
        </div>
      </section>

      <section class="flex flex-col gap-2">
        <div class="flex h-7 items-center justify-between gap-2">
          <p class="text-[13px] text-ink-3">{t('chats.details.description')}</p>
          {@render localMark('description', () => { description = ''; run(() => putLocal('description', null)) })}
        </div>
        <Field data-testid="chat-dialog-description" bind:value={description} size="sm" placeholder={t('chats.details.descriptionPlaceholder')} aria-label={t('chats.details.description')} onblur={saveDescription} onkeydown={(e) => e.key === 'Enter' && saveDescription()} />
      </section>

      {#if group}
        <section class="flex flex-col gap-2">
          <div class="flex h-7 items-center">
            <p class="text-[13px] text-ink-3">{t('chats.details.tags')}</p>
          </div>
          <div data-testid="chat-dialog-tags" class="flex flex-wrap items-center gap-1">
            {#each groupTags as tag (tag.id)}
              <span data-testid="chat-dialog-tag" data-tag={tag.id} class="inline-flex min-w-0 items-center gap-0.5 rounded-full bg-paper-3/60 py-px pl-2 pr-0.5 text-[11px] leading-4">
                <TagMark {tag} size="xs" label ink="ink-2" />
                <button type="button" data-testid="chat-dialog-tag-remove" class="flex size-4 shrink-0 items-center justify-center rounded-full text-ink-3 hover:bg-paper-3 hover:text-ink" aria-label={t('people.tags.remove', { name: tag.name })} onclick={() => toggleObjectTag('merge_group', [group.groupId], tag.id).catch((error) => (failure = String(error)))}>
                  <Icon icon="lucide:x" class="text-[10px]" />
                </button>
              </span>
            {/each}
            <button type="button" data-testid="chat-dialog-tag-add" class="inline-flex items-center gap-1 rounded-full px-2 py-px text-[11px] leading-4 text-ink-2 hover:bg-paper-3 hover:text-ink" aria-haspopup="dialog" aria-expanded={!!tagAt} onclick={(e) => (tagAt = tagAt ? null : boxOf(e.currentTarget))}>
              <Icon icon="lucide:plus" class="text-[11px] text-ink-3" />{groupTags.length ? t('common.add') : t('people.tags.add')}
            </button>
          </div>
          {#if memberTags.length}
            <div data-testid="chat-dialog-member-tags" class="flex flex-wrap items-center gap-1">
              <span class="mr-1 text-[12px] text-ink-3">{t('chats.details.fromChats')}</span>
              {#each memberTags as tag (tag.id)}
                <button type="button" data-testid="chat-dialog-member-tag" data-tag={tag.id} class="inline-flex min-w-0 cursor-pointer items-center rounded-full bg-paper-3/60 px-2 py-px text-[11px] leading-4 hover:bg-paper-3" onclick={() => { openTag(tag.id); closeChatDetails() }}><TagMark {tag} size="xs" label ink="ink-2" /></button>
              {/each}
            </div>
          {/if}
          {#if tagAt}
            <TagPicker objectType="merge_group" ids={[group.groupId]} at={tagAt} onclose={() => (tagAt = null)} />
          {/if}
        </section>
      {/if}

      <section class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 gap-y-3 border-t border-line pt-4">
        <p class="text-[13px] text-ink">{t('conversation.notify.label')}</p>
        <Segmented testid="chat-dialog-notify" size="sm" label={t('conversation.notify.label')} value={conversation.notification_level || 'all'} options={NOTIFY_LEVELS.map((l) => ({ value: l.value, label: t(l.label), icon: l.icon }))} onchange={setLevel} />
        {#if group}
          <p class="text-[13px] text-ink">{t('chats.mode.label')}</p>
          <Segmented testid="chat-dialog-mode" size="sm" label={t('chats.mode.label')} value={reading ? 'read' : 'write'} options={CHAT_MODES.map((m) => ({ value: m.value, label: t(m.label), icon: m.icon }))} onchange={(v) => run(() => putChatMode(conversation.id, v as 'read' | 'write'))} />
        {/if}
      </section>

      {#if !group && provider && (leavable || archivable)}
        <section class="flex flex-col gap-2 border-t border-line pt-4" data-testid="chat-dialog-membership">
          <p class="text-[13px] text-ink-3">{t('modals.channel.membership', { provider: provider.name })}</p>
          <div class="flex flex-wrap items-center gap-2">
            {#if leavable}
              <Button testid="chat-dialog-leave" size="sm" variant={confirming === 'leave' ? 'danger' : 'secondary'} icon="lucide:log-out" {busy} onclick={() => confirmOr('leave', leave)}>{t(confirming === 'leave' ? 'modals.channel.leaveConfirm' : 'modals.channel.leave')}</Button>
            {/if}
            {#if archivable}
              <Button testid="chat-dialog-archive" size="sm" variant={confirming === 'archive' ? 'danger' : 'secondary'} icon="lucide:archive" {busy} onclick={() => confirmOr('archive', archive)}>{t(confirming === 'archive' ? 'modals.channel.archiveConfirm' : 'modals.channel.archive')}</Button>
            {/if}
            {#if confirming}<Button size="sm" variant="ghost" onclick={() => (confirming = null)}>{t('common.cancel')}</Button>{/if}
          </div>
          <p class="text-[12px] text-ink-3">{t(confirming === 'archive' ? 'modals.channel.archiveHint' : 'modals.channel.leaveHint', { provider: provider.name })}</p>
        </section>
      {/if}

      {#if !group}
        <div class="border-t border-line">
          <ConversationInfo {conversation} />
          <Shared scope={{ conversation_ids: [conversation.id] }} />
          <div class="pt-4">
            <p class="text-[13px] font-semibold text-ink">{t('conversation.details.appearance')}</p>
            <p class="text-[12.5px] text-ink-3">{t('conversation.details.appliesChat')}</p>
            <Appearance scope={conversationKey(conversation.id)} />
          </div>
        </div>
      {/if}
      {#if failure}<p data-testid="chat-dialog-error" class="text-[12.5px] text-error-600">{failure}</p>{/if}
    </div>
    {#snippet footer()}
      {#if !group}<Button testid="chat-dialog-panel" icon="lucide:panel-right-open" onclick={openDetails}>{t('chats.details.moreInPanel')}</Button>{/if}
      <Button testid="chat-dialog-close" class="ml-auto" onclick={closeChatDetails}>{t('common.close')}</Button>
    {/snippet}
  </Sheet>
{/if}
