<script lang="ts">
  import { LIGHT, type Light } from '$lib/components/settings/Connections.svelte'
  import { onMount } from 'svelte'
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { listen } from '@tauri-apps/api/event'
  import { api } from '$lib/tauri'
  import type { HistoryEntry, HistoryOption } from '$lib/tauri/types'
  import { Button, Segmented } from '$lib/components/ui'
  import { formatDate, t, type MessageKey } from '$lib/i18n.svelte'

  let entries = $state<HistoryEntry[]>([])
  let filter = $state<'all' | 'sent' | 'problems'>('all')
  let open = $state<Set<string>>(new Set())
  let busy = $state<string | null>(null)
  let failure = $state<string | null>(null)

  const KIND: Record<HistoryEntry['kind'], MessageKey> = { sent: 'app.history.kind.sent', failed: 'app.history.kind.failed', error: 'app.history.kind.error', warning: 'app.history.kind.warning', info: 'app.history.kind.info' }
  const ICON: Record<HistoryEntry['kind'], string> = { sent: 'lucide:send', failed: 'lucide:send-horizontal', error: 'lucide:circle-x', warning: 'lucide:triangle-alert', info: 'lucide:info' }

  const shown = $derived(entries.filter((entry) => (filter === 'all' ? true : filter === 'sent' ? entry.kind === 'sent' : entry.kind !== 'sent' && entry.kind !== 'info')))
  const days = $derived.by(() => {
    const groups = new Map<string, HistoryEntry[]>()
    for (const entry of shown) {
      const day = formatDate(entry.created_at, { weekday: 'long', day: 'numeric', month: 'long' })
      groups.set(day, [...(groups.get(day) ?? []), entry])
    }
    return [...groups.entries()]
  })

  function light(entry: HistoryEntry): Light {
    if (entry.status === 'cancelled') return 'grey'
    if (entry.status === 'waiting') return 'amber'
    if (entry.kind === 'sent' || entry.status === 'resolved') return 'green'
    if (entry.kind === 'warning' || entry.kind === 'info') return 'amber'
    return 'red'
  }

  async function load() {
    try {
      entries = await api.getHistory(300)
      failure = null
    } catch (e) {
      failure = String(e)
    }
  }

  async function choose(entry: HistoryEntry, option: HistoryOption) {
    busy = entry.id
    try {
      const updated = await api.putHistory(entry.id, option)
      entries = entries.map((e) => (e.id === updated.id ? updated : e))
      failure = null
    } catch (e) {
      failure = String(e)
    } finally {
      busy = null
    }
  }

  function toggle(id: string) {
    const next = new Set(open)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    open = next
  }

  function when(iso: string): string {
    return formatDate(iso, { hour: '2-digit', minute: '2-digit', second: '2-digit' })
  }

  onMount(() => {
    load()
    const off = listen<HistoryEntry>('history-updated', () => load())
    return () => {
      off.then((stop) => stop())
    }
  })
</script>

