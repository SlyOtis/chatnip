<script lang="ts">
  import { layoutSettings } from '$lib/layoutSettings'
  import { collapsedSections, holdTimeline, splitSections } from '$lib/sections.svelte'
  import SectionHeader from './messages/SectionHeader.svelte'
  import SectionSummaryCard from './messages/SectionSummaryCard.svelte'
  import { onMount, setContext, untrack } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { api, composerOpen, conversationsStore, conversationStore, peopleStore, profileStore, providersStore, readStore } from '$lib/tauri'
  import type { Message, Person } from '$lib/tauri/types'
  import { composerSize, holdPane, loadMergeGroups, mergedItems, rowModel, selectConversation, selectPerson } from '$lib/inbox'
  import { messageHref } from '$lib/messageLink'
  import { holdConversationPanel } from '$lib/shell.svelte'
  import { DraggableEdge } from 'sly-svelte-pane'
  import { profileSettingsStore } from '$lib/tauri/profile'
  import { threadPanelStore } from '$lib/tauri/threads'
  import TitleBar from './TitleBar.svelte'
  import ChatInput from './ChatInput.svelte'
  import TypingIndicator from './TypingIndicator.svelte'
  import MessageGroup from './messages/MessageGroup.svelte'
  import EmptySpinner from './EmptySpinner.svelte'
  import Icon from '@iconify/svelte'
  import { Button, Kbd } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'
  import FoldDivider from './messages/FoldDivider.svelte'
  import { folds, foldPlan, isOpen } from '$lib/collapse'
  import { moveHighlight } from '$lib/reply.svelte'
  import { myUserIds, isMine } from '$lib/tauri/people'

  interface Props {
    personId?: string | null
    mergedId?: string | null
  }

  let { personId = null, mergedId = null }: Props = $props()

  let person = $state<Person | null>(null)
  let messages = $state<Message[]>([])
  let loading = $state(true)
  let failure = $state<string | null>(null)
  let scroller = $state<HTMLDivElement | null>(null)

  const fromStore = $derived(personId ? ($peopleStore.find((p) => p.id === personId) ?? null) : null)
  const merged = $derived(mergedId ? ($mergedItems.get(mergedId) ?? null) : null)
  const mergedRow = $derived(merged ? rowModel(merged, $peopleStore, $myUserIds) : null)
  const conversations = $derived(mergedId ? (merged?.conversations ?? []) : $conversationsStore.data.filter((c) => c.person_id === personId || (person?.conversation_ids ?? []).includes(c.id)))
  const mergedKey = $derived(mergedId ? conversations.map((c) => c.id).join(',') : '')
  const writable = $derived(composerOpen(conversations.map((c) => c.id), $readStore))
  const latest = $derived([...conversations].sort((a, b) => b.last_activity.localeCompare(a.last_activity))[0] ?? null)
  const providerById = $derived(new Map($providersStore.data.map((p) => [p.id, p])))
  const timestampInterval = $derived($profileStore.data?.settings?.message_timestamp_interval || 5)
  const title = $derived(mergedRow?.title ?? person?.display_name ?? null)

  $effect(() => {
    if (fromStore) person = fromStore
  })

  async function load() {
    const ids = conversations.map((c) => c.id)
    if (mergedId && ids.length === 0) return
    loading = true
    failure = null
    try {
      if (mergedId) {
        messages = await api.getMergeGroupMessages(mergedId, 200)
        selectConversation(mergedId)
      } else if (personId) {
        const [loadedPerson, loadedMessages] = await Promise.all([api.getPerson(personId), api.getPersonMessages(personId, 200)])
        person = loadedPerson
        messages = [...loadedMessages].sort((a, b) => a.timestamp.localeCompare(b.timestamp))
        selectPerson(personId)
      }
    } catch (error) {
      failure = String(error)
    } finally {
      loading = false
    }
  }

  $effect(() => {
    if (mergedKey) untrack(() => load())
  })

  $effect(() => {
    const target = latest
    if (target && $conversationStore.data?.id !== target.id) api.putCurrentConversation(target.id).catch(() => {})
  })

  onMount(() => {
    if (mergedId) loadMergeGroups().catch(() => {})
    else load()
    const unlisteners: Array<() => void> = []
    listen('messages-updated', () => load()).then((u) => unlisteners.push(u))
    listen('conversations-updated', () => load()).then((u) => unlisteners.push(u))
    return () => unlisteners.forEach((u) => u())
  })

  type Run = { messages: Message[]; conversationId: string }

  const runs = $derived.by((): Run[] => {
    const out: Run[] = []
    for (const message of messages) {
      const last = out[out.length - 1]
      const previous = last?.messages[last.messages.length - 1]
      const gap = previous ? (new Date(message.timestamp).getTime() - new Date(previous.timestamp).getTime()) / 1000 : Infinity
      if (last && previous && previous.sender_id === message.sender_id && previous.conversation_id === message.conversation_id && gap <= timestampInterval * 60) {
        last.messages.push(message)
      } else {
        out.push({ messages: [message], conversationId: message.conversation_id })
      }
    }
    return out
  })

  const repeatedSubjects = new Set<string>()
  setContext('subject-repeats', repeatedSubjects)

  $effect(() => {
    repeatedSubjects.clear()
    let previous: Message | null = null
    for (const message of messages) {
      const subject = message.subject?.replace(/^(\s*(re|fwd?|sv|vs)\s*:\s*)+/i, '').trim().toLowerCase() ?? ''
      const before = previous?.subject?.replace(/^(\s*(re|fwd?|sv|vs)\s*:\s*)+/i, '').trim().toLowerCase() ?? ''
      if (previous && message.type === 'email' && previous.type === 'email' && previous.conversation_id === message.conversation_id && subject && subject === before) {
        repeatedSubjects.add(message.id)
      }
      previous = message
    }
  })

  const scope = $derived(mergedId ? `merged:${mergedId}` : `person:${personId}`)
  const split = $derived(splitSections($layoutSettings, runs, (run) => ({ key: run.messages[0].id, first: run.messages[0].timestamp, last: run.messages[run.messages.length - 1].timestamp, messages: run.messages })))
  const plan = $derived(foldPlan(runs.map((run) => ({ id: run.messages[0].id, conversationId: run.conversationId, email: run.messages[0].type === 'email', mine: isMine($myUserIds, run.messages[0].sender_id), chat: run.messages[0].type !== 'email' && run.messages[0].type !== 'system', size: run.messages.length, section: split.headed ? split.owner.get(run.messages[0].id)?.id : undefined }))))
  const collapsed = $derived(split.headed ? collapsedSections($layoutSettings, scope) : new Set<string>())

  holdTimeline(() => (messages.length ? { scope, source: mergedId ? { kind: 'merged', id: mergedId } : { kind: 'person', id: personId ?? '' }, conversationId: null, title, sections: split.sections } : null))

  function channelOf(run: Run): string | null {
    const conversation = conversations.find((c) => c.id === run.conversationId)
    return conversation && (conversation.type === 'channel' || conversation.type === 'group_message') ? conversation.name : null
  }

  function sourceOf(run: Run) {
    const conversation = conversations.find((c) => c.id === run.conversationId)
    const provider = providerById.get(run.messages[0].provider_id) ?? conversation?.providers[0]
    return {
      marks: provider ? [{ ...provider, conversation_type: conversation?.type }] : [],
      name: mergedId && conversation ? rowModel({ kind: 'conversation', conversation }, $peopleStore, $myUserIds).title : (provider?.name ?? run.messages[0].provider_name)
    }
  }

  $effect(() => {
    if (runs.length > 0 && scroller) {
      setTimeout(() => scroller?.scrollTo({ top: scroller.scrollHeight }), 50)
    }
  })

  let lastThread = $state<{ thread_id: string; root_message: Message } | null>(null)
  $effect(() => {
    const panel = $threadPanelStore
    if (panel.open && panel.thread_id && panel.root_message) lastThread = { thread_id: panel.thread_id, root_message: panel.root_message }
  })

  holdConversationPanel(() => ({ thread: lastThread, threadOpen: $threadPanelStore.open, detailsOpen: !!$profileSettingsStore?.details_panel_open }))
