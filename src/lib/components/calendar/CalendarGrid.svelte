<script lang="ts">
  import { startOfWeek } from '$lib/dates'
  import { onMount } from 'svelte'
  import type { CalendarEvent as CalendarEventType, CalendarView } from './types'
  import CalendarEventItem from './CalendarEvent.svelte'
  import { formatDate, t } from '$lib/i18n.svelte'

  interface Props {
    events: CalendarEventType[]
    view: CalendarView
    selected_date: Date
    span?: number
    oncreate: (date: Date) => void
    oneventclick: (event: CalendarEventType) => void
    onzoom?: (view: CalendarView, date: Date) => void
  }

  interface Unit {
    label: string
    start: Date
    end: Date
    zoom: CalendarView
  }

  interface Placed {
    event: CalendarEventType
    dayIndex: number
    topPct: number
    heightPct: number
    leftPct: number
    widthPct: number
    part: 'single' | 'first' | 'middle' | 'last'
  }

  let { events, view, selected_date, span = 1, oncreate, oneventclick, onzoom }: Props = $props()

  const HOURS = Array.from({ length: 24 }, (_, i) => i)
  const LADDER: CalendarView[] = ['year', 'quarter', 'month', 'week', 'day', 'agenda']
  const DAY_MS = 24 * 60 * 60 * 1000
  const CHIP_REM = 1.45

  let scroller = $state<HTMLDivElement | null>(null)
  let firstCell = $state<HTMLDivElement | null>(null)
  let monthFit = $state(3)
  let now = $state(new Date())

  const dayNames = $derived(Array.from({ length: 7 }, (_, i) => formatDate(new Date(2024, 0, 1 + i), { weekday: 'short' })))

  function sameDay(a: Date, b: Date): boolean {
    return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
  }

  function dayRun(from: Date, count: number): Date[] {
    return Array.from({ length: count }, (_, i) => new Date(from.getFullYear(), from.getMonth(), from.getDate() + i))
  }

  const weekDays = $derived(dayRun(startOfWeek(selected_date), 7 * span))
  const dayDays = $derived(dayRun(new Date(selected_date.getFullYear(), selected_date.getMonth(), selected_date.getDate()), span))
  const columns = $derived(view === 'day' ? dayDays : view === 'agenda' ? dayRun(startOfWeek(selected_date), 7) : weekDays)

  function isoWeek(d: Date): number {
    const date = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()))
    const day = date.getUTCDay() || 7
    date.setUTCDate(date.getUTCDate() + 4 - day)
    const yearStart = new Date(Date.UTC(date.getUTCFullYear(), 0, 1))
    return Math.ceil(((date.getTime() - yearStart.getTime()) / DAY_MS + 1) / 7)
  }

  function between(start: Date, end: Date): CalendarEventType[] {
    return events.filter((event) => new Date(event.starts_at).getTime() < end.getTime() && new Date(event.ends_at).getTime() > start.getTime()).sort((a, b) => new Date(a.starts_at).getTime() - new Date(b.starts_at).getTime())
  }

  function hoursIn(start: Date, end: Date): number {
    const total = between(start, end).reduce((sum, event) => sum + Math.max(0, Math.min(new Date(event.ends_at).getTime(), end.getTime()) - Math.max(new Date(event.starts_at).getTime(), start.getTime())), 0)
    return Math.round(total / 360000) / 10
  }

  const units = $derived.by((): Unit[] => {
    const y = selected_date.getFullYear()
    if (view === 'year' && span > 1) {
      return Array.from({ length: span * 4 }, (_, i) => {
        const start = new Date(y, i * 3, 1)
        return { label: `Q${(i % 4) + 1} ${start.getFullYear()}`, start, end: new Date(y, i * 3 + 3, 1), zoom: 'quarter' as CalendarView }
      })
    }
    if (view === 'year') {
      return Array.from({ length: 12 }, (_, i) => {
        const start = new Date(y, i, 1)
        const label = formatDate(start, { month: 'long' })
        return { label: label.charAt(0).toUpperCase() + label.slice(1), start, end: new Date(y, i + 1, 1), zoom: 'month' as CalendarView }
      })
    }
    if (view === 'quarter') {
      const first = new Date(y, Math.floor(selected_date.getMonth() / 3) * 3, 1)
      const end = new Date(y, first.getMonth() + 3, 1)
      const out: Unit[] = []
      for (let start = startOfWeek(first); start < end; start = new Date(start.getFullYear(), start.getMonth(), start.getDate() + 7)) {
        out.push({ label: t('calendar.page.weekNumber', { week: isoWeek(start) }), start, end: new Date(start.getFullYear(), start.getMonth(), start.getDate() + 7), zoom: 'week' })
      }
      return out
    }
    return []
  })

  const monthWeeks = $derived.by(() => {
    const first = new Date(selected_date.getFullYear(), selected_date.getMonth(), 1)
    const last = new Date(selected_date.getFullYear(), selected_date.getMonth() + span, 0)
    const start = startOfWeek(first)
    const end = startOfWeek(last)
    const weeks = Math.round((end.getTime() - start.getTime()) / (7 * DAY_MS)) + 1
    return Array.from({ length: weeks }, (_, w) => dayRun(new Date(start.getFullYear(), start.getMonth(), start.getDate() + w * 7), 7))
  })

  const timed = $derived(events.filter((event) => !event.all_day))
  const allDay = $derived(events.filter((event) => event.all_day))

  function allDayOn(day: Date): CalendarEventType[] {
    const start = day.getTime()
    const end = start + DAY_MS
    return allDay.filter((event) => new Date(event.starts_at).getTime() < end && new Date(event.ends_at).getTime() > start)
  }

  function eventsOn(day: Date): CalendarEventType[] {
    const start = day.getTime()
    const end = start + DAY_MS
    return events.filter((event) => new Date(event.starts_at).getTime() < end && new Date(event.ends_at).getTime() > start).sort((a, b) => new Date(a.starts_at).getTime() - new Date(b.starts_at).getTime())
  }

  function place(days: Date[], list: CalendarEventType[]): Placed[] {
    const out: Placed[] = []
    const byDay = new Map<number, CalendarEventType[]>()
    for (const event of list) {
      const startMs = new Date(event.starts_at).getTime()
      const endMs = Math.max(new Date(event.ends_at).getTime(), startMs + 30 * 60 * 1000)
      let startIdx = days.findIndex((d) => sameDay(d, new Date(startMs)))
      let endIdx = days.findIndex((d) => sameDay(d, new Date(endMs - 1)))
      if (startIdx < 0 && endIdx < 0) {
        const firstMs = days[0]?.getTime() ?? 0
        const lastMs = (days[days.length - 1]?.getTime() ?? firstMs) + DAY_MS
        if (endMs <= firstMs || startMs >= lastMs) continue
        startIdx = 0
        endIdx = days.length - 1
      } else if (startIdx < 0) startIdx = 0
      else if (endIdx < 0) endIdx = days.length - 1
      for (let i = startIdx; i <= endIdx; i += 1) byDay.set(i, [...(byDay.get(i) ?? []), event])
    }
    for (const [dayIdx, list] of byDay) {
      const dayStart = days[dayIdx].getTime()
      const dayEnd = dayStart + DAY_MS
      const bounds = (event: CalendarEventType) => {
        const s = new Date(event.starts_at).getTime()
        return { s, e: Math.max(new Date(event.ends_at).getTime(), s + 30 * 60 * 1000) }
      }
      list.sort((a, b) => bounds(a).s - bounds(b).s || bounds(b).e - bounds(a).e)
      const finalize = (cluster: { event: CalendarEventType; col: number }[]) => {
        const count = cluster.reduce((m, s) => Math.max(m, s.col + 1), 0)
        for (const slot of cluster) {
          const { s, e } = bounds(slot.event)
          const segStart = Math.max(s, dayStart)
          const segEnd = Math.min(e, dayEnd)
          const topPct = ((segStart - dayStart) / DAY_MS) * 100
          const heightPct = Math.min(100 - topPct, ((segEnd - segStart) / DAY_MS) * 100)
          if (heightPct <= 0) continue
          const startsHere = s >= dayStart
          const endsHere = e <= dayEnd
          out.push({ event: slot.event, dayIndex: dayIdx, topPct, heightPct, leftPct: (slot.col / count) * 100, widthPct: 100 / count, part: startsHere ? (endsHere ? 'single' : 'first') : endsHere ? 'last' : 'middle' })
        }
      }
      let cluster: { event: CalendarEventType; col: number }[] = []
      let clusterEnd = -Infinity
      let columnEnds: number[] = []
      for (const event of list) {
        const { s, e } = bounds(event)
        if (s >= clusterEnd) {
          if (cluster.length) finalize(cluster)
          cluster = []
          columnEnds = []
          clusterEnd = -Infinity
        }
        let col = columnEnds.findIndex((end) => end <= s)
        if (col === -1) {
          col = columnEnds.length
          columnEnds.push(e)
        } else columnEnds[col] = e
        cluster.push({ event, col })
        clusterEnd = Math.max(clusterEnd, e)
      }
      if (cluster.length) finalize(cluster)
    }
    return out
  }

  const placed = $derived(view === 'month' ? [] : place(columns, timed))
  const nowPct = $derived(((now.getHours() * 60 + now.getMinutes()) / (24 * 60)) * 100)

  function formatHour(hour: number): string {
    return `${String(hour).padStart(2, '0')}:00`
  }

  function createAt(day: Date, e: MouseEvent) {
    const slot = (e.target as HTMLElement).closest<HTMLElement>('[data-testid="calendar-slot"]')
    if (!slot) return
    const target = new Date(day)
    target.setHours(Number(slot.dataset.hour), 0, 0, 0)
    oncreate(target)
  }

  function anchorAt(target: EventTarget | null): Date {
    const cell = target instanceof Element ? target.closest<HTMLElement>('[data-date]') : null
    const stamp = cell?.dataset.date
    return stamp ? new Date(stamp) : new Date(selected_date)
  }

  function stepZoom(direction: number, anchor: Date) {
    const at = LADDER.indexOf(view)
    const next = LADDER[Math.min(LADDER.length - 1, Math.max(0, at + direction))]
    if (next !== view) onzoom?.(next, anchor)
  }

  function pinchZoom(node: HTMLElement) {
    let gathered = 0
    let settle: ReturnType<typeof setTimeout> | null = null
    let lastScale = 1
    const reset = () => {
      if (settle) clearTimeout(settle)
      settle = setTimeout(() => (gathered = 0), 300)
    }
    const wheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return
      e.preventDefault()
      gathered += e.deltaY
      reset()
      if (Math.abs(gathered) < 40) return
      const direction = gathered > 0 ? -1 : 1
      gathered = 0
      stepZoom(direction, anchorAt(e.target))
    }
    const gestureStart = (e: Event) => {
      e.preventDefault()
      lastScale = 1
    }
    const gestureChange = (e: Event) => {
      e.preventDefault()
      const scale = (e as Event & { scale?: number }).scale ?? 1
      if (scale > lastScale * 1.25) {
        lastScale = scale
        stepZoom(1, anchorAt(e.target))
      } else if (scale < lastScale / 1.25) {
        lastScale = scale
        stepZoom(-1, anchorAt(e.target))
      }
    }
    node.addEventListener('wheel', wheel, { passive: false })
    node.addEventListener('gesturestart', gestureStart, { passive: false })
    node.addEventListener('gesturechange', gestureChange, { passive: false })
    return {
      destroy() {
        if (settle) clearTimeout(settle)
        node.removeEventListener('wheel', wheel)
        node.removeEventListener('gesturestart', gestureStart)
        node.removeEventListener('gesturechange', gestureChange)
      }
    }
  }

  function measureFit() {
    if (!firstCell) return
    const rem = parseFloat(getComputedStyle(document.documentElement).fontSize)
    const date = firstCell.querySelector<HTMLElement>('.month-date')
    const available = firstCell.clientHeight - (date?.offsetHeight ?? 0) - 0.9 * rem
    const next = Math.max(1, Math.floor(available / ((CHIP_REM + 0.25) * rem)))
    if (next !== monthFit) monthFit = next
  }

  $effect(() => {
    view
    const target = scroller
    if (!target || view === 'month') return
    const rem = parseFloat(getComputedStyle(document.documentElement).fontSize)
    target.scrollTop = Math.max(0, (now.getHours() - 1) * 4 * rem)
  })

  onMount(() => {
    const tick = setInterval(() => (now = new Date()), 60000)
    const fit = () => requestAnimationFrame(measureFit)
    window.addEventListener('resize', fit)
    return () => {
      clearInterval(tick)
      window.removeEventListener('resize', fit)
    }
  })

  $effect(() => {
    if (firstCell) requestAnimationFrame(measureFit)
  })
