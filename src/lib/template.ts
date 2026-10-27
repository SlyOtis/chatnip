import { SvelteSet } from 'svelte/reactivity'
import type { Message, Presentation, PresentationKind } from '$lib/tauri/types'
import type { Invite } from '$lib/messageKind'
import { prepareMail } from '$lib/content'
import { isHtml } from '$lib/links'
import { messageLayoutKey } from '$lib/layoutSettings'
import { renderEmojiHtml } from '$lib/emoji'

export type TemplateKind = Exclude<PresentationKind, 'auto' | 'hidden'>

export interface TemplateDiagnostic {
  message: string
  line: number | null
  column: number | null
}

export type CompiledTemplate = { ok: true; js: string; css: string | null } | { ok: false; diagnostics: TemplateDiagnostic[] }

export interface TemplateProps {
  message: {
    id: string
    type: string
    subject: string | null
    text: string
    html: string | null
    mail: string | null
    sender: string
    sender_email: string | null
    at: string
  }
  presentation: Presentation
  invite: Invite | null
  theme: 'day' | 'night'
}

export const THEME_TOKENS = ['--paper', '--paper-2', '--paper-3', '--ink', '--ink-2', '--ink-3', '--line', '--line-strong', '--accent', '--accent-ink', '--accent-soft', '--signal', '--font-sans', '--font-mono']

const ALLOWED_IMPORT = /^svelte\/internal\/(client|disclose-version|flags\/[a-z-]+)$/

