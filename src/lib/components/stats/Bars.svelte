<script lang="ts">
  import Panel from './Panel.svelte'
  import { compact } from './stats'
  import { t } from '$lib/i18n.svelte'

  interface Row {
    key: string
    label: string
    value: number
    other?: number
    detail?: string
    color?: string
  }

  interface Props {
    title: string
    rows: Row[]
    takeaway?: string | null
    aside?: string | null
    legend?: [string, string] | null
    empty?: string
    format?: (value: number) => string
    unit?: (value: number) => string
    onpick?: (key: string) => void
  }

  let { title, rows, takeaway = null, aside, legend = null, empty, format = compact, unit = (value: number) => t('stats.bars.messages', { count: value }), onpick }: Props = $props()

  const paired = $derived(rows.some((r) => r.other !== undefined))
  const max = $derived(Math.max(1, ...rows.map((r) => Math.max(r.value, r.other ?? 0))))
  const total = $derived(rows.reduce((s, r) => s + r.value, 0))
  let hover = $state<string | null>(null)
</script>

<Panel
  {title}
  {takeaway}
  aside={aside === undefined ? t('stats.count.messages', { count: total, value: compact(total) }) : aside}
  legend={paired && legend ? [{ label: legend[0], color: 'var(--series-1)' }, { label: legend[1], color: 'var(--series-2)' }] : []}
>
  <ul class="flex flex-col gap-1.5">
    {#each rows as row (row.key)}
      <li>
        <button
          data-testid="stats-row"
          data-key={row.key}
          class="grid w-full grid-cols-[minmax(0,2fr)_minmax(0,3fr)_minmax(0,1.4fr)] items-center gap-3 rounded-[var(--radius-row)] px-1 py-0.5 text-left {hover === row.key ? 'bg-paper-2' : 'hover:bg-paper-2'} {onpick ? '' : 'cursor-default'}"
          onmouseenter={() => (hover = row.key)}
          onmouseleave={() => (hover = null)}
          onfocus={() => (hover = row.key)}
          onblur={() => (hover = null)}
          onclick={() => onpick?.(row.key)}
          aria-label={row.other !== undefined ? t('stats.bars.pair', { label: row.label, value: format(row.value), other: format(row.other) }) : t('stats.bars.label', { label: row.label, value: format(row.value), unit: unit(row.value) })}
        >
          <span class="flex items-center gap-1.5 overflow-hidden text-ellipsis whitespace-nowrap text-[13px] text-ink-2">
            {#if row.color}
              <span class="size-2 shrink-0 rounded-full" style:background={row.color}></span>
            {/if}
            {row.label}
          </span>
          <span class="flex flex-col gap-[2px]">
            <span class="{paired ? 'h-2' : 'h-3'} rounded-r-[4px]" style:width="{Math.max(row.value > 0 ? 1 : 0, (row.value / max) * 100)}%" style:background={paired ? 'var(--series-1)' : (row.color ?? 'var(--series-1)')}></span>
            {#if paired}
              <span class="h-2 rounded-r-[4px]" style:width="{Math.max((row.other ?? 0) > 0 ? 1 : 0, ((row.other ?? 0) / max) * 100)}%" style:background="var(--series-2)"></span>
            {/if}
          </span>
          <span class="truncate text-right font-mono text-[12px] tabular-nums text-ink">
            {format(row.value)}{#if paired}<span class="text-ink-3"> · {format(row.other ?? 0)}</span>{/if}
            {#if row.detail}
              <span class="text-ink-3"> {row.detail}</span>
            {/if}
          </span>
        </button>
      </li>
    {/each}
    {#if rows.length === 0}
      <li data-testid="stats-empty" class="py-3 text-center text-[13px] text-ink-3">{empty ?? t('stats.nothingInRange')}</li>
    {/if}
  </ul>
</Panel>
