import type { Presentation } from '$lib/tauri/types'
import { t } from '$lib/i18n.svelte'
import { canonicalEmoji, findShortcodes, getCustom, renderEmojiText } from '$lib/emoji'

export type MessageKind = 'email' | 'chat' | 'channel' | 'bot' | 'system' | 'invite' | 'media' | 'emoji' | 'hidden'

export interface KindSource {
  type: string
  content: string | null
  content_type: string | null
  metadata: string | null
  has_attachments?: boolean
  attachments?: { content_type: string }[]
  presentation?: Presentation | null
}

export interface KindContext {
  conversationType?: string | null
  senderHuman?: boolean | null
  conversationPresentation?: Presentation | null
}

export interface Reaction {
  emoji: string
  count: number
  users: string[]
  names: string[]
}

export interface Invite {
  title: string
  starts_at: string
  ends_at: string | null
  location: string
  organizer: string
  join_url?: string | null
}

export function parseMetadata(raw: string | null | undefined): Record<string, unknown> {
  if (!raw) return {}
  try {
    const value = JSON.parse(raw)
    return value && typeof value === 'object' && !Array.isArray(value) ? value : {}
  } catch {
    return {}
  }
}

export function plainText(content: string | null | undefined): string {
  if (!content) return ''
  return content
    .replace(/<style[\s\S]*?<\/style>/gi, ' ')
    .replace(/<script[\s\S]*?<\/script>/gi, ' ')
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/(p|div|li|tr|h[1-6])>/gi, '\n')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/[ \t]+/g, ' ')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .join('\n')
}

const PICTOGRAPHIC = /\p{Extended_Pictographic}/u
const NON_EMOJI = /[\p{L}\p{N}\p{P}]/u

function emojiText(content: string | null | undefined): string {
  const text = renderEmojiText(plainText(content))
  let out = ''
  let at = 0
  for (const hit of findShortcodes(text)) {
    if (!getCustom(hit.code)) continue
    out += text.slice(at, hit.start) + '🖼'
    at = hit.end
  }
  return out + text.slice(at)
}

export function emojiCount(content: string | null | undefined): number {
  const text = emojiText(content).replace(/\s+/g, '')
  if (!text) return 0
  const segments = [...new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text)].map((s) => s.segment)
  for (const segment of segments) {
    if (!PICTOGRAPHIC.test(segment) && !/^[\u{1F1E6}-\u{1F1FF}]{2}$/u.test(segment)) return 0
    if (NON_EMOJI.test(segment.replace(/\u200d|\ufe0f|\u20e3/g, '').replace(/\p{Extended_Pictographic}|\p{Emoji_Modifier}|[\u{1F1E6}-\u{1F1FF}]/gu, ''))) return 0
  }
  return segments.length
}

export function isEmojiOnly(content: string | null | undefined): boolean {
  const count = emojiCount(content)
  return count >= 1 && count <= 3
}

export function isMediaOnly(source: KindSource): boolean {
  const attachments = source.attachments ?? []
  if (attachments.length === 0) return false
  if (plainText(source.content).length > 0) return false
  return attachments.every((a) => a.content_type.startsWith('image/') || a.content_type.startsWith('video/'))
}

function icsValue(text: string, key: string): string | null {
  const unfolded = text.replace(/\r?\n[ \t]/g, '')
  const match = unfolded.match(new RegExp(`^${key}(?:;[^:\\n]*)?:(.*)$`, 'mi'))
  return match ? match[1].trim().replace(/\\,/g, ',').replace(/\\n/g, ' ') : null
}

function icsDate(value: string | null): string | null {
  if (!value) return null
  const m = value.match(/^(\d{4})(\d{2})(\d{2})(?:T(\d{2})(\d{2})(\d{2})(Z)?)?$/)
  if (!m) return null
  const [, y, mo, d, h = '00', mi = '00', s = '00', z] = m
  return `${y}-${mo}-${d}T${h}:${mi}:${s}${z ? 'Z' : ''}`
}

export function presented(source: Pick<KindSource, 'presentation'>, context: Pick<KindContext, 'conversationPresentation'> = {}): Presentation {
  const merged: Record<string, unknown> = { ...(context.conversationPresentation ?? {}) }
  for (const [key, value] of Object.entries(source.presentation ?? {})) {
    if (value === null || value === undefined) delete merged[key]
    else merged[key] = value
  }
  return merged as Presentation
}

