<script module lang="ts">
  import { postActionRun } from '$lib/tauri/api'

  const summaries = new Map<string, Promise<string | null>>()

  function summaryFor(id: string): Promise<string | null> {
    let pending = summaries.get(id)
    if (!pending) {
      pending = postActionRun('summarise_mail', null, false, id).then((run) => (typeof run.outputs?.summary === 'string' && run.outputs.summary ? run.outputs.summary : null)).catch(() => null)
      summaries.set(id, pending)
    }
    return pending
  }
</script>

<script lang="ts">
  import { sendReaction } from '$lib/media'
  import { goto } from '$app/navigation'
  import { displayName, myUserIds, isMine, personHref, personOf, personProfileHref } from '$lib/tauri/people'
  import { userAvatar, type AvatarSource } from '$lib/avatar'
  import Icon from '@iconify/svelte'
  import type { Message as MessageType, User } from '$lib/tauri'
  import { api, conversationStore, conversationsStore, messagesStore, usersStore, peopleStore, providersStore, readStore, putAccountUsed } from '$lib/tauri'
  import { accountOf, ownAuthor } from '$lib/accounts'
  import { isHtml } from '$lib/links'
  import { canForward, editRequest, forwardSet, highlighted, loadedOriginal, openForward, pointed, quotedReply, replyBody, replyMode, replyTo, selection, toggleSelected } from '$lib/reply.svelte'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { openThread } from '$lib/tauri/threads'
  import { formatTime } from '$lib/dates'
  import { messageKind, plainText, inviteOf, linksOf, presented, type MessageKind } from '$lib/messageKind'
  import { renderEmojiText } from '$lib/emoji'
  import { failedTemplates, failTemplate, renderPlan, templateProps, templateSourceOf, type TemplateKind } from '$lib/template'
  import TemplateFrame from './TemplateFrame.svelte'
  import { splitQuoted } from '$lib/content'
  import { Avatar, Button } from '$lib/components/ui'
  import { formatDate, t } from '$lib/i18n.svelte'
  import { folds, isOpen, toggleFold, focusFold, registerFold } from '$lib/collapse'
  import { layoutSettings, messageSetting } from '$lib/layoutSettings'
  import Message from './Message.svelte'
  import EmailMessage from './EmailMessage.svelte'
  import SystemMessage from './SystemMessage.svelte'
  import MessageContent from './MessageContent.svelte'
  import MessageActions from './MessageActions.svelte'
  import MessageContextMenu from './MessageContextMenu.svelte'
  import ReplyQuote from './ReplyQuote.svelte'
  import MessageTags from './MessageTags.svelte'
  import UserProfilePopover from '$lib/components/UserProfilePopover.svelte'
  import MarkdownInput from '$lib/components/composer/MarkdownInput.svelte'
  import { emptyDoc, fromMarkdown, htmlToDoc, isEmpty, toMarkdown, type Doc } from '$lib/components/composer/markdown'
  import { sendKeys } from '$lib/shortcuts'
  import { tick, untrack } from 'svelte'
  import { appearanceSettings, globalAppearance, resolved } from '$lib/appearance'

  interface Props {
    messages: MessageType[];
    collapsed?: boolean;
    channelName?: string | null;
    read?: boolean;
    chat?: { name: string; marks: { provider_type: string; name?: string; conversation_type?: string }[]; href: string } | null;
  }

  let { messages, collapsed = false, channelName = null, read = false, chat = null }: Props = $props();

  let popoverVisible = $state(false)
  let popoverX = $state(0)
  let popoverY = $state(0)
  let popoverUser = $state<User | null>(null)
  let menuVisible = $state(false)
  let menuX = $state(0)
  let menuY = $state(0)
  let response = $state<'accepted' | 'maybe' | 'declined' | null>(null)
  let accepting = $state(false)

  const first = $derived(messages[0])
  const last = $derived(messages[messages.length - 1])
  const reading = $derived(read || (!!first && $readStore.has(first.conversation_id)))
  const isMe = $derived(isMine($myUserIds, first?.sender_id))
  const home = $derived(first ? ($conversationStore.data?.id === first.conversation_id ? $conversationStore.data : ($conversationsStore.data.find((c) => c.id === first.conversation_id) ?? null)) : null)
  const own = $derived(isMe ? ownAuthor(first, $providersStore.data, t('conversation.group.you')) : null)
  const senderName = $derived(own ? own.name : displayName($peopleStore, home, first?.sender_id, first?.sender_name || first?.sender_email || t('common.unknown')))
  const address = $derived(isMe ? '' : first?.sender_email ?? '')
  const senderPerson = $derived(personOf($peopleStore, { user_id: first?.sender_id }))
  const kind = $derived<MessageKind>(first ? messageKind(first, { conversationType: $conversationStore.data?.type, senderHuman: isMe ? true : senderPerson ? senderPerson.is_human : null, conversationPresentation: $conversationStore.data?.presentation }) : 'chat')
  const face = $derived<AvatarSource>(own ? own.face : userAvatar(findUser(first?.sender_id, first?.sender_name) ?? { id: first?.sender_id ?? '', display_name: senderName, email: first?.sender_email ?? null, avatar_url: null }, $peopleStore, senderName))
  const invite = $derived(kind === 'invite' && first ? inviteOf(first, { conversationPresentation: $conversationStore.data?.presentation }) : null)
  const shown = $derived(first ? presented(first, { conversationPresentation: $conversationStore.data?.presentation }) : {})
  const LAYOUT_KINDS: Partial<Record<MessageKind, TemplateKind>> = { email: 'mail', chat: 'chat', channel: 'chat', bot: 'bot', invite: 'invite', system: 'system', media: 'media' }
  const layoutKind = $derived<TemplateKind | null>(shown.kind && shown.kind !== 'auto' && shown.kind !== 'hidden' ? shown.kind : (LAYOUT_KINDS[kind] ?? null))
  const plan = $derived(renderPlan(shown, failedTemplates, $layoutSettings, layoutKind))
  let templateSource = $state<string | null>(null)

  $effect(() => {
    const current = plan
    templateSource = null
    if (current.use === 'layout') {
      templateSource = current.source
      return
    }
    if (current.use !== 'template') return
    let live = true
    templateSourceOf(current, (action_id, node_id) => api.getPresentationTemplate(action_id, node_id).then((t) => t.source))
      .then((source) => {
        if (live) templateSource = source.trim() ? source : null
      })
      .catch((error) => templateFailed(String(error)))
    return () => {
      live = false
    }
  })

  function templateFailed(error: string) {
    const current = plan
    const message = first
    if (!message) return
    if (current.use === 'layout') {
      failTemplate({ action_id: 'layout', node_id: current.kind }, () => api.postHistory({ kind: 'error', title: t('settings.layouts.failed', { kind: current.kind }), detail: error, message_id: message.id, href: '/main/profile/layouts' }))
      return
    }
    if (current.use !== 'template') return
    failTemplate(current, () => api.postPresentationTemplateError(current.action_id, current.node_id, message.id, error))
  }
  const links = $derived(kind === 'bot' ? linksOf(messages.map((m) => m.content).join(' ')) : [])
  const folded = $derived(first ? !isOpen($folds, first.id, 'open', !collapsed) : false)
  const mailLike = $derived(kind === 'email' || (kind === 'bot' && first?.type === 'email'))
  const source = $derived.by(() => {
    const provider = accountOf($providersStore.data, first?.provider_id) ?? undefined
    return provider ? [{ ...provider, conversation_type: first?.type === 'email' ? 'email' : $conversationStore.data?.type ?? 'direct_message' }] : []
  })
  const preview = $derived(first ? renderEmojiText(plainText(mailLike ? splitQuoted(first.content, isHtml(first.content_type)).visible : first.content)).replace(/\s+/g, ' ').trim() : '')
  let generated = $state<string | null>(null)
  const summaryMode = $derived(messageSetting($layoutSettings, 'messages.mail_summaries'))
  const summary = $derived(summaryMode === 'ai' ? first?.summary || generated : null)

  $effect(() => {
    const message = first
    if (summaryMode !== 'ai' || !folded || !mailLike || !message || message.summary) return
    let live = true
    summaryFor(message.id).then((text) => {
      if (live) generated = text
    })
    return () => {
      live = false
    }
  })

  $effect(() => {
    if (first) registerFold(first.id, !collapsed)
  })

  function toggleOpen() {
    if (first) toggleFold(first.id, 'open', !collapsed)
  }

  function findUser(senderId: string | undefined, name: string | null | undefined): User | null {
    if (!senderId) return null
    const memberMatch = ($conversationStore.data?.members || []).find(m => m.id === senderId)
    if (memberMatch) return memberMatch
    const userMatch = ($usersStore.data || []).find(u => u.id === senderId)
    if (userMatch) return userMatch
    return { id: senderId, email: null, phone: null, display_name: name ?? null, first_name: null, last_name: null, avatar_url: null, avatar_data: null, job_title: null, department: null, company: null, office_location: null, first_seen: '', last_updated: '' }
  }

  function handleSenderClick(e: MouseEvent) {
    const user = findUser(first.sender_id, first.sender_name)
    if (!user) return
    popoverUser = user
    popoverX = e.clientX
    popoverY = e.clientY
    popoverVisible = true
  }

  function openSender(e: MouseEvent) {
    if (senderPerson && !isMe) goto(personHref(senderPerson.id))
    else handleSenderClick(e)
  }

  function openSenderProfile(e: MouseEvent) {
    if (senderPerson && !isMe) goto(personProfileHref(senderPerson.id))
    else handleSenderClick(e)
  }

  function positionOf(index: number): 'first' | 'middle' | 'last' | 'single' {
    if (messages.length === 1) return 'single'
    if (index === 0) return 'first'
    if (index === messages.length - 1) return 'last'
    return 'middle'
  }

  async function react(reaction: string) {
    try {
      await sendReaction({ conversation_id: last.conversation_id, message_id: last.id, reaction })
      putAccountUsed(last.provider_id)
    } catch {}
  }

  const lastProvider = $derived(accountOf($providersStore.data, last?.provider_id) ?? undefined)
  const mode = $derived(replyMode(last ?? null, lastProvider, reading))

  function reply() {
    if (last) replyTo(last)
  }

  function forward() {
    if (last) openForward(forwardSet(last, $messagesStore.data ?? [], selection.ids))
  }

  let editing = $state<string | null>(null)
  let editDoc = $state<Doc>(emptyDoc())
  let editSaving = $state(false)
  let editError = $state<string | null>(null)
  let editor = $state<ReturnType<typeof MarkdownInput>>()

  const editable = $derived(!!last && last.type === 'chat' && isMe && !reading && !last.deleted_datetime && !!lastProvider?.capabilities.can_edit_messages)
  const editOnEnter = $derived(messageSetting($layoutSettings, 'messages.enter_sends') === 'enter')
  const editKey = $derived(sendKeys(editOnEnter ? 'enter' : 'mod'))

  async function startEdit() {
    if (!last) return
    editDoc = isHtml(last.content_type) ? htmlToDoc(last.content ?? '') : fromMarkdown(last.content ?? '')
    editError = null
    editing = last.id
    await tick()
    editor?.focus()
    const box = document.activeElement
    if (box instanceof HTMLElement && box.isContentEditable) {
      const caret = window.getSelection()
      caret?.selectAllChildren(box)
      caret?.collapseToEnd()
    }
  }

  function cancelEdit() {
    editing = null
    editError = null
  }

  $effect(() => {
    const wanted = editRequest.id
    if (!wanted || wanted !== last?.id || !editable) return
    untrack(() => {
      editRequest.id = null
      startEdit()
    })
  })

  async function saveEdit() {
    const message = messages.find((m) => m.id === editing)
    if (!message || editSaving) return
    const content = toMarkdown(editDoc).trim()
    if (isEmpty(editDoc) || content === (message.content ?? '').trim()) return cancelEdit()
    editSaving = true
    editError = null
    try {
      await api.putMessage({ message_id: message.id, content })
      editing = null
    } catch {
      editError = t('conversation.message.editFailed')
    } finally {
      editSaving = false
    }
  }

  function editKeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape') return
    e.preventDefault()
    e.stopPropagation()
    cancelEdit()
  }

  const selecting = $derived(selection.ids.length > 0)
  const actionIds = $derived(kind === 'bot' || kind === 'invite' ? messages.map((m) => m.id) : last ? [last.id] : [])

  function picked(ids: string[]): boolean {
    return ids.length > 0 && ids.every((id) => selection.ids.includes(id))
  }

  function pickRow(e: MouseEvent, ids: string[]) {
    if ((e.target as Element).closest('a, button, input, textarea, summary, iframe, [role="toolbar"], [contenteditable="true"]')) return
    if (selecting) return toggleSelected(...ids)
    if (window.getSelection()?.toString()) return
    const message = messages.find((m) => m.id === ids[ids.length - 1])
    if (!message || !mode) return
    highlighted.id = highlighted.id === message.id ? null : message.id
  }

  const lit = $derived(!!highlighted.id && messages.some((m) => m.id === highlighted.id))
  const look = $derived(resolved($appearanceSettings, $globalAppearance, first?.conversation_id ?? null, senderPerson?.id ?? null))
  const avatarSize = $derived(({ small: 'xs', normal: 'sm', large: 'md' } as const)[$globalAppearance.avatarScale])
  const fresh = untrack(() => !!last && Date.now() - new Date(last.timestamp).getTime() < 10000)
  const entry = $derived(fresh && look.entry !== 'none' ? `enter-${look.entry}` : '')

  function point() {
    if (last) pointed.message = last
  }

  function unpoint() {
    if (pointed.message?.id === last?.id) pointed.message = null
  }

  async function thread() {
    try {
      if (last.thread_id) return openThread(last.thread_id, last)
      const existing = $conversationStore.data?.threads?.find(t => t.root_message_id === last.id)
      if (existing) return openThread(existing.id, last)
      const created = await api.postThread({ message_id: last.id })
      openThread(created.id, last)
    } catch {}
  }

  function more(e: MouseEvent) {
    menuX = e.clientX
    menuY = e.clientY
    menuVisible = true
  }

  async function accept() {
    if (!invite) return
    accepting = true
    try {
      await api.postEvent({ draft: { title: invite.title, starts_at: invite.starts_at, ends_at: invite.ends_at, location: invite.location, notes: t('conversation.group.from', { name: senderName }), attendees: invite.organizer ? [invite.organizer] : [] } })
      response = 'accepted'
    } catch {
    } finally {
      accepting = false
    }
  }

  const inviteDate = $derived(invite ? new Date(invite.starts_at) : null)
  const inviteRange = $derived.by(() => {
    if (!invite || !inviteDate || Number.isNaN(inviteDate.getTime())) return ''
    const time = (d: Date) => formatDate(d, { hour: '2-digit', minute: '2-digit' })
    const day = formatDate(inviteDate, { weekday: 'short', day: 'numeric', month: 'short' })
    const end = invite.ends_at ? new Date(invite.ends_at) : null
    return `${day} · ${time(inviteDate)}${end && !Number.isNaN(end.getTime()) ? `–${time(end)}` : ''}`
  })
