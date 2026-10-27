<script lang="ts">
  import Icon from '@iconify/svelte'
  import { api } from '$lib/tauri'
  import type { ChatItem } from '$lib/tauri/types'
  import { iconFor } from '$lib/grid'
  import { formatDate, formatNumber, t, type MessageKey } from '$lib/i18n.svelte'

  interface Props {
    item: ChatItem
    onshow?: (conversationId: string) => void
    compact?: boolean
  }

  let { item, onshow, compact = false }: Props = $props()

  let failure = $state<string | null>(null)

  const record = $derived(item.record ?? {})
  const attachment = $derived((record.attachment ?? null) as { id?: string; name?: string; content_type?: string; path?: string | null } | null)
  const receipt = $derived(record.kind === 'receipt')
  const statusCard = $derived(record.kind === 'status')
  const STATES: Record<string, { icon: string; tone: string }> = {
    ok: { icon: 'lucide:circle-check', tone: 'text-accent' },
    warn: { icon: 'lucide:triangle-alert', tone: 'text-warn-600' },
    fail: { icon: 'lucide:circle-x', tone: 'text-error-600' },
    info: { icon: 'lucide:info', tone: 'text-ink-3' }
  }
  const look = $derived(STATES[typeof record.state === 'string' ? record.state : 'info'] ?? STATES.info)

  const FIELDS: { key: string; label: MessageKey; mono?: boolean }[] = [
    { key: 'vendor', label: 'chats.record.vendor' },
    { key: 'date', label: 'chats.record.date', mono: true },
    { key: 'total', label: 'chats.record.total', mono: true },
    { key: 'vat', label: 'chats.record.vat', mono: true },
    { key: 'order_number', label: 'chats.record.order', mono: true },
    { key: 'card_ending', label: 'chats.record.card', mono: true },
    { key: 'org_number', label: 'chats.record.org', mono: true },
    { key: 'conversation_name', label: 'chats.record.from' }
  ]

  const SKIP = new Set(['id', 'kind', 'title', 'text', 'attachment', 'excerpt', 'hits', 'confirmed', 'confidence', 'score', 'message_id', 'conversation_id', 'provider', 'connection', 'subject', 'sender', 'at', 'currency'])

  function shown(key: string, value: unknown): string {
    if (value === null || value === undefined || value === '') return ''
    if (key === 'total' || key === 'vat') {
      const currency = typeof record.currency === 'string' ? record.currency : ''
      const amount = typeof value === 'number' ? formatNumber(value, { minimumFractionDigits: 2, maximumFractionDigits: 2 }) : String(value)
      return currency ? `${amount} ${currency}` : amount
    }
    if (key === 'date' && typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value)) return formatDate(`${value}T12:00:00`, { dateStyle: 'medium' })
    if (key === 'card_ending' && typeof value === 'string') return `•••• ${value}`
    if (typeof value === 'object') return JSON.stringify(value)
    return String(value)
  }

  const rows = $derived.by(() => {
    const known = FIELDS.map((f) => ({ label: t(f.label), value: shown(f.key, record[f.key]), mono: !!f.mono })).filter((r) => r.value)
    if (receipt) return known
    const rest = Object.entries(record)
      .filter(([key, value]) => !SKIP.has(key) && !FIELDS.some((f) => f.key === key) && (typeof value !== 'object' || value === null))
      .map(([key, value]) => ({ label: key.replace(/_/g, ' '), value: shown(key, value), mono: false }))
      .filter((r) => r.value)
    return [...known, ...rest]
  })

  const confidence = $derived(typeof record.confidence === 'number' ? Math.round(record.confidence * 100) : null)
  const status = $derived(record.confirmed === true ? t('chats.record.confirmed') : record.confirmed === false ? t('chats.record.rejected') : receipt ? t('chats.record.unconfirmed') : null)

  async function open(quick_look: boolean) {
    failure = null
    try {
      if (typeof record.message_id === 'string' && attachment?.id) await api.postAttachmentOpen({ message_id: record.message_id, attachment_id: attachment.id, quick_look })
      else if (attachment?.path) await api.postAttachmentOpen({ path: attachment.path, quick_look })
    } catch (error) {
      failure = String(error)
    }
  }
</script>

