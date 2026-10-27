<script lang="ts">
  import { formatSize } from '$lib/links'
  import Icon from '@iconify/svelte'
  import { untrack } from 'svelte'
  import type { PendingAttachment } from '$lib/media'
  import { attachmentMime, attachmentName, contentSrc, isVisual } from '$lib/media'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    items: PendingAttachment[];
    onremove: (id: string) => void;
  }

  let { items, onremove }: Props = $props()

  let thumbnails = $state<Map<string, string>>(new Map())

  function getFileIcon(type: string): string {
    if (type.startsWith('image/')) return 'lucide:image'
    if (type === 'application/pdf') return 'lucide:file-text'
    if (type.includes('word') || type.includes('.document')) return 'lucide:file-text'
    if (type.includes('excel') || type.includes('.sheet')) return 'lucide:table'
    if (type.includes('powerpoint') || type.includes('.presentation')) return 'lucide:presentation'
    if (type.includes('zip') || type.includes('tar') || type.includes('gzip')) return 'lucide:folder-archive'
    if (type.startsWith('audio/')) return 'lucide:file-audio'
    if (type.startsWith('video/')) return 'lucide:file-video'
    return 'lucide:paperclip'
  }

  $effect(() => {
    const current = items
    untrack(() => {
      const next = new Map<string, string>()
      let changed = false
      const revoke: string[] = []
      for (const item of current) {
        if (item.file?.type.startsWith('image/')) {
          const existing = thumbnails.get(item.id)
          if (existing) {
            next.set(item.id, existing)
          } else {
            next.set(item.id, URL.createObjectURL(item.file))
            changed = true
          }
        }
      }
      for (const [key, url] of thumbnails) {
        if (!next.has(key)) {
          revoke.push(url)
          changed = true
        }
      }
      if (!changed) return
      revoke.forEach((url) => URL.revokeObjectURL(url))
      thumbnails = next
    })
  })
</script>

{#if items.length > 0}
  <div class="flex flex-wrap gap-1.5 pb-1.5">
    {#each items as item (item.id)}
      {@const mime = attachmentMime(item)}
      {@const src = item.content ? (isVisual(item.content) ? contentSrc(item.content) : null) : thumbnails.get(item.id)}
      <div class="flex h-8 max-w-[16rem] items-center gap-2 rounded-[0.5rem] pl-1.5 pr-1 hairline">
        {#if src}
          <img {src} alt={attachmentName(item)} class="size-6 rounded-[0.25rem] object-cover" />
        {:else}
          <Icon icon={getFileIcon(mime)} class="shrink-0 text-[15px] text-ink-3" />
        {/if}
        <span class="min-w-0 truncate text-[13px] text-ink">{attachmentName(item)}</span>
        <span class="shrink-0 font-mono text-[11px] text-ink-3">{item.file ? formatSize(item.file.size) : item.content?.kind === 'gif' ? 'GIF' : item.content?.kind === 'emoji' ? t('conversation.attachment.emoji') : mime}</span>
        <button class="flex size-6 shrink-0 items-center justify-center rounded-[0.375rem] text-ink-3 hover:bg-paper-3 hover:text-ink" onclick={() => onremove(item.id)} aria-label={t('conversation.attachment.remove')}>
          <Icon icon="lucide:x" class="text-[12px]" />
        </button>
      </div>
    {/each}
  </div>
{/if}
