<script lang="ts">
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { formatDateSeparator, formatTime } from '$lib/dates'
  import { layoutSettings } from '$lib/layoutSettings'
  import { collapsedSections, deleteWarning, postTimelineSummary, sections, summarisable, summaryKey, toggleSection, type TimelineSection } from '$lib/sections.svelte'
  import { anchored, boxOf, type Box } from '$lib/popover'
  import type { SummaryOptions } from '$lib/tauri/types'
  import SummarizePopover from './SummarizePopover.svelte'
  import { requirementBadge } from '$lib/actions'
  import { displayName, myUserIds, isMine, peopleStore } from '$lib/tauri/people'
  import { userAvatar, type AvatarSource } from '$lib/avatar'
  import { ownAuthor } from '$lib/accounts'
  import { providersStore } from '$lib/tauri/providers'
  import { Avatar, IconButton } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'
  import SectionSummaryCard from './SectionSummaryCard.svelte'

  interface Props {
    section: TimelineSection
    scope: string
    headed?: boolean
  }

  let { section, scope, headed = true }: Props = $props()

  const collapsed = $derived(headed && collapsedSections($layoutSettings, scope).has(section.id))
  const key = $derived(section.messages[0] ? summaryKey(scope, 'section', section.messages[0].id) : null)
  const summary = $derived(key ? (sections.summaries[key] ?? null) : null)
  const running = $derived(!!key && !!sections.running[key])
  let asked = $state(false)
  let asking = $state<Box | null>(null)
  const enough = $derived(summarisable(section.messages))
  const badge = $derived(asked ? requirementBadge(sections.warned) : null)
  const unread = $derived(section.messages.filter((m) => !m.is_read && !isMine($myUserIds, m.sender_id)).length)
  const people = $derived.by(() => {
    const seen = new Map<string, { id: string; name: string; face: AvatarSource; mine: boolean }>()
    for (const m of section.messages) {
      if (seen.has(m.sender_id)) continue
      const mine = isMine($myUserIds, m.sender_id)
      const own = mine ? ownAuthor(m, $providersStore.data, t('conversation.group.you')) : null
      const name = own ? own.name : displayName($peopleStore, null, m.sender_id, m.sender_name || m.sender_email || t('common.unknown'))
      const face: AvatarSource = own ? own.face : userAvatar({ id: m.sender_id, display_name: name, email: m.sender_email, avatar_url: null }, $peopleStore, name)
      seen.set(m.sender_id, { id: m.sender_id, name, mine, face })
    }
    return [...seen.values()]
  })
  const span = $derived.by(() => {
    const from = formatTime(section.from)
    const to = formatTime(section.to)
    if (new Date(section.from).toDateString() !== new Date(section.to).toDateString()) return `${from}–${formatDateSeparator(section.to)} ${to}`
    return from === to ? from : `${from}–${to}`
  })

  function toggle() {
    if (headed) toggleSection(scope, section.id)
  }

  function summarize(options: SummaryOptions) {
    const timeline = sections.timeline
    asked = true
    if (timeline?.scope === scope) postTimelineSummary(timeline, 'section', [section], false, options)
  }

  let small = $state<Box | null>(null)
  let smallTimer: ReturnType<typeof setTimeout> | null = null

  function ask(button: EventTarget | null) {
    if (sections.requirements.length) {
      asked = true
      sections.warned = sections.requirements
      return
    }
    if (!enough) {
      small = boxOf(button)
      if (smallTimer) clearTimeout(smallTimer)
      smallTimer = setTimeout(() => (small = null), 2600)
      return
    }
    asking = asking ? null : boxOf(button)
  }

  function configure() {
    asked = false
    const href = deleteWarning()
    if (href) goto(href)
  }
</script>

