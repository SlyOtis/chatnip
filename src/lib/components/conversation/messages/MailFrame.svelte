<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { openLink } from '$lib/messageLink'
  import { renderEmojiHtml, workspaceEmoji } from '$lib/emoji'
  import type { Attachment } from '$lib/tauri/types'
  import { lightSurface, mailBackgrounds, mailDocument, prepareMail } from '$lib/content'
  import { layoutSettings, messageSetting } from '$lib/layoutSettings'
  import { t } from '$lib/i18n.svelte'

  const MAIL_MIN_WIDTH_REM = 24

  interface Props {
    html: string
    attachments: Attachment[]
    remote: boolean
    quote: boolean
    blocked?: number
    foldable?: boolean
    providerId?: string | null
  }

  let { html, attachments, remote, quote, blocked = $bindable(0), foldable = $bindable(false), providerId = null }: Props = $props()

  let host = $state<HTMLDivElement | null>(null)
  let box = $state<HTMLDivElement | null>(null)
  let frame = $state<HTMLIFrameElement | null>(null)
  let height = $state(0)
  let natural = $state(0)
  let hostWidth = $state(0)
  let loaded = $state(0)
  let palette = $state({ dark: false, ink: '#1d1a17', accent: '#2f4fd8' })
  let light = $state(true)

  const cids = $derived(Object.fromEntries(attachments.filter((a) => a.content_id && a.url).map((a) => [a.content_id as string, a.url as string])))
  const prepared = $derived.by(() => {
    $workspaceEmoji
    return prepareMail(renderEmojiHtml(html, providerId), { cids, remote })
  })
  const inverted = $derived(palette.dark && light && messageSetting($layoutSettings, 'messages.save_eyes') === 'on')
  const card = $derived(palette.dark ? 'bg-[#fbfaf7] shadow-[0_0_0_1px_rgb(0_0_0/0.06),0_1px_3px_rgb(0_0_0/0.18)]' : 'bg-white hairline')
  const srcdoc = $derived(mailDocument(prepared, { remote, dark: false, ink: '#1d1a17', accent: palette.dark ? '#2f5bd8' : palette.accent, quote: untrack(() => quote) }))
  const scale = $derived(natural && hostWidth ? Math.min(1, Math.max(hostWidth, MAIL_MIN_WIDTH_REM * rootFontSize()) / natural) : 1)

  $effect(() => {
    blocked = remote ? 0 : prepared.remote - (quote ? 0 : prepared.folded)
    foldable = prepared.foldable
  })

  function readPalette() {
    if (!host) return
    const style = getComputedStyle(host)
    const channels = (style.color.match(/[\d.]+/g) ?? ['0', '0', '0']).slice(0, 3).map(Number)
    const luminance = (0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]) / 255
    const accent = style.getPropertyValue('--accent').trim() || '#2f4fd8'
    const next = { dark: luminance > 0.5, ink: style.color, accent }
    if (next.dark !== palette.dark || next.ink !== palette.ink || next.accent !== palette.accent) palette = next
  }

  function rootFontSize(): number {
    return parseFloat(getComputedStyle(document.documentElement).fontSize) || 16
  }

  function measure() {
    const doc = frame?.contentDocument
    if (!doc?.body || !frame) return
    frame.style.width = `${hostWidth}px`
    const wide = Math.ceil(Math.max(doc.documentElement.scrollWidth, doc.body.scrollWidth))
    const width = Math.max(hostWidth, wide)
    frame.style.width = `${width}px`
    if (width !== natural) natural = width
    const next = Math.ceil(Math.max(doc.body.scrollHeight, doc.body.getBoundingClientRect().height))
    if (next !== height) height = next
  }

  function open(event: MouseEvent) {
    const target = event.target as Element | null
    const link = target?.closest?.('a[href]')
    if (!link) return
    event.preventDefault()
    const href = link.getAttribute('href') ?? ''
    openLink(href).catch(() => {})
  }

  function attach() {
    const doc = frame?.contentDocument
    if (!doc) return
    doc.documentElement.classList.toggle('cn-quote', quote)
    doc.addEventListener('click', open)
    light = lightSurface(mailBackgrounds(doc))
    loaded += 1
    measure()
  }

  $effect(() => {
    const doc = frame?.contentDocument
    if (!loaded || !doc?.body) return
    const view = doc.defaultView
    const observer = new MutationObserver(measure)
    observer.observe(doc.body, { childList: true, subtree: true, characterData: true, attributes: true })
    doc.querySelectorAll('img').forEach((img) => img.addEventListener('load', measure, { once: true }))
    view?.addEventListener('resize', measure)
    return () => {
      observer.disconnect()
      view?.removeEventListener('resize', measure)
    }
  })

  $effect(() => {
    const on = inverted
    if (!loaded) return
    frame?.contentDocument?.documentElement.classList.toggle('cn-invert', on)
  })

  $effect(() => {
    const on = quote
    if (!loaded) return
    frame?.contentDocument?.documentElement.classList.toggle('cn-quote', on)
    measure()
  })

  onMount(() => {
    readPalette()
    const observer = new MutationObserver(readPalette)
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class', 'data-theme', 'style'] })
    const fit = () => {
      const next = Math.floor(box?.clientWidth ?? 0)
      if (!next || next === hostWidth) return
      hostWidth = next
      requestAnimationFrame(measure)
    }
    fit()
    window.addEventListener('resize', fit)
    return () => {
      observer.disconnect()
      window.removeEventListener('resize', fit)
    }
  })
</script>

<div bind:this={host} class="min-w-0 rounded-[0.75rem] p-[clamp(0.75rem,3%,1.25rem)] text-ink {inverted ? 'bg-[#ebe8e2] [filter:invert(1)_hue-rotate(180deg)]' : card}">
  <div bind:this={box} data-testid="mail-frame" data-scale={scale.toFixed(3)} class="overflow-x-auto overflow-y-hidden" style:height="{Math.ceil(height * scale)}px">
  <iframe
    bind:this={frame}
    title={t('conversation.mail.content')}
    class="block origin-top-left border-0 bg-transparent"
    style:width="{natural || hostWidth}px"
    style:height="{height}px"
    style:transform={scale < 1 ? `scale(${scale})` : undefined}
    sandbox="allow-same-origin allow-popups allow-popups-to-escape-sandbox"
    referrerpolicy="no-referrer"
    {srcdoc}
    onload={attach}
  ></iframe>
  </div>
</div>
