<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener'
  import runtime from '$lib/generated/svelte-runtime.js?raw'
  import { compileTemplate, templateDocument, THEME_TOKENS, type TemplateDiagnostic, type TemplateProps } from '$lib/template'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    source: string
    props: TemplateProps
    onerror?: (message: string) => void
    oncompiled?: (diagnostics: TemplateDiagnostic[]) => void
  }

  let { source, props, onerror, oncompiled }: Props = $props()

  let host = $state<HTMLDivElement | null>(null)
  let frame = $state<HTMLIFrameElement | null>(null)
  let srcdoc = $state('')
  let height = $state(0)
  let ready = $state(false)

  function tokens(): Record<string, string> {
    if (!host) return {}
    const style = getComputedStyle(host)
    return Object.fromEntries(THEME_TOKENS.map((token) => [token, style.getPropertyValue(token).trim()]).filter(([, value]) => value))
  }

  $effect(() => {
    const current = source
    const theme = props.theme
    let cancelled = false
    ready = false
    compileTemplate(current).then((compiled) => {
      if (cancelled || theme !== props.theme) return
      if (!compiled.ok) {
        srcdoc = ''
        oncompiled?.(compiled.diagnostics)
        onerror?.(compiled.diagnostics.map((d) => (d.line ? `${d.line}:${d.column ?? 0} ${d.message}` : d.message)).join('\n'))
        return
      }
      oncompiled?.([])
      srcdoc = templateDocument(compiled.js, runtime, tokens())
    })
    return () => {
      cancelled = true
    }
  })

  $effect(() => {
    const payload = $state.snapshot(props)
    if (ready && frame?.contentWindow) frame.contentWindow.postMessage({ type: 'props', props: payload }, '*')
  })

  function receive(event: MessageEvent) {
    if (!frame || event.source !== frame.contentWindow) return
    const data = event.data as { type?: string; height?: number; url?: string; message?: string }
    if (data.type === 'ready') ready = true
    else if (data.type === 'height' && typeof data.height === 'number') height = data.height
    else if (data.type === 'open' && typeof data.url === 'string' && /^(https?:|mailto:)/i.test(data.url)) openUrl(data.url).catch(() => {})
    else if (data.type === 'error') onerror?.(String(data.message ?? t('conversation.template.failed')))
  }
</script>

<svelte:window onmessage={receive} />

<div bind:this={host} class="min-w-0">
  {#if srcdoc}
    <iframe bind:this={frame} title={t('conversation.template.title')} sandbox="allow-scripts" {srcdoc} class="block w-full border-0 bg-transparent" style:height="{height}px"></iframe>
  {/if}
</div>
