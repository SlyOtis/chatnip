import { convertFileSrc } from '@tauri-apps/api/core'
import { writable } from 'svelte/store'
import type { Attachment, ContentGroup, ContentItem, ContentKind, ContentSourceKind, ContentTag, ContentTemplateKind, Conversation, GifResult, Message, OutgoingAttachmentInput, Person, PostContentInput, ReactionUse } from '$lib/tauri/types'
import type { MessageKey } from '$lib/i18n.svelte'
import { buildTree, flattenTree } from '$lib/tauri/groups'
import { canonicalEmoji } from '$lib/emoji'
import {  } from '$lib/tauri/conversations'
import { conversationHref } from '$lib/tauri/conversations'

export interface PendingAttachment {
  id: string
  content?: ContentItem
  file?: File
}

export const RECENT_EMOJI_KEY = 'chatnip_recent_emojis'
export const RECENT_LIMIT = 24
export const GIF_CATEGORIES = ['reactions', 'yes', 'no', 'thanks', 'wow', 'lol', 'love', 'facepalm', 'celebrate', 'sad', 'ok', 'hello']

let counter = 0

function nextId(): string {
  counter += 1
  return `pending-${counter}`
}

export function addFiles(list: PendingAttachment[], files: File[]): PendingAttachment[] {
  return [...list, ...files.map((file) => ({ id: nextId(), file }))]
}

export function addContent(list: PendingAttachment[], content: ContentItem): PendingAttachment[] {
  if (list.some((item) => item.content?.id === content.id)) return list
  return [...list, { id: nextId(), content }]
}

export function removeAttachment(list: PendingAttachment[], id: string): PendingAttachment[] {
  return list.filter((item) => item.id !== id)
}

export function attachmentName(item: PendingAttachment): string {
  return item.content?.name ?? item.file?.name ?? ''
}

export function attachmentMime(item: PendingAttachment): string {
  return item.content?.mime_type ?? item.file?.type ?? ''
}

export function contentSrc(item: ContentItem): string {
  if (item.file_path) return convertFileSrc(item.file_path)
  if (item.path) return convertFileSrc(item.path)
  return item.source_url ?? ''
}

export function readFileAsBase64(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = String(reader.result ?? '')
      const comma = result.indexOf(',')
      resolve(comma >= 0 ? result.slice(comma + 1) : result)
    }
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(file)
  })
}

export async function toOutgoing(list: PendingAttachment[]): Promise<OutgoingAttachmentInput[]> {
  const out: OutgoingAttachmentInput[] = []
  for (const item of list) {
    if (item.content) {
      out.push({ content_id: item.content.id })
    } else if (item.file) {
      out.push({ name: item.file.name, mime_type: item.file.type || 'application/octet-stream', data: await readFileAsBase64(item.file) })
    }
  }
  return out
}

export function fromOutgoing(content: ContentItem[], files: OutgoingAttachmentInput[]): PendingAttachment[] {
  const restored = files
    .filter((input) => !!input.data)
    .map((input) => {
      const raw = input.data!.includes(';base64,') ? input.data!.split(';base64,')[1] : input.data!
      return new File([Uint8Array.from(atob(raw), (c) => c.charCodeAt(0))], input.name ?? 'attachment', { type: input.mime_type ?? 'application/octet-stream' })
    })
  return addFiles(content.reduce<PendingAttachment[]>((list, item) => addContent(list, item), []), restored)
}

export function pushRecent(list: string[], value: string, limit = RECENT_LIMIT): string[] {
  return [value, ...list.filter((entry) => entry !== value)].slice(0, limit)
}

export function loadRecent(key = RECENT_EMOJI_KEY): string[] {
  try {
    const stored = localStorage.getItem(key)
    const parsed = stored ? JSON.parse(stored) : []
    return Array.isArray(parsed) ? parsed.filter((entry) => typeof entry === 'string') : []
  } catch {
    return []
  }
}

export function saveRecent(list: string[], key = RECENT_EMOJI_KEY) {
  try {
    localStorage.setItem(key, JSON.stringify(list.slice(0, RECENT_LIMIT)))
  } catch {}
}

export function sortLibrary(items: ContentItem[]): ContentItem[] {
  return [...items].sort((a, b) => {
    if (a.is_favourite !== b.is_favourite) return a.is_favourite ? -1 : 1
    if (a.is_favourite && (a.favourite_position ?? 0) !== (b.favourite_position ?? 0)) return (a.favourite_position ?? 0) - (b.favourite_position ?? 0)
    const au = a.last_used_at ?? a.created_at
    const bu = b.last_used_at ?? b.created_at
    return bu.localeCompare(au)
  })
}

