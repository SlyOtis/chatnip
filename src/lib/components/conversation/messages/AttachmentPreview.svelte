<script lang="ts" module>
  export function getAttachmentIcon(contentType: string): string {
    if (contentType === 'application/pdf') return 'lucide:file-text'
    if (contentType.includes('word') || contentType.includes('.document')) return 'lucide:file-text'
    if (contentType.includes('excel') || contentType.includes('.sheet')) return 'lucide:table'
    if (contentType.includes('powerpoint') || contentType.includes('.presentation')) return 'lucide:presentation'
    if (contentType.includes('zip') || contentType.includes('tar') || contentType.includes('gzip')) return 'lucide:folder-archive'
    if (contentType.startsWith('audio/')) return 'lucide:file-audio'
    if (contentType.startsWith('video/')) return 'lucide:file-video'
    if (contentType.startsWith('image/')) return 'lucide:image'
    return 'lucide:paperclip'
  }

</script>

<script lang="ts">
  import { formatSize } from '$lib/links'
  import Icon from '@iconify/svelte'
  import type { Attachment } from '$lib/tauri/types'
  import { modalsShow } from '$lib/modals'
  import { api } from '$lib/tauri'
  import { attachmentInput, readFileAsBase64 } from '$lib/media'
  import { showTooltip } from '$lib/tooltip'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    attachments: Attachment[];
    framed?: boolean;
    messageId?: string;
    source?: Attachment[];
  }

  let { attachments, framed = false, messageId, source }: Props = $props()

  let saved = $state<string[]>([])
  let saving = $state<string[]>([])
  let failure = $state<string | null>(null)

  const isMedia = (a: Attachment) => a.content_type.startsWith('image/') || a.content_type.startsWith('video/')
  const images = $derived(attachments.filter(isMedia))
  const files = $derived(attachments.filter((a) => !isMedia(a)))

  async function favourite(attachment: Attachment) {
    if (saving.includes(attachment.id) || saved.includes(attachment.id)) return
    saving = [...saving, attachment.id]
    failure = null
    try {
      if (messageId) {
        await api.postContentFromMessage(messageId, (source ?? attachments).findIndex((a) => a.id === attachment.id))
      } else {
        const response = await fetch(attachment.url ?? `asset://localhost/${attachment.id}`)
        if (!response.ok) throw new Error(t('conversation.attachment.notDownloaded', { name: attachment.name }))
        const data = await readFileAsBase64(await response.blob())
        await api.postContent(attachmentInput(attachment, data))
      }
      saved = [...saved, attachment.id]
    } catch (error) {
      failure = String(error)
    } finally {
      saving = saving.filter((id) => id !== attachment.id)
    }
  }

  let opening = $state<string[]>([])
  const busy = (attachment: Attachment) => opening.includes(attachment.id)

  async function preview(attachment: Attachment, quick_look: boolean) {
    if (!messageId || busy(attachment)) return
    opening = [...opening, attachment.id]
    failure = null
    try {
      await api.postAttachmentOpen({ message_id: messageId, attachment_id: attachment.id, quick_look })
    } catch (error) {
      failure = String(error)
    } finally {
      opening = opening.filter((id) => id !== attachment.id)
    }
  }

  function handleImageClick(attachment: Attachment, e: MouseEvent) {
    if (e.altKey) {
      e.preventDefault()
      favourite(attachment)
      return
    }
    modalsShow({ source: attachment.url ?? `asset://localhost/${attachment.id}`, alt: attachment.name })
  }
</script>

{#if attachments.length > 0}
  <div class="mt-2 {framed ? 'border-t border-line pt-3' : ''}">
    {#if images.length > 0}
      <div class="grid max-w-[30rem] gap-1.5 {images.length === 1 ? 'grid-cols-1' : images.length === 2 ? 'grid-cols-2' : 'grid-cols-3'}">
        {#each images as attachment (attachment.id)}
          {@const kept = saved.includes(attachment.id)}
          {@const busy = saving.includes(attachment.id)}
          <div class="group/att relative overflow-hidden rounded-[8px] bg-paper-2 hairline {images.length === 1 ? 'aspect-[4/3] max-h-72' : 'aspect-square'}">
            <button class="block size-full" onclick={(e) => handleImageClick(attachment, e)} aria-label={t('conversation.link.open', { name: attachment.name })}>
              {#if attachment.content_type.startsWith('video/')}
                <video src={attachment.url ?? `asset://localhost/${attachment.id}`} class="size-full object-cover" muted playsinline preload="metadata"></video>
              {:else}
                <img src={attachment.url ?? `asset://localhost/${attachment.id}`} alt={attachment.name} class="size-full object-cover" />
              {/if}
              <span class="absolute inset-0 flex items-center justify-center opacity-0 group-hover/att:bg-ink/25 group-hover/att:opacity-100">
                <Icon icon="lucide:zoom-in" class="text-[18px] text-paper" />
              </span>
            </button>
            <button
              type="button"
              class="absolute top-1.5 right-1.5 flex size-6 items-center justify-center rounded-full {kept ? 'text-signal opacity-100' : 'text-paper opacity-0 group-hover/att:opacity-80 hover:opacity-100 focus-visible:opacity-100'}"
              aria-label={kept ? t('content.save.saved') : t('content.save.media')}
              onmouseenter={(e) => showTooltip(e.currentTarget, kept ? t('content.save.inContent') : t('content.save.mediaKeys'))}
              onclick={() => favourite(attachment)}
            >
              <Icon icon={busy ? 'lucide:loader-circle' : kept ? 'lucide:check' : 'lucide:smile-plus'} class="text-[14px] drop-shadow-[0_1px_1px_rgb(0_0_0/0.5)] {busy ? 'animate-spin' : ''}" />
            </button>
          </div>
        {/each}
      </div>
    {/if}
    {#if files.length > 0}
      <div class="flex flex-wrap gap-1.5 {images.length > 0 ? 'mt-2' : ''}">
        {#each files as attachment (attachment.id)}
          <button
            type="button"
            data-testid="file-chip"
            data-id={attachment.id}
            class="group/file flex h-8 max-w-[16rem] items-center gap-2 rounded-[0.5rem] px-2.5 text-left hairline disabled:cursor-default"
            disabled={!messageId}
            onmouseenter={(e) => showTooltip(e.currentTarget, t('conversation.attachment.preview'))}
            onclick={(e) => preview(attachment, !e.metaKey)}
          >
            <Icon icon={busy(attachment) ? 'lucide:loader-circle' : getAttachmentIcon(attachment.content_type)} class="shrink-0 text-[14px] text-ink-3 {busy(attachment) ? 'animate-spin' : ''}" />
            <span class="min-w-0 truncate text-[13px] text-ink group-enabled/file:group-hover/file:underline">{attachment.name}</span>
            <span class="shrink-0 font-mono text-[11px] text-ink-3">{formatSize(attachment.size)}</span>
          </button>
        {/each}
      </div>
    {/if}
    {#if failure}
      <p data-testid="attachment-error" class="mt-1 text-[12px] text-error-500">{failure}</p>
    {/if}
  </div>
{/if}
