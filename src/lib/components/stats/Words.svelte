<script lang="ts">
  import { compact, wordScale } from './stats'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    words: { word: string; count: number }[]
  }

  let { words }: Props = $props()

  const scale = $derived(wordScale(words))
  const sorted = $derived([...words].sort((a, b) => b.count - a.count))
</script>

<figure class="min-w-0">
  <figcaption class="mb-2 flex items-baseline justify-between">
    <span class="text-[13px] font-medium text-ink">{t('stats.words.title')}</span>
    <span class="font-mono text-[11px] text-ink-3">{t('stats.count.words', { count: words.length })}</span>
  </figcaption>
  <ol class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
    {#each sorted as w, i (w.word)}
      <li class="flex items-baseline gap-1" title={t('stats.count.times', { count: w.count })}>
        <span class="font-medium leading-none {i < 3 ? 'text-ink' : 'text-ink-2'}" style:font-size="{scale(w.count)}px">{w.word}</span>
        <span class="font-mono text-[10px] text-ink-3">{compact(w.count)}</span>
      </li>
    {/each}
    {#if sorted.length === 0}
      <li data-testid="stats-empty" class="text-[13px] text-ink-3">{t('stats.words.empty')}</li>
    {/if}
  </ol>
</figure>
