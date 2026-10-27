<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import { startOfWeek } from '$lib/dates'
  import Icon from '@iconify/svelte'
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import { listen } from '@tauri-apps/api/event'
  import { api } from '$lib/tauri'
  import CalendarGrid from '$lib/components/calendar/CalendarGrid.svelte'
  import CreateEventModal from '$lib/components/calendar/CreateEventModal.svelte'
  import EventDetailModal from '$lib/components/calendar/EventDetailModal.svelte'
  import type { CalendarEvent, CalendarView } from '$lib/components/calendar/types'
  import type { PostEventInput, ProviderCalendar } from '$lib/tauri/types'
  import { Button, IconButton, PaneGrid, Segmented, Select, Switch } from '$lib/components/ui'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { connections } from '$lib/calendar'
  import { formatDate, formatNumber, t } from '$lib/i18n.svelte'
  import { calendarCollapsedKey, layoutSettings, flag, toggleFlag } from '$lib/layoutSettings'

  let events = $state<CalendarEvent[]>([])
  let proposed = $state<CalendarEvent[]>([])
  let calendars = $state<ProviderCalendar[]>([])
  const groups = $derived(connections(calendars))
  let selected_date = $state(new Date())
  let current_view = $state<CalendarView>('week')
  let span = $state(1)
  const SPAN_MAX: Partial<Record<CalendarView, number>> = { day: 14, week: 6, month: 6, year: 4 }
  const spanOptions = $derived(Array.from({ length: SPAN_MAX[current_view] ?? 1 }, (_, i) => ({ value: String(i + 1), label: `×${i + 1}` })))
  let scanning = $state(false)
  let scanned = $state<number | null>(null)
  let exported = $state<string | null>(null)
  let failure = $state<string | null>(null)

  let create_modal_visible = $state(false)
  let create_modal_date = $state(new Date())
  let detail_modal_visible = $state(false)
  let detail_event = $state<CalendarEvent | null>(null)

  const VIEW_OPTIONS = $derived([
    { value: 'year', label: t('calendar.view.year') },
    { value: 'quarter', label: t('calendar.view.quarter') },
    { value: 'month', label: t('calendar.view.month') },
    { value: 'week', label: t('calendar.view.week') },
    { value: 'day', label: t('calendar.view.day') },
    { value: 'agenda', label: t('calendar.view.agenda') }
  ])

  $effect(() => {
    current_view
    span = 1
  })

  function windowOf(view: CalendarView, date: Date, count: number): { start: Date; end: Date } {
    const y = date.getFullYear()
    const m = date.getMonth()
    if (view === 'year') return { start: new Date(y, 0, 1), end: new Date(y + count, 0, 1) }
    if (view === 'quarter') return { start: new Date(y, Math.floor(m / 3) * 3, 1), end: new Date(y, Math.floor(m / 3) * 3 + 3, 1) }
    if (view === 'month') return { start: new Date(y, m, 1), end: new Date(y, m + count, 1) }
    if (view === 'week' || view === 'agenda') {
      const start = startOfWeek(date)
      return { start, end: new Date(start.getFullYear(), start.getMonth(), start.getDate() + 7 * (view === 'agenda' ? 1 : count)) }
    }
    const start = new Date(y, m, date.getDate())
    return { start, end: new Date(y, m, date.getDate() + count) }
  }

  let headerLabel = $derived.by(() => {
    const { start, end } = windowOf(current_view, selected_date, span)
    const last = new Date(end.getTime() - 1)
    const cap = (text: string) => text.charAt(0).toUpperCase() + text.slice(1)
    if (current_view === 'year') return span > 1 ? `${start.getFullYear()} – ${last.getFullYear()}` : String(start.getFullYear())
    if (current_view === 'quarter') return `Q${Math.floor(start.getMonth() / 3) + 1} ${start.getFullYear()}`
    if (current_view === 'month') return span > 1 ? `${cap(formatDate(start, { month: 'short', year: 'numeric' }))} – ${cap(formatDate(last, { month: 'short', year: 'numeric' }))}` : cap(formatDate(start, { month: 'long', year: 'numeric' }))
    if (current_view === 'day' && span === 1) return cap(formatDate(start, { weekday: 'long', month: 'long', day: 'numeric', year: 'numeric' }))
    return `${cap(formatDate(start, { day: 'numeric', month: 'short' }))} – ${cap(formatDate(last, { day: 'numeric', month: 'short', year: 'numeric' }))}`
  })

  let range = $derived.by(() => {
    const { start, end } = windowOf(current_view, selected_date, span)
    const lead = current_view === 'month' || current_view === 'quarter' || current_view === 'year' ? 7 : 0
    const from = new Date(start.getFullYear(), start.getMonth(), start.getDate() - lead)
    const to = new Date(end.getFullYear(), end.getMonth(), end.getDate() + lead)
    to.setMilliseconds(-1)
    return { start: from.toISOString(), end: to.toISOString() }
  })

  async function reload() {
    failure = null
    try {
      const [visible, pending, known] = await Promise.all([api.getEvents({ start: range.start, end: range.end }), api.getProposedEvents(), api.getCalendars()])
      events = visible
      proposed = pending
      calendars = known
    } catch (e) {
      failure = String(e)
    }
  }

  $effect(() => {
    range
    reload()
  })

  onMount(() => {
    let dispose: (() => void) | undefined
    listen('events-updated', () => reload()).then((off) => (dispose = off))
    return () => dispose?.()
  })

  function navigateDate(direction: number) {
    const next = new Date(selected_date)
    if (current_view === 'year') next.setFullYear(next.getFullYear() + direction * span)
    else if (current_view === 'quarter') next.setMonth(next.getMonth() + direction * 3)
    else if (current_view === 'month') next.setMonth(next.getMonth() + direction * span)
    else if (current_view === 'week' || current_view === 'agenda') next.setDate(next.getDate() + direction * 7 * (current_view === 'agenda' ? 1 : span))
    else next.setDate(next.getDate() + direction * span)
    selected_date = next
  }

  function zoomTo(view: CalendarView, date: Date) {
    current_view = view
    selected_date = date
  }

  function handleCreate(date: Date) {
    create_modal_date = date
    create_modal_visible = true
  }

  function handleEventClick(event: CalendarEvent) {
    detail_event = event
    detail_modal_visible = true
  }

  async function handleEventCreate(input: PostEventInput) {
    try {
      const result = await api.postEvent(input)
      if (result.failures.length) failure = t('calendar.page.partialCreate', { count: result.events.length, failures: result.failures.join('; ') })
      await reload()
    } catch (e) {
      failure = String(e)
    }
  }

  async function setStatus(event: CalendarEvent, status: 'confirmed' | 'dismissed') {
    detail_modal_visible = false
    try {
      await api.putEvent({ event_id: event.id, status })
      await reload()
    } catch (e) {
      failure = String(e)
    }
  }

  async function toggleCalendar(calendar: ProviderCalendar, enabled: boolean) {
    try {
      await api.putCalendar(calendar.id, enabled)
      await reload()
    } catch (e) {
      failure = String(e)
    }
  }

  async function unmerge(event: CalendarEvent) {
    detail_modal_visible = false
    try {
      await api.putEventMerge(event.sources.map((s) => s.event_id), false)
      await reload()
    } catch (e) {
      failure = String(e)
    }
  }

  async function remove(event: CalendarEvent) {
    detail_modal_visible = false
    try {
      await api.deleteEvent(event.id)
      await reload()
    } catch (e) {
      failure = String(e)
    }
  }

  async function scan() {
    scanning = true
    scanned = null
    try {
      scanned = (await api.postActionBackfill('find_appointments', 50)).runs
      await reload()
    } catch (e) {
      failure = String(e)
    } finally {
      scanning = false
    }
  }

  async function exportIcs() {
    try {
      exported = await api.getCalendarExport()
    } catch (e) {
      failure = String(e)
    }
  }

  function proposedTime(event: CalendarEvent): string {
    const d = new Date(event.starts_at)
    const day = formatDate(d, { weekday: 'short', day: 'numeric', month: 'short' })
    return event.all_day ? day : `${day} ${formatDate(d, { hour: '2-digit', minute: '2-digit' })}`
  }
