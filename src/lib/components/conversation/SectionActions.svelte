<script lang="ts">
  import { goto } from '$app/navigation'
  import { boxOf } from '$lib/popover'
  import { layoutSettings } from '$lib/layoutSettings'
  import { allSectionsCollapsed, chooseSummary, deleteWarning, sections, summarizeAll, toggleAllSections } from '$lib/sections.svelte'
  import { requirementBadge } from '$lib/actions'
  import { Button, IconButton, Segmented } from '$lib/components/ui'
  import SummarizePopover from './messages/SummarizePopover.svelte'
  import type { SummaryChoice } from '$lib/sections.svelte'
  import { t } from '$lib/i18n.svelte'

  let { labelled = false }: { labelled?: boolean } = $props()

  let anchor = $state<HTMLDivElement>()
  let choice = $state<SummaryChoice>('all')

  const timeline = $derived(sections.timeline)
  const ids = $derived(timeline?.sections.map((s) => s.id) ?? [])
  const folded = $derived(!!timeline && allSectionsCollapsed($layoutSettings, timeline.scope, ids))
  const busy = $derived(!!timeline && Object.keys(sections.running).some((key) => key.startsWith(`${timeline.scope}|`)))
  const badge = $derived(requirementBadge(sections.warned))

  function configure() {
    const href = deleteWarning()
    if (href) goto(href)
  }
</script>

{#if timeline && ids.length}
  {#if labelled}
    <Button testid="sections-toggle-all" size="sm" icon={folded ? 'lucide:chevrons-up-down' : 'lucide:chevrons-down-up'} keys="Z" onclick={() => toggleAllSections()}>{folded ? t('conversation.section.expandAll') : t('conversation.section.collapseAll')}</Button>
  {:else}
    <IconButton testid="sections-toggle-all" icon={folded ? 'lucide:chevrons-up-down' : 'lucide:chevrons-down-up'} label={folded ? t('conversation.section.expandAll') : t('conversation.section.collapseAll')} keys="Z" active={folded} onclick={() => toggleAllSections()} />
  {/if}
  <div class="relative" bind:this={anchor}>
    {#if badge}
      <IconButton testid="sections-needs" tone="warn" icon="lucide:triangle-alert" label={badge.title} onclick={configure} />
    {:else}
      {#if labelled}
        <Button testid="sections-summarize" size="sm" icon="lucide:sparkles" keys="⇧Z" {busy} onclick={chooseSummary}>{t('conversation.section.summarizeAll')}</Button>
      {:else}
        <IconButton testid="sections-summarize" icon="lucide:sparkles" label={t('conversation.section.summarizeAll')} keys="⇧Z" active={sections.choosing} {busy} onclick={chooseSummary} />
      {/if}
    {/if}
    {#if sections.choosing && !badge}
      <SummarizePopover at={() => boxOf(anchor ?? null)} title={t('conversation.section.summarizeAll')} onsubmit={(options) => summarizeAll(choice, choice === 'all', options)} onclose={() => (sections.choosing = false)}>
        <Segmented size="sm" stretch testid="summary-choice" label={t('conversation.section.summarizeAll')} value={choice} options={[{ value: 'all', label: t('conversation.section.everything'), icon: 'lucide:text-quote' }, { value: 'each', label: t('conversation.section.each'), icon: 'lucide:rows-3' }]} onchange={(v) => (choice = v as SummaryChoice)} />
        <p class="text-[12px] leading-4 text-ink-3">{choice === 'all' ? t('conversation.section.everythingDetail') : t('conversation.section.eachDetail')}</p>
      </SummarizePopover>
    {/if}
  </div>
{/if}
