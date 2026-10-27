<script lang="ts">
  import { accountOf } from '$lib/accounts'
  import Icon from '@iconify/svelte'
  import type { Message } from '$lib/tauri'
  import { api, conversationStore, messagesStore, openCasePicker, pinnedStore, providersStore, putPin } from '$lib/tauri'
  import { openThread } from '$lib/tauri/threads'
  import { canForward, forwardSet, openForward, replyMode, selection, startReply } from '$lib/reply.svelte'
  import { anchored, pointBox } from '$lib/popover'
  import { reactionUses, loadReactionUses, quickReactions, sendReaction, templateFromMessage } from '$lib/media'
  import { modalsShow } from '$lib/modals'
  import { FEATURES } from '$lib/settings'
  import EmojiPicker from '$lib/components/EmojiPicker.svelte'
  import { MenuItem, TagPicker } from '$lib/components/ui'
  import { objectTagsStore, tagIdsOn } from '$lib/tauri/tags'
  import { t } from '$lib/i18n.svelte'
  import EmojiGlyph from '$lib/components/shared/EmojiGlyph.svelte'
  import { providerApp } from '$lib/providerIcons'
  import { parseMetadata } from '$lib/messageKind'
  import { openUrl } from '@tauri-apps/plugin-opener'

  interface Props {
    message: Message;
    is_me: boolean;
    x: number;
    y: number;
    visible: boolean;
    onclose: () => void;
    onedit?: () => void;
    ondelete?: () => void;
    read?: boolean;
  }

  let { message, is_me, x, y, visible, onclose, onedit, ondelete, read = false }: Props = $props()

  let full_picker_open = $state(false)
  let tagging = $state(false)
  let reminding = $state(false)
  const pinned = $derived($pinnedStore.conversation_id === message.conversation_id && $pinnedStore.data.some((p) => p.message_id === message.id))

  const tagged = $derived(tagIdsOn($objectTagsStore, 'message', message.id).length)
  const tagTargets = $derived(selection.ids.includes(message.id) && selection.ids.length > 1 ? selection.ids : [message.id])

  const quick_reactions = $derived(quickReactions($reactionUses))

  let web_link = $state<string | null>(null)
  const provider = $derived((accountOf($providersStore.data, message.provider_id) ?? undefined))

  $effect(() => {
    if (!visible) {
      full_picker_open = false
      tagging = false
      reminding = false
      return
    }
    loadReactionUses().catch(() => {})
    const stored = parseMetadata(message.metadata).web_url
    web_link = typeof stored === 'string' && stored ? stored : null
    if (web_link) return
    const id = message.id
    api.getMessageLink(id).then((link) => {
      if (message.id === id) web_link = link
    }).catch(() => {})
  })

  function handleOpenIn() {
    if (web_link) openUrl(web_link).catch(() => {})
    onclose()
  }

  async function handleReaction(emoji: string) {
    try {
      await sendReaction({
        conversation_id: message.conversation_id,
        message_id: message.id,
        reaction: emoji,
      })
    } catch {}
    onclose()
  }

  const mediaIndex = $derived((message.attachments ?? []).findIndex((a) => a.content_type.startsWith('image/') || a.content_type.startsWith('video/')))

  async function handleSaveContent() {
    try {
      await api.postContentFromMessage(message.id, mediaIndex)
    } catch {}
    onclose()
  }

  async function handleSaveTemplate() {
    try {
      await api.postContent(templateFromMessage(message))
    } catch {}
    onclose()
  }

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(message.content || '')
    } catch {}
    onclose()
  }

  async function handleReplyInThread() {
    try {
      const existing = message.thread_id ?? $conversationStore.data?.threads?.find((thread) => thread.root_message_id === message.id)?.id ?? null
      const thread = existing ?? (await api.postThread({ message_id: message.id })).id
      openThread(thread, message)
    } catch {}
    onclose()
  }

  async function handleCopyLink() {
    try {
      const link = web_link ?? (await api.getMessageLink(message.id))
      if (link) await navigator.clipboard.writeText(link)
    } catch {}
    onclose()
  }

  function remindAt(choice: 'soon' | 'hour' | 'tomorrow' | 'monday'): string {
    const at = new Date()
    if (choice === 'soon') at.setMinutes(at.getMinutes() + 20)
    else if (choice === 'hour') at.setHours(at.getHours() + 1)
    else {
      const days = choice === 'tomorrow' ? 1 : ((8 - at.getDay()) % 7 || 7)
      at.setDate(at.getDate() + days)
      at.setHours(9, 0, 0, 0)
    }
    return at.toISOString()
  }

  async function handleRemind(choice: 'soon' | 'hour' | 'tomorrow' | 'monday') {
    try {
      await api.postReminder({ message_id: message.id, at: remindAt(choice) })
    } catch {}
    onclose()
  }

  const mode = $derived(replyMode(message, (accountOf($providersStore.data, message.provider_id) ?? undefined), read))

  function handleReplyQuote() {
    startReply(message)
    onclose()
    document.querySelector<HTMLElement>('[data-composer]')?.focus()
  }

  function handleForward() {
    openForward(forwardSet(message, $messagesStore.data ?? [], selection.ids))
    onclose()
  }

  function handleEdit() {
    onedit?.()
    onclose()
  }

  function handleDelete() {
    ondelete?.()
    onclose()
  }

  async function handlePin() {
    try {
      await putPin(message.conversation_id, message.id, !pinned)
    } catch {}
    onclose()
  }

  async function handleBookmark() {
    try {
      await api.postBookmark({
        message_id: message.id,
      })
    } catch {}
    onclose()
  }

  async function handleMarkUnread() {
    try {
      await api.putMessageUnread({
        message_id: message.id,
      })
    } catch {}
    onclose()
  }

  function handleEmailReply() {
    modalsShow({
      mode: 'reply',
      original_message: message,
      conversation_id: message.conversation_id,
      provider_id: message.provider_id,
    })
    onclose()
  }

  function handleEmailReplyAll() {
    modalsShow({
      mode: 'reply_all',
      original_message: message,
      conversation_id: message.conversation_id,
      provider_id: message.provider_id,
    })
    onclose()
  }

  function handleEmailForward() {
    modalsShow({
      mode: 'forward',
      original_message: message,
      conversation_id: message.conversation_id,
      provider_id: message.provider_id,
    })
    onclose()
  }

  function handleAddToCase() {
    openCasePicker({ conversationIds: [], messageIds: [message.id] })
    onclose()
  }

  function handleDiscuss() {
    modalsShow({
      message,
      conversation_id: message.conversation_id,
    })
    onclose()
  }
