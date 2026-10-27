<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '$lib/tauri'
  import type { CalendarEvent } from '$lib/tauri/types'
  import { Button, IconButton, Section } from '$lib/components/ui'
  import { formatDate, t } from '$lib/i18n.svelte'

  let proposed = $state<CalendarEvent[]>([])
  let exported = $state<string | null>(null)
  let exporting = $state(false)
  let failure = $state<string | null>(null)

  onMount(load)

  async function load() {
    try {
      proposed = await api.getProposedEvents()
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  async function exportIcs() {
    exporting = true
    try {
      exported = await api.getCalendarExport()
      failure = null
    } catch (error) {
      failure = String(error)
    } finally {
      exporting = false
    }
  }

  async function decide(event: CalendarEvent, status: 'confirmed' | 'dismissed') {
    try {
      await api.putEvent({ event_id: event.id, status })
      await load()
    } catch (error) {
      failure = String(error)
    }
  }

  function span(event: CalendarEvent): string {
    const start = new Date(event.starts_at)
    const end = new Date(event.ends_at)
    const day = formatDate(start, { weekday: 'short', day: 'numeric', month: 'short' })
    if (event.all_day) return day
    const time = (d: Date) => formatDate(d, { hour: '2-digit', minute: '2-digit' })
    return `${day} ${time(start)}–${time(end)}`
  }
</script>

<div>
  <Section id="calendar.export" title={t('settings.tab.calendar')} description={t('settings.calendar.description')}>
    {#snippet actions()}
      <Button variant="primary" icon="lucide:download" busy={exporting} onclick={exportIcs}>{t('settings.calendar.export')}</Button>
    {/snippet}
    {#if failure}
      <p class="mb-2 text-[13px] text-error-500">{failure}</p>
    {/if}
    {#if exported}
      <p class="truncate font-mono text-[12px] text-ink-2 selectable" title={exported}>{exported}</p>
    {/if}
  </Section>

  <Section id="calendar.proposed" title={t('settings.calendar.proposed')} description={t('settings.calendar.proposedDescription')} total={proposed.length}>
    {#snippet children(shown)}
    <ul>
      {#each proposed.slice(0, shown) as event (event.id)}
        <li class="group grid h-10 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-line last:border-b-0">
          <div class="flex min-w-0 items-baseline gap-2">
            <span class="truncate text-[14px] font-medium text-ink">{event.title}</span>
            <span class="shrink-0 font-mono text-[11px] text-ink-3">{span(event)}</span>
            {#if event.location}
              <span class="truncate text-[13px] text-ink-3">{event.location}</span>
            {/if}
            <span class="shrink-0 font-mono text-[11px] text-ink-3">{Math.round(event.confidence * 100)}%</span>
          </div>
          <div class="flex items-center gap-0.5">
            <IconButton size="sm" icon="lucide:check" label={t('common.confirm')} onclick={() => decide(event, 'confirmed')} />
            <IconButton size="sm" icon="lucide:x" label={t('common.dismiss')} tone="danger" onclick={() => decide(event, 'dismissed')} />
          </div>
        </li>
      {:else}
        <li class="py-4 text-[13px] text-ink-3">{t('settings.calendar.nothing')}</li>
      {/each}
    </ul>
    {/snippet}
  </Section>
</div>
