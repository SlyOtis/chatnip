<script lang="ts">
  import { sendReaction } from '$lib/media'
  import type { Message } from '$lib/tauri'
  import { api, conversationStore } from '$lib/tauri'
  import { openThread } from '$lib/tauri/threads'
  import { formatTime } from '$lib/dates'
  import { Button } from '$lib/components/ui'
  import { formatDate, t } from '$lib/i18n.svelte'
  import MessageContextMenu from './MessageContextMenu.svelte'
  import AttachmentPreview from './AttachmentPreview.svelte'
  import MessageContent from './MessageContent.svelte'
  import SystemMessage from './SystemMessage.svelte'
  import { isEmojiOnly, plainText, reactionsOf } from '$lib/messageKind'
  import { renderEmojiHtml, workspaceEmoji } from '$lib/emoji'
  import { escapeHtml } from '$lib/content'
  import EmojiGlyph from '$lib/components/shared/EmojiGlyph.svelte'
  import { folds, isOpen, toggleFold } from '$lib/collapse'
  import { collapseLimitRem, layoutSettings, messageSetting } from '$lib/layoutSettings'
  import { onMount } from 'svelte'
  import { elementSize } from '$lib/window'

  interface Props {
    message: Message;
    is_me: boolean;
    is_first: boolean;
    is_last: boolean;
  }

  let { message, is_me, is_first }: Props = $props()
  let context_menu_visible = $state(false)
  let context_menu_x = $state(0)
  let context_menu_y = $state(0)
  let is_editing = $state(false)
  let edit_content = $state('')
  let is_confirming_delete = $state(false)
  let edit_textarea: HTMLTextAreaElement | undefined = $state()

  let emoji_only = $derived(isEmojiOnly(message.content) && !(message.attachments?.length))
  let reactions = $derived(reactionsOf(message.metadata))
  let emoji_html = $derived.by(() => {
    $workspaceEmoji
    return renderEmojiHtml(escapeHtml(plainText(message.content)), message.provider_id)
  })
  let onBubble = $derived(!emoji_only && !!plainText(message.content))
  let body_height = $state(0)
  let rem = $state(16)
  const limitRem = $derived(collapseLimitRem($layoutSettings))
  const limit = $derived(limitRem * rem)
  const emojiSize = $derived({ small: 'text-[22px] leading-8', medium: 'text-[28px] leading-9', large: 'text-[32px] leading-10' }[messageSetting($layoutSettings, 'messages.emoji_size')] ?? 'text-[32px] leading-10')
  const showEdited = $derived(messageSetting($layoutSettings, 'messages.show_edited') !== 'off')
  let full = $derived(isOpen($folds, message.id, 'full', false))
  let tall = $derived(limitRem > 0 && body_height > limit)

  onMount(() => {
    rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16
  })

  async function react(reaction: string) {
    try {
      await sendReaction({ conversation_id: message.conversation_id, message_id: message.id, reaction })
    } catch {}
  }

  let is_deleted = $derived(!!message.deleted_datetime)
  let is_edited = $derived(!!message.last_modified_datetime && new Date(message.last_modified_datetime).getTime() - new Date(message.timestamp).getTime() > 60000 && !is_deleted)

  let thread_info = $derived.by(() => {
    const conversation = $conversationStore.data
    if (!conversation?.threads) return null
    return conversation.threads.find(t => t.root_message_id === message.id) ?? null
  })

  function startEditing() {
    edit_content = message.content || ''
    is_editing = true
    requestAnimationFrame(() => {
      if (edit_textarea) {
        edit_textarea.focus()
        edit_textarea.setSelectionRange(edit_content.length, edit_content.length)
      }
    })
  }

  function cancelEditing() {
    is_editing = false
    edit_content = ''
  }

  async function saveEdit() {
    if (!edit_content.trim() || edit_content === message.content) {
      cancelEditing()
      return
    }
    try {
      await api.putMessage({ message_id: message.id, content: edit_content.trim() })
      is_editing = false
      edit_content = ''
    } catch {}
  }

  function handleEditKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      cancelEditing()
    } else if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      saveEdit()
    }
  }

  async function confirmDelete() {
    try {
      await api.deleteMessage({ message_id: message.id })
      is_confirming_delete = false
    } catch {}
  }

  function handleOpenThread() {
    if (thread_info) openThread(thread_info.id, message)
    else if (message.thread_id) openThread(message.thread_id, message)
  }

  function handleContextMenu(event: MouseEvent) {
    event.preventDefault()
    context_menu_x = event.clientX
    context_menu_y = event.clientY
    context_menu_visible = true
  }
</script>