{#if statusCard}
<article class="my-2 rounded-[0.5rem] bg-paper-2 px-4 py-2.5" data-testid="status-card" data-state={record.state}>
  <header class="flex min-w-0 items-center gap-2">
    <Icon icon={look.icon} class="shrink-0 text-[15px] {look.tone}" />
    <h3 class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-[14px] font-semibold text-ink">{item.title}</h3>
    <time class="shrink-0 font-mono text-[11px] text-ink-3" datetime={item.at}>{formatDate(item.at, { dateStyle: 'short', timeStyle: 'short' })}</time>
  </header>
  {#if typeof record.text === 'string' && record.text}
    <p class="mt-1 whitespace-pre-wrap break-words text-[14px] leading-5 text-ink">{record.text}</p>
  {/if}
</article>
{:else}
<article class="group/record {compact ? '' : 'border-b border-line py-3'}">
  <header class="flex min-w-0 items-center gap-2">
    <Icon icon={receipt ? 'lucide:receipt' : 'lucide:file-search'} class="shrink-0 text-[15px] text-ink-2" />
    <h3 class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-[14px] font-semibold text-ink">{item.title}</h3>
    {#if status}<span class="shrink-0 text-[12px] {record.confirmed === true ? 'text-accent' : 'text-ink-3'}">{status}</span>{/if}
    {#if confidence !== null}<span class="shrink-0 font-mono text-[11px] text-ink-3" title={t('chats.record.confidence')}>{confidence}%</span>{/if}
    <time class="shrink-0 font-mono text-[11px] text-ink-3" datetime={item.at}>{formatDate(item.at, { dateStyle: 'short', timeStyle: 'short' })}</time>
  </header>
  {#if rows.length}
    <dl class="{compact ? 'mt-1' : 'mt-2'} grid grid-cols-[minmax(0,max-content)_minmax(0,1fr)] gap-x-4 gap-y-0.5 text-[13px]">
      {#each compact ? rows.slice(0, 4) : rows as row (row.label)}
        <dt class="text-ink-3">{row.label}</dt>
        <dd class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-ink {row.mono ? 'font-mono text-[12.5px]' : ''}">{row.value}</dd>
      {/each}
    </dl>
  {/if}
  {#if typeof record.text === 'string' && record.text && record.text !== item.title}
    <p class="mt-1.5 whitespace-pre-wrap break-words text-[13.5px] leading-5 text-ink">{record.text}</p>
  {/if}
  {#if !compact && typeof record.excerpt === 'string' && record.excerpt}
    <p class="mt-2 line-clamp-2 text-[12.5px] leading-5 text-ink-3">{record.excerpt}</p>
  {/if}
  {#if !compact}
  <footer class="mt-2 flex flex-wrap items-center gap-1.5">
    {#if attachment?.name}
      <button type="button" class="flex h-7 max-w-full items-center gap-2 rounded-[0.5rem] px-2.5 text-[12.5px] text-ink hairline hover:bg-paper-3" onclick={() => open(false)} ondblclick={() => open(false)} onkeydown={(e) => e.key === ' ' && (e.preventDefault(), open(true))} title={t('chats.grid.open')}>
        <Icon icon={iconFor({ name: attachment.name, content_type: attachment.content_type ?? '', kind: 'file', record: null })} class="shrink-0 text-[14px] text-ink-3" />
        <span class="min-w-0 truncate">{attachment.name}</span>
      </button>
      <button type="button" class="flex h-7 items-center gap-1 rounded-[0.5rem] px-2 text-[12.5px] text-ink-2 hover:bg-paper-3 hover:text-ink" onclick={() => open(true)}>
        <Icon icon="lucide:eye" class="text-[13px]" />{t('chats.grid.quickLook')}
      </button>
    {/if}
    {#if onshow && typeof record.conversation_id === 'string' && record.conversation_id}
      <button type="button" class="flex h-7 items-center gap-1 rounded-[0.5rem] px-2 text-[12.5px] text-ink-2 hover:bg-paper-3 hover:text-ink" onclick={() => onshow?.(String(record.conversation_id))}>
        <Icon icon="lucide:message-square" class="text-[13px]" />{t('chats.grid.showInChat')}
      </button>
    {/if}
  </footer>
  {/if}
  {#if failure}<p class="mt-1 text-[12px] text-error-600">{failure}</p>{/if}
</article>
{/if}
