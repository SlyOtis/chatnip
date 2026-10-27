import type { Conversation, Delivery, Message, Provider } from '$lib/tauri/types'
import { get } from 'svelte/store'
import { plainText } from '$lib/messageKind'
import { renderEmojiText } from '$lib/emoji'
import { messagesStore, providersStore, readStore } from '$lib/tauri'
import { modalsShow } from '$lib/modals'
import { pulse } from '$lib/window'
import { accountOf } from '$lib/accounts'

export interface ReplyPreview {
  id: string
  author: string
  excerpt: string
  attachments: number
  attachmentNames: string[]
  mail: boolean
}

export type ReplyMode = 'chat' | 'mail' | null

export const replyState = $state<{ replying: Message | null }>({ replying: null })
export const forwarding = $state<{ messages: Message[]; comment: string; targets: string[] }>({ messages: [], comment: '', targets: [] })
export const selection = $state<{ ids: string[] }>({ ids: [] })
export const pointed = $state<{ message: Message | null }>({ message: null })
export const highlighted = $state<{ id: string | null }>({ id: null })
export const editRequest = $state<{ id: string | null }>({ id: null })

export function excerptOf(content: string | null | undefined, limit = 140): string {
  const flat = plainText(content).replace(/\s+/g, ' ').trim()
  return flat.length > limit ? `${flat.slice(0, limit).trimEnd()}…` : flat
}

export function replyPreview(message: Message, unknown = ''): ReplyPreview {
  const attachments = (message.attachments ?? []).filter((a) => !a.is_inline)
  return {
    id: message.id,
    author: message.sender_name || message.sender_email || unknown,
    excerpt: renderEmojiText(excerptOf(message.content)),
    attachments: attachments.length,
    attachmentNames: attachments.map((a) => a.name),
    mail: message.type === 'email'
  }
}

export function replyMode(message: Message | null, provider: Provider | undefined, read: boolean): ReplyMode {
  if (!message || read || message.type === 'system') return null
  if (message.type === 'email') return 'mail'
  if (provider && !provider.capabilities.can_send_messages) return null
  return 'chat'
}

export function canForward(message: Message | null): boolean {
  return !!message && message.type !== 'system'
}

export function quotedReply(message: Message): boolean {
  return !!message.reply_to_id && !message.thread_id
}

export interface EmbeddedQuote {
  author: string
  text: string
  content: string
}

const MARKDOWN_QUOTE = /^>\s?\*\*(.+?)\*\*:?\s*(.*)$/

function blank(node: Node): boolean {
  if (node.nodeType === Node.TEXT_NODE) return !(node.textContent ?? '').trim()
  if (!(node instanceof Element)) return true
  if (node.tagName === 'BR') return true
  return !node.matches('img, video, picture, svg, table') && !node.querySelector('img, video, picture, svg, table') && !(node.textContent ?? '').trim()
}

function trimLeading(node: Node) {
  for (let first = node.firstChild; first; first = node.firstChild) {
    if (blank(first)) {
      first.remove()
      continue
    }
    if (first instanceof Element) trimLeading(first)
    return
  }
}

export function embeddedQuote(content: string | null | undefined): EmbeddedQuote | null {
  const text = content ?? ''
  const lines = text.split('\n')
  const head = lines[0]?.match(MARKDOWN_QUOTE)
  if (head) {
    let end = 1
    while (end < lines.length && lines[end].startsWith('>')) end++
    const quoted = [head[2], ...lines.slice(1, end).map((line) => line.replace(/^>\s?/, ''))].join(' ').trim()
    while (end < lines.length && !lines[end].trim()) end++
    return { author: head[1].trim(), text: quoted, content: lines.slice(end).join('\n') }
  }
  if (!text.includes('reply-quote')) return null
  const holder = document.createElement('div')
  holder.innerHTML = text
  const quote = holder.querySelector('blockquote.reply-quote')
  if (!quote) return null
  const author = quote.querySelector('strong')?.textContent?.trim() ?? ''
  const quoted = Array.from(quote.querySelectorAll('p'), (p) => p.textContent ?? '').join(' ').replace(/\s+/g, ' ').trim()
  quote.remove()
  trimLeading(holder)
  return { author, text: quoted, content: holder.innerHTML }
}

export function quoteMatches(quote: EmbeddedQuote, original: Message | null): boolean {
  if (!original) return true
  const same = (name: string | null | undefined) => !!name && name.trim().toLowerCase() === quote.author.toLowerCase()
  const head = quote.text.replace(/(…|\.\.\.)$/, '').replace(/\s+/g, ' ').trim().slice(0, 40)
  return same(original.sender_name) || same(original.sender_email) || (!!head && excerptOf(original.content, Number.MAX_SAFE_INTEGER).includes(head))
}

