<script lang="ts">
  import { scaleLinear, scaleTime } from 'd3-scale'
  import { area, line, curveMonotoneX } from 'd3-shape'
  import type { TimelinePoint } from '$lib/tauri/types'
  import Panel from './Panel.svelte'
  import { compact, volumeSeries, type VolumePoint } from './stats'
  import { formatDate, t } from '$lib/i18n.svelte'

  interface Props {
    points: TimelinePoint[]
    range?: { start: string; end: string }
    other?: { points: TimelinePoint[]; range: { start: string; end: string } } | null
    labels?: [string, string]
    takeaway?: string | null
    onbrush?: (start: string, end: string) => void
  }

  let { points, range, other = null, labels, takeaway = null, onbrush }: Props = $props()

  const width = 720
  const height = 180
  const pad = { top: 8, right: 8, bottom: 22, left: 36 }

  const span = $derived(range ?? { start: points[0]?.date ?? '', end: points[points.length - 1]?.date ?? '' })
  const series = $derived(span.start ? volumeSeries({ points, range: span }, other) : [])
  const names = $derived(labels ?? [t('stats.timeline.received'), t('stats.timeline.sent')])
  const day = (p: VolumePoint) => new Date(`${p.date}T00:00:00`)
  const x = $derived(scaleTime().domain([series[0] ? day(series[0]) : new Date(), series.length ? day(series[series.length - 1]) : new Date()]).range([pad.left, width - pad.right]))
  const y = $derived(scaleLinear().domain([0, Math.max(1, ...series.map((p) => Math.max(p.values[0], p.values[1])))]).nice().range([height - pad.bottom, pad.top]))
  const path = (index: 0 | 1) => line<VolumePoint>().x((p) => x(day(p))).y((p) => y(p.values[index])).curve(curveMonotoneX)(series) ?? ''
  const first = $derived(path(0))
  const second = $derived(path(1))
  const wash = $derived(area<VolumePoint>().x((p) => x(day(p))).y0(y(0)).y1((p) => y(p.values[0])).curve(curveMonotoneX)(series) ?? '')
  const ticks = $derived(y.ticks(4))
  const dates = $derived(x.ticks(Math.min(6, series.length)))

  let hover = $state<number | null>(null)
  let brushStart = $state<number | null>(null)
  let brushEnd = $state<number | null>(null)

  function nearest(clientX: number, svg: SVGSVGElement): number {
    const rect = svg.getBoundingClientRect()
    const px = ((clientX - rect.left) / rect.width) * width
    let best = 0
    let dist = Infinity
    series.forEach((p, i) => {
      const d = Math.abs(x(day(p)) - px)
      if (d < dist) {
        dist = d
        best = i
      }
    })
    return best
  }

  function move(e: PointerEvent) {
    if (!series.length) return
    const i = nearest(e.clientX, e.currentTarget as SVGSVGElement)
    hover = i
    if (brushStart !== null) brushEnd = i
  }

  function down(e: PointerEvent) {
    if (!series.length || !onbrush) return
    brushStart = nearest(e.clientX, e.currentTarget as SVGSVGElement)
    brushEnd = brushStart
  }

  function up() {
    if (brushStart !== null && brushEnd !== null && brushStart !== brushEnd) {
      const [a, b] = [Math.min(brushStart, brushEnd), Math.max(brushStart, brushEnd)]
      onbrush?.(series[a].date, series[b].date)
    }
    brushStart = null
    brushEnd = null
  }

  const brush = $derived(brushStart !== null && brushEnd !== null ? [Math.min(brushStart, brushEnd), Math.max(brushStart, brushEnd)] : null)
  const current = $derived(hover !== null ? series[hover] : null)
  const readout = $derived(
    current
      ? other
        ? t('stats.timeline.pointCompare', { date: formatDate(day(current), { day: 'numeric', month: 'short' }), a: current.values[0], b: current.values[1] })
        : t('stats.timeline.point', { date: formatDate(day(current), { day: 'numeric', month: 'short' }), received: current.values[0], sent: current.values[1] })
      : onbrush
        ? t('stats.timeline.hint')
        : null
  )
</script>

<Panel title={t('stats.timeline.title')} {takeaway} aside={readout} legend={[{ label: names[0], color: 'var(--series-1)', mark: 'line' }, { label: names[1], color: 'var(--series-2)', mark: 'line' }]}>
  <svg viewBox="0 0 {width} {height}" class="w-full touch-none select-none" role="img" aria-label={t('stats.timeline.label')} onpointermove={move} onpointerleave={() => (hover = null)} onpointerdown={down} onpointerup={up}>
    {#each ticks as tick (tick)}
      <line x1={pad.left} x2={width - pad.right} y1={y(tick)} y2={y(tick)} stroke="var(--line)" stroke-width="1" />
      <text x={pad.left - 6} y={y(tick)} dy="0.32em" text-anchor="end" class="fill-[var(--ink-3)] font-mono text-[10px]">{compact(tick)}</text>
    {/each}
    {#each dates as d (d.getTime())}
      <text x={x(d)} y={height - 6} text-anchor="middle" class="fill-[var(--ink-3)] font-mono text-[10px]">{formatDate(d, { day: 'numeric', month: 'short' })}</text>
    {/each}
    {#if brush}
      <rect x={x(day(series[brush[0]]))} y={pad.top} width={Math.max(1, x(day(series[brush[1]])) - x(day(series[brush[0]])))} height={height - pad.top - pad.bottom} fill="var(--accent-soft)" />
    {/if}
    <path d={wash} fill="var(--series-1)" fill-opacity="0.1" />
    <path d={first} fill="none" stroke="var(--series-1)" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" />
    <path d={second} fill="none" stroke="var(--series-2)" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" />
    {#if current}
      <line x1={x(day(current))} x2={x(day(current))} y1={pad.top} y2={height - pad.bottom} stroke="var(--line-strong)" stroke-width="1" />
      <circle cx={x(day(current))} cy={y(current.values[0])} r="4" fill="var(--series-1)" stroke="var(--paper)" stroke-width="2" />
      <circle cx={x(day(current))} cy={y(current.values[1])} r="4" fill="var(--series-2)" stroke="var(--paper)" stroke-width="2" />
    {/if}
  </svg>
</Panel>