export function filterLibrary(items: ContentItem[], query: string): ContentItem[] {
  const q = query.trim().toLowerCase()
  if (!q) return items
  return items.filter(
    (item) =>
      item.name.toLowerCase().includes(q) ||
      item.tags.some((tag) => tag.toLowerCase().includes(q)) ||
      item.keywords.some((word) => word.toLowerCase().includes(q)) ||
      (item.query ?? '').toLowerCase().includes(q) ||
      (item.body ?? '').toLowerCase().includes(q)
  )
}

export function favouriteItems(items: ContentItem[], group?: ContentGroup): ContentItem[] {
  if (!group) return sortLibrary(items).filter((item) => item.is_favourite)
  const byId = new Map(items.map((item) => [item.id, item]))
  return groupMembers(group, items, byId)
}

export function resultKey(result: GifResult): string {
  return `${result.source}:${result.id}`
}

export function gifSrc(result: GifResult, full = false): string {
  if (result.source === 'library') return convertFileSrc(result.url)
  return full ? result.url : result.preview_url
}

export function appendResults(existing: GifResult[], incoming: GifResult[]): GifResult[] {
  const seen = new Set(existing.map(resultKey))
  const urls = new Set(existing.map((result) => result.url))
  const out = [...existing]
  for (const result of incoming) {
    if (seen.has(resultKey(result)) || urls.has(result.url)) continue
    seen.add(resultKey(result))
    urls.add(result.url)
    out.push(result)
  }
  return out
}

export function favouriteInput(result: GifResult, query: string, favourite = true, groupIds?: string[]): PostContentInput {
  const q = query.trim()
  return {
    kind: 'gif',
    name: result.title.trim() || q || 'gif',
    source_url: result.url,
    query: q || undefined,
    tags: q ? [q.toLowerCase()] : [],
    is_favourite: favourite,
    group_ids: groupIds
  }
}

export function savedUrls(items: ContentItem[]): Map<string, ContentItem> {
  return new Map(items.filter((item) => item.source_url).map((item) => [item.source_url as string, item]))
}

export function parseTags(text: string): string[] {
  return [...new Set(text.split(/[,\s]+/).map((tag) => tag.trim().toLowerCase()).filter(Boolean))]
}

export function attachmentInput(attachment: Attachment, data: string): PostContentInput {
  return {
    kind: attachment.content_type === 'image/gif' ? 'gif' : 'image',
    name: attachment.name.replace(/\.[^.]+$/, '') || 'image',
    source_url: `attachment:${attachment.id}`,
    data,
    mime_type: attachment.content_type,
    is_favourite: true
  }
}

export const TEMPLATE_KINDS: ContentTemplateKind[] = ['reply', 'email', 'prompt', 'snippet', 'signature']

export const CONTENT_KINDS: { value: Exclude<ContentKind, 'emoji'>; label: MessageKey; icon: string }[] = [
  { value: 'gif', label: 'content.kind.gif', icon: 'lucide:sparkles' },
  { value: 'sticker', label: 'content.kind.sticker', icon: 'lucide:sticker' },
  { value: 'image', label: 'content.kind.image', icon: 'lucide:image' },
  { value: 'video', label: 'content.kind.video', icon: 'lucide:film' },
  { value: 'file', label: 'content.kind.file', icon: 'lucide:file' },
  { value: 'reply', label: 'content.kind.reply', icon: 'lucide:message-square-reply' },
  { value: 'email', label: 'content.kind.email', icon: 'lucide:mail' },
  { value: 'prompt', label: 'content.kind.prompt', icon: 'lucide:sparkle' },
  { value: 'snippet', label: 'content.kind.snippet', icon: 'lucide:text-quote' },
  { value: 'signature', label: 'content.kind.signature', icon: 'lucide:signature' }
]

export function isTemplate(item: Pick<ContentItem, 'kind'>): boolean {
  return (TEMPLATE_KINDS as string[]).includes(item.kind)
}

export function kindIcon(kind: ContentKind): string {
  return CONTENT_KINDS.find((entry) => entry.value === kind)?.icon ?? 'lucide:file'
}

export function isVideo(item: Pick<ContentItem, 'kind' | 'mime_type'>): boolean {
  return item.kind === 'video' || item.mime_type.startsWith('video/')
}

export function isVisual(item: Pick<ContentItem, 'kind' | 'mime_type'>): boolean {
  return !isTemplate(item) && item.kind !== 'file'
}