</script>

<div data-testid="calendar-grid" data-view={view} class="cal" style:--week-cols={columns.length}>
  <div class="cal-inner" bind:this={scroller} use:pinchZoom>
    {#if view === 'month'}
      <div class="month-grid">
        {#each dayNames as name (name)}
          <div class="month-dow">{name}</div>
        {/each}
        {#each monthWeeks as week, w (w)}
          {#each week as day, d (day.getTime())}
            {@const list = eventsOn(day)}
            {@const inMonth = day >= new Date(selected_date.getFullYear(), selected_date.getMonth(), 1) && day < new Date(selected_date.getFullYear(), selected_date.getMonth() + span, 1)}
            <div
              data-testid="calendar-day"
              data-date={day.toDateString()}
              role="button"
              tabindex="0"
              class="month-cell {inMonth ? '' : 'is-other-month'} {sameDay(day, now) ? 'is-today' : ''}"
              bind:this={() => null, (el) => { if (w === 0 && d === 0) firstCell = el }}
              onclick={(e) => { if (!(e.target as HTMLElement).closest('[data-testid="calendar-event"]')) oncreate(new Date(day)) }}
              onkeydown={(e) => e.key === 'Enter' && oncreate(new Date(day))}
            >
              <span class="month-date">{day.getDate()}</span>
              {#each list.slice(0, monthFit) as event (event.id)}
                <div class="month-chip"><CalendarEventItem {event} compact onclick={() => oneventclick(event)} /></div>
              {/each}
              {#if list.length > monthFit}
                <span class="month-more">+{list.length - monthFit}</span>
              {/if}
            </div>
          {/each}
        {/each}
      </div>
    {:else if view === 'year' || view === 'quarter'}
      <div class="unit-grid" data-testid="calendar-units">
        {#each units as unit (unit.start.getTime())}
          {@const list = between(unit.start, unit.end)}
          {@const hours = hoursIn(unit.start, unit.end)}
          <div data-testid="calendar-unit" role="button" tabindex="0" class="month-cell unit-cell {now >= unit.start && now < unit.end ? 'is-today' : ''}" data-date={unit.start.toDateString()} onclick={() => onzoom?.(unit.zoom, unit.start)} onkeydown={(e) => e.key === 'Enter' && onzoom?.(unit.zoom, unit.start)}>
            <span class="unit-head"><span class="month-date">{unit.label}</span>{#if hours}<span class="unit-total">{t('calendar.page.hours', { count: hours })}</span>{/if}</span>
            {#each list.slice(0, monthFit) as event (event.id)}
              <div class="month-chip"><CalendarEventItem {event} compact onclick={() => oneventclick(event)} /></div>
            {/each}
            {#if list.length > monthFit}
              <span class="month-more">+{list.length - monthFit}</span>
            {/if}
          </div>
        {/each}
      </div>
    {:else if view === 'agenda'}
      <div class="agenda" data-testid="calendar-agenda">
        {#each columns as day (day.getTime())}
          {@const list = eventsOn(day)}
          {#if list.length}
            <h2 class="agenda-day {sameDay(day, now) ? 'is-today' : ''}">{dayNames[(day.getDay() + 6) % 7]} {day.getDate()}</h2>
            <ol class="agenda-list">
              {#each list as event (event.id)}
                <li class="agenda-item"><CalendarEventItem {event} compact={false} onclick={() => oneventclick(event)} /></li>
              {/each}
            </ol>
          {/if}
        {/each}
        {#if !columns.some((day) => eventsOn(day).length)}
          <p class="agenda-empty">{t('calendar.page.nothingInRange')}</p>
        {/if}
      </div>
    {:else}
      <div class="week">
        <div class="week-head">
          <div class="time-col"></div>
          {#each columns as day (day.getTime())}
            <div class="day-head {sameDay(day, now) ? 'is-today' : ''}">{dayNames[(day.getDay() + 6) % 7]} {day.getDate()}</div>
          {/each}
        </div>
        {#if columns.some((day) => allDayOn(day).length)}
          <div class="week-allday">
            <div class="time-col allday-label">{t('calendar.allDay')}</div>
            {#each columns as day (day.getTime())}
              <div class="allday-cell">
                {#each allDayOn(day) as event (event.id)}
                  <div class="month-chip"><CalendarEventItem {event} compact onclick={() => oneventclick(event)} /></div>
                {/each}
              </div>
            {/each}
          </div>
        {/if}
        <div class="week-body">
          <div class="time-col">
            {#each HOURS as hour (hour)}
              <div class="hour-label {hour === 0 ? 'is-first' : ''}">{hour === 0 ? '' : formatHour(hour)}</div>
            {/each}
          </div>
          {#each columns as day, dayIndex (day.getTime())}
            <div
              class="week-day {sameDay(day, now) ? 'is-today' : ''}"
              data-date={day.toDateString()}
              role="button"
              tabindex="0"
              aria-label={t('calendar.grid.newEventAt', { day: dayNames[(day.getDay() + 6) % 7], time: '' })}
              onclick={(e) => createAt(day, e)}
              onkeydown={(e) => e.key === 'Enter' && oncreate(new Date(day))}
            >
              {#each HOURS as hour (hour)}
                <div class="hour-row" data-testid="calendar-slot" data-day={dayIndex} data-hour={hour}></div>
              {/each}
              {#each placed.filter((p) => p.dayIndex === dayIndex) as p (p.event.id + ':' + p.dayIndex)}
                <div class="block part-{p.part}" style:top="{p.topPct}%" style:height="{p.heightPct}%" style:left="{p.leftPct}%" style:width="calc({p.widthPct}% - 4px)">
                  <CalendarEventItem event={p.event} compact={false} part={p.part} onclick={() => oneventclick(p.event)} />
                </div>
              {/each}
              {#if sameDay(day, now)}
                <div class="now-line" style:top="{nowPct}%"><span class="now-dot"></span></div>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .cal {
    --hour-min: 4rem;
    --cell-min-h: 6rem;
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    color: var(--ink);
  }
  .cal-inner {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }
  .week {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-height: 100%;
  }
  .week-head,
  .week-allday,
  .week-body {
    display: grid;
    grid-template-columns: 4rem repeat(var(--week-cols, 7), minmax(0, 1fr));
  }
  .week-head {
    position: sticky;
    top: 0;
    z-index: 5;
    background: var(--paper);
    border-bottom: 1px solid var(--line-strong);
  }
  .day-head {
    padding: 0.5rem;
    border-left: 1px solid var(--line);
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--ink-2);
    text-align: center;
    user-select: none;
    font-variant-numeric: tabular-nums;
  }
  .day-head.is-today {
    color: var(--accent);
    box-shadow: inset 0 -2px 0 0 var(--accent);
  }
  .week-allday {
    position: sticky;
    top: 2.1rem;
    z-index: 4;
    background: var(--paper);
    border-bottom: 1px solid var(--line);
  }
  .allday-label {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding-right: 0.55rem;
    font-size: 0.6875rem;
    color: var(--ink-3);
  }
  .allday-cell {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    padding: 0.25rem;
    border-left: 1px solid var(--line);
  }
  .week-body {
    grid-template-rows: 1fr;
    flex: 1 0 auto;
    min-height: calc(24 * var(--hour-min));
    isolation: isolate;
  }
  .time-col {
    grid-column: 1;
    border-right: 1px solid var(--line-strong);
    min-height: 0;
  }
  .week-body .time-col {
    display: grid;
    grid-template-rows: repeat(24, minmax(var(--hour-min), 1fr));
  }
  .hour-label {
    position: relative;
    display: flex;
    align-items: flex-start;
    justify-content: flex-end;
    padding-right: 0.55rem;
    margin-top: -0.5em;
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    font-variant-numeric: tabular-nums;
    line-height: 1;
    color: var(--ink-3);
  }
  .hour-label::after {
    content: '';
    position: absolute;
    right: -1px;
    top: calc(0.5em - 0.5px);
    width: 6px;
    height: 1px;
    background: var(--line-strong);
  }
  .hour-label.is-first::after {
    display: none;
  }
  .week-day {
    position: relative;
    display: grid;
    grid-template-rows: repeat(24, minmax(var(--hour-min), 1fr));
    border-left: 1px solid var(--line);
    min-height: 0;
    cursor: cell;
  }
  .week-day:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .week-day.is-today {
    background: color-mix(in srgb, var(--accent) 4%, transparent);
  }
  .hour-row {
    position: relative;
    border-top: 1px solid var(--line);
    pointer-events: none;
  }
  .hour-row::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    border-top: 1px dashed var(--line);
    opacity: 0.55;
  }
  .hour-row:first-child {
    border-top-color: var(--line-strong);
  }
  .block {
    position: absolute;
    z-index: 4;
    margin-left: 2px;
    min-height: 1.25rem;
    container-type: size;
  }
  .now-line {
    position: absolute;
    left: 0;
    right: 0;
    z-index: 6;
    height: 1px;
    background: var(--signal);
    pointer-events: none;
  }
  .now-dot {
    position: absolute;
    left: -0.25rem;
    top: -3px;
    width: 7px;
    height: 7px;
    border-radius: 9999px;
    background: var(--signal);
  }
  .month-grid {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    grid-template-rows: auto;
    grid-auto-rows: minmax(var(--cell-min-h), 1fr);
    gap: 1px;
    background: var(--line);
    min-height: 100%;
  }
  .month-dow {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--paper);
    padding: 0.4rem 0.6rem;
    font-size: 0.6875rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--ink-3);
  }
  .month-cell {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-height: 0;
    padding: 0.4rem 0.5rem;
    overflow: hidden;
    background: var(--paper);
    text-align: left;
    cursor: cell;
  }
  .month-cell:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .month-cell.is-other-month {
    background: color-mix(in srgb, var(--paper-2) 60%, var(--paper));
  }
  .month-cell.is-other-month .month-date {
    opacity: 0.4;
  }
  .month-cell.is-today .month-date {
    color: var(--accent);
    font-weight: 700;
  }
  .month-date {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--ink-2);
  }
  .month-chip {
    height: 1.45rem;
    width: 100%;
    flex: 0 0 auto;
  }
  .unit-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    grid-auto-rows: minmax(var(--cell-min-h), 1fr);
    gap: 1px;
    background: var(--line);
    min-height: 100%;
  }
  .unit-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .unit-total {
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    color: var(--ink-3);
  }
  .agenda {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.75rem 1rem 1.5rem;
  }
  .agenda-day {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 0.5rem 0 0.25rem;
    background: var(--paper);
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--ink-2);
  }
  .agenda-day.is-today {
    color: var(--accent);
  }
  .agenda-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .agenda-item {
    height: 3.5rem;
    container-type: size;
  }
  .agenda-empty {
    padding: 2rem 0;
    text-align: center;
    font-size: 0.8125rem;
    color: var(--ink-3);
  }
  .month-more {
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    color: var(--ink-3);
  }
</style>
