<script lang="ts">
  import type { Snippet } from 'svelte'
  import { t } from '$lib/i18n.svelte'

  interface Key {
    label: string
    color: string
    mark?: 'bar' | 'line'
  }

  interface Props {
    title: string
    takeaway?: string | null
    aside?: string | null
    legend?: Key[]
    children: Snippet
  }

  let { title, takeaway = null, aside = null, legend = [], children }: Props = $props()
</script>

<figure class="flex min-w-0 flex-col gap-3">
  <figcaption class="flex flex-col gap-1">
    <span class="flex items-baseline justify-between gap-3">
      <span class="text-[13px] font-medium text-ink">{title}</span>
      {#if aside}
        <span class="truncate font-mono text-[11px] text-ink-3">{aside}</span>
      {/if}
    </span>
    {#if takeaway}
      <span class="text-[13px] leading-5 text-ink-2">{takeaway}</span>
    {/if}
    {#if legend.length > 1}
      <span class="flex flex-wrap items-center gap-x-3 gap-y-1 font-mono text-[11px] text-ink-3" aria-label={t('stats.legend.label')}>
        {#each legend as key (key.label)}
          <span class="flex items-center gap-1.5">
            {#if key.mark === 'line'}
              <span class="inline-block h-0.5 w-3 rounded" style:background={key.color}></span>
            {:else}
              <span class="inline-block size-2 rounded-[2px]" style:background={key.color}></span>
            {/if}
            {key.label}
          </span>
        {/each}
      </span>
    {/if}
  </figcaption>
  {@render children()}
</figure>
