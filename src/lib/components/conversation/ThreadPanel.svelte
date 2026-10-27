<script lang="ts">
  import { listen } from '@tauri-apps/api/event'
  import type { Message } from '$lib/tauri'
  import { api, conversationStore, providersStore } from '$lib/tauri'
  import { accountOf } from '$lib/accounts'
  import { Button, IconButton } from '$lib/components/ui'
  import Composer from '$lib/components/composer/Composer.svelte'
  import { emptyDoc, isEmpty, toMarkdown, type Doc } from '$lib/components/composer/markdown'
  import MessageGroup from './messages/MessageGroup.svelte'
  import TypingIndicator from './TypingIndicator.svelte'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    thread_id: string
    root_message: Message
    onclose: () => void
  }

  let { thread_id, root_message, onclose }: Props = $props()

  let thread_messages = $state<Message[]>([])
  let scroll_container = $state<HTMLDivElement | null>(null)
  let is_loading = $state(false)
  let doc = $state<Doc>(emptyDoc())
  let editor = $state<ReturnType<typeof Composer>>()
  let broadcast = $state(false)
  let sending = $state(false)
  let error = $state<string | null>(null)

  const conversation_name = $derived($conversationStore.data?.name ?? t('conversation.thread.conversation'))
  const capabilities = $derived(accountOf($providersStore.data, root_message.provider_id)?.capabilities ?? null)
  const broadcastable = $derived(!!capabilities && capabilities.thread_broadcast !== 'unsupported')

  const groups = $derived.by(() => {
    const out: Message[][] = []
    for (const message of thread_messages) {
      const current = out[out.length - 1]
      if (current && current[current.length - 1].sender_id === message.sender_id) current.push(message)
      else out.push([message])
    }
    return out
  })

  $effect(() => {
    if (thread_id) loadThreadMessages()
  })

  $effect(() => {
    const conversation = root_message.conversation_id
    const changed = listen<string>('conversation-changed', (event) => {
      if (event.payload === conversation) loadThreadMessages()
    })
    const updated = listen('messages-updated', () => loadThreadMessages())
    return () => {
      changed.then((stop) => stop())
      updated.then((stop) => stop())
    }
  })

  $effect(() => {
    if (thread_messages.length > 0 && scroll_container) {
      setTimeout(() => {
        scroll_container?.scrollTo({ top: scroll_container.scrollHeight, behavior: 'smooth' })
      }, 100)
    }
  })

  $effect(() => {
    const id = thread_id
    let live = true
    api
      .getDrafts(root_message.conversation_id)
      .then((rows) => {
        const row = rows.find((r) => r.thread_id === id)
        if (live && row?.markdown.trim() && editor?.isBlank()) editor?.insertMarkdown(row.markdown)
      })
      .catch(() => {})
    return () => {
      live = false
    }
  })

  $effect(() => {
    const markdown = toMarkdown(doc)
    const conversation_id = root_message.conversation_id
    const id = thread_id
    const copy = $state.snapshot(doc)
    const timer = setTimeout(() => {
      if (markdown.trim()) api.putDraft({ conversation_id, thread_id: id, doc: copy, markdown }).catch(() => {})
    }, 800)
    return () => clearTimeout(timer)
  })

  async function loadThreadMessages() {
    is_loading = true
    try {
      thread_messages = await api.getThreadMessages({ thread_id })
    } catch {
    } finally {
      is_loading = false
    }
  }

  async function handleSendReply() {
    if (isEmpty(doc) || sending) return
    sending = true
    error = null
    try {
      await api.postThreadReply({ thread_id, content: toMarkdown(doc), content_type: 'markdown', broadcast: broadcastable && broadcast })
      editor?.clear()
      broadcast = false
      api.deleteDraft(root_message.conversation_id, thread_id).catch(() => {})
      await loadThreadMessages()
    } catch (e) {
      error = String(e)
    } finally {
      sending = false
    }
  }
</script>

<aside class="flex h-full min-w-0 flex-col bg-paper" data-testid="thread-panel" aria-label={t('conversation.thread.title')}>
  <header class="flex shrink-0 items-start gap-2 px-5 pb-2 pt-4">
    <div class="min-w-0 flex-1">
      <h2 class="text-[14px] font-semibold leading-5 text-ink">{t('conversation.thread.title')}</h2>
      <p class="truncate text-[13px] text-ink-3">{conversation_name}</p>
    </div>
    <IconButton icon="lucide:x" label={t('common.close')} keys="esc" onclick={onclose} />
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto overflow-x-hidden px-2" bind:this={scroll_container}>
    <div class="border-b border-line pb-2" data-testid="thread-root">
      <MessageGroup messages={[root_message]} />
    </div>

    <p class="px-3 pt-3 text-[13px] text-ink-3" data-testid="thread-count">{t('conversation.threads.replies', { count: thread_messages.length })}</p>

    {#if is_loading && thread_messages.length === 0}
      <div class="flex flex-col gap-3 px-3 py-3">
        {#each Array(3) as _, i (i)}
          <div class="h-10 animate-pulse rounded-[0.5rem] bg-paper-2"></div>
        {/each}
      </div>
    {:else if thread_messages.length === 0}
      <p class="px-3 py-6 text-[13px] text-ink-3">{t('conversation.thread.noReplies')}</p>
    {:else}
      <div class="pb-4" data-testid="thread-replies">
        {#each groups as messages (messages[0].id)}
          <MessageGroup {messages} />
        {/each}
      </div>
    {/if}
  </div>

  <TypingIndicator conversationId={root_message.conversation_id} />

  <div class="flex shrink-0 flex-col border-t border-line px-5 pb-4 pt-2">
    <Composer bind:this={editor} bind:doc conversationId={root_message.conversation_id} providerId={root_message.provider_id} placeholder={t('conversation.thread.replyPlaceholder')} maxLines={8} onsubmit={handleSendReply} />
    {#if error}
      <p class="pt-1 text-[12px] text-error-600" data-testid="thread-error">{error}</p>
    {/if}
    <div class="flex items-center gap-2 pt-1">
      {#if broadcastable}
        <label class="flex items-center gap-2 text-[12.5px] text-ink-2" data-testid="thread-broadcast">
          <input type="checkbox" bind:checked={broadcast} class="accent-[var(--accent)]" />
          <span>{t('conversation.thread.broadcast')}</span>
        </label>
      {/if}
      <span class="ml-auto"><Button variant="primary" testid="thread-send" keys="⌘↵" disabled={isEmpty(doc) || sending} onclick={handleSendReply}>{t('common.send')}</Button></span>
    </div>
  </div>
</aside>
