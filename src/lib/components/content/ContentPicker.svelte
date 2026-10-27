<script lang="ts">
  import Icon from '@iconify/svelte'
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import { api } from '$lib/tauri'
  import type { ContentItem, ContentKind } from '$lib/tauri/types'
  import { CONTENT_KINDS, filterLibrary, kindIcon } from '$lib/media'
  import { Field, Sheet } from '$lib/components/ui'
  import { anchored, type Align, type Box, type Side } from '$lib/popover'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    mail: boolean
    busy?: boolean
    onpick: (item: ContentItem) => void
    onclose: () => void
    at?: Box | (() => Box | null) | null
    side?: Side
    align?: Align
  }

  let { mail, busy = false, onpick, onclose, at = null, side = 'above', align = 'start' }: Props = $props()

  let items = $state<ContentItem[]>([])
  let query = $state('')
  let index = $state(0)
  let failure = $state<string | null>(null)

  const order = $derived<ContentKind[]>(mail ? ['email', 'snippet', 'reply', 'file'] : ['reply', 'snippet', 'file'])
  const groups = $derived(
    order
      .map((kind) => ({ kind, label: CONTENT_KINDS.find((entry) => entry.value === kind)?.label ?? 'content.kind.snippet', items: filterLibrary(items.filter((item) => item.kind === kind), query) }))
      .filter((group) => group.items.length > 0)
  )
  const flat = $derived(groups.flatMap((group) => group.items))

  onMount(() => {
    api
      .getContent(undefined, undefined, 'used')
      .then((list) => (items = list))
      .catch((error) => (failure = String(error)))
  })

  function keydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      index = Math.min(flat.length - 1, index + 1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      index = Math.max(0, index - 1)
    } else if (e.key === 'Enter' && flat[index]) {
      e.preventDefault()
      onpick(flat[index])
    }
  }
</script>

<div data-testid="content-picker" class="relative w-[24rem] max-w-[90vw]" use:anchored={{ at, side, align, onclose }}>
  <Sheet label={t('content.title')} width="100%" anchored {onclose}>
    <div class="px-2 pt-2">
      <Field data-testid="content-picker-search" bind:value={query} size="sm" placeholder={t('content.picker.templates')} oninput={() => (index = 0)} onkeydown={keydown} autofocus />
    </div>
    <div class="max-h-[22rem] min-h-[8rem] overflow-y-auto p-1.5" role="listbox" aria-label={t('content.title')} aria-busy={busy}>
      {#if failure}
        <p class="px-2 py-1 text-[12px] text-error-500">{failure}</p>
      {/if}
      {#each groups as group (group.kind)}
        <p class="flex items-center gap-1.5 px-2 pt-2 pb-1 text-[12px] font-medium text-ink-3"><Icon icon={kindIcon(group.kind)} class="text-[12px]" />{t(group.label)}</p>
        {#each group.items as item (item.id)}
          {@const position = flat.indexOf(item)}
          <button type="button" role="option" data-testid="content-picker-item" data-id={item.id} data-kind={item.kind} aria-selected={position === index} class="flex w-full min-w-0 flex-col rounded-[0.5rem] px-2.5 py-1.5 text-left {position === index ? 'bg-accent-soft' : 'hover:bg-paper-3'}" onmouseenter={() => (index = position)} onclick={() => onpick(item)}>
            <span class="flex min-w-0 items-center gap-2 text-[13.5px] text-ink">
              <span class="truncate">{item.name}</span>
              {#if item.used_count > 0}<span class="ml-auto shrink-0 font-mono text-[10.5px] text-ink-3">×{item.used_count}</span>{/if}
            </span>
            <span class="truncate text-[12px] text-ink-3">{item.kind === 'file' ? item.mime_type : (item.body ?? '').replace(/\s+/g, ' ')}</span>
          </button>
        {/each}
      {:else}
        <p class="px-6 py-8 text-center text-[13px] text-ink-3">{items.length ? t('content.noMatches') : t('content.picker.empty')}</p>
      {/each}
    </div>
    {#snippet footer()}
      <span class="text-[12px] text-ink-3">{t('content.picker.help')}</span>
      <button type="button" data-testid="content-picker-manage" class="ml-auto text-[12px] text-ink-3 hover:text-ink" onclick={() => goto('/main/content')}>{t('content.picker.manage')}</button>
    {/snippet}
  </Sheet>
</div>
