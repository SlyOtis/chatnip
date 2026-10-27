<script lang="ts">
  import Icon from '@iconify/svelte'
  import { untrack } from 'svelte'
  import { api, conversationsById } from '$lib/tauri'
  import type { ChatSource, Delivery } from '$lib/tauri/types'
  import { Button, Kbd, Sheet } from '$lib/components/ui'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import SourcePicker from '$lib/components/conversation/SourcePicker.svelte'
  import { closeForward, deliveryFor, forwarding, replyPreview } from '$lib/reply.svelte'
  import { formatDate, t, type MessageKey } from '$lib/i18n.svelte'

  let sources = $state<ChatSource[]>(untrack(() => forwarding.targets.map((id) => ({ kind: 'conversation' as const, id }))))
  let comment = $state(untrack(() => forwarding.comment))
  let sending = $state(false)
  let error = $state<string | null>(null)

  const messages = $derived(forwarding.messages)
  const previews = $derived(messages.map((m) => ({ message: m, preview: replyPreview(m, t('common.unknown')) })))
  const targets = $derived(sources.filter((s) => s.kind === 'conversation' && s.id).map((s) => $conversationsById.get(s.id ?? '')).filter((c): c is NonNullable<typeof c> => !!c))
  const canSend = $derived(!sending && messages.length > 0 && targets.length > 0)

  const DELIVERY: Record<Delivery, MessageKey> = {
    native: 'conversation.forward.native',
    repost: 'conversation.forward.repost',
    mail: 'conversation.forward.mail',
    unsupported: 'conversation.forward.repost'
  }

  async function send() {
    if (!canSend) return
    sending = true
    error = null
    try {
      const ids = targets.map((c) => c.id)
      await api.postMessageForward({ message_ids: messages.map((m) => m.id), targets: ids.map((id) => ({ conversation_id: id })), comment: comment.trim() || null }, { surface: 'forward', conversation_id: messages[0]?.conversation_id ?? null, draft: { messages: $state.snapshot(messages), comment, targets: ids } })
      closeForward()
    } catch (e) {
      error = String(e)
    } finally {
      sending = false
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault()
      send()
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<Sheet testid="forward-dialog" label={t('conversation.forward.title', { count: messages.length })} width="40rem" onclose={closeForward}>
  <div class="flex flex-col gap-3 px-5 pb-2 pt-4">
    <h2 class="text-[15px] font-semibold tracking-tight text-ink">{t('conversation.forward.title', { count: messages.length })}</h2>
    <section class="flex flex-col gap-1.5" aria-label={t('conversation.forward.preview')}>
      <p class="font-mono text-[11px] text-ink-3">{t('conversation.forward.preview')}</p>
      <ul class="flex max-h-[11rem] flex-col gap-1.5 overflow-y-auto">
        {#each previews as { message, preview } (message.id)}
          <li data-testid="forward-preview" data-id={message.id} class="flex min-w-0 flex-col border-l-2 border-line pl-2">
            <span class="flex min-w-0 items-center gap-1.5 text-[12.5px]">
              <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-medium text-ink-2">{preview.author}</span>
              <span class="shrink-0 font-mono text-[11px] text-ink-3">{formatDate(message.timestamp, { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' })}</span>
            </span>
            {#if preview.excerpt}
              <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[13px] text-ink">{preview.excerpt}</span>
            {/if}
            {#if preview.attachments}
              <span class="flex min-w-0 items-center gap-1 text-[12px] text-ink-3">
                <Icon icon="lucide:paperclip" class="shrink-0 text-[11px]" />
                <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{preview.attachmentNames.join(', ')}</span>
              </span>
            {/if}
          </li>
        {/each}
      </ul>
    </section>
    <textarea data-testid="forward-comment" bind:value={comment} rows="2" placeholder={t('conversation.forward.comment')} aria-label={t('conversation.forward.comment')} class="w-full resize-none border-b border-line bg-transparent py-1.5 text-[14px] text-ink outline-none placeholder:text-ink-3 focus:border-ink-3"></textarea>
    <section class="flex flex-col gap-1.5" aria-label={t('conversation.forward.to')}>
      <p class="font-mono text-[11px] text-ink-3">{t('conversation.forward.to')}</p>
      {#if targets.length}
        <ul class="flex flex-col">
          {#each targets as target (target.id)}
            <li data-testid="forward-target" data-id={target.id} data-delivery={deliveryFor(messages, target)} class="grid h-6 grid-cols-[1.25rem_minmax(0,1fr)_auto] items-center gap-1.5 text-[12.5px]">
              <ProviderIcon providers={target.providers.map((p) => ({ ...p, conversation_type: target.type }))} size="xs" max={1} />
              <span class="overflow-hidden text-ellipsis whitespace-nowrap text-ink-2">{target.name}</span>
              <span class="font-mono text-[11px] text-ink-3">{t(DELIVERY[deliveryFor(messages, target)])}</span>
            </li>
          {/each}
        </ul>
      {/if}
      <SourcePicker {sources} writes chosen={false} onchange={(next) => (sources = next.filter((s) => s.kind === 'conversation'))} />
    </section>
    {#if error}
      <p data-testid="forward-error" class="text-[12px] text-error-600">{error}</p>
    {/if}
  </div>
  {#snippet footer()}
    {#if !targets.length}
      <span class="text-[12px] text-ink-3">{t('conversation.forward.pick')}</span>
    {/if}
    <span class="ml-auto flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="esc" /> {t('modals.footer.close')}</span>
    <Button testid="forward-send" variant="primary" icon="lucide:forward" keys="⌘↵" busy={sending} disabled={!canSend} onclick={send}>{t('conversation.forward.send')}</Button>
  {/snippet}
</Sheet>
