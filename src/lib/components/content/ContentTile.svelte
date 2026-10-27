<script lang="ts">
  import Icon from '@iconify/svelte'
  import type { ContentItem } from '$lib/tauri/types'
  import { contentSrc, isVideo, isVisual, kindIcon } from '$lib/media'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    item: ContentItem
    selected?: boolean
    active?: boolean
    autoplay?: boolean
    dropping?: boolean
    onopen: (item: ContentItem, e: MouseEvent) => void
    ontoggle: (item: ContentItem) => void
    ondragstart?: (item: ContentItem, e: DragEvent) => void
    ondrop?: (item: ContentItem, e: DragEvent) => void
  }

  let { item, selected = false, active = false, autoplay = true, dropping = false, onopen, ontoggle, ondragstart, ondrop }: Props = $props()
  let video = $state<HTMLVideoElement>()
  let over = $state(false)

  const visual = $derived(isVisual(item))
  const preview = $derived((item.body ?? '').replace(/\s+/g, ' ').trim())

  function enter() {
    if (autoplay) video?.play().catch(() => {})
  }

  function leave() {
    if (!video) return
    video.pause()
    video.currentTime = 0
  }
</script>

<div
  data-testid="content-tile"
  data-id={item.id}
  data-kind={item.kind}
  class="group relative block w-full overflow-hidden rounded-[0.5rem] bg-paper-3 {visual ? 'aspect-square' : 'min-h-[4.5rem]'} {active ? 'ring-2 ring-accent' : selected ? 'ring-2 ring-accent/60' : over && dropping ? 'ring-2 ring-accent/40' : ''}"
  role="button"
  tabindex="0"
  draggable="true"
  aria-label={item.name}
  aria-pressed={selected}
  onmouseenter={enter}
  onmouseleave={leave}
  onclick={(e) => onopen(item, e)}
  onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && onopen(item, e as unknown as MouseEvent)}
  ondragstart={(e) => ondragstart?.(item, e)}
  ondragover={(e) => {
    if (!dropping) return
    e.preventDefault()
    over = true
  }}
  ondragleave={() => (over = false)}
  ondrop={(e) => {
    over = false
    if (!dropping) return
    e.preventDefault()
    e.stopPropagation()
    ondrop?.(item, e)
  }}
>
  {#if !visual}
    <div class="flex h-full flex-col gap-1 p-2.5 pr-8">
      <span class="flex min-w-0 items-center gap-1.5 text-[13px] font-medium text-ink">
        <Icon icon={kindIcon(item.kind)} class="shrink-0 text-[13px] text-ink-3" />
        <span class="truncate">{item.name}</span>
      </span>
      {#if item.kind === 'file'}
        <span class="truncate font-mono text-[11px] text-ink-3">{item.mime_type}</span>
      {:else}
        <span class="line-clamp-2 text-[12px] leading-4 text-ink-3">{preview || t('content.tile.empty')}</span>
      {/if}
    </div>
  {:else if isVideo(item)}
    <video bind:this={video} src={contentSrc(item)} class="absolute inset-0 size-full object-cover" muted loop playsinline preload="metadata"></video>
    <span class="absolute bottom-1.5 left-1.5 flex items-center gap-1 rounded-[0.3rem] bg-black/55 px-1 text-[10.5px] text-white">
      <Icon icon="lucide:play" class="text-[10px]" />
      {#if item.duration_ms}{Math.round(item.duration_ms / 1000)}s{/if}
    </span>
  {:else}
    <img src={contentSrc(item)} alt={item.name} loading="lazy" class="absolute inset-0 size-full object-cover" />
  {/if}
  {#if item.is_favourite}
    <Icon icon="lucide:heart" class="absolute right-1.5 bottom-1.5 fill-current text-[13px] {visual ? 'text-white drop-shadow' : 'text-signal'}" />
  {/if}
  <button
    type="button"
    data-testid="content-tile-select"
    class="absolute top-1.5 flex size-5 items-center justify-center rounded-[0.3rem] text-white {visual ? 'left-1.5' : 'right-1.5'} {selected ? 'bg-accent' : 'bg-black/45 opacity-0 group-hover:opacity-100'}"
    aria-label={selected ? t('content.tile.deselect', { name: item.name }) : t('content.tile.select', { name: item.name })}
    onclick={(e) => {
      e.stopPropagation()
      ontoggle(item)
    }}
  >
    <Icon icon={selected ? 'lucide:check' : 'lucide:plus'} class="text-[12px]" />
  </button>
  {#if item.used_count > 0}
    <span class="absolute rounded-[0.3rem] px-1 font-mono text-[10px] opacity-0 group-hover:opacity-100 {visual ? 'top-1.5 right-1.5 bg-black/45 text-white' : 'bottom-1.5 left-1.5 text-ink-3'}">×{item.used_count}</span>
  {/if}
</div>