const PRESENTED: Record<string, MessageKind> = { chat: 'chat', mail: 'email', digest: 'email', invite: 'invite', notification: 'bot', bot: 'bot', system: 'system', media: 'media', hidden: 'hidden' }

export function presentedKind(presentation: Presentation): MessageKind | null {
  return presentation.kind ? (PRESENTED[presentation.kind] ?? null) : null
}

export function inviteOf(source: KindSource, context: KindContext = {}): Invite | null {
  const meta = parseMetadata(source.metadata)
  const event = meta.event as Record<string, unknown> | undefined
  if (event && typeof event.starts_at === 'string') {
    return {
      title: String(event.title ?? t('conversation.invite.untitled')),
      starts_at: event.starts_at,
      ends_at: typeof event.ends_at === 'string' ? event.ends_at : null,
      location: String(event.location ?? ''),
      organizer: String(event.organizer ?? '')
    }
  }
  const shown = presented(source, context).invite
  if (shown) {
    return {
      title: shown.title || t('conversation.invite.untitled'),
      starts_at: shown.starts_at ?? '',
      ends_at: shown.ends_at ?? null,
      location: shown.location ?? '',
      organizer: shown.organizer ?? '',
      join_url: shown.join_url ?? null
    }
  }
  const content = source.content ?? ''
  if (!/BEGIN:VEVENT/i.test(content)) return null
  const starts = icsDate(icsValue(content, 'DTSTART'))
  if (!starts) return null
  return {
    title: icsValue(content, 'SUMMARY') ?? t('conversation.invite.untitled'),
    starts_at: starts,
    ends_at: icsDate(icsValue(content, 'DTEND')),
    location: icsValue(content, 'LOCATION') ?? '',
    organizer: (icsValue(content, 'ORGANIZER') ?? '').replace(/^mailto:/i, '')
  }
}

export function isInvite(source: KindSource): boolean {
  const type = (source.content_type ?? '').toLowerCase()
  if (type.includes('text/calendar')) return true
  if (parseMetadata(source.metadata).event) return true
  return /BEGIN:VEVENT/i.test(source.content ?? '')
}

export function messageKind(source: KindSource, context: KindContext = {}): MessageKind {
  const chosen = presentedKind(presented(source, context))
  if (chosen) return chosen
  if (source.type === 'system') return 'system'
  if (isInvite(source)) return 'invite'
  if (source.type === 'bot' || context.senderHuman === false) return 'bot'
  if (source.type === 'email') return 'email'
  if (isMediaOnly(source)) return 'media'
  if (isEmojiOnly(source.content)) return 'emoji'
  if (context.conversationType === 'channel' || context.conversationType === 'group_message') return 'channel'
  return 'chat'
}

export function reactionsOf(metadata: string | null | undefined): Reaction[] {
  const raw = parseMetadata(metadata).reactions
  if (!Array.isArray(raw)) return []
  const byEmoji = new Map<string, Reaction>()
  for (const item of raw) {
    if (!item || typeof item !== 'object') continue
    const emoji = canonicalEmoji(String((item as Record<string, unknown>).emoji ?? ''))
    if (!emoji) continue
    const user = String((item as Record<string, unknown>).user ?? '')
    const name = String((item as Record<string, unknown>).name ?? '') || user
    const entry = byEmoji.get(emoji) ?? { emoji, count: 0, users: [], names: [] }
    entry.count += 1
    if (user) entry.users.push(user)
    if (name) entry.names.push(name)
    byEmoji.set(emoji, entry)
  }
  return [...byEmoji.values()]
}

export function linksOf(content: string | null | undefined, max = 6): string[] {
  if (!content) return []
  const found = new Set<string>()
  for (const match of content.matchAll(/href="(https?:\/\/[^"]+)"/gi)) found.add(match[1])
  for (const match of plainText(content).matchAll(/https?:\/\/[^\s<>"')\]]+/g)) found.add(match[0].replace(/[.,;:]+$/, ''))
  return [...found].slice(0, max)
}

export function collapsedEmails<T extends { id: string; kind: MessageKind; conversationId: string }>(items: T[], keep = 2): Set<string> {
  const byConversation = new Map<string, string[]>()
  for (const item of items) {
    if (item.kind !== 'email') continue
    const list = byConversation.get(item.conversationId) ?? []
    list.push(item.id)
    byConversation.set(item.conversationId, list)
  }
  const out = new Set<string>()
  for (const list of byConversation.values()) for (const id of list.slice(0, Math.max(0, list.length - keep))) out.add(id)
  return out
}