<section data-testid="history-page" class="flex h-full min-h-0 flex-col bg-paper" aria-label={t('app.rail.history')}>
  <header class="flex flex-wrap items-center gap-3 border-b border-line px-5 py-3">
    <Icon icon="lucide:history" class="text-[16px] text-ink-3" />
    <div class="min-w-0 flex-1">
      <h1 class="text-[15px] font-semibold tracking-tight text-ink">{t('app.rail.history')}</h1>
      <p class="text-[12.5px] text-ink-3">{t('app.history.description')}</p>
    </div>
    <Segmented size="sm" testid="history-filter" label={t('app.rail.history')} value={filter} options={[{ value: 'all', label: t('app.history.all') }, { value: 'sent', label: t('app.history.sent'), icon: 'lucide:send' }, { value: 'problems', label: t('app.history.problems'), icon: 'lucide:triangle-alert' }]} onchange={(v) => (filter = v as typeof filter)} />
  </header>
  {#if failure}<p class="px-5 py-2 text-[12.5px] text-error-600">{failure}</p>{/if}
  <div class="min-h-0 flex-1 overflow-y-auto px-5 pb-6">
    {#if !shown.length}
      <p class="py-8 text-center text-[13px] text-ink-3">{t('app.history.empty')}</p>
    {/if}
    {#each days as [day, list] (day)}
      <h2 class="sticky top-0 z-10 bg-paper py-2 text-[12px] font-medium uppercase tracking-wide text-ink-3">{day}</h2>
      <ol class="flex flex-col gap-px">
        {#each list as entry (entry.id)}
          {@const expanded = open.has(entry.id)}
          {@const tone = light(entry)}
          <li data-testid="history-entry" data-kind={entry.kind} data-status={entry.status} class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-start gap-3 rounded-[0.5rem] px-2 py-2 hover:bg-paper-2">
            <span class="mt-1.5 flex items-center gap-2">
              <span class="size-2 rounded-full {LIGHT[tone]}" title={t(KIND[entry.kind])}></span>
              <Icon icon={ICON[entry.kind]} class="text-[14px] text-ink-3" />
            </span>
            <div class="flex min-w-0 flex-col gap-0.5">
              <div class="flex min-w-0 flex-wrap items-baseline gap-x-2">
                <span class="font-mono text-[11px] tabular-nums text-ink-3">{when(entry.created_at)}</span>
                <span class="text-[12px] text-ink-3">{t(KIND[entry.kind])}</span>
                {#if entry.status === 'waiting' || entry.status === 'resolved' || entry.status === 'cancelled'}
                  <span class="rounded-full bg-paper-3 px-1.5 text-[10.5px] leading-4 text-ink-2">{t(`app.history.status.${entry.status}` as MessageKey)}</span>
                {/if}
              </div>
              <p class="text-[13.5px] text-ink {expanded ? 'whitespace-pre-wrap' : 'overflow-hidden text-ellipsis whitespace-nowrap'}">{entry.title}</p>
              {#if entry.detail}
                <p class="text-[12.5px] leading-5 text-ink-2 {expanded ? 'whitespace-pre-wrap' : 'line-clamp-2'}">{entry.detail}</p>
              {/if}
              {#if entry.kind === 'failed' && entry.status !== 'cancelled' && entry.status !== 'resolved'}
                <div class="flex flex-wrap items-center gap-1 pt-1">
                  <Button size="sm" variant="primary" icon="lucide:send" testid="history-send-now" busy={busy === entry.id} onclick={() => choose(entry, 'send_now')}>{t('app.history.sendNow')}</Button>
                  {#if entry.status !== 'waiting'}
                    <Button size="sm" icon="lucide:wifi" testid="history-send-when-online" busy={busy === entry.id} onclick={() => choose(entry, 'send_when_online')}>{t('app.history.sendWhenOnline')}</Button>
                  {/if}
                  <Button size="sm" icon="lucide:infinity" testid="history-always-wait" busy={busy === entry.id} onclick={() => choose(entry, 'always_wait')}>{t('app.history.alwaysWait')}</Button>
                  <Button size="sm" variant="danger" icon="lucide:x" testid="history-cancel" busy={busy === entry.id} onclick={() => choose(entry, 'cancel')}>{t('app.history.cancelSend')}</Button>
                </div>
              {:else if (entry.kind === 'error' || entry.kind === 'warning') && entry.status === 'open'}
                <div class="flex items-center gap-1 pt-1">
                  <Button size="sm" icon="lucide:check" testid="history-resolve" busy={busy === entry.id} onclick={() => choose(entry, 'resolve')}>{t('app.history.resolve')}</Button>
                </div>
              {/if}
            </div>
            <div class="flex items-center gap-0.5">
              {#if entry.detail || entry.title.length > 80}
                <Button size="sm" icon={expanded ? 'lucide:chevron-up' : 'lucide:chevron-down'} onclick={() => toggle(entry.id)}>{expanded ? t('app.history.less') : t('app.history.more')}</Button>
              {/if}
              {#if entry.href}
                <Button size="sm" icon="lucide:arrow-up-right" testid="history-open" onclick={() => goto(entry.href!)}>{t('app.history.open')}</Button>
              {/if}
            </div>
          </li>
        {/each}
      </ol>
    {/each}
  </div>
</section>
