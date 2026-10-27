import { convertFileSrc } from '@tauri-apps/api/core'
import { writable } from 'svelte/store'
import compact from 'emojibase-data/en/compact.json'
import iamcal from 'emojibase-data/en/shortcodes/iamcal.json'
import github from 'emojibase-data/en/shortcodes/github.json'
import presets from 'emojibase-data/en/shortcodes/emojibase.json'
import messages from 'emojibase-data/en/messages.json'
import type { WorkspaceEmoji } from '$lib/tauri/types'
import { escapeHtml } from '$lib/content'

interface RawSkin {
  hexcode: string
  unicode: string
}

interface RawEmoji {
  hexcode: string
  unicode: string
  label: string
  group?: number
  order?: number
  tags?: string[]
  skins?: RawSkin[]
}

type Shortcodes = Record<string, string | string[]>

export interface StandardEmoji {
  unicode: string
  name: string
  names: string[]
  label: string
  tags: string[]
  group: number
  order: number
  skins: Record<string, string>
}

export interface EmojiGroup {
  key: string
  label: string
  emojis: StandardEmoji[]
}

export interface EmojiMatch {
  name: string
  unicode: string | null
  custom: WorkspaceEmoji | null
}

const COMPONENT_GROUP = 2
const TONES: Record<string, string> = { '1F3FB': '2', '1F3FC': '3', '1F3FD': '4', '1F3FE': '5', '1F3FF': '6' }
const TONE_MODIFIERS = ['', '🏻', '🏼', '🏽', '🏾', '🏿']
const SHORTCODE_AT = /:([a-z0-9_+'-]+):(?::(skin-tone-[2-6](?:-[2-6])?):)?/iy
const SKIP_TAGS = new Set(['code', 'pre', 'script', 'style', 'textarea', 'kbd', 'samp'])

function list(value: string | string[] | undefined): string[] {
  if (!value) return []
  return Array.isArray(value) ? value : [value]
}

function strip(unicode: string): string {
  return unicode.replace(/\uFE0F/g, '')
}

function presented(unicode: string): string {
  const chars = [...unicode]
  if (chars.length === 2 && chars[1] === '\uFE0F' && /\p{Emoji_Presentation}/u.test(chars[0])) return chars[0]
  return unicode
}

function toneOf(hexcode: string): string | null {
  const tones: string[] = []
  for (const part of hexcode.split('-')) {
    const tone = TONES[part]
    if (tone && !tones.includes(tone)) tones.push(tone)
  }
  return tones.length ? `skin-tone-${tones.join('-')}` : null
}

function build() {
  const emojis: StandardEmoji[] = []
  const names = new Map<string, string>()
  const codes = new Map<string, string>()
  for (const raw of compact as unknown as RawEmoji[]) {
    const all: string[] = []
    for (const source of [iamcal, github, presets] as unknown as Shortcodes[]) for (const name of list(source[raw.hexcode])) if (!all.includes(name)) all.push(name)
    if (!all.length) continue
    const primary = all[0]
    const skins: Record<string, string> = {}
    for (const skin of raw.skins ?? []) {
      const tone = toneOf(skin.hexcode)
      if (!tone) continue
      skins[tone] = skin.unicode
      for (const name of all) if (!names.has(`${name}::${tone}`)) names.set(`${name}::${tone}`, skin.unicode)
      if (!codes.has(strip(skin.unicode))) codes.set(strip(skin.unicode), `${primary}::${tone}`)
    }
    const unicode = presented(raw.unicode)
    for (const name of all) if (!names.has(name)) names.set(name, unicode)
    if (!codes.has(strip(raw.unicode))) codes.set(strip(raw.unicode), primary)
    emojis.push({ unicode, name: primary, names: all, label: raw.label, tags: raw.tags ?? [], group: raw.group ?? -1, order: raw.order ?? 0, skins })
  }
  emojis.sort((a, b) => a.order - b.order)
  return { emojis, names, codes }
}

const TABLE = build()

export const STANDARD_EMOJI: readonly StandardEmoji[] = TABLE.emojis

export const workspaceEmoji = writable<WorkspaceEmoji[]>([])

let custom: WorkspaceEmoji[] = []

workspaceEmoji.subscribe((value) => {
  custom = value
})

export function putWorkspaceEmoji(value: WorkspaceEmoji[]) {
  workspaceEmoji.set(value.map((e) => (e.alias_for ? { ...e, alias_for: bare(e.alias_for).replace(/^alias:/, '') } : e)))
}

let loading: Promise<void> | null = null

export function loadWorkspaceEmoji(): Promise<void> {
  if (loading) return loading
  loading = (async () => {
    try {
      const { getWorkspaceEmoji } = await import('$lib/tauri/api')
      putWorkspaceEmoji(await getWorkspaceEmoji())
    } catch {
      loading = null
    }
  })()
  return loading
}

function bare(code: string): string {
  const trimmed = code.trim()
  return trimmed.startsWith(':') && trimmed.endsWith(':') && trimmed.length > 2 ? trimmed.slice(1, -1) : trimmed
}

export function isShortcode(value: string): boolean {
  SHORTCODE_AT.lastIndex = 0
  const match = SHORTCODE_AT.exec(value.trim())
  return !!match && match[0].length === value.trim().length
}

export function getUnicode(code: string): string | null {
  return TABLE.names.get(bare(code).toLowerCase()) ?? TABLE.names.get(bare(code)) ?? null
}

export function getShortcode(unicode: string): string | null {
  const name = TABLE.codes.get(strip(unicode.trim()))
  return name ? `:${name}:` : null
}

export function canonicalEmoji(value: string): string {
  const trimmed = value.trim()
  if (!trimmed) return trimmed
  if (isShortcode(trimmed)) return trimmed
  return getShortcode(trimmed) ?? trimmed
}

export function getCustom(code: string, providerId?: string | null): WorkspaceEmoji | null {
  let name = bare(code)
  const pool = providerId ? custom.filter((e) => e.provider_id === providerId) : custom
  for (let hops = 0; hops < 4; hops++) {
    const found = pool.find((e) => e.name === name)
    if (!found) return null
    if (!found.alias_for) return found
    const target = bare(found.alias_for).replace(/^alias:/, '')
    if (getUnicode(target)) return { ...found, alias_for: target }
    name = target
  }
  return null
}

export function customSrc(emoji: WorkspaceEmoji): string {
  if (emoji.file_path) return convertFileSrc(emoji.file_path)
  return emoji.url ?? ''
}

export function customHtml(emoji: WorkspaceEmoji, code: string): string {
  const alias = emoji.alias_for ? getUnicode(emoji.alias_for) : null
  if (alias) return alias
  const src = customSrc(emoji)
  if (!src) return code
  return `<img class="emoji-custom" src="${escapeHtml(src)}" alt="${escapeHtml(code)}" title="${escapeHtml(code)}" data-emoji="${escapeHtml(code)}" style="height:1.25em;width:auto;margin:0;vertical-align:-0.25em;border-radius:0">`
}

export interface ShortcodeHit {
  start: number
  end: number
  code: string
}

export function findShortcodes(text: string): ShortcodeHit[] {
  const out: ShortcodeHit[] = []
  let i = text.indexOf(':')
  while (i >= 0 && i < text.length) {
    SHORTCODE_AT.lastIndex = i
    const match = SHORTCODE_AT.exec(text)
    if (match) {
      out.push({ start: i, end: i + match[0].length, code: match[0] })
      i = text.indexOf(':', i + match[0].length)
    } else {
      i = text.indexOf(':', i + 1)
    }
  }
  return out
}

function replaceShortcodes(text: string, render: (code: string) => string | null): string {
  const hits = findShortcodes(text)
  if (!hits.length) return text
  let out = ''
  let at = 0
  for (const hit of hits) {
    const rendered = render(hit.code)
    if (rendered === null) continue
    out += text.slice(at, hit.start) + rendered
    at = hit.end
  }
  return out + text.slice(at)
}

export function renderEmojiText(text: string): string {
  return replaceShortcodes(text, (code) => getUnicode(code))
}

export function renderEmojiSegment(text: string, providerId?: string | null): string {
  return replaceShortcodes(text, (code) => {
    const unicode = getUnicode(code)
    if (unicode) return unicode
    const found = getCustom(code, providerId)
    return found ? customHtml(found, code) : null
  })
}

export function renderEmojiHtml(html: string, providerId?: string | null): string {
  if (!html || !html.includes(':')) return html
  const parts = html.split(/(<[^>]*>)/)
  const skipping: string[] = []
  return parts
    .map((part) => {
      if (part.startsWith('<')) {
        const tag = /^<\s*(\/)?\s*([a-zA-Z0-9]+)/.exec(part)
        if (tag) {
          const name = tag[2].toLowerCase()
          if (SKIP_TAGS.has(name)) {
            if (tag[1]) {
              const at = skipping.lastIndexOf(name)
              if (at >= 0) skipping.splice(at, 1)
            } else if (!/\/\s*>$/.test(part)) skipping.push(name)
          }
        }
        return part
      }
      return skipping.length ? part : renderEmojiSegment(part, providerId)
    })
    .join('')
}

export function emojiDisplay(value: string, providerId?: string | null): { unicode: string | null; custom: WorkspaceEmoji | null; code: string } {
  const code = canonicalEmoji(value)
  if (!isShortcode(code)) return { unicode: value, custom: null, code: value }
  const unicode = getUnicode(code)
  if (unicode) return { unicode, custom: null, code }
  const found = getCustom(code, providerId)
  const alias = found?.alias_for ? getUnicode(found.alias_for) : null
  return { unicode: alias, custom: alias ? null : found, code }
}

export function withSkinTone(emoji: StandardEmoji, tone: number): { unicode: string; code: string } {
  const key = tone > 0 ? `skin-tone-${tone + 1}` : ''
  const skin = key ? emoji.skins[key] : undefined
  if (!skin) return { unicode: emoji.unicode, code: `:${emoji.name}:` }
  return { unicode: skin, code: `:${emoji.name}::${key}:` }
}

export function toneModifier(tone: number): string {
  return TONE_MODIFIERS[tone] ?? ''
}

export function getEmojiGroups(): EmojiGroup[] {
  const groups = (messages as unknown as { groups: { key: string; message: string; order: number }[] }).groups
  return groups
    .filter((g) => g.order !== COMPONENT_GROUP)
    .sort((a, b) => a.order - b.order)
    .map((g) => ({ key: g.key, label: g.message, emojis: STANDARD_EMOJI.filter((e) => e.group === g.order) }))
    .filter((g) => g.emojis.length > 0)
}

function rank(names: string[], query: string): number {
  if (names.some((n) => n === query)) return 0
  if (names.some((n) => n.startsWith(query))) return 1
  if (names.some((n) => n.split(/[_-]/).some((part) => part.startsWith(query)))) return 2
  if (names.some((n) => n.includes(query))) return 3
  return -1
}

export function searchEmoji(query: string, options: { limit?: number; providerId?: string | null; labels?: boolean } = {}): EmojiMatch[] {
  const q = bare(query).toLowerCase().trim()
  const limit = options.limit ?? 8
  if (!q) return []
  const scored: { score: number; order: number; match: EmojiMatch }[] = []
  for (const emoji of STANDARD_EMOJI) {
    let score = rank(emoji.names, q)
    if (score < 0 && options.labels) {
      if (emoji.label.toLowerCase().includes(q) || emoji.tags.some((tag) => tag.toLowerCase().startsWith(q))) score = 4
    }
    if (score >= 0) scored.push({ score, order: emoji.order, match: { name: emoji.name, unicode: emoji.unicode, custom: null } })
  }
  const seen = new Set<string>()
  const pool = options.providerId ? custom.filter((e) => e.provider_id === options.providerId) : custom
  for (const [index, emoji] of pool.entries()) {
    if (seen.has(emoji.name)) continue
    seen.add(emoji.name)
    const score = rank([emoji.name], q)
    if (score < 0) continue
    const alias = emoji.alias_for ? getUnicode(emoji.alias_for) : null
    scored.push({ score, order: 100000 + index, match: { name: emoji.name, unicode: alias, custom: alias ? null : emoji } })
  }
  scored.sort((a, b) => a.score - b.score || a.match.name.length - b.match.name.length || a.order - b.order)
  return scored.slice(0, limit).map((s) => s.match)
}

export interface ShortcodeQuery {
  query: string
  start: number
}

export function shortcodeQuery(before: string): ShortcodeQuery | null {
  const match = /(?:^|[\s(])(:([a-z0-9_+'-]{2,}))$/i.exec(before)
  if (!match) return null
  return { query: match[2], start: before.length - match[1].length }
}

export function groupWorkspaceEmoji(list: WorkspaceEmoji[]): { provider_id: string; provider_name: string; provider_type: string; emojis: WorkspaceEmoji[] }[] {
  const out = new Map<string, { provider_id: string; provider_name: string; provider_type: string; emojis: WorkspaceEmoji[] }>()
  for (const emoji of list) {
    const entry = out.get(emoji.provider_id) ?? { provider_id: emoji.provider_id, provider_name: emoji.provider_name, provider_type: emoji.provider_type, emojis: [] }
    entry.emojis.push(emoji)
    out.set(emoji.provider_id, entry)
  }
  for (const entry of out.values()) entry.emojis.sort((a, b) => a.name.localeCompare(b.name))
  return [...out.values()].sort((a, b) => a.provider_name.localeCompare(b.provider_name))
}