</script>

{#if visible && reminding}
  <div class="menu w-56" use:anchored={{ at: pointBox(x, y), onclose }} role="menu" data-testid="message-remind-menu" aria-label={t('conversation.menu.remind')}>
    <MenuItem testid="menu-remind-soon" icon="lucide:timer" onclick={() => handleRemind('soon')}>{t('conversation.menu.remindSoon')}</MenuItem>
    <MenuItem testid="menu-remind-hour" icon="lucide:clock-3" onclick={() => handleRemind('hour')}>{t('conversation.menu.remindHour')}</MenuItem>
    <MenuItem testid="menu-remind-tomorrow" icon="lucide:sunrise" onclick={() => handleRemind('tomorrow')}>{t('conversation.menu.remindTomorrow')}</MenuItem>
    <MenuItem testid="menu-remind-monday" icon="lucide:calendar-days" onclick={() => handleRemind('monday')}>{t('conversation.menu.remindMonday')}</MenuItem>
  </div>
{:else if visible && tagging}
  <TagPicker objectType="message" ids={tagTargets} at={pointBox(x, y)} onclose={() => { tagging = false; onclose() }} />
{:else if visible && full_picker_open}
  <EmojiPicker at={pointBox(x, y)} providerId={message.provider_id} onselect={(emoji) => handleReaction(emoji)} onclose={() => { full_picker_open = false; onclose() }} />
{:else if visible}
  <div
    class="menu w-56"
    use:anchored={{ at: pointBox(x, y), onclose }}
    role="menu"
    data-testid="message-menu"
    data-id={message.id}
  >
    {#if !read}
    <div class="flex items-center gap-0.5 px-1 pb-1.5">
      {#each quick_reactions as emoji (emoji)}
        <button data-testid="quick-reaction" data-emoji={emoji} onclick={() => handleReaction(emoji)} class="flex size-7 items-center justify-center rounded-[0.5rem] text-[15px] hover:bg-paper-3"><EmojiGlyph {emoji} providerId={message.provider_id} /></button>
      {/each}
      <button data-testid="more-reactions" onclick={() => (full_picker_open = true)} class="ml-auto flex size-7 items-center justify-center rounded-[0.5rem] text-ink-3 hover:bg-paper-3 hover:text-ink" aria-label={t('conversation.menu.moreReactions')}>
        <Icon icon="lucide:plus" class="text-[15px]" />
      </button>
    </div>

    <div class="menu-sep"></div>
    {/if}
    <MenuItem testid="menu-copy" icon="lucide:copy" onclick={handleCopy}>{t('conversation.menu.copy')}</MenuItem>
    <MenuItem testid="menu-copy-link" icon="lucide:link" onclick={handleCopyLink}>{t('conversation.menu.copyLink')}</MenuItem>
    {#if web_link}
      <MenuItem testid="menu-open-in" icon="lucide:external-link" onclick={handleOpenIn}>{t('conversation.menu.openIn', { app: providerApp(provider ?? { provider_type: message.provider_name ?? '' }, message.type) })}</MenuItem>
    {/if}
    {#if !read}
      <MenuItem testid="menu-thread" icon="lucide:messages-square" onclick={handleReplyInThread}>{t('conversation.menu.replyInThread')}</MenuItem>
    {/if}
    {#if mode === 'chat'}
      <MenuItem testid="menu-reply-quote" icon="lucide:reply" keys="R" onclick={handleReplyQuote}>{t('conversation.menu.replyQuote')}</MenuItem>
    {/if}
    {#if canForward(message)}
      <MenuItem testid="menu-forward" icon="lucide:forward" keys="F" onclick={handleForward}>{t('conversation.menu.forward')}</MenuItem>
    {/if}

    <div class="menu-sep"></div>
    {#if pinned}
      <MenuItem testid="menu-unpin" icon="lucide:pin-off" onclick={handlePin}>{t('conversation.menu.unpin')}</MenuItem>
    {:else}
      <MenuItem testid="menu-pin" icon="lucide:pin" onclick={handlePin}>{t('conversation.menu.pin')}</MenuItem>
    {/if}
    <MenuItem testid="menu-bookmark" icon="lucide:bookmark" onclick={handleBookmark}>{t('conversation.menu.save')}</MenuItem>
    <MenuItem testid="menu-remind" icon="lucide:alarm-clock" onclick={() => (reminding = true)}>{t('conversation.menu.remind')}</MenuItem>
    <MenuItem testid="menu-tags" icon="lucide:tag" detail={tagged ? String(tagged) : undefined} onclick={() => (tagging = true)}>{tagTargets.length > 1 ? t('conversation.menu.tagSelected', { count: tagTargets.length }) : t('conversation.menu.tags')}</MenuItem>
    {#if mediaIndex >= 0}
      <MenuItem testid="menu-save-media" icon="lucide:library" onclick={handleSaveContent}>{t('content.save.media')}</MenuItem>
    {/if}
    {#if message.content.trim()}
      <MenuItem testid="menu-save-template" icon="lucide:message-square-reply" onclick={handleSaveTemplate}>{message.type === 'email' ? t('content.save.email') : t('content.save.reply')}</MenuItem>
    {/if}
    {#if FEATURES.cases}
      <MenuItem icon="lucide:briefcase" onclick={handleAddToCase}>{t('conversation.menu.addToCase')}</MenuItem>
    {/if}
    <MenuItem testid="menu-mark-unread" icon="lucide:mail-open" keys="U" onclick={handleMarkUnread}>{t('conversation.menu.markUnread')}</MenuItem>

    {#if message.type === 'chat' && is_me && !read && (onedit || ondelete)}
      <div class="menu-sep"></div>
      {#if onedit}
        <MenuItem testid="menu-edit" icon="lucide:pencil" onclick={handleEdit}>{t('common.edit')}</MenuItem>
      {/if}
      {#if ondelete}
        <MenuItem testid="menu-delete" icon="lucide:trash-2" danger onclick={handleDelete}>{t('common.delete')}</MenuItem>
      {/if}
    {/if}

    {#if message.type === 'email'}
      <div class="menu-sep"></div>
      {#if !read}
        <MenuItem testid="menu-email-reply" icon="lucide:reply" keys="R" onclick={handleEmailReply}>{t('conversation.menu.reply')}</MenuItem>
        <MenuItem testid="menu-email-reply-all" icon="lucide:reply-all" keys="⇧R" onclick={handleEmailReplyAll}>{t('conversation.menu.replyAll')}</MenuItem>
      {/if}
      <MenuItem testid="menu-email-forward" icon="lucide:forward" onclick={handleEmailForward}>{t('conversation.menu.forwardEmail')}</MenuItem>
      <MenuItem testid="menu-discuss" icon="lucide:messages-square" onclick={handleDiscuss}>{t('conversation.menu.discuss')}</MenuItem>
    {/if}
  </div>
{/if}
