<script module lang="ts">
  export interface ViewItem {
    id: string
    conversationId: string | null
    messageId: string | null
    icon: string
    title: string
    detail: string
    at: string
    action?: { icon: string; label: string; run: () => unknown }
  }
</script>

<script lang="ts">
  import Icon from '@iconify/svelte'
  import { openMessage, relativeTime } from '$lib/inbox'
  import { t } from '$lib/i18n.svelte'
  import { IconButton } from '$lib/components/ui'

  let {
    title,
    icon,
    testid,
    items,
    loading = false,
    empty,
    hint,
    more = null
  }: { title: string; icon: string; testid: string; items: ViewItem[]; loading?: boolean; empty: string; hint: string; more?: (() => unknown) | null } = $props()

  let list = $state<HTMLDivElement | null>(null)

  function open(item: ViewItem) {
    if (item.conversationId) openMessage(item.conversationId, item.messageId)
  }

  function move(e: KeyboardEvent) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
    const rows = [...(list?.querySelectorAll<HTMLElement>('[data-row]') ?? [])]
    const at = rows.indexOf(document.activeElement as HTMLElement)
    const next = rows[Math.max(0, Math.min(rows.length - 1, at + (e.key === 'ArrowDown' ? 1 : -1)))]
    if (!next) return
    e.preventDefault()
    e.stopPropagation()
    next.focus()
  }
</script>

<div class="flex h-full flex-col bg-paper" data-testid={testid}>
  <header class="flex h-12 shrink-0 items-center gap-2 border-b border-line px-5 pt-1">
    <Icon {icon} class="text-[16px] text-ink-3" />
    <h1 class="text-[15px] font-semibold tracking-tight text-ink">{title}</h1>
    <span class="font-mono text-[11px] text-ink-3">{items.length}</span>
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto">
    {#if loading && items.length === 0}
      <div class="flex flex-col gap-2 p-4">
        {#each Array(5) as _, i (i)}
          <div class="h-14 animate-pulse rounded-[var(--radius-row)] bg-paper-2"></div>
        {/each}
      </div>
    {:else if items.length === 0}
      <div class="flex h-full flex-col items-center justify-center gap-2 px-8 text-center" data-testid="{testid}-empty">
        <Icon {icon} class="text-[40px] text-accent" />
        <p class="text-[15px] font-semibold text-ink">{empty}</p>
        <p class="text-[13px] text-ink-3">{hint}</p>
      </div>
    {:else}
      <div bind:this={list} class="mx-auto flex max-w-[46rem] flex-col divide-y divide-line px-4 py-2" role="list" onkeydown={move}>
        {#each items as item (item.id)}
          <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2" role="listitem" data-testid="{testid}-item" data-id={item.id}>
            <button data-row class="grid min-w-0 grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-3 rounded-[var(--radius-row)] px-3 py-3 text-left hover:bg-paper-2 focus-visible:bg-paper-2 focus-visible:outline-none" onclick={() => open(item)} disabled={!item.conversationId}>
              <Icon icon={item.icon} class="text-[16px] text-ink-3" />
              <span class="min-w-0">
                <span class="block truncate text-[14px] font-medium text-ink">{item.title}</span>
                <span class="block truncate text-[13px] text-ink-3">{item.detail}</span>
              </span>
              <span class="font-mono text-[11px] text-ink-3">{relativeTime(item.at)}</span>
            </button>
            {#if item.action}
              <IconButton size="sm" icon={item.action.icon} label={item.action.label} testid="{testid}-action" onclick={item.action.run} />
            {/if}
          </div>
        {/each}
        {#if more}
          <button class="mt-2 rounded-[var(--radius-row)] px-3 py-2 text-[13px] text-ink-3 hover:bg-paper-2 hover:text-ink" data-testid="{testid}-more" onclick={more}>{t('inbox.views.more')}</button>
        {/if}
      </div>
    {/if}
  </div>
</div>
