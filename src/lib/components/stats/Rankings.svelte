<script lang="ts">
  import type { Ranking } from '$lib/tauri/types'
  import { rankingDescription, rankingTitle, rankingValue } from './stats'
  import { t } from '$lib/i18n.svelte'
  import { Button } from '$lib/components/ui'

  interface Props {
    rankings: Ranking[]
    onpick?: (personId: string) => void
  }

  let { rankings, onpick }: Props = $props()

  let expanded = $state<Set<string>>(new Set())

  function toggle(key: string) {
    const next = new Set(expanded)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    expanded = next
  }
</script>

<section class="@container min-w-0">
  <div class="mb-3 flex items-baseline justify-between">
    <h2 class="text-[15px] font-semibold tracking-tight text-ink">{t('stats.rankings.title')}</h2>
    <span class="font-mono text-[11px] text-ink-3">{t('stats.count.categories', { count: rankings.length })}</span>
  </div>
  <div class="columns-1 gap-8 @2xl:columns-2 @5xl:columns-3 @[96rem]:columns-4">
    {#each rankings as ranking (ranking.key)}
      {@const open = expanded.has(ranking.key)}
      {@const shown = open ? ranking.entries.slice(0, 10) : ranking.entries.slice(0, 5)}
      {@const max = Math.max(1, ...ranking.entries.map((e) => e.value))}
      <figure class="mb-8 break-inside-avoid">
        <figcaption class="flex items-baseline justify-between gap-3">
          <span class="text-[14px] font-semibold text-ink">{rankingTitle(ranking)}</span>
          <span class="text-[13px] text-ink-3">{rankingDescription(ranking)}</span>
        </figcaption>
        <ol class="mt-2 divide-y divide-line">
          {#each shown as entry, index (entry.person_id ?? `${entry.name}:${index}`)}
            <li class="grid grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-2 py-1.5">
              <span class="font-mono text-[11px] tabular-nums {index === 0 ? 'text-accent' : 'text-ink-3'}">{index + 1}</span>
              <span class="min-w-0">
                <button data-testid="stats-ranking-person" data-id={entry.person_id} class="block max-w-full overflow-hidden text-ellipsis whitespace-nowrap text-left text-[13px] {index === 0 ? 'font-semibold text-ink' : 'font-medium text-ink-2'} {entry.person_id ? 'hover:underline' : ''}" onclick={() => entry.person_id && onpick?.(entry.person_id)} disabled={!entry.person_id}>
                  {entry.name}
                </button>
                <span class="mt-1 block h-1 rounded-r-[2px] {index === 0 ? 'bg-accent' : 'bg-paper-3'}" style:width="{Math.max(3, (entry.value / max) * 100)}%"></span>
              </span>
              <span class="text-right font-mono text-[12px] tabular-nums text-ink">
                {rankingValue(entry)}
                {#if entry.detail}
                  <span class="text-ink-3"> {entry.detail}</span>
                {/if}
              </span>
            </li>
          {/each}
          {#if ranking.entries.length === 0}
            <li class="py-2 text-[13px] text-ink-3">{t('stats.nothingInRange')}</li>
          {/if}
        </ol>
        {#if ranking.entries.length > 5}
          <Button size="sm" class="mt-1 -ml-2.5" onclick={() => toggle(ranking.key)}>{open ? t('stats.rankings.fewer') : t('stats.rankings.all', { count: Math.min(10, ranking.entries.length) })}</Button>
        {/if}
      </figure>
    {/each}
  </div>
</section>