</script>

{#snippet bubble(message: MessageType, index: number, mine: boolean)}
  <div data-message-id={message.id} class="flex min-w-0 flex-col rounded-[0.375rem] {mine ? 'items-end' : ''}">
    {#if editing === message.id}
      <div data-testid="message-edit-form" class="w-full max-w-[min(68ch,80%)] py-1">
        <div class="rounded-[0.5rem] bg-paper-2 px-3 py-2 hairline">
          <MarkdownInput bind:this={editor} bind:doc={editDoc} placeholder={t('composer.message')} submitOnEnter={editOnEnter} onsubmit={saveEdit} onkeydown={editKeydown} />
        </div>
        {#if editError}
          <p class="pt-1 text-[12px] text-error-600" data-testid="message-edit-error">{editError}</p>
        {/if}
        <div class="mt-2 flex items-center justify-end gap-1">
          <Button testid="message-edit-cancel" size="sm" keys="esc" onclick={cancelEdit}>{t('common.cancel')}</Button>
          <Button testid="message-edit-save" variant="primary" size="sm" keys={editKey} busy={editSaving} onclick={saveEdit}>{t('common.save')}</Button>
        </div>
      </div>
    {:else}
      <Message message={replyBody(message, loadedOriginal(message, $messagesStore.data ?? [])).message} showAvatar={false} showSenderName={false} position={positionOf(index)} />
    {/if}
    <MessageTags messageId={message.id} />
  </div>
{/snippet}

{#snippet check(ids: string[], place: string)}
  <button type="button" data-testid="message-check" data-id={ids[ids.length - 1]} aria-pressed={picked(ids)} aria-label={t('conversation.actions.select')} class="flex shrink-0 items-center justify-center rounded-full {picked(ids) ? 'text-accent' : 'text-ink-3 hover:text-ink'} {place}" onclick={() => toggleSelected(...ids)}>
    <Icon icon={picked(ids) ? 'lucide:circle-check' : 'lucide:circle'} />
  </button>
{/snippet}

{#snippet actions()}
  <MessageActions message={last} oncollapse={toggleOpen} is_me={isMe} onreact={react} onreply={reply} onforward={forward} replies={!!mode} forwards={canForward(last)} onthread={thread} onmore={more} read={reading} selected={picked(actionIds)} onselect={canForward(last) ? () => toggleSelected(...actionIds) : undefined} onedit={editable ? startEdit : undefined} />
{/snippet}

{#snippet compact()}
<button class="my-1 grid h-9 w-full grid-cols-[1.5rem_1.25rem_minmax(0,8.5rem)_minmax(0,1fr)_auto] items-center gap-x-2.5 rounded-[0.5rem] pl-1.5 pr-3 text-left {picked([first.id]) ? 'bg-accent-soft' : lit ? 'bg-paper-3' : 'bg-paper-2 hover:bg-paper-3'}" data-testid="message-folded" data-lit={lit || undefined} class:enter-rise={entry === 'enter-rise'} class:enter-fade={entry === 'enter-fade'} class:enter-slide={entry === 'enter-slide'} onclick={() => (selecting ? toggleSelected(first.id) : toggleOpen())} onmouseenter={() => first && focusFold(first.id)} aria-expanded="false" aria-pressed={selecting ? picked([first.id]) : undefined} title={summary ? `${first.subject ?? ''}\n${preview}` : (first.subject ?? '')}>
  <span class="flex size-5 items-center justify-center rounded-[0.3rem] bg-paper-3 text-ink-2"><Icon icon="lucide:chevron-right" class="text-[13px]" /></span>
  {#if selecting}
    <Icon icon={picked([first.id]) ? 'lucide:circle-check' : 'lucide:circle'} class="text-[16px] {picked([first.id]) ? 'text-accent' : 'text-ink-3'}" />
  {:else}
    <Avatar source={face} size="xs" />
  {/if}
  <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[13px] font-medium text-ink-2">{senderName}</span>
  <span class="flex min-w-0 items-center gap-1.5 text-[13px] text-ink-3">
    {#if summary}
      <Icon icon="lucide:sparkles" class="shrink-0 text-[11px] text-ink-3" aria-label={t('conversation.group.summary')} />
    {/if}
    <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{summary ? renderEmojiText(summary) : (summaryMode === 'off' ? '' : preview)}</span>
  </span>
  <span class="flex shrink-0 items-center gap-1.5 font-mono text-[11px] text-ink-3">
    {#if first.has_attachments}
      <Icon icon="lucide:paperclip" class="text-[12px]" />
    {/if}
    <ProviderIcon providers={source} size="xs" />
    {formatTime(first.timestamp)}
  </span>
</button>
{/snippet}

{#snippet origin()}
  {#if chat}
    {@const provider = $providersStore.data.length > 1 && chat.marks[0]?.name && chat.marks[0].name !== chat.name ? chat.marks[0].name : null}
    {@const named = !!channelName && kind !== 'chat' && channelName === chat.name}
    <button type="button" data-testid="message-origin" class="flex min-w-0 shrink items-center gap-1 text-[11.5px] text-ink-3 hover:text-ink-2 hover:underline" title={t('chats.timeline.openInChat')} onclick={() => goto(chat.href)}>
      <ProviderIcon providers={chat.marks} size="xs" max={1} />
      <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{[named ? null : chat.name, provider].filter(Boolean).join(' · ')}</span>
    </button>
  {/if}
{/snippet}

{#snippet meta()}
  <span class="relative ml-auto flex h-5 shrink-0 items-center">
    <span class="flex items-center gap-1.5 font-mono text-[11px] text-ink-3 group-hover:invisible"><ProviderIcon providers={source} size="xs" />{first ? formatTime(first.timestamp) : ''}</span>
    <span class="absolute right-0 top-1/2 hidden -translate-y-1/2 group-hover:flex">
      {@render actions()}
    </span>
  </span>
{/snippet}

{#snippet mailHeader(message: MessageType)}
  <header class="flex h-6 min-w-0 cursor-default items-center gap-2" onclick={(e) => !selecting && e.target === e.currentTarget && toggleOpen()} role="presentation">
    {#if selecting}
      {@render check([message.id], 'size-6 text-[16px]')}
    {:else}
      <Avatar source={face} size={avatarSize} label={senderName} hint={senderPerson && !isMe ? t('inbox.row.openProfile', { name: senderName }) : undefined} onclick={openSenderProfile} />
    {/if}
    <button class="min-w-0 truncate text-[14px] font-semibold text-ink hover:underline" onclick={openSender}>{senderName}</button>
    {#if address}
      <span class="min-w-0 truncate font-mono text-[12px] text-ink-3">{address}</span>
    {/if}
    {@render origin()}{@render meta()}
  </header>
{/snippet}

{#if kind === 'hidden'}
{:else if plan.use !== 'builtin' && templateSource}
  <article class="group -mx-4 grid gap-1 px-4 py-2 {lit ? 'bg-paper-2' : ''}" data-lit={lit || undefined} class:enter-rise={entry === 'enter-rise'} class:enter-fade={entry === 'enter-fade'} class:enter-slide={entry === 'enter-slide'} onmouseenter={() => first && focusFold(first.id)}>
    {#each messages as message (message.id)}
      {@const context = { conversationPresentation: $conversationStore.data?.presentation }}
      <TemplateFrame source={templateSource} props={templateProps(message, presented(message, context), inviteOf(message, context), document.documentElement.classList.contains('dark') ? 'night' : 'day')} onerror={templateFailed} />
    {/each}
  </article>
{:else if kind === 'system'}
  {#each messages as message (message.id)}
    <SystemMessage {message} />
  {/each}
{:else if mailLike}
  {#if folded}
    {@render compact()}
  {:else}
    <article class="group -mx-4 border-b border-line px-4 pb-4 pt-3 {lit ? 'bg-paper-2' : ''}" data-lit={lit || undefined} class:enter-rise={entry === 'enter-rise'} class:enter-fade={entry === 'enter-fade'} class:enter-slide={entry === 'enter-slide'} onmouseenter={() => { point(); if (first) focusFold(first.id) }} onmouseleave={unpoint}>
      {#each messages as message (message.id)}
        {#snippet header()}
          {@render mailHeader(message)}
        {/snippet}
        <div data-message-id={message.id} class="-mx-4 px-4 {picked([message.id]) ? 'bg-accent-soft' : highlighted.id === message.id ? 'bg-paper-2' : ''}" onclick={(e) => pickRow(e, [message.id])} role="presentation">
          <EmailMessage {message} {header} />
          <MessageTags messageId={message.id} />
        </div>
      {/each}
    </article>
  {/if}
{:else if kind === 'invite' && invite}
  {@const mailOpen = first ? isOpen($folds, first.id, 'mail', false) : false}
  <article class="group -mx-4 border-b border-line px-4 py-3 {picked(actionIds) ? 'bg-accent-soft' : lit ? 'bg-paper-2' : ''}" data-lit={lit || undefined} class:enter-rise={entry === 'enter-rise'} class:enter-fade={entry === 'enter-fade'} class:enter-slide={entry === 'enter-slide'} onmouseenter={() => { point(); if (first) focusFold(first.id) }} onmouseleave={unpoint}>
    <div onclick={(e) => pickRow(e, actionIds)} role="presentation">
      <header class="flex min-w-0 items-center gap-2">
        {#if selecting}
          {@render check(actionIds, 'size-6 text-[16px]')}
        {:else}
          <Avatar source={face} size={avatarSize} label={senderName} hint={senderPerson && !isMe ? t('inbox.row.openProfile', { name: senderName }) : undefined} onclick={openSenderProfile} />
        {/if}
        <button class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[14px] font-semibold text-ink hover:underline" onclick={openSender}>{senderName}</button>
        <span class="shrink-0 text-[13px] text-ink-3">{t('conversation.group.invitedYou')}</span>
        {@render origin()}{@render meta()}
      </header>
      <div class="mt-1.5 flex min-w-0 items-start gap-2">
        <Icon icon="lucide:calendar" class="mt-0.5 w-6 shrink-0 text-[15px] text-accent" />
        <div class="min-w-0 flex-1">
          <p class="overflow-hidden text-ellipsis whitespace-nowrap text-[14px] font-semibold text-ink">{invite.title}</p>
          <p class="font-mono text-[12px] text-ink-2">{inviteRange}</p>
          {#if invite.location}
            <p class="overflow-hidden text-ellipsis whitespace-nowrap text-[13px] text-ink-3">{invite.location}</p>
          {/if}
          <div class="mt-1.5 flex flex-wrap items-center gap-1">
            {#if response}
              <span class="text-[13px] text-ink-2">{response === 'accepted' ? t('conversation.group.added') : response === 'maybe' ? t('conversation.group.markedMaybe') : t('conversation.group.declined')}</span>
            {:else}
              <Button size="sm" variant="secondary" icon="lucide:check" busy={accepting} title={t('conversation.group.acceptTitle')} onclick={accept}>{t('conversation.group.accept')}</Button>
              <Button size="sm" title={t('conversation.group.maybeTitle')} onclick={() => (response = 'maybe')}>{t('conversation.group.maybe')}</Button>
              <Button size="sm" variant="danger" title={t('conversation.group.declineTitle')} onclick={() => (response = 'declined')}>{t('conversation.group.decline')}</Button>
            {/if}
            {#if first?.type === 'email' && first.content}
              <Button size="sm" icon={mailOpen ? 'lucide:chevron-up' : 'lucide:chevron-down'} title={mailOpen ? t('conversation.group.hideOriginal') : t('conversation.group.showOriginal')} onclick={() => first && toggleFold(first.id, 'mail', false)}>{mailOpen ? t('conversation.group.hideEmail') : t('conversation.group.showEmail')}</Button>
            {/if}
          </div>
        </div>
      </div>
      {#if mailOpen && first}
        <div class="mt-3">
          <EmailMessage message={first} />
        </div>
      {/if}
    </div>
  </article>

{:else if (kind === 'bot' || kind === 'invite') && folded}
  {@render compact()}
{:else if kind === 'bot' || kind === 'invite'}
  <article class="group my-2 rounded-[0.5rem] px-4 py-2.5 {picked(actionIds) ? 'bg-accent-soft' : 'bg-paper-2'} {lit ? 'ring-1 ring-accent' : ''}" data-lit={lit || undefined} class:enter-rise={entry === 'enter-rise'} class:enter-fade={entry === 'enter-fade'} class:enter-slide={entry === 'enter-slide'} onmouseenter={point} onmouseleave={unpoint}>
    <div onclick={(e) => pickRow(e, actionIds)} role="presentation">
      <header class="flex h-6 min-w-0 items-center gap-2 mb-1.5">
        {#if selecting}
          {@render check(actionIds, 'text-[14px]')}
        {:else}
          <Icon icon={kind === 'invite' ? 'lucide:calendar' : 'lucide:bot'} class="shrink-0 text-[14px] text-ink-3" />
        {/if}
        <button class="min-w-0 truncate text-[13px] font-semibold text-ink-2 hover:underline" onclick={openSender}>{senderName}</button>
        {#if kind === 'invite'}
          <span class="text-[12px] text-ink-3" title={t('conversation.group.noEventData')}>{t('conversation.group.invitation')}</span>
        {/if}
        {@render origin()}{@render meta()}
      </header>
      <div class="space-y-1 text-[14px] leading-[22px] text-ink-2">
        {#each messages as message (message.id)}
          <div class="max-w-[68ch] selectable">
            <MessageContent content={message.content} content_type={message.content_type} metadata={message.metadata} providerId={message.provider_id} />
          </div>
        {/each}
      </div>
      {#if links.length > 0}
        <ul class="mt-2 border-t border-line pt-2">
          {#each links as link (link)}
            <li class="flex h-6 min-w-0 items-center gap-2">
              <Icon icon="lucide:link" class="shrink-0 text-[12px] text-ink-3" />
              <a href={link} target="_blank" rel="noopener noreferrer" class="min-w-0 truncate font-mono text-[12px] text-ink-2 hover:text-accent">{link.replace(/^https?:\/\//, '')}</a>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </article>
{:else if folded}
  {@render compact()}
{:else if isMe}
  <article class="group flex flex-col gap-0.5 py-1.5" onmouseenter={point} onmouseleave={unpoint}>
    {#each messages as message, index (message.id)}
      <div class="relative -mx-4 flex flex-col items-end pl-14 pr-4 {picked([message.id]) ? 'bg-accent-soft' : highlighted.id === message.id ? 'bg-paper-2' : ''}" onclick={(e) => pickRow(e, [message.id])} role="presentation">
        {#if selecting}
          {@render check([message.id], `absolute left-4 size-6 text-[16px] ${index === 0 ? 'top-0' : 'top-1.5'}`)}
        {/if}
        {#if quotedReply(message)}
          <ReplyQuote {message} class="mb-0.5 items-end" />
        {/if}
        {#if index === 0}
          <header class="relative mb-2 flex h-6 items-center gap-2">
            <span class="flex items-center gap-1.5 font-mono text-[11px] text-ink-3 group-hover:invisible"><ProviderIcon providers={source} size="xs" />{first ? formatTime(first.timestamp) : ''}</span>
            <span class="absolute right-0 top-1/2 hidden -translate-y-1/2 group-hover:flex">
              {@render actions()}
            </span>
          </header>
        {/if}
        {@render bubble(message, index, true)}
      </div>
    {/each}
  </article>
{:else}
  <article class="group flex flex-col gap-0.5 py-1.5" onmouseenter={point} onmouseleave={unpoint}>
    {#each messages as message, index (message.id)}
      <div class="-mx-4 grid grid-cols-[1.5rem_minmax(0,1fr)] gap-x-3 px-4 {picked([message.id]) ? 'bg-accent-soft' : highlighted.id === message.id ? 'bg-paper-2' : ''}" onclick={(e) => pickRow(e, [message.id])} role="presentation">
        {#if quotedReply(message)}
          <ReplyQuote {message} class="col-span-2 mb-0.5" />
        {/if}
        <div class="relative">
          {#if selecting}
            {@render check([message.id], `absolute left-0 size-6 text-[16px] ${index === 0 ? 'top-0.5' : 'top-1.5'}`)}
          {:else if index === 0}
            <Avatar source={face} size={avatarSize} class="mt-0.5" label={senderName} hint={senderPerson && !isMe ? t('inbox.row.openProfile', { name: senderName }) : undefined} onclick={openSenderProfile} />
          {/if}
        </div>
        <div class="min-w-0">
          {#if index === 0}
            <header class="flex h-6 min-w-0 items-center gap-2 mb-1.5">
              <button class="min-w-0 truncate text-[14px] font-semibold text-ink hover:underline" onclick={openSender}>{senderName}</button>
              {#if channelName && kind !== 'chat'}
                <span class="min-w-0 truncate text-[13px] text-ink-3">{t('conversation.group.inChannel', { name: channelName })}</span>
              {/if}
              {@render origin()}{@render meta()}
            </header>
          {/if}
          {@render bubble(message, index, false)}
        </div>
      </div>
    {/each}
  </article>
{/if}

{#if kind !== 'system'}
  <MessageContextMenu message={last} is_me={isMe} x={menuX} y={menuY} visible={menuVisible} onclose={() => menuVisible = false} onedit={editable ? startEdit : undefined} read={reading} />
{/if}

{#if popoverUser}
  <UserProfilePopover user={popoverUser} x={popoverX} y={popoverY} visible={popoverVisible} onclose={() => popoverVisible = false} />
{/if}