export function mostUsed(items: ContentItem[], limit = 12): ContentItem[] {
  return items.filter((item) => item.used_count > 0).sort((a, b) => b.used_count - a.used_count || (b.last_used_at ?? '').localeCompare(a.last_used_at ?? '')).slice(0, limit)
}

export function recentlyUsed(items: ContentItem[], limit = 12): ContentItem[] {
  return items.filter((item) => item.last_used_at).sort((a, b) => (b.last_used_at ?? '').localeCompare(a.last_used_at ?? '')).slice(0, limit)
}

export function aspect(item: Pick<ContentItem, 'width' | 'height'>): number {
  return item.width && item.height ? item.height / item.width : 1
}

export function tagSuggestions(tags: ContentTag[], typed: string, existing: string[], limit = 8): string[] {
  const needle = typed.trim().toLowerCase()
  return tags
    .map((tag) => tag.name)
    .filter((name) => !existing.includes(name) && (!needle || name.includes(needle)))
    .sort((a, b) => Number(!a.startsWith(needle)) - Number(!b.startsWith(needle)))
    .slice(0, limit)
}

export function toggleWord(list: string[], word: string): string[] {
  const value = word.trim().toLowerCase()
  if (!value) return list
  return list.includes(value) ? list.filter((entry) => entry !== value) : [...list, value]
}

function groupMembers(group: ContentGroup, items: ContentItem[], byId: Map<string, ContentItem>): ContentItem[] {
  if (group.kind === 'manual') return group.content_ids.map((id) => byId.get(id)).filter((item): item is ContentItem => !!item)
  if (group.kind === 'kind') return items.filter((item) => item.kind === group.rule)
  const tag = (group.rule ?? '').toLowerCase()
  return items.filter((item) => item.tags.some((name) => name.toLowerCase() === tag))
}

export interface ContentSection {
  key: string
  group: ContentGroup | null
  depth: number
  items: ContentItem[]
}

export function contentSections(groups: ContentGroup[], items: ContentItem[]): ContentSection[] {
  const byId = new Map(items.map((item) => [item.id, item]))
  const nodes = flattenTree(buildTree(groups))
  const placed = new Set<string>()
  const hidden = new Set<string>()
  const out: ContentSection[] = []
  const childless = (id: string) => !groups.some((group) => group.parent_id === id)
  for (const node of nodes) {
    const group = node.group
    const members = groupMembers(group, items, byId)
    for (const member of members) placed.add(member.id)
    if (group.parent_id && hidden.has(group.parent_id)) {
      hidden.add(group.id)
      continue
    }
    if (group.is_collapsed) hidden.add(group.id)
    if (group.kind !== 'manual' && members.length === 0 && childless(group.id)) continue
    out.push({ key: group.id, group, depth: node.depth, items: members })
  }
  const rest = items.filter((item) => !placed.has(item.id))
  if (rest.length) out.push({ key: 'ungrouped', group: null, depth: 0, items: rest })
  return out
}

export function placementIndex(group: ContentGroup, beforeId: string | null, moving: string[]): number {
  const kept = group.content_ids.filter((id) => !moving.includes(id))
  if (!beforeId) return kept.length
  const index = kept.indexOf(beforeId)
  return index < 0 ? kept.length : index
}

export function sourceLink(item: Pick<ContentItem, 'source_kind' | 'source_url' | 'source_conversation_id' | 'source_person_id' | 'source_action_id'>): { href: string; external: boolean } | null {
  if (item.source_conversation_id && (item.source_kind === 'message' || item.source_kind === 'conversation')) return { href: conversationHref(item.source_conversation_id), external: false }
  if (item.source_kind === 'person' && item.source_person_id) return { href: `/main/person/${item.source_person_id}`, external: false }
  if (item.source_kind === 'action' && item.source_action_id) return { href: `/main/actions?id=${encodeURIComponent(item.source_action_id)}`, external: false }
  if (item.source_url && /^https?:\/\//i.test(item.source_url)) return { href: item.source_url, external: true }
  return null
}

export const SOURCE_LABELS: Record<ContentSourceKind, MessageKey> = {
  upload: 'content.source.upload',
  message: 'content.source.message',
  tenor: 'content.source.tenor',
  giphy: 'content.source.giphy',
  library: 'content.source.library',
  url: 'content.source.url',
  conversation: 'content.source.conversation',
  person: 'content.source.person',
  action: 'content.source.action',
  manual: 'content.source.manual'
}

export function firstName(name: string | null | undefined): string {
  return (name ?? '').trim().split(/\s+/)[0] ?? ''
}

export interface ComposerContext {
  conversation: Pick<Conversation, 'id' | 'name' | 'type'> | null
  person: Pick<Person, 'id' | 'display_name' | 'primary_email'> | null
  me: { name: string | null | undefined; email?: string | null } | null
  message: Pick<Message, 'id' | 'subject' | 'sender_name' | 'sender_email'> | null
  subject?: string | null
}

export function composerVariables(context: ComposerContext): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  const myName = context.me?.name?.trim()
  if (myName) out.me = { name: myName, first_name: firstName(myName), ...(context.me?.email ? { email: context.me.email } : {}) }
  const name = context.person?.display_name?.trim() || context.message?.sender_name?.trim() || ''
  const email = context.person?.primary_email || context.message?.sender_email || ''
  if (name || email) out.person = { ...(context.person ? { id: context.person.id } : {}), name: name || email, first_name: firstName(name || email.split('@')[0]), email }
  if (context.conversation) out.conversation = { id: context.conversation.id, name: context.conversation.name, type: context.conversation.type }
  if (context.message) {
    const subject = context.subject?.trim() || context.message.subject?.trim() || ''
    out.message = { id: context.message.id, subject, from_name: context.message.sender_name ?? '', from_email: context.message.sender_email ?? '' }
  }
  return out
}

