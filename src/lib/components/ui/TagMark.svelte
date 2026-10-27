<script lang="ts">
  import Icon from '@iconify/svelte'
  import type { Tag } from '$lib/tauri/types'
  import { tagTone } from '$lib/tauri/tags'

  interface Props {
    tag: Pick<Tag, 'color' | 'icon' | 'name'>
    size?: 'xs' | 'sm' | 'md'
    label?: boolean
    ink?: 'ink' | 'ink-2' | 'ink-3'
    class?: string
  }

  let { tag, size = 'sm', label = false, ink = 'ink', class: className = '' }: Props = $props()

  const dots = { xs: 'size-1.5', sm: 'size-2', md: 'size-2.5' }
  const glyphs = { xs: 'text-[11px]', sm: 'text-[13px]', md: 'text-[15px]' }
  const inks = { ink: 'text-ink', 'ink-2': 'text-ink-2', 'ink-3': 'text-ink-3' }
  const color = $derived(tag.color || '#8c93a3')
</script>

<span
  class="tag-mark inline-flex min-w-0 items-center gap-1.5 {className}"
  style:--tag-day={tagTone(color, 'day')}
  style:--tag-night={tagTone(color, 'night')}
  data-tag-mark={tag.icon ? 'icon' : 'dot'}
  title={label ? undefined : tag.name}
>
  {#if tag.icon}
    <Icon icon={tag.icon} class="shrink-0 {glyphs[size]}" aria-hidden="true" />
  {:else}
    <span class="shrink-0 rounded-full bg-current {dots[size]}" aria-hidden="true"></span>
  {/if}
  {#if label}
    <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap {inks[ink]}">{tag.name}</span>
  {/if}
</span>