</script>

  <div
    data-pane-grid
    data-testid="person-thread-view"
    data-person={personId}
    data-merged={mergedId}
    data-count={messages.length}
    class="relative grid h-full min-w-0 grid-cols-1 overflow-hidden"
    style:--composer-size={$composerSize}
    style:--composer-size-min="6rem"
    style:--composer-size-max="70vh"
    style:--composer-track={!latest || !writable ? '0px' : 'clamp(var(--composer-size-min), var(--composer-size), var(--composer-size-max))'}
    style:grid-template-rows={writable ? 'auto minmax(0, 1fr) auto var(--composer-track)' : 'auto minmax(0, 1fr) 0px 0px'}
  >
    <div class="relative z-0 min-w-0"><TitleBar {person} merged={mergedRow} mergeGroupId={merged?.groupId ?? null} personConversations={conversations} /></div>
    <div class="relative z-10 min-h-0 overflow-hidden bg-paper">
      {#if loading && messages.length === 0}
        <EmptySpinner />
      {:else if failure}
        <div class="flex h-full flex-col items-center justify-center gap-2 px-8 text-center">
          <Icon icon="lucide:circle-alert" class="text-[32px] text-error-500" />
          <p class="selectable text-[13px] text-ink-2">{failure}</p>
          <Button variant="primary" onclick={load}>{t('common.retry')}</Button>
        </div>
      {:else if messages.length === 0}
        <div class="empty-host relative flex h-full flex-col items-center justify-center px-6 text-center">
          <div class="empty-frost" aria-hidden="true"></div>
          <p class="text-[15px] font-semibold tracking-tight text-ink">{t('conversation.empty.nothing')}</p>
          <p class="mt-1 max-w-sm text-[13px] text-ink-3">{t('conversation.person.emptyHint', { name: title ?? t('conversation.person.thisPerson') })} <Kbd keys="R" /> {t('conversation.person.emptyHintAfter')}</p>
        </div>
      {:else}
        <div class="absolute inset-0 overflow-y-auto overflow-x-hidden outline-none" bind:this={scroller} tabindex="-1" data-testid="thread-scroller" onkeydown={(e) => { if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); moveHighlight(e.key === 'ArrowDown' ? 1 : -1, runs.map((run) => run.messages[run.messages.length - 1])) } }}>
          <div class="w-full px-4 pb-6 pt-1">
            <SectionSummaryCard {scope} kind="all" />
            <SectionSummaryCard {scope} kind="selection" />
            {#each runs as run, index (run.messages[0].id)}
              {@const previous = index > 0 ? runs[index - 1] : null}
              {@const source = sourceOf(run)}
              {@const section = split.owner.get(run.messages[0].id)}
              {@const opens = !!section && section !== split.owner.get(previous?.messages[0].id ?? '')}
              {#if section && opens}
                <SectionHeader {section} {scope} headed={split.headed} />
              {/if}
              {#if !section || !collapsed.has(section.id)}
                {@const marker = plan.markers.get(run.messages[0].id)}
                {@const hiddenBy = plan.hidden.get(run.messages[0].id)}
                {#if marker && !isOpen($folds, marker.key, 'open', false)}
                  <FoldDivider {marker} />
                {/if}
                {#if !hiddenBy || isOpen($folds, hiddenBy, 'open', false)}
                  <MessageGroup messages={run.messages} collapsed={plan.summary.has(run.messages[0].id)} channelName={channelOf(run)} chat={{ name: source.name, marks: source.marks, href: messageHref(run.conversationId, run.messages[0].id) }} />
                {/if}
              {/if}
            {/each}
          </div>
        </div>
      {/if}
    </div>
    <div class="relative z-10 min-w-0 bg-paper"><TypingIndicator /></div>
    <div class="relative z-10 flex min-h-0 min-w-0 flex-col bg-paper">
      {#if latest && writable}
        <div class="pane-edge top" data-axis="y" role="presentation" onpointerdowncapture={(e) => holdPane(e, 'chatnip.composer-size', '--composer-size', (value) => composerSize.set(value))} ondblclick={() => { composerSize.set('18vh'); localStorage.removeItem('chatnip.composer-size') }}>
          <DraggableEdge options={{ property: '--composer-size', anchor: 'bottom', gridSelector: '[data-pane-grid]' }} />
        </div>
        <ChatInput />
      {/if}
    </div>
  </div>