{#snippet action()}
  {#if section.messages.length}
    {#if badge}
      <IconButton testid="section-needs" size="sm" tone="warn" icon="lucide:triangle-alert" label={badge.title} onclick={configure} />
    {:else}
      <IconButton testid="section-summarize" size="sm" icon="lucide:sparkles" label={t('conversation.section.summarize')} busy={running} active={!!asking} class="{running || asking ? 'opacity-100' : 'opacity-0'} group-hover/section:opacity-100 focus:opacity-100" onclick={(e) => ask(e.currentTarget)} />
    {/if}
  {/if}
  {#if small}
    <p class="menu max-w-[16rem] text-[12.5px] leading-5 text-ink-2" role="status" data-testid="section-too-small" use:anchored={{ at: small, align: 'end', onclose: () => (small = null) }}>{t('conversation.section.tooSmall')}</p>
  {/if}
  {#if asking}
    <SummarizePopover at={asking} title={t('conversation.summarize.section')} onsubmit={summarize} onclose={() => (asking = null)} />
  {/if}
{/snippet}

{#if !headed}
  <SectionSummaryCard {scope} kind="section" {section} />
{:else if collapsed}
  <div class="my-1 rounded-[0.5rem] bg-paper-2">
  <div data-testid="section" data-id={section.id} data-collapsed="true" class="group/section flex h-9 min-w-0 items-center gap-1 rounded-[0.5rem] pr-1 hover:bg-paper-3">
    <button type="button" data-testid="section-toggle" class="grid h-full min-w-0 flex-1 grid-cols-[1.5rem_auto_auto_auto_minmax(0,1fr)_auto] items-center gap-x-2.5 rounded-[0.5rem] pl-1.5 text-left" aria-expanded="false" title={t('conversation.section.expand')} onclick={toggle}>
      <span class="flex size-5 items-center justify-center rounded-[0.3rem] bg-paper-3 text-ink-2"><Icon icon="lucide:chevron-right" class="text-[13px]" /></span>
      <span class="whitespace-nowrap text-[13px] font-medium text-ink">{formatDateSeparator(section.from)}</span>
      <span class="whitespace-nowrap font-mono text-[11px] text-ink-3">{span}</span>
      <span class="flex items-center -space-x-1">
        {#each people.slice(0, 4) as person (person.id)}
          <Avatar source={person.face} size="xs" ring hint={person.name} />
        {/each}
      </span>
      <span class="flex min-w-0 items-center gap-1.5 text-[13px] text-ink-3">
        <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{people.map((p) => p.name).join(', ')}</span>
      </span>
      <span class="flex items-center gap-1.5 whitespace-nowrap font-mono text-[11px] text-ink-3">
        {t('conversation.section.messages', { count: section.messages.length })}
        {#if unread}
          <span class="rounded-[0.3rem] bg-accent-soft px-1.5 text-accent" title={t('conversation.section.unread', { count: unread })}>{unread}</span>
        {/if}
      </span>
    </button>
    {@render action()}
  </div>
  <SectionSummaryCard {scope} kind="section" {section} collapsed inside />
  </div>
{:else}
  <div data-testid="section" data-id={section.id} data-collapsed="false" class="group/section flex min-w-0 items-center gap-1 py-2">
    <button type="button" data-testid="section-toggle" class="flex h-7 min-w-0 flex-1 items-center gap-3 text-ink-3 hover:text-ink" aria-expanded="true" title={t('conversation.section.collapse')} onclick={toggle}>
      <span class="h-px flex-1 bg-line"></span>
      <span class="flex items-center gap-1.5 whitespace-nowrap text-[13px]"><span class="flex size-5 items-center justify-center rounded-[0.3rem] text-ink-3 group-hover/section:bg-paper-3"><Icon icon="lucide:chevron-down" class="text-[13px]" /></span>{formatDateSeparator(section.from)}</span>
      <span class="h-px flex-1 bg-line"></span>
    </button>
    {@render action()}
  </div>
  <SectionSummaryCard {scope} kind="section" {section} collapsed={false} ontoggle={toggle} />
{/if}
