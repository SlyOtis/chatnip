<script lang="ts">
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { conversationsStore } from '$lib/tauri'
  import type { Thread, Conversation, Message } from '$lib/tauri'
  import { api } from '$lib/tauri'
  import { openThread } from '$lib/tauri/threads'
  import { relativeTime } from '$lib/inbox'
  import { t } from '$lib/i18n.svelte'

  interface ThreadCard {
    thread: Thread
    conversation: Conversation
    root_message: Message | null
  }

  let is_loading = $derived($conversationsStore.is_loading)

  let thread_cards = $derived.by(() => {
    const cards: ThreadCard[] = []
    for (const conversation of $conversationsStore.data) {
      for (const thread of conversation.threads ?? []) {
        cards.push({ thread, conversation, root_message: null })
      }
    }
    cards.sort((a, b) => new Date(b.thread.last_activity).getTime() - new Date(a.thread.last_activity).getTime())
    return cards
  })

  async function handleThreadClick(card: ThreadCard) {
    try {
      await api.putCurrentConversation(card.conversation.id)
      if (card.root_message) openThread(card.thread.id, card.root_message)
      await goto(`/main/conversation/${card.conversation.id}/chat`)
    } catch {}
  }
</script>

<div class="flex h-full flex-col bg-paper">
  <header class="flex h-12 shrink-0 items-center gap-2 border-b border-line px-5 pt-1">
    <h1 class="text-[15px] font-semibold tracking-tight text-ink">{t('conversation.threads.title')}</h1>
    <span class="font-mono text-[11px] text-ink-3">{thread_cards.length}</span>
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto">
    {#if is_loading && thread_cards.length === 0}
      <div class="flex flex-col gap-2 p-4">
        {#each Array(5) as _, i (i)}
          <div class="h-14 animate-pulse rounded-[var(--radius-row)] bg-paper-2"></div>
        {/each}
      </div>
    {:else if thread_cards.length === 0}
      <div class="flex h-full flex-col items-center justify-center gap-2 px-8 text-center">
        <Icon icon="lucide:messages-square" class="text-[40px] text-accent" />
        <p class="text-[15px] font-semibold text-ink">{t('conversation.threads.emptyTitle')}</p>
        <p class="text-[13px] text-ink-3">{t('conversation.threads.emptyHint')}</p>
      </div>
    {:else}
      <div class="mx-auto max-w-[46rem] divide-y divide-line px-4 py-2">
        {#each thread_cards as card (card.thread.id)}
          <button class="grid w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-[var(--radius-row)] px-3 py-3 text-left hover:bg-paper-2" onclick={() => handleThreadClick(card)}>
            <div class="min-w-0">
              <div class="flex items-center gap-2">
                <span class="truncate text-[14px] font-medium text-ink">{card.thread.name || card.conversation.name}</span>
                {#if card.thread.is_resolved}
                  <span class="shrink-0 rounded-full bg-success-100 px-1.5 font-mono text-[10px] leading-4 text-success-700">{t('conversation.threads.resolved')}</span>
                {/if}
              </div>
              <div class="truncate text-[13px] text-ink-3">
                {card.conversation.name} · {t('conversation.threads.replies', { count: card.thread.message_count })}
              </div>
            </div>
            <span class="font-mono text-[11px] text-ink-3">{relativeTime(card.thread.last_activity)}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>
