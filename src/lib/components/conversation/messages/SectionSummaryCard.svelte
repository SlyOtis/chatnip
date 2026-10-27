<script lang="ts">
  import Icon from '@iconify/svelte'
  import { formatDateSeparator } from '$lib/dates'
  import { goto } from '$app/navigation'
  import { deleteWarning, postSummaryContent, postTimelineSummary, sections, sectionInput, summaryKey, summaryText, type TimelineSection } from '$lib/sections.svelte'
  import { boxOf, type Box } from '$lib/popover'
  import type { SummaryOptions } from '$lib/tauri/types'
  import SummarizePopover from './SummarizePopover.svelte'
  import { requirementBadge } from '$lib/actions'
  import type { SectionKind } from '$lib/tauri/types'
  import { IconButton } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    scope: string
    kind: SectionKind
    section?: TimelineSection | null
    collapsed?: boolean
    ontoggle?: () => void
    inside?: boolean
  }

  let { scope, kind, section = null, collapsed = false, ontoggle, inside = false }: Props = $props()
  let asking = $state<Box | null>(null)

  let copied = $state(false)
  let saved = $state(false)
  let saving = $state(false)
  let failure = $state<string | null>(null)
  let asked = $state(false)

  const timeline = $derived(sections.timeline?.scope === scope ? sections.timeline : null)
  const list = $derived(kind === 'all' ? (timeline?.sections ?? []) : kind === 'selection' ? (sections.selection ? [sections.selection] : []) : section ? [section] : [])
  const start = $derived(list.find((s) => s.messages.length)?.messages[0]?.id ?? null)
  const key = $derived(start ? summaryKey(scope, kind, start) : null)
  const summary = $derived(key ? (sections.summaries[key] ?? null) : null)
  const running = $derived(!!key && !!sections.running[key])
  const failed = $derived(key ? (sections.failures[key] ?? null) : null)
  const label = $derived(kind === 'all' ? t('conversation.section.summaryAll') : kind === 'selection' ? t('conversation.section.summarySelection') : t('conversation.section.summary'))
  const count = $derived(list.reduce((sum, s) => sum + s.messages.length, 0))
  const badge = $derived(asked ? requirementBadge(sections.warned) : null)

  $effect(() => {
    summary?.updated_at
    copied = false
    saved = false
  })

  function regenerate(options: SummaryOptions) {
    asked = true
    if (timeline && sectionInput(kind, list)) postTimelineSummary(timeline, kind, list, true, options)
  }

  function configure() {
    asked = false
    const href = deleteWarning()
    if (href) goto(href)
  }

  async function copy() {
    if (!summary) return
    await navigator.clipboard.writeText(summaryText(summary)).catch(() => {})
    copied = true
    setTimeout(() => (copied = false), 1600)
  }

  async function save() {
    if (!summary || !timeline || saving) return
    saving = true
    failure = null
    try {
      const when = list[0] ? formatDateSeparator(list[0].from) : ''
      await postSummaryContent(timeline, summary, [timeline.title, when].filter(Boolean).join(' · '))
      saved = true
    } catch {
      failure = t('actions.requirement.failed')
    } finally {
      saving = false
    }
  }
</script>

{#if summary || running || failed}
  <aside class="{inside ? 'px-3.5 pb-2.5' : 'my-2 rounded-[0.5rem] bg-paper-2 px-3.5 pb-2.5 pt-1.5'}" aria-label={label} aria-busy={running}>
    <header class="flex h-7 min-w-0 items-center gap-2 text-[12px] text-ink-3">
      <Icon icon="lucide:sparkles" class="shrink-0 text-[13px] text-accent" />
      <span class="shrink-0 font-medium text-ink-2">{label}</span>
      {#if summary}
        <span class="shrink-0 font-mono text-[11px]">{t('conversation.section.messages', { count: summary.message_count })}</span>
      {/if}
      <span class="ml-auto flex shrink-0 items-center gap-0.5">
        {#if badge}
          <IconButton size="sm" testid="summary-needs" tone="warn" icon="lucide:triangle-alert" label={badge.title} onclick={configure} />
        {:else}
          <IconButton size="sm" testid="summary-regenerate" icon="lucide:refresh-cw" label={t('conversation.section.regenerate')} busy={running} active={!!asking} onclick={(e) => (asking = asking ? null : boxOf(e.currentTarget))} />
        {/if}
        {#if summary}
          <IconButton size="sm" icon={copied ? 'lucide:check' : 'lucide:copy'} label={copied ? t('conversation.section.copied') : t('conversation.section.copy')} active={copied} onclick={copy} />
          <IconButton size="sm" icon={saved ? 'lucide:check' : 'lucide:bookmark-plus'} label={saved ? t('conversation.section.saved') : t('conversation.section.save')} active={saved} busy={saving} onclick={save} />
        {/if}
        {#if ontoggle}
          <button type="button" data-testid="summary-toggle" class="ml-1 flex h-6 items-center gap-1 rounded-[0.375rem] px-1.5 text-[12px] text-ink-2 hover:bg-paper-3 hover:text-ink" aria-expanded={!collapsed} onclick={ontoggle}>
            <Icon icon={collapsed ? 'lucide:chevron-down' : 'lucide:chevron-up'} class="text-[12px]" />{collapsed ? t('conversation.section.showMessages', { count }) : t('conversation.section.hideMessages')}
          </button>
        {/if}
      </span>
    </header>
    {#if summary}
      <p class="selectable max-w-[72ch] whitespace-pre-line text-[14px] leading-[22px] text-ink {running ? 'opacity-60' : ''}">{summary.summary}</p>
      {#if summary.action_items.length}
        <ul class="mt-1.5 flex max-w-[72ch] flex-col gap-0.5">
          {#each summary.action_items as item, i (i)}
            <li class="flex gap-2 text-[13.5px] leading-5 text-ink-2"><Icon icon="lucide:circle-dot" class="mt-[0.3rem] shrink-0 text-[10px] text-ink-3" /><span class="selectable min-w-0">{item}</span></li>
          {/each}
        </ul>
      {/if}
    {:else if running}
      <p class="text-[13px] leading-5 text-ink-3">{t('conversation.section.summarizing')}</p>
    {/if}
    {#if failed || failure}
      <p class="mt-1 text-[12.5px] leading-4 text-ink-3">{failed ?? failure}</p>
    {/if}
  </aside>
{/if}

{#if asking}
  <SummarizePopover at={asking} title={label} onsubmit={regenerate} onclose={() => (asking = null)} />
{/if}
