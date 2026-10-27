<script lang="ts">
  import Icon from '@iconify/svelte'
  import type { Snippet } from 'svelte'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    title: string
    description?: string
    id?: string
    limit?: number
    total?: number
    actions?: Snippet
    children?: Snippet<[number]>
    footer?: Snippet
  }

  let { title, description, id, limit = 10, total = 0, actions, children, footer }: Props = $props()

  let expanded = $state(false)

  const shown = $derived(expanded ? Infinity : limit)
  const folds = $derived(total > limit)
</script>

<section {id} class="py-7 first:pt-2">
  <div class="mb-3 flex items-end gap-4">
    <div class="min-w-0 flex-1">
      <h2 class="text-[15px] font-semibold tracking-tight text-ink">{title}</h2>
      {#if description}
        <p class="mt-0.5 max-w-[68ch] text-[13.5px] leading-5 text-ink-3">{description}</p>
      {/if}
    </div>
    {#if actions}
      <div class="flex shrink-0 items-center gap-1">{@render actions()}</div>
    {/if}
  </div>
  {@render children?.(shown)}
  {#if folds}
    <button type="button" data-testid="section-more" class="flex h-9 items-center gap-1.5 text-[13px] font-medium text-ink-2 hover:text-ink" aria-expanded={expanded} onclick={() => (expanded = !expanded)}>
      <Icon icon={expanded ? 'lucide:chevron-up' : 'lucide:chevron-down'} class="text-[14px]" />
      {expanded ? t('common.showLess') : t('common.showAll', { count: total })}
    </button>
  {/if}
  {@render footer?.()}
</section>
