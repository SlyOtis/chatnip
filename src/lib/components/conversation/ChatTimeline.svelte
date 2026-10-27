<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import { listen } from '@tauri-apps/api/event'
  import Icon from '@iconify/svelte'
  import { api, conversationsById, providersStore } from '$lib/tauri'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { messageHref } from '$lib/messageLink'
  import type { ChatItem, Conversation } from '$lib/tauri/types'
  import { Button } from '$lib/components/ui'
  import { formatDate, t } from '$lib/i18n.svelte'
  import MessageGroup from './messages/MessageGroup.svelte'
  import MessageContent from './messages/MessageContent.svelte'
  import RecordCard from './RecordCard.svelte'
  import SectionHeader from './messages/SectionHeader.svelte'
  import SectionSummaryCard from './messages/SectionSummaryCard.svelte'
  import { layoutSettings } from '$lib/layoutSettings'
  import { collapsedSections, holdTimeline, splitSections } from '$lib/sections.svelte'

  interface Props {
    conversation: Conversation
    read: boolean
    attached: number
  }

  let { conversation, read, attached }: Props = $props()

  let items = $state<ChatItem[]>([])
  let next = $state<string | null>(null)
  let loading = $state(false)
  let running = $state(false)
  let failure = $state<string | null>(null)

  const sources = $derived(conversation.view_settings?.chat?.sources ?? false)
  const ordered = $derived(read ? items : [...items].reverse())
  const scope = $derived(conversation.id)
  const split = $derived(read ? splitSections($layoutSettings, [...items].sort((a, b) => a.at.localeCompare(b.at)), (item) => ({ key: item.id, first: item.message?.timestamp ?? item.at, last: item.message?.timestamp ?? item.at, messages: item.message ? [item.message] : [] })) : null)
  const collapsed = $derived(split?.headed ? collapsedSections($layoutSettings, scope) : new Set<string>())

  holdTimeline(() => (split && items.length ? { scope, source: { kind: 'conversation', id: conversation.id }, conversationId: conversation.id, title: conversation.name, sections: split.sections } : null))

  async function load(more: boolean) {
    loading = true
    failure = null
    try {
      const page = await api.getChatItems(conversation.id, more ? next : null, 40)
      items = more ? [...items, ...page.items] : page.items
      next = page.next_before
    } catch (error) {
      failure = String(error)
    } finally {
      loading = false
    }
  }

  $effect(() => {
    conversation.id
    items = []
    next = null
    load(false)
  })

  onMount(() => {
    const offs = [listen<string>('chat-items-updated', (e) => e.payload === conversation.id && load(false)), listen('messages-updated', () => load(false))]
    return () => offs.forEach((off) => off.then((stop) => stop()))
  })

  async function open(conversationId: string) {
    await api.putCurrentConversation(conversationId).catch(() => {})
    goto(conversationHref(conversationId))
  }

  async function backfill() {
    running = true
    failure = null
    try {
      await api.postChatBackfill(conversation.id)
      await load(false)
    } catch (error) {
      failure = String(error)
    } finally {
      running = false
    }
  }
</script>

<div class="absolute inset-0 overflow-y-auto overflow-x-hidden">
  <div class="w-full px-4 pb-6 pt-3">
    {#if failure}
      <p class="py-6 text-center text-[13px] text-error-600">{failure}</p>
    {:else if !loading && items.length === 0}
      <div class="flex flex-col items-center gap-2 py-16 text-center">
        <Icon icon={read ? 'lucide:book-open' : 'lucide:pen-line'} class="text-[22px] text-ink-3" />
        <p class="text-[15px] font-semibold text-ink">{read ? t('chats.timeline.emptyRead') : t('chats.timeline.emptyWrite')}</p>
        <p class="max-w-[28rem] text-[13px] text-ink-3">{attached ? t('chats.timeline.emptyAttached') : read ? t('chats.timeline.emptyReadHint') : t('chats.timeline.emptyWriteHint')}</p>
        {#if attached}
          <Button size="sm" icon="lucide:play" busy={running} onclick={backfill}>{t('chats.timeline.runNow')}</Button>
        {/if}
      </div>
    {/if}
    {#if !read && next}
      <div class="flex justify-center py-4">
        <Button size="sm" busy={loading} onclick={() => load(true)}>{t('chats.timeline.loadOlder')}</Button>
      </div>
    {/if}
    {#if split}
      <SectionSummaryCard {scope} kind="all" />
      <SectionSummaryCard {scope} kind="selection" />
    {/if}
    {#each ordered as item, index (item.id)}
      {@const section = split?.owner.get(item.id)}
      {#if split && section && section !== split.owner.get(ordered[index - 1]?.id ?? '')}
        <SectionHeader {section} {scope} headed={split.headed} />
      {/if}
      {#if !section || !collapsed.has(section.id)}
        {#if item.kind === 'message' && item.message}
          <section class="group/item relative border-b border-line py-2">
            {#if read || sources || item.conversation_id !== conversation.id}
              {@const providers = $conversationsById.get(item.conversation_id)?.providers ?? []}
              {@const origin = providers[0]}
              <header class="flex min-w-0 items-center gap-2 text-[12px] text-ink-3">
                <button type="button" data-testid="message-source" class="flex min-w-0 items-center gap-1.5 rounded-[0.375rem] hover:text-ink" title={t('chats.timeline.openInChat')} onclick={() => item.message && goto(messageHref(item.conversation_id, item.message.id))}>
                  <ProviderIcon {providers} conversationType={item.conversation_type} size="sm" />
                  <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{[item.conversation_name, $providersStore.data.length > 1 && origin && origin.name !== item.conversation_name ? origin.name : null].filter(Boolean).join(' · ')}</span>
                </button>
              </header>
            {/if}
            <MessageGroup messages={[item.message]} read />
          </section>
        {:else if item.kind === 'record'}
          <RecordCard {item} onshow={open} />
        {:else}
          <article class="flex justify-end py-1.5">
            <div class="max-w-[min(36rem,85%)] rounded-[0.875rem] bg-accent-soft px-3 py-2 text-[14px] leading-5 text-ink">
              <MessageContent content={item.text ?? item.title} content_type="markdown" previews={false} />
              <time class="mt-0.5 block text-right font-mono text-[10.5px] text-ink-3" datetime={item.at}>{formatDate(item.at, { timeStyle: 'short' })}</time>
            </div>
          </article>
        {/if}
      {/if}
    {/each}
    {#if read && next}
      <div class="flex justify-center py-4">
        <Button size="sm" busy={loading} onclick={() => load(true)}>{t('chats.timeline.loadOlder')}</Button>
      </div>
    {/if}
  </div>
</div>
