<script lang="ts">
  import Icon from '@iconify/svelte'
  import { convertFileSrc } from '@tauri-apps/api/core'
  import type { LinkPreview } from '$lib/tauri/types'
  import { domain, getPreview, isWide, previewMeta } from '$lib/links'
  import { showTooltip } from '$lib/tooltip'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    url: string
    ondismiss?: () => void
    claim?: (key: string) => boolean
  }

  let { url, ondismiss, claim }: Props = $props()

  let preview = $state<LinkPreview | null>(null)
  let status = $state<'loading' | 'ready' | 'hidden' | 'error'>('loading')
  let imageBroken = $state(false)
  let iconBroken = $state(false)

  $effect(() => {
    const target = url
    let live = true
    status = 'loading'
    preview = null
    imageBroken = false
    iconBroken = false
    getPreview(target)
      .then((result) => {
        if (!live) return
        preview = result
        status = result && (!claim || !(result.title || result.image_url) || claim(`${result.title ?? ''}|${result.image_url ?? ''}`)) ? 'ready' : 'hidden'
      })
      .catch(() => {
        if (live) status = 'error'
      })
    return () => {
      live = false
    }
  })

  const host = $derived(domain(url))
  const image = $derived(preview?.image ? convertFileSrc(preview.image) : (preview?.image_url ?? null))
  const favicon = $derived(preview?.favicon ? convertFileSrc(preview.favicon) : null)
  const wide = $derived(preview ? isWide(preview) : false)
  const meta = $derived(preview ? previewMeta(preview) : '')
  const title = $derived(preview?.title || preview?.file_name || null)
  const ratio = $derived(preview?.image_width && preview?.image_height ? `${preview.image_width} / ${preview.image_height}` : '16 / 9')
  const stateTone: Record<string, string> = { open: 'text-success-600', draft: 'text-ink-3', merged: 'text-accent', closed: 'text-error-600', not_planned: 'text-ink-3', archived: 'text-ink-3' }
  const kindIcon: Record<string, string> = { pdf: 'lucide:file-text', pull: 'lucide:git-pull-request', issue: 'lucide:circle-dot', repo: 'lucide:book-marked', audio: 'lucide:music', post: 'lucide:message-square-quote' }
</script>

{#if status === 'error'}
  <a href={url} target="_blank" rel="noopener noreferrer" class="inline-flex max-w-full items-center gap-1.5 text-[13px] text-ink-3 hover:text-ink">
    <Icon icon="lucide:link" class="shrink-0 text-[13px]" />
    <span class="truncate">{host}</span>
  </a>
{:else if status !== 'hidden'}
  <div class="group/link relative flex w-full gap-3 border-l-2 py-0.5 pr-7 pl-3 {preview?.theme_color ? '' : 'border-line'}" style:border-left-color={preview?.theme_color ?? undefined} aria-busy={status === 'loading'}>
    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
      <div class="flex min-w-0 items-center gap-1.5 text-[12px] text-ink-3">
        {#if favicon && !iconBroken}
          <img src={favicon} alt="" class="size-4 shrink-0 rounded-[0.2rem] object-contain" onerror={() => (iconBroken = true)} />
        {:else}
          <Icon icon={(preview && kindIcon[preview.kind]) || 'lucide:globe'} class="size-4 shrink-0" />
        {/if}
        <span class="truncate">{preview?.site_name || host}</span>
        {#if meta}
          <span aria-hidden="true">·</span>
          <span class="truncate {preview?.state ? (stateTone[preview.state] ?? '') : ''}">{meta}</span>
        {/if}
      </div>
      {#if status === 'loading'}
        <div class="mt-1 h-3.5 w-3/4 rounded-[0.25rem] bg-paper-3"></div>
        <div class="mt-1 h-3 w-11/12 rounded-[0.25rem] bg-paper-3"></div>
      {:else}
        {#if title}
          <a href={url} target="_blank" rel="noopener noreferrer" class="line-clamp-2 text-[14px] leading-snug font-semibold text-ink hover:underline">{title}</a>
        {:else}
          <a href={url} target="_blank" rel="noopener noreferrer" class="truncate text-[14px] font-semibold text-ink hover:underline">{host}</a>
        {/if}
        {#if preview?.description}
          <p class="line-clamp-2 text-[13px] leading-snug text-ink-3">{preview.description}</p>
        {/if}
        {#if wide && image && !imageBroken}
          <a href={url} target="_blank" rel="noopener noreferrer" class="relative mt-1.5 block w-full max-w-[22.5rem] overflow-hidden rounded-[0.5rem] bg-paper-2" style:aspect-ratio={ratio} aria-label={t('conversation.link.open', { name: title ?? host })}>
            <img src={image} alt="" class="size-full object-cover" loading="lazy" onerror={() => (imageBroken = true)} />
            {#if preview?.kind === 'video'}
              <span class="absolute inset-0 flex items-center justify-center">
                <span class="flex size-11 items-center justify-center rounded-full bg-ink/60 text-paper">
                  <Icon icon="lucide:play" class="ml-0.5 fill-current text-[18px]" />
                </span>
              </span>
            {/if}
          </a>
        {/if}
      {/if}
    </div>
    {#if status === 'loading'}
      <div class="size-24 shrink-0 rounded-[0.5rem] bg-paper-3"></div>
    {:else if !wide && image && !imageBroken}
      <a href={url} target="_blank" rel="noopener noreferrer" class="size-24 shrink-0 overflow-hidden rounded-[0.5rem] bg-paper-2" tabindex="-1" aria-hidden="true">
        <img src={image} alt="" class="size-full object-cover" loading="lazy" onerror={() => (imageBroken = true)} />
      </a>
    {/if}
    {#if ondismiss}
      <button
        type="button"
        class="absolute top-0 right-0 flex size-6 items-center justify-center rounded-[0.375rem] text-ink-3 opacity-0 group-hover/link:opacity-100 hover:bg-paper-3 hover:text-ink focus-visible:opacity-100"
        aria-label={t('conversation.link.hide')}
        onmouseenter={(e) => showTooltip(e.currentTarget, t('conversation.link.hide'))}
        onclick={ondismiss}
      >
        <Icon icon="lucide:x" class="text-[14px]" />
      </button>
    {/if}
  </div>
{/if}
