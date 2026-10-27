<script lang="ts">
  import Panel from './Panel.svelte'
  import { heatmapMax, hourLabel, rampColor, weekdayLabel } from './stats'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    grid: number[][]
    title?: string
    takeaway?: string | null
  }

  let { grid, title, takeaway = null }: Props = $props()

  const max = $derived(heatmapMax(grid))
  let hover = $state<{ day: number; hour: number } | null>(null)
</script>

<Panel
  title={title ?? t('stats.heatmap.title')}
  {takeaway}
  aside={hover ? t('stats.heatmap.cell', { weekday: weekdayLabel(hover.day), hour: hourLabel(hover.hour), count: grid[hover.day][hover.hour] }) : t('stats.heatmap.hint')}
>
  <div class="heat grid gap-[2px]" role="img" aria-label={t('stats.heatmap.label')}>
    {#each grid as row, d (d)}
      <span class="flex items-center pr-1.5 font-mono text-[10px] text-ink-3">{weekdayLabel(d)}</span>
      {#each row as value, h (h)}
        <button
          class="aspect-square min-w-0 rounded-[2px] outline-offset-1 transition-[filter] hover:brightness-110 focus-visible:outline-2 focus-visible:outline-accent {hover && hover.day === d && hover.hour === h ? 'ring-2 ring-[var(--paper)]' : ''}"
          style:background={rampColor(value, max)}
          aria-label={t('stats.heatmap.cellLabel', { weekday: weekdayLabel(d), hour: hourLabel(h), count: value })}
          onmouseenter={() => (hover = { day: d, hour: h })}
          onmouseleave={() => (hover = null)}
          onfocus={() => (hover = { day: d, hour: h })}
          onblur={() => (hover = null)}
        ></button>
      {/each}
    {/each}
    <span></span>
    <div class="col-span-24 mt-1 flex justify-between font-mono text-[10px] text-ink-3">
      <span>00</span><span>06</span><span>12</span><span>18</span><span>23</span>
    </div>
  </div>
  <div class="flex items-center justify-end gap-2 font-mono text-[10px] text-ink-3">
    <span>{t('stats.heatmap.less')}</span>
    <span class="scale h-2 w-[clamp(4rem,30%,8rem)] rounded-[2px]"></span>
    <span>{t('stats.heatmap.more')}</span>
    <span class="tabular-nums text-ink-2">{max}</span>
  </div>
</Panel>

<style>
  .heat {
    grid-template-columns: auto repeat(24, minmax(0, 1fr));
  }
  .scale {
    background: linear-gradient(to right, var(--ramp-low), var(--ramp-mid), var(--ramp-high));
  }
</style>