{#if message.type === 'system'}
  <SystemMessage {message} />
{:else if is_deleted}
  <p data-testid="message-deleted" data-id={message.id} class="py-0.5 text-[14px] italic text-ink-3">{t('conversation.message.deleted')}</p>
{:else if message.content?.trim() || message.attachments?.length}
  <div data-testid="message" data-id={message.id} data-mine={is_me} class="group/line relative flex flex-col {is_me ? 'items-end' : 'items-start'}" oncontextmenu={handleContextMenu} role="article">
    {#if is_editing}
      <div class="py-1">
        <textarea
          data-testid="message-edit-field"
          bind:this={edit_textarea}
          bind:value={edit_content}
          onkeydown={handleEditKeydown}
          class="w-full resize-none rounded-[0.25rem] bg-paper-2 px-3 py-2 text-[14px] leading-6 text-ink outline-none"
          rows="3"
        ></textarea>
        <div class="mt-2 flex items-center gap-1">
          <Button testid="message-edit-save" variant="primary" size="sm" keys="↵" onclick={saveEdit}>{t('common.save')}</Button>
          <Button testid="message-edit-cancel" size="sm" keys="esc" onclick={cancelEditing}>{t('common.cancel')}</Button>
        </div>
      </div>
    {:else if is_confirming_delete}
      <div class="py-1">
        <p class="text-[14px] text-ink">{t('conversation.message.confirmDelete')}</p>
        <div class="mt-2 flex items-center gap-1">
          <Button testid="message-delete-confirm" variant="danger" size="sm" icon="lucide:trash-2" onclick={confirmDelete}>{t('common.delete')}</Button>
          <Button size="sm" onclick={() => is_confirming_delete = false}>{t('common.cancel')}</Button>
        </div>
      </div>
    {:else}
      {#if emoji_only}
        <p class="py-0.5 {emojiSize} selectable" aria-label={message.content}>{@html emoji_html}</p>
      {:else if plainText(message.content)}
        <div class="relative max-w-[min(68ch,80%)] rounded-[1.125rem] px-3.5 py-2 {onBubble && reactions.length > 0 ? 'mb-3' : ''} text-[14px] leading-[22px] selectable {is_me ? 'rounded-br-[0.375rem] bg-[var(--bubble-mine)] text-[var(--bubble-mine-ink)]' : 'rounded-bl-[0.375rem] bg-[var(--bubble-theirs)] text-[var(--bubble-theirs-ink)] hairline'} {is_first ? '' : is_me ? 'rounded-tr-[0.375rem]' : 'rounded-tl-[0.375rem]'}">
          <div class="overflow-hidden {tall && !full ? '[mask-image:linear-gradient(to_bottom,black_75%,transparent)]' : ''}" style:max-height={tall && !full ? `${limitRem}rem` : undefined}>
            <div use:elementSize={(size) => (body_height = size.height)}>
              <MessageContent content={message.content} content_type={message.content_type} metadata={message.metadata} providerId={message.provider_id} />
            </div>
          </div>
          {#if is_edited && showEdited}
            <span data-testid="message-edited" class="ml-1 text-[12px] text-ink-3">{t('conversation.message.edited')}</span>
          {/if}
          {#if reactions.length > 0}
            <div class="absolute -bottom-3 z-[1] flex gap-1 {is_me ? 'left-2' : 'right-2'}">
              {#each reactions as reaction (reaction.emoji)}
                <button data-testid="reaction" data-emoji={reaction.emoji} class="inline-flex h-6 items-center gap-1 rounded-full bg-paper px-1.5 text-[12px] text-ink-2 shadow-[0_1px_3px_rgb(0_0_0/0.18)] hairline hover:bg-paper-3" title={reaction.names.join(', ')} onclick={() => react(reaction.emoji)}>
                  <EmojiGlyph emoji={reaction.emoji} providerId={message.provider_id} />
                  {#if reaction.count > 1}<span class="font-mono text-[11px] text-ink-3">{reaction.count}</span>{/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>
        {#if tall}
          <Button size="sm" icon={full ? 'lucide:chevron-up' : 'lucide:chevron-down'} onclick={() => toggleFold(message.id, 'full', false)}>{full ? t('common.showLess') : t('common.showMore')}</Button>
        {/if}
      {/if}
      {#if message.has_attachments && message.attachments?.length > 0}
        <AttachmentPreview attachments={message.attachments} messageId={message.id} />
      {/if}
      {#if reactions.length > 0 && !onBubble}
        <div class="mt-1 flex flex-wrap gap-1">
          {#each reactions as reaction (reaction.emoji)}
            <button data-testid="reaction" data-emoji={reaction.emoji} class="inline-flex h-6 items-center gap-1 rounded-full px-2 text-[12px] text-ink-2 hairline hover:bg-paper-3" title={reaction.names.join(', ')} onclick={() => react(reaction.emoji)}>
              <EmojiGlyph emoji={reaction.emoji} providerId={message.provider_id} />
              <span class="font-mono text-[11px] text-ink-3">{reaction.count}</span>
            </button>
          {/each}
        </div>
      {/if}
      {#if !is_first}
        <span class="pointer-events-none absolute top-2.5 hidden font-mono text-[11px] text-ink-3 group-hover/line:block {is_me ? 'left-0' : 'right-0'}">
          {formatDate(message.timestamp, { hour: '2-digit', minute: '2-digit' })}
        </span>
      {/if}
    {/if}
  </div>

  {#if thread_info && thread_info.message_count > 0}
    <Button size="sm" icon="lucide:messages-square" class="-ml-2.5 mt-1 text-accent" onclick={handleOpenThread}>
      {t('conversation.threads.replies', { count: thread_info.message_count })}
      <span class="font-mono text-[11px] text-ink-3">{formatTime(thread_info.last_activity)}</span>
    </Button>
  {/if}

  <MessageContextMenu
    {message}
    {is_me}
    x={context_menu_x}
    y={context_menu_y}
    visible={context_menu_visible}
    onclose={() => context_menu_visible = false}
    onedit={startEditing}
    ondelete={() => is_confirming_delete = true}
  />
{/if}
