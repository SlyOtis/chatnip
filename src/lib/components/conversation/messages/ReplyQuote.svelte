<script module lang="ts">
  import { api } from '$lib/tauri'
  import type { Message } from '$lib/tauri'

  const originals = new Map<string, Promise<Message | null>>()

  function originalOf(id: string): Promise<Message | null> {
    let pending = originals.get(id)
    if (!pending) {
      pending = api.getMessageQuote(id).catch(() => null)
      originals.set(id, pending)
    }
    return pending
  }
</script>

<script lang="ts">
  import Icon from '@iconify/svelte'
  import { messagesStore, peopleStore } from '$lib/tauri'
  import { embeddedQuote, excerptOf, loadedOriginal } from '$lib/reply.svelte'
  import { revealMessage } from '$lib/messageLink'
  import { renderEmojiText } from '$lib/emoji'
  import { userAvatar } from '$lib/avatar'
  import { Avatar } from '$lib/components/ui'
  import MessageContent from './MessageContent.svelte'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    message: Message
    class?: string
  }

  let { message, class: className = '' }: Props = $props()

  let fetched = $state<Message | null>(null)
  let open = $state(false)

  const loaded = $derived(loadedOriginal(message, $messagesStore.data ?? []))
  const original = $derived(loaded ?? fetched)
  const embedded = $derived(original ? null : embeddedQuote(message.content))
  const author = $derived(original ? original.sender_name || original.sender_email || t('common.unknown') : embedded?.author || t('common.unknown'))
  const excerpt = $derived(renderEmojiText(original ? excerptOf(original.content, 160) : excerptOf(embedded?.text, 160)))
  const face = $derived(userAvatar({ id: original?.sender_id ?? '', display_name: author, email: original?.sender_email ?? null, avatar_url: null }, $peopleStore, author))
  const files = $derived((original?.attachments ?? []).filter((a) => !a.is_inline).length)

  $effect(() => {
    const id = message.id
    if (loaded) return
    let live = true
    originalOf(id).then((found) => {
      if (live) fetched = found
    })
    return () => {
      live = false
    }
  })

  function jump() {
    if (!original) return
    if (open) {
      open = false
      return
    }
    revealMessage(original.id, { delay: 0, attempts: 1 }).done.then((found) => {
      if (!found) open = true
    })
  }
</script>

{#snippet line()}
  <span class="mb-[-0.125rem] ml-[calc(0.75rem-1px)] h-[0.6875rem] w-[1.125rem] shrink-0 self-end rounded-tl-[0.375rem] border-l-2 border-t-2 border-line-strong group-hover/reply:border-ink-3"></span>
  <Avatar source={face} size="xs" class="size-4! text-[0.4375rem]!" />
  <span class="shrink-0 font-semibold text-ink-2 group-hover/reply:text-ink">{author}</span>
  <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-ink-3 group-hover/reply:text-ink-2">{excerpt}</span>
  {#if files}
    <Icon icon="lucide:paperclip" class="shrink-0 text-[0.6875rem] text-ink-3" />
  {/if}
{/snippet}

{#if original || embedded}
  <div class="flex min-w-0 max-w-full flex-col {className}" data-reply-quote data-testid="reply-quote" data-reply={message.id} data-original={original?.id}>
    {#if original}
      <button type="button" class="group/reply flex h-5 min-w-0 max-w-[68ch] items-center gap-1.5 text-left text-[0.75rem] leading-none" onclick={jump} title={t('conversation.reply.jump')} aria-expanded={open}>
        {@render line()}
      </button>
    {:else}
      <div class="flex h-5 min-w-0 max-w-[68ch] items-center gap-1.5 text-[0.75rem] leading-none">
        {@render line()}
      </div>
    {/if}
    {#if open && original}
      <div class="ml-[2.25rem] mt-1 max-w-[68ch] border-l-2 border-line-strong pl-2 text-[0.8125rem] text-ink-2 selectable">
        <MessageContent content={original.content} content_type={original.content_type} providerId={original.provider_id} />
      </div>
    {/if}
  </div>
{/if}