export function importsOf(js: string): string[] {
  return [...js.matchAll(/(?:^|[;\n])\s*import\s+(?:[^'"]*?\s+from\s+)?['"]([^'"]+)['"]/g)].map((m) => m[1])
}

const compiled = new Map<string, Promise<CompiledTemplate>>()

export function compileTemplate(source: string): Promise<CompiledTemplate> {
  let pending = compiled.get(source)
  if (!pending) {
    pending = compileSource(source)
    compiled.set(source, pending)
  }
  return pending
}

async function compileSource(source: string): Promise<CompiledTemplate> {
  if (/^\s*import\s/m.test(source.match(/<script[^>]*>([\s\S]*?)<\/script>/)?.[1] ?? '')) {
    return { ok: false, diagnostics: [{ message: 'Templates cannot import anything; everything they need arrives as props.', line: null, column: null }] }
  }
  try {
    const { compile } = await import('svelte/compiler')
    const result = compile(source, { generate: 'client', runes: true, css: 'injected', dev: false, filename: 'Template.svelte' })
    const banned = importsOf(result.js.code).filter((specifier) => !ALLOWED_IMPORT.test(specifier))
    if (banned.length) return { ok: false, diagnostics: banned.map((specifier) => ({ message: `Templates cannot import ${specifier}`, line: null, column: null })) }
    return { ok: true, js: result.js.code, css: result.css?.code ?? null }
  } catch (error) {
    const e = error as { message?: string; start?: { line: number; column: number } }
    return { ok: false, diagnostics: [{ message: e.message ?? String(error), line: e.start?.line ?? null, column: e.start?.column ?? null }] }
  }
}

function base64(text: string): string {
  const bytes = new TextEncoder().encode(text)
  let binary = ''
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
  return btoa(binary)
}

export function moduleUrl(code: string): string {
  return `data:text/javascript;base64,${base64(code)}`
}

export function templateDocument(js: string, runtime: string, tokens: Record<string, string>): string {
  const imports: Record<string, string> = {
    'svelte/internal/client': moduleUrl(runtime),
    'svelte/internal/disclose-version': 'data:text/javascript,',
    'svelte/internal/flags/legacy': 'data:text/javascript,',
    'svelte/internal/flags/tracing': 'data:text/javascript,',
    'svelte/internal/flags/async': 'data:text/javascript,'
  }
  const vars = Object.entries(tokens).map(([k, v]) => `${k}:${v}`).join(';')
  const boot = `
import Template from ${JSON.stringify(moduleUrl(js))};
import { mount, unmount } from 'svelte/internal/client';
const root = document.getElementById('root');
let app = null;
const report = (message) => parent.postMessage({ type: 'error', message: String(message) }, '*');
const size = () => parent.postMessage({ type: 'height', height: Math.ceil(document.documentElement.getBoundingClientRect().height) }, '*');
window.addEventListener('error', (e) => report(e.message));
window.addEventListener('unhandledrejection', (e) => report(e.reason));
window.addEventListener('message', (e) => {
  if (!e.data || e.data.type !== 'props') return;
  try {
    if (app) unmount(app);
    app = mount(Template, { target: root, props: e.data.props });
    requestAnimationFrame(size);
  } catch (error) {
    report(error && error.message ? error.message : error);
  }
});
new MutationObserver(() => requestAnimationFrame(size)).observe(document.body, { childList: true, subtree: true, characterData: true, attributes: true });
window.addEventListener('resize', size);
document.addEventListener('load', size, true);
document.addEventListener('click', (e) => {
  const link = e.target && e.target.closest ? e.target.closest('a[href]') : null;
  if (!link) return;
  e.preventDefault();
  parent.postMessage({ type: 'open', url: link.href }, '*');
});
parent.postMessage({ type: 'ready' }, '*');
`
  const csp = "default-src 'none'; script-src 'unsafe-inline' data:; style-src 'unsafe-inline'; img-src data:; font-src data:"
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${csp}"><script type="importmap">${JSON.stringify({ imports })}</script><style>:root{${vars}}html,body{margin:0;padding:0;background:transparent;color:var(--ink);font:14px/1.5 var(--font-sans);overflow:hidden}</style></head><body><div id="root"></div><script type="module">${boot}</script></body></html>`
}

export function templateProps(message: Message, presentation: Presentation, invite: Invite | null, theme: 'day' | 'night'): TemplateProps {
  const html = isHtml(message.content_type) ? message.content : null
  const cids = Object.fromEntries((message.attachments ?? []).filter((a) => a.content_id && a.url).map((a) => [a.content_id as string, a.url as string]))
  const mail = html ? prepareMail(renderEmojiHtml(html, message.provider_id), { cids, remote: false }).html : null
  const text = (message.content ?? '').replace(/<[^>]+>/g, ' ').replace(/&nbsp;/g, ' ').replace(/\s+/g, ' ').trim()
  return JSON.parse(
    JSON.stringify({
      message: { id: message.id, type: message.type, subject: message.subject ?? null, text, html, mail, sender: message.sender_name ?? message.sender_email ?? '', sender_email: message.sender_email ?? null, at: message.timestamp },
      presentation,
      invite,
      theme
    })
  )
}

export type RenderPlan = { use: 'builtin' } | { use: 'template'; action_id: string; node_id: string } | { use: 'layout'; kind: TemplateKind; source: string }

export function renderPlan(presentation: Presentation, failed: ReadonlySet<string>, layouts: Record<string, string> = {}, kind: TemplateKind | null = null): RenderPlan {
  const ref = presentation.template
  if (ref && ref.action_id && ref.node_id && !failed.has(`${ref.action_id}:${ref.node_id}`)) return { use: 'template', action_id: ref.action_id, node_id: ref.node_id }
  const source = kind ? layouts[messageLayoutKey(kind)]?.trim() : ''
  if (kind && source && !failed.has(`layout:${kind}`)) return { use: 'layout', kind, source }
  return { use: 'builtin' }
}

export const failedTemplates = new SvelteSet<string>()
const sources = new Map<string, Promise<string>>()

export function templateSourceOf(ref: { action_id: string; node_id: string }, load: (action_id: string, node_id: string) => Promise<string>): Promise<string> {
  const key = `${ref.action_id}:${ref.node_id}`
  let pending = sources.get(key)
  if (!pending) {
    pending = load(ref.action_id, ref.node_id)
    sources.set(key, pending)
    pending.catch(() => sources.delete(key))
  }
  return pending
}

export function failTemplate(ref: { action_id: string; node_id: string }, report: () => Promise<unknown>): void {
  const key = `${ref.action_id}:${ref.node_id}`
  if (failedTemplates.has(key)) return
  failedTemplates.add(key)
  report().catch(() => {})
}

export function forgetTemplates(): void {
  sources.clear()
  failedTemplates.clear()
}

const STYLE = `
  .card { display: grid; gap: 0.25rem; padding: 0.75rem 1rem; border-radius: 0.75rem; background: var(--paper-2); color: var(--ink); }
  .meta { color: var(--ink-3); font: 12px/1.4 var(--font-mono); }
  .title { font-weight: 600; font-size: 15px; }
  .body { color: var(--ink-2); white-space: pre-wrap; overflow-wrap: anywhere; }
  a { color: var(--accent); }`

export const DEFAULT_TEMPLATES: Record<TemplateKind, string> = {
  invite: `<script>
  let { message, invite } = $props()
  const start = invite?.starts_at ? new Date(invite.starts_at) : null
  const end = invite?.ends_at ? new Date(invite.ends_at) : null
  const time = (d) => d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })
  const when = start && !isNaN(start) ? start.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' }) + ' · ' + time(start) + (end && !isNaN(end) ? '–' + time(end) : '') : ''
</script>

<article class="card">
  <span class="title">{invite?.title || message.subject || 'Invitation'}</span>
  {#if when}<span class="meta">{when}</span>{/if}
  {#if invite?.location}<span class="body">{invite.location}</span>{/if}
  {#if invite?.organizer}<span class="meta">from {invite.organizer}</span>{/if}
  {#if invite?.join_url}<a href={invite.join_url}>Join the meeting</a>{/if}
</article>

<style>${STYLE}
</style>
`,
  chat: `<script>
  let { message } = $props()
</script>

<div class="bubble">{message.text}</div>

<style>
  .bubble { display: inline-block; max-width: min(68ch, 80%); padding: 0.5rem 0.875rem; border-radius: 1.125rem; background: var(--paper-2); color: var(--ink); overflow-wrap: anywhere; }
</style>
`,
  mail: `<script>
  let { message } = $props()
</script>

<article class="card">
  {#if message.subject}<span class="title">{message.subject}</span>{/if}
  <span class="meta">{message.sender}</span>
  {#if message.mail}
    <div class="mail">{@html message.mail}</div>
  {:else}
    <p class="body">{message.text}</p>
  {/if}
</article>

<style>${STYLE}
  .mail { color: var(--ink); overflow-wrap: anywhere; }
  .mail img { max-width: 100%; height: auto; }
  .mail table { max-width: 100%; }
</style>
`,
  notification: `<script>
  let { message, presentation } = $props()
</script>

<p class="line">{presentation.badge ? presentation.badge + ' · ' : ''}{message.subject || message.text}</p>

<style>
  .line { margin: 0; color: var(--ink-3); font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
`,
  bot: `<script>
  let { message } = $props()
</script>

<div class="card">
  <span class="meta">{message.sender}</span>
  <p class="body">{message.text}</p>
</div>

<style>${STYLE}
</style>
`,
  digest: `<script>
  let { message, presentation } = $props()
</script>

<article class="card">
  <span class="title">{message.subject || message.sender}</span>
  <p class="body">{presentation.summary_only ? message.text.slice(0, 280) : message.text}</p>
</article>

<style>${STYLE}
</style>
`,
  system: `<script>
  let { message } = $props()
</script>

<p class="system">{message.text}</p>

<style>
  .system { margin: 0; text-align: center; color: var(--ink-3); font-size: 12px; }
</style>
`,
  media: `<script>
  let { message } = $props()
</script>

<p class="meta">{message.sender} sent media</p>

<style>${STYLE}
</style>
`
}

export function defaultTemplate(kind: string): string {
  return DEFAULT_TEMPLATES[(kind in DEFAULT_TEMPLATES ? kind : 'mail') as TemplateKind]
}

export const SAMPLE_PROPS: Record<TemplateKind, TemplateProps> = Object.fromEntries(
  (Object.keys(DEFAULT_TEMPLATES) as TemplateKind[]).map((kind) => [
    kind,
    {
      message: { id: 'sample', type: kind === 'mail' || kind === 'invite' || kind === 'digest' ? 'email' : 'chat', subject: kind === 'invite' ? 'Sprintdemo' : 'Avtale om plattformdrift 2027', text: 'Vi tar demo av sprint 31 på fredag. Ta med det dere har, også det som ikke er ferdig.', html: null, mail: kind === 'mail' ? '<p>Vi tar demo av sprint 31 på fredag.</p><p>Ta med det dere har, også det som ikke er ferdig.</p>' : null, sender: 'Lea Lindqvist', sender_email: 'lea@client.example', at: '2026-10-11T19:00:00Z' },
      presentation: { kind, badge: kind === 'notification' ? 'Service' : undefined },
      invite: kind === 'invite' ? { title: 'Sprintdemo', starts_at: '2026-10-16T12:00:00Z', ends_at: '2026-10-16T13:00:00Z', location: 'Microsoft Teams-møte', organizer: 'Lea Lindqvist', join_url: 'https://teams.microsoft.com/l/meetup-join/sample' } : null,
      theme: 'day'
    }
  ])
) as Record<TemplateKind, TemplateProps>
