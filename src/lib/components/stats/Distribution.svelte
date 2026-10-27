<script lang="ts">
  import type { ResponseBucket } from '$lib/tauri/types'
  import Panel from './Panel.svelte'
  import { compact, minutes, percent, replySeries, withinShare } from './stats'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    buckets: ResponseBucket[]
    other?: ResponseBucket[] | null
    labels?: [string, string]
    median: number | null
    mine: number | null
  }

  let { buckets, other = null, labels, median, mine }: Props = $props()

  const rows = $derived(replySeries(buckets, other))
  const names = $derived(labels ?? [t('stats.legend.theirs'), t('stats.legend.mine')])
  const heights = $derived(other ? rows.map((r) => r.shares) : rows.map((r) => r.counts))
  const max = $derived(Math.max(other ? 0.01 : 1, ...heights.flat()))
  const takeaway = $derived(
    rows.some((r) => r.counts[0] + r.counts[1] > 0)
      ? other
        ? t('stats.distribution.takeawayCompare', { a: percent(withinShare(rows, 60, 0), 1), b: percent(withinShare(rows, 60, 1), 1) })
        : t('stats.distribution.takeaway', { mine: percent(withinShare(rows, 60, 1), 1), theirs: percent(withinShare(rows, 60, 0), 1) })
      : null
  )
  let hover = $state<string | null>(null)
  const focused = $derived(rows.find((r) => r.key === hover) ?? null)
  const value = (i: number, side: 0 | 1) => (other ? percent(rows[i].shares[side], 1) : compact(rows[i].counts[side]))
</script>

<Panel
  title={t('stats.distribution.title')}
  {takeaway}
  aside={focused ? t('stats.distribution.pair', { label: focused.label, first: names[0], a: value(rows.indexOf(focused), 0), second: names[1], b: value(rows.indexOf(focused), 1) }) : other ? t('stats.distribution.shares') : t('stats.distribution.medians', { theirs: minutes(median), mine: minutes(mine) })}
  legend={[{ label: names[0], color: 'var(--series-1)' }, { label: names[1], color: 'var(--series-2)' }]}
>
  <div class="grid aspect-[3/1] min-h-28 grid-rows-[minmax(0,1fr)_auto] gap-1" role="img" aria-label={t('stats.distribution.label')}>
    <div class="flex min-h-0 items-end gap-1">
      {#each rows as row, i (row.key)}
        <button
          class="flex h-full min-w-0 flex-1 items-end justify-center gap-[2px] rounded-[var(--radius-row)] px-0.5 {hover === row.key ? 'bg-paper-2' : 'hover:bg-paper-2'}"
          onmouseenter={() => (hover = row.key)}
          onmouseleave={() => (hover = null)}
          onfocus={() => (hover = row.key)}
          onblur={() => (hover = null)}
          aria-label={t('stats.distribution.pair', { label: row.label, first: names[0], a: value(i, 0), second: names[1], b: value(i, 1) })}
        >
          {#each [0, 1] as side (side)}
            <span class="w-full max-w-3 rounded-t-[4px]" style:height="{Math.max(heights[i][side] > 0 ? 2 : 0, (heights[i][side] / max) * 100)}%" style:background="var(--series-{side + 1})"></span>
          {/each}
        </button>
      {/each}
    </div>
    <div class="flex gap-1">
      {#each rows as row (row.key)}
        <span class="min-w-0 flex-1 truncate text-center font-mono text-[10px] text-ink-3">{row.label}</span>
      {/each}
    </div>
  </div>
</Panel>