export function messageText(message: Pick<Message, 'content' | 'content_type'>): string {
  if (!/html/i.test(message.content_type)) return message.content.trim()
  const doc = new DOMParser().parseFromString(message.content, 'text/html')
  doc.querySelectorAll('br').forEach((br) => br.replaceWith('\n'))
  doc.querySelectorAll('p').forEach((block) => block.append('\n\n'))
  doc.querySelectorAll('div, li').forEach((block) => block.append('\n'))
  return (doc.body.textContent ?? '').replace(/\n{3,}/g, '\n\n').trim()
}

export function templateFromMessage(message: Pick<Message, 'id' | 'conversation_id' | 'type' | 'subject' | 'content' | 'content_type'>): PostContentInput {
  const text = messageText(message)
  const body = text.replace(/\{\{/g, '{{ "{{" }}')
  const line = text.split('\n').map((entry) => entry.trim()).find(Boolean) ?? ''
  return {
    kind: message.type === 'email' ? 'email' : 'reply',
    name: (message.subject?.trim() || line).slice(0, 48),
    body,
    source_kind: 'message',
    source_message_id: message.id,
    source_conversation_id: message.conversation_id
  }
}

export function nestValues(flat: Record<string, string>): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (const [path, value] of Object.entries(flat)) {
    const parts = path.split('.')
    let node = out
    parts.forEach((part, index) => {
      if (index === parts.length - 1) node[part] = value
      else {
        if (typeof node[part] !== 'object' || node[part] === null) node[part] = {}
        node = node[part] as Record<string, unknown>
      }
    })
  }
  return out
}

export const DEFAULT_REACTIONS = [':+1:', ':heart:', ':joy:', ':open_mouth:', ':cry:', ':tada:']

export const reactionUses = writable<ReactionUse[]>([])

export function quickReactions(uses: ReactionUse[], defaults: string[] = DEFAULT_REACTIONS, count = defaults.length): string[] {
  const ranked = [...uses].sort((a, b) => b.used_count - a.used_count || b.last_used_at.localeCompare(a.last_used_at)).map((use) => canonicalEmoji(use.emoji))
  const merged: string[] = []
  for (const emoji of [...ranked, ...defaults.map(canonicalEmoji)]) if (!merged.includes(emoji)) merged.push(emoji)
  return merged.slice(0, count)
}

export function noteReaction(uses: ReactionUse[], value: string, now = new Date().toISOString()): ReactionUse[] {
  const emoji = canonicalEmoji(value)
  const known = uses.find((use) => use.emoji === emoji)
  if (known) return uses.map((use) => (use.emoji === emoji ? { ...use, used_count: use.used_count + 1, last_used_at: now } : use))
  return [...uses, { emoji, used_count: 1, last_used_at: now }]
}

export async function loadReactionUses(): Promise<ReactionUse[]> {
  const { getReactionUses } = await import('$lib/tauri/api')
  const uses = await getReactionUses(48)
  reactionUses.set(uses)
  return uses
}

export async function sendReaction(input: { conversation_id: string; message_id: string; reaction: string }): Promise<void> {
  const { postMessageReaction } = await import('$lib/tauri/api')
  await postMessageReaction({ ...input, reaction: canonicalEmoji(input.reaction) })
  reactionUses.update((uses) => noteReaction(uses, input.reaction))
}
