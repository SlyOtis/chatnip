<script lang="ts">
  import Icon from '@iconify/svelte'
  import { conversationStore, loadPinned, pinnedStore, putPin } from '$lib/tauri'
  import { formatTime } from '$lib/dates'
  import { IconButton } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    collapsible?: boolean
    onpick?: (message_id: string) => void
  }

  let { collapsible = true, onpick }: Props = $props()

  let is_loading = $state(false)
  let is_expanded = $state(true)

  const conversation_id = $derived($conversationStore.data?.id ?? null)
  const pinned_messages = $derived(conversation_id && $pinnedStore.conversation_id === conversation_id ? $pinnedStore.data : [])
  const open = $derived(!collapsible || is_expanded)

  $effect(() => {
    if (!conversation_id || !open) return
    loadPinnedMessages(conversation_id)
  })

  async function loadPinnedMessages(id: string) {
    is_loading = true
    await loadPinned(id)
    is_loading = false
  }

  async function handleUnpin(message_id: string) {
    if (!conversation_id) return
    try {
      await putPin(conversation_id, message_id, false)
    } catch {}
  }
</script>

<section class="py-3" data-testid="pinned">
  {#if collapsible}
    <button data-testid="pinned-toggle" onclick={() => is_expanded = !is_expanded} class="flex h-7 w-full items-center gap-1.5 text-[13px] text-ink-3 hover:text-ink">
      <span>{t('conversation.pinned.title')}</span>
      {#if pinned_messages.length}
        <span class="font-mono text-[11px]">{pinned_messages.length}</span>
      {/if}
      <Icon icon={is_expanded ? 'lucide:chevron-down' : 'lucide:chevron-right'} class="ml-auto text-[14px]" />
    </button>
  {:else}
    <p class="flex h-7 items-center gap-1.5 text-[13px] text-ink-3">
      <span>{t('conversation.pinned.title')}</span>
      {#if pinned_messages.length}
        <span class="font-mono text-[11px]">{pinned_messages.length}</span>
      {/if}
    </p>
  {/if}

  {#if open}
    {#if is_loading && pinned_messages.length === 0}
      <div class="mt-1 h-10 animate-pulse rounded-[0.5rem] bg-paper-2"></div>
    {:else if pinned_messages.length === 0}
      <p class="mt-1 text-[13px] text-ink-3">{t('conversation.pinned.none')}</p>
    {:else}
      {#each pinned_messages as pin (pin.id)}
        <div data-testid="pinned-message" data-id={pin.message_id} class="group flex items-start gap-2 border-t border-line py-2.5">
          <svelte:element
            this={onpick ? 'button' : 'div'}
            type={onpick ? 'button' : undefined}
            role={onpick ? undefined : 'presentation'}
            data-testid={onpick ? 'pinned-open' : undefined}
            class="min-w-0 flex-1 text-left {onpick ? 'cursor-pointer rounded-[0.4rem] hover:text-ink' : ''}"
            onclick={onpick ? () => onpick(pin.message_id) : undefined}
          >
            <div class="flex items-center gap-2">
              <span class="truncate text-[13px] font-medium text-ink">{pin.message?.sender_name ?? t('common.unknown')}</span>
              <span class="ml-auto font-mono text-[11px] text-ink-3">{formatTime(pin.created_at)}</span>
            </div>
            <p class="mt-0.5 line-clamp-2 text-[13px] text-ink-2">{pin.message?.content ?? ''}</p>
          </svelte:element>
          <IconButton testid="pinned-unpin" icon="lucide:x" label={t('conversation.pinned.unpin')} size="sm" class="opacity-0 group-hover:opacity-100" onclick={() => handleUnpin(pin.message_id)} />
        </div>
      {/each}
    {/if}
  {/if}
</section>
