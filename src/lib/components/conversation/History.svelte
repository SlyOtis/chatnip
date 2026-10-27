<script lang="ts">
  import { api, conversationStore, messagesStore, profileStore } from '$lib/tauri'
  import Icon from '@iconify/svelte'
  import { t } from '$lib/i18n.svelte'
  import type { Message } from '$lib/tauri'
  import MessageGroup from './messages/MessageGroup.svelte'
  import EmptyChat from './EmptyChat.svelte'
  import EmptySpinner from './EmptySpinner.svelte'
  import FoldDivider from './messages/FoldDivider.svelte'
  import { folds, foldPlan, isOpen } from '$lib/collapse'
  import { myUserIds, isMine } from '$lib/tauri/people'
  import { layoutSettings } from '$lib/layoutSettings'
  import { collapsedSections, holdTimeline, splitSections } from '$lib/sections.svelte'
  import SectionHeader from './messages/SectionHeader.svelte'
  import SectionSummaryCard from './messages/SectionSummaryCard.svelte'

  let scroll_container = $state<HTMLDivElement | null>(null);

  let allMessages = $derived($messagesStore.data || []);
  let isLoading = $derived($messagesStore.is_loading);
  let profile = $derived($profileStore.data);
  let timestampInterval = $derived(profile?.settings?.message_timestamp_interval || 5)

  type ReducedMessageGroup = { groups: Array<Message[]>, group: Message[], msg: Message | null }

  let groupedMessages = $derived.by(() => {
    const {groups, group} = allMessages.reduce<ReducedMessageGroup>((prev, curr) => {
      if (prev.msg) {
        const timeDiff = (new Date(curr.timestamp).getTime() - new Date(prev.msg.timestamp).getTime()) / 1000
        if (timeDiff > timestampInterval * 60 || prev.msg.sender_id !== curr.sender_id || curr.type === 'email') {
          return { ...prev, groups: [...prev.groups, prev.group], group: [curr], msg: curr }
        }
      }
      return { ...prev, group: [...prev.group, curr], msg: curr }
    }, { groups: [], group: [], msg: null});
    return group.length ? [...groups, group] : groups;
  })

  const conversation = $derived($conversationStore.data)
  const scope = $derived(conversation?.id ?? '')
  const split = $derived(splitSections($layoutSettings, groupedMessages, (group) => ({ key: group[0].id, first: group[0].timestamp, last: group[group.length - 1].timestamp, messages: group })))
  let plan = $derived(foldPlan(groupedMessages.map((group) => ({ id: group[0].id, conversationId: group[0].conversation_id, email: group[0].type === 'email', mine: isMine($myUserIds, group[0].sender_id), chat: group[0].type !== 'email' && group[0].type !== 'system', size: group.length, section: split.headed ? split.owner.get(group[0].id)?.id : undefined }))))
  const collapsed = $derived(split.headed ? collapsedSections($layoutSettings, scope) : new Set<string>())

  holdTimeline(() => (conversation ? { scope: conversation.id, source: { kind: 'conversation', id: conversation.id }, conversationId: conversation.id, title: conversation.name, sections: split.sections } : null))

  const firstUnread = $derived(allMessages.find((m) => !m.is_read && !m.deleted_datetime && !isMine($myUserIds, m.sender_id)) ?? null)
  const unreadGroup = $derived(firstUnread ? (groupedMessages.find((group) => group.some((m) => m.id === firstUnread.id))?.[0].id ?? null) : null)
  let divider = $state<HTMLDivElement | null>(null)
  let dividerAbove = $state(false)
  let landed: string | null = null

  $effect(() => {
    const id = conversation?.id ?? null
    if (allMessages.length > 0 && scroll_container) {
      setTimeout(() => {
        if (divider && landed !== id) divider.scrollIntoView({ block: 'center' })
        else scroll_container?.scrollTo({ top: scroll_container.scrollHeight, behavior: 'smooth' });
        landed = id
      }, 100);
    }
  });

  $effect(() => {
    if (!divider || !scroll_container) {
      dividerAbove = false
      return
    }
    const target = divider
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) dividerAbove = !entry.isIntersecting && entry.boundingClientRect.top < (entry.rootBounds?.top ?? 0)
    }, { root: scroll_container })
    observer.observe(target)
    return () => observer.disconnect()
  })

  function jumpUnread() {
    divider?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  }

  function markAllRead() {
    if (conversation) api.putConversationRead({ conversation_id: conversation.id }).catch(() => {})
  }
</script>

<div class="relative min-h-0 flex-1 overflow-hidden bg-paper">
  {#if isLoading && allMessages.length > 0}
    <div class="absolute inset-x-0 top-0 z-10 h-px overflow-hidden">
      <div class="h-full w-[30%] animate-pulse bg-accent"></div>
    </div>
  {/if}

  {#if allMessages.length === 0 && isLoading}
    <EmptySpinner />
  {:else if allMessages.length === 0}
    <EmptyChat />
  {:else}
    <div class="absolute inset-0 overflow-y-auto overflow-x-hidden" bind:this={scroll_container}>
      <div class="w-full px-4 pb-6 pt-1">
        <SectionSummaryCard {scope} kind="all" />
        <SectionSummaryCard {scope} kind="selection" />
        {#each groupedMessages as messages, index (messages[0].id)}
          {@const section = split.owner.get(messages[0].id)}
          {@const marker = plan.markers.get(messages[0].id)}
          {@const hiddenBy = plan.hidden.get(messages[0].id)}
          {#if section && section !== split.owner.get(groupedMessages[index - 1]?.[0].id ?? '')}
            <SectionHeader {section} {scope} headed={split.headed} />
          {/if}
          {#if !section || !collapsed.has(section.id)}
            {#if marker && !isOpen($folds, marker.key, 'open', false)}
              <FoldDivider {marker} />
            {/if}
            {#if messages[0].id === unreadGroup}
              <div bind:this={divider} class="my-1.5 flex h-7 w-full items-center gap-3 text-[12px] text-accent" data-testid="new-messages-divider" role="separator">
                <span class="h-0 flex-1 border-t border-accent"></span>
                <span class="font-mono text-[11px]">{t('conversation.history.newMessages')}</span>
                <button type="button" class="rounded-full bg-accent-soft px-2.5 py-0.5 font-mono text-[11px] hover:bg-paper-2" data-testid="mark-all-read" onclick={markAllRead}>{t('conversation.history.markAllRead')}</button>
                <span class="h-0 flex-1 border-t border-accent"></span>
              </div>
            {/if}
            {#if !hiddenBy || isOpen($folds, hiddenBy, 'open', false)}
              <MessageGroup {messages} collapsed={plan.summary.has(messages[0].id)} />
            {/if}
          {/if}
        {/each}
      </div>
    </div>
    {#if dividerAbove}
      <button type="button" class="absolute left-1/2 top-2 z-10 flex -translate-x-1/2 items-center gap-1.5 rounded-full bg-accent px-3 py-1 text-[12px] font-medium text-paper shadow-md" data-testid="jump-unread" onclick={jumpUnread}>
        <Icon icon="lucide:arrow-up" class="text-[13px]" />{t('conversation.history.jumpUnread')}
      </button>
    {/if}
  {/if}
</div>