export function loadedOriginal(message: Message, loaded: Message[]): Message | null {
  return loaded.find((m) => m.id !== message.id && (m.id === message.reply_to_id || (m.provider_id === message.provider_id && m.external_id === message.reply_to_id))) ?? null
}

export function replyBody(message: Message, original: Message | null): { message: Message; quote: EmbeddedQuote | null } {
  if (!quotedReply(message)) return { message, quote: null }
  const quote = embeddedQuote(message.content)
  if (!quote || !quoteMatches(quote, original)) return { message, quote: null }
  return { message: { ...message, content: quote.content }, quote }
}

export function startReply(message: Message) {
  replyState.replying = message
}

export function clearReply() {
  replyState.replying = null
}

export function orderForForward(messages: Message[]): Message[] {
  const seen = new Set<string>()
  return messages.filter((m) => !seen.has(m.id) && !!seen.add(m.id)).sort((a, b) => a.timestamp.localeCompare(b.timestamp))
}

export function openForward(messages: Message[], comment = '', targets: string[] = []) {
  forwarding.messages = orderForForward(messages)
  forwarding.comment = comment
  forwarding.targets = targets
}

export function closeForward() {
  forwarding.messages = []
  forwarding.comment = ''
  forwarding.targets = []
  selection.ids = []
}

export function toggleSelected(...ids: string[]) {
  selection.ids = ids.every((id) => selection.ids.includes(id)) ? selection.ids.filter((x) => !ids.includes(x)) : [...selection.ids, ...ids.filter((id) => !selection.ids.includes(id))]
}

export function clearSelection() {
  selection.ids = []
  highlighted.id = null
}

export function forwardSet(focus: Message, loaded: Message[], selected: string[]): Message[] {
  if (!selected.length) return [focus]
  const picked = loaded.filter((m) => selected.includes(m.id))
  return orderForForward(picked.some((m) => m.id === focus.id) ? picked : [...picked, focus])
}

export function deliveryFor(sources: Message[], target: Conversation | undefined): Delivery {
  if (!target) return 'unsupported'
  if (target.type === 'email') return 'mail'
  const provider = target.providers[0]
  if (provider && provider.capabilities.forward === 'native' && sources.length > 0 && sources.every((m) => m.provider_id === provider.id)) return 'native'
  return 'repost'
}

export function scrollToMessage(id: string): boolean {
  const element = document.querySelector<HTMLElement>(`[data-message-id="${CSS.escape(id)}"]`)
  if (!element) return false
  element.scrollIntoView({ block: 'center', behavior: 'smooth' })
  pulse(element)
  return true
}

function focusComposer() {
  document.querySelector<HTMLElement>('[data-composer]')?.focus()
}

function pointedIn(loaded: Message[]): Message | null {
  const message = pointed.message
  return message && loaded.some((m) => m.id === message.id) ? message : null
}

export function replyTo(message: Message, all = false) {
  const provider = accountOf(get(providersStore).data, message.provider_id) ?? undefined
  const mode = replyMode(message, provider, get(readStore).has(message.conversation_id))
  if (mode === 'mail') {
    modalsShow({ mode: all ? 'reply_all' : 'reply', original_message: message, conversation_id: message.conversation_id, provider_id: message.provider_id })
    return
  }
  if (mode === 'chat' && !all) {
    startReply(message)
    focusComposer()
  }
}

export function shortcutReply(all = false) {
  const loaded = get(messagesStore).data ?? []
  const message = pointedIn(loaded) ?? (all ? [...loaded].reverse().find((m) => m.type === 'email') ?? null : null)
  if (message) return replyTo(message, all)
  if (!all) focusComposer()
}

export function shortcutForward() {
  const loaded = get(messagesStore).data ?? []
  const focus = pointedIn(loaded) ?? loaded.find((m) => selection.ids.includes(m.id)) ?? null
  if (focus && canForward(focus)) openForward(forwardSet(focus, loaded, selection.ids))
}

export function moveHighlight(step: number, loaded: Message[]) {
  if (loaded.length === 0) return
  const index = loaded.findIndex((m) => m.id === highlighted.id)
  const next = loaded[index < 0 ? (step > 0 ? 0 : loaded.length - 1) : Math.max(0, Math.min(loaded.length - 1, index + step))]
  highlighted.id = next.id
  pointed.message = next
  requestAnimationFrame(() => document.querySelector<HTMLElement>('[data-lit]')?.scrollIntoView({ block: 'nearest' }))
}
