<script lang="ts">
  import { onMount } from 'svelte'
  import { geoOrthographic, geoPath, geoGraticule10, geoDistance } from 'd3-geo'
  import { feature } from 'topojson-client'
  import type { Topology, GeometryCollection } from 'topojson-specification'
  import type { FeatureCollection, Geometry } from 'geojson'
  import land110 from 'world-atlas/land-110m.json'
  import type { Origin } from '$lib/tauri/types'
  import { compact, originTotals } from './stats'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    origins: Origin[]
  }

  let { origins }: Props = $props()

  const size = 420
  const land = feature(land110 as unknown as Topology, (land110 as unknown as Topology).objects.land as GeometryCollection) as FeatureCollection<Geometry>
  const graticule = geoGraticule10()

  let rotation = $state<[number, number]>([-10, -30])
  let dragging = $state(false)
  let hover = $state<Origin | null>(null)
  let frame = 0
  let last = 0

  const projection = $derived(geoOrthographic().scale(size / 2 - 8).translate([size / 2, size / 2]).rotate([rotation[0], rotation[1]]).clipAngle(90))
  const path = $derived(geoPath(projection))
  const landPath = $derived(path(land) ?? '')
  const graticulePath = $derived(path(graticule) ?? '')
  const maxCount = $derived(Math.max(1, ...origins.map((o) => o.count)))
  const totals = $derived(originTotals(origins))

  const points = $derived(
    origins
      .map((o) => {
        const visible = geoDistance([o.lon, o.lat], [-rotation[0], -rotation[1]]) < Math.PI / 2
        const xy = projection([o.lon, o.lat])
        return xy && visible ? { origin: o, x: xy[0], y: xy[1], r: 3 + Math.sqrt(o.count / maxCount) * 11 } : null
      })
      .filter((p): p is NonNullable<typeof p> => p !== null)
  )

  function tick(now: number) {
    if (!dragging && !hover) {
      const dt = last ? now - last : 16
      rotation = [rotation[0] + dt * 0.008, rotation[1]]
    }
    last = now
    frame = requestAnimationFrame(tick)
  }

  onMount(() => {
    if (!window.matchMedia('(prefers-reduced-motion: reduce)').matches) frame = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(frame)
  })

  let start: { x: number; y: number; rotation: [number, number] } | null = null

  function down(e: PointerEvent) {
    dragging = true
    start = { x: e.clientX, y: e.clientY, rotation: [rotation[0], rotation[1]] }
    ;(e.currentTarget as Element).setPointerCapture(e.pointerId)
  }

  function move(e: PointerEvent) {
    if (!dragging || !start) return
    const k = 0.5
    rotation = [start.rotation[0] + (e.clientX - start.x) * k, Math.max(-90, Math.min(90, start.rotation[1] - (e.clientY - start.y) * k))]
  }

  function up() {
    dragging = false
    start = null
  }

  function focus(o: Origin) {
    hover = o
    rotation = [-o.lon, -o.lat]
  }
</script>

<figure class="min-w-0">
  <figcaption class="mb-2 flex items-baseline justify-between">
    <span class="text-[13px] font-medium text-ink">{t('stats.globe.title')}</span>
    <span class="font-mono text-[11px] text-ink-3">{t('stats.globe.totals', { messages: t('stats.count.messages', { count: totals.messages, value: compact(totals.messages) }), cities: totals.cities, countries: totals.countries })}</span>
  </figcaption>
  <div class="grid gap-4 md:grid-cols-[minmax(0,1fr)_16rem]">
  <div>
    <div class="relative">
      <svg viewBox="0 0 {size} {size}" class="mx-auto w-full max-w-[26rem] touch-none select-none {dragging ? 'cursor-grabbing' : 'cursor-grab'}" role="img" aria-label={t('stats.globe.label')} onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up}>
        <circle cx={size / 2} cy={size / 2} r={size / 2 - 8} fill="var(--paper-2)" stroke="var(--line-strong)" stroke-width="1" />
        <path d={graticulePath} fill="none" stroke="var(--line)" stroke-width="0.6" />
        <path d={landPath} fill="var(--paper-3)" stroke="var(--line-strong)" stroke-width="0.5" />
        {#each points as p (p.origin.ip)}
          <circle cx={p.x} cy={p.y} r={p.r} fill="var(--series-1)" fill-opacity={hover && hover !== p.origin ? 0.35 : 0.8} stroke="var(--paper)" stroke-width="2" />
          <circle
            cx={p.x}
            cy={p.y}
            r={Math.max(10, p.r + 4)}
            fill="transparent"
            role="button"
            tabindex="0"
            aria-label={t('stats.globe.point', { place: p.origin.city ?? p.origin.ip, country: p.origin.country ?? '', count: p.origin.count })}
            onpointerenter={() => (hover = p.origin)}
            onpointerleave={() => (hover = null)}
            onfocus={() => (hover = p.origin)}
            onblur={() => (hover = null)}
          />
        {/each}
      </svg>
      {#if hover}
        {@const p = points.find((q) => q.origin === hover)}
        {#if p}
          <div class="pointer-events-none absolute z-10 -translate-x-1/2 rounded-[var(--radius-row)] px-2 py-1 text-[12px] text-ink glass" style:left="{(p.x / size) * 100}%" style:top="calc({(p.y / size) * 100}% - 2.5rem)">
            <span class="font-medium">{hover.city ?? hover.ip}</span>
            {#if hover.country}<span class="text-ink-3">, {hover.country}</span>{/if}
            <span class="ml-1 font-mono text-[11px] text-ink-2">{hover.count}</span>
            {#if hover.person_name}<span class="block text-[11px] text-ink-3">{hover.person_name}</span>{/if}
          </div>
        {/if}
      {/if}
    </div>
  </div>
  <ol class="flex max-h-[26rem] flex-col gap-0.5 overflow-y-auto" aria-label={t('stats.globe.origins')}>
    {#each [...origins].sort((a, b) => b.count - a.count) as o (o.ip)}
      <li>
        <button class="grid w-full grid-cols-[minmax(0,1fr)_3rem] items-center gap-2 rounded-[var(--radius-row)] px-2 py-1 text-left {hover === o ? 'bg-paper-2' : 'hover:bg-paper-2'}" onmouseenter={() => (hover = o)} onmouseleave={() => (hover = null)} onclick={() => focus(o)}>
          <span class="min-w-0">
            <span class="block overflow-hidden text-ellipsis whitespace-nowrap text-[13px] text-ink">{o.city ?? o.ip}<span class="text-ink-3">{o.country ? `, ${o.country}` : ''}</span></span>
            <span class="block overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[10px] text-ink-3">{o.person_name ?? o.ip} · {o.provider_type}</span>
          </span>
          <span class="text-right font-mono text-[12px] tabular-nums text-ink">{compact(o.count)}</span>
        </button>
      </li>
    {/each}
    {#if origins.length === 0}
      <li data-testid="stats-empty" class="px-2 py-3 text-[13px] text-ink-3">{t('stats.globe.empty')}</li>
    {/if}
  </ol>
  </div>
</figure>