</script>

<PaneGrid name="calendar" class="bg-paper" panes={[{ key: 'grid' }, { key: 'calendars', size: '22rem', min: '16rem', max: '36vw', edge: 'start' }]}>
  {#snippet pane(key)}
  {#if key === 'grid'}
  <div class="flex min-h-0 min-w-0 flex-1 flex-col">
    <header class="flex h-12 shrink-0 items-center gap-1 border-b border-line px-4 pt-1">
      <h1 data-testid="calendar-title" class="mr-2 text-[15px] font-semibold tracking-tight text-ink">{headerLabel}</h1>
      <IconButton testid="calendar-prev" icon="lucide:chevron-left" label={t('common.previous')} onclick={() => navigateDate(-1)} />
      <IconButton testid="calendar-next" icon="lucide:chevron-right" label={t('common.next')} onclick={() => navigateDate(1)} />
      <Button testid="calendar-today" size="sm" onclick={() => (selected_date = new Date())}>{t('common.today')}</Button>
      <div class="ml-auto flex items-center gap-1">
        <Segmented testid="calendar-view" bind:value={current_view} options={VIEW_OPTIONS} size="sm" label={t('calendar.page.view')} />
        {#if (SPAN_MAX[current_view] ?? 1) > 1}
          <Select testid="calendar-span" size="sm" value={String(span)} options={spanOptions} onchange={(v) => (span = Number(v))} />
        {/if}
        <Button testid="calendar-scan" icon="lucide:scan-search" busy={scanning} onclick={scan}>
          {t('calendar.page.scan')}
          {#if scanned !== null}
            <span data-testid="calendar-scanned" class="font-mono text-[11px] text-accent">{scanned}</span>
          {/if}
        </Button>
        <Button testid="calendar-export" icon="lucide:download" onclick={exportIcs}>{t('calendar.page.export')}</Button>
        <Button testid="calendar-create" variant="primary" icon="lucide:plus" onclick={() => handleCreate(new Date())}>{t('calendar.create.title')}</Button>
      </div>
    </header>

    {#if failure || exported}
      <div data-testid="calendar-banner" data-kind={failure ? 'failure' : 'exported'} class="flex shrink-0 items-center gap-2 border-b border-line px-4 py-1.5 text-[12.5px] {failure ? 'text-error-500' : 'text-ink-3'}">
        {#if failure}
          {failure}
        {:else}
          {t('calendar.page.exportedTo')} <span data-testid="calendar-exported" class="font-mono">{exported}</span>
        {/if}
        <IconButton class="ml-auto" size="sm" icon="lucide:x" label={t('common.dismiss')} onclick={() => { failure = null; exported = null }} />
      </div>
    {/if}

    <div class="min-h-0 flex-1">
      <CalendarGrid {events} view={current_view} {span} {selected_date} oncreate={handleCreate} oneventclick={handleEventClick} onzoom={zoomTo} />
    </div>
  </div>
  {:else}
  <aside class="flex min-h-0 flex-1 flex-col">
    {#if groups.length > 0}
      <div class="flex h-12 shrink-0 items-center gap-2 px-5 pt-1">
        <h2 class="text-[14px] font-semibold text-ink">{t('calendar.page.calendars')}</h2>
        <span class="font-mono text-[11px] text-ink-3">{calendars.filter((c) => c.enabled).length}/{calendars.length}</span>
      </div>
      <div class="max-h-[45%] shrink-0 overflow-y-auto border-b border-line px-5 pb-3">
        {#each groups as group (group.provider_id)}
          {@const folded = flag($layoutSettings, calendarCollapsedKey(group.provider_id))}
          {@const on = group.calendars.filter((c) => c.enabled).length}
          <button type="button" data-testid="calendar-group" data-id={group.provider_id} aria-expanded={!folded} class="mt-1 flex h-8 w-full items-center gap-2 rounded-[0.5rem] px-1 text-left text-[12.5px] text-ink-2 hover:bg-paper-2" onclick={() => toggleFlag(calendarCollapsedKey(group.provider_id))}>
            <Icon icon={folded ? 'lucide:chevron-right' : 'lucide:chevron-down'} class="shrink-0 text-[13px] text-ink-3" />
            <ProviderIcon providers={[group.mark]} conversationType="email" />
            <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap font-medium text-ink">{group.name}</span>
            <span class="flex shrink-0 items-center gap-1">
              {#each group.calendars as calendar (calendar.id)}
                <span class="size-1.5 rounded-full {calendar.enabled ? '' : 'opacity-30'}" style:background={calendar.colour ?? 'var(--line-strong)'}></span>
              {/each}
            </span>
            <span class="shrink-0 font-mono text-[11px] text-ink-3">{on}/{group.calendars.length}</span>
          </button>
          {#each folded ? [] : group.calendars as calendar (calendar.id)}
            <div data-testid="calendar-calendar" data-id={calendar.id} data-enabled={calendar.enabled} class="flex h-8 items-center gap-2.5 pl-6 text-[13.5px] {calendar.enabled ? 'text-ink' : 'text-ink-3'}">
              <span class="size-2 shrink-0 rounded-full" style:background={calendar.colour ?? 'var(--line-strong)'}></span>
              <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{calendar.name}</span>
              {#if !calendar.can_write}<span class="font-mono text-[10px] text-ink-3">{t('calendar.page.readOnly')}</span>{/if}
              <Switch testid="calendar-toggle" checked={calendar.enabled} label={t('calendar.page.showCalendar', { name: calendar.name })} onchange={(on) => toggleCalendar(calendar, on)} />
            </div>
          {/each}
        {/each}
      </div>
    {/if}
    <div class="flex h-12 shrink-0 items-center gap-2 px-5 pt-1">
      <h2 class="text-[14px] font-semibold text-ink">{t('calendar.page.proposed')}</h2>
      {#if proposed.length > 0}
        <span class="font-mono text-[11px] text-ink-3">{proposed.length}</span>
      {/if}
    </div>
    <div class="min-h-0 flex-1 overflow-y-auto px-5 pb-4">
      {#if proposed.length === 0}
        <p class="pt-1 text-[13.5px] leading-5 text-ink-3">{t('calendar.page.nothingProposed')}</p>
      {:else}
        {#each proposed as event (event.id)}
          <div data-testid="calendar-proposed" data-id={event.id} class="border-b border-line py-3 last:border-b-0">
            <button data-testid="calendar-proposed-detail" class="w-full text-left" onclick={() => handleEventClick(event)}>
              <div class="text-[14px] font-medium leading-5 text-ink">{event.title}</div>
              <div class="mt-0.5 font-mono text-[11px] text-ink-3">{proposedTime(event)} · {formatNumber(event.confidence, { style: 'percent' })}</div>
            </button>
            <div class="mt-2 flex items-center gap-1">
              <Button testid="calendar-proposed-confirm" size="sm" variant="secondary" onclick={() => setStatus(event, 'confirmed')}>{t('common.confirm')}</Button>
              <Button testid="calendar-proposed-dismiss" size="sm" onclick={() => setStatus(event, 'dismissed')}>{t('common.dismiss')}</Button>
              {#if event.conversation_id}
                <Button testid="calendar-proposed-open" size="sm" class="ml-auto text-ink-3" onclick={() => goto(conversationHref(event.conversation_id ?? ''))}>{t('common.open')}</Button>
              {/if}
            </div>
          </div>
        {/each}
      {/if}
    </div>
  </aside>
  {/if}
  {/snippet}
</PaneGrid>

<CreateEventModal initial_date={create_modal_date} visible={create_modal_visible} {calendars} onclose={() => (create_modal_visible = false)} oncreate={handleEventCreate} />

<EventDetailModal event={detail_event} visible={detail_modal_visible} onclose={() => (detail_modal_visible = false)} onstatus={setStatus} ondelete={remove} onunmerge={unmerge} />
