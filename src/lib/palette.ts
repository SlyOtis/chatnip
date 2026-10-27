import type { MessageKey } from '$lib/i18n.svelte'
import { featureShown } from '$lib/settings'

export type Scope = 'all' | 'people' | 'chats' | 'mail' | 'actions' | 'cases' | 'places' | 'content' | 'events' | 'commands'

export type GroupKind = 'people' | 'chats' | 'mail' | 'messages' | 'provider' | 'actions' | 'cases' | 'places' | 'tags' | 'content' | 'events' | 'commands'

export interface Token {
  kind: 'person' | 'chat' | 'tag'
  id: string
  label: string
  icon?: string
}

export interface Operator {
  op: '@' | '#'
  term: string
  start: number
}

export interface SnippetPart {
  text: string
  hit: boolean
}

export const SCOPES: { value: Scope; label: MessageKey }[] = ([
  { value: 'all', label: 'common.all' },
  { value: 'people', label: 'modals.scope.people' },
  { value: 'chats', label: 'modals.scope.chats' },
  { value: 'mail', label: 'modals.scope.mail' },
  { value: 'actions', label: 'modals.scope.actions' },
  { value: 'cases', label: 'modals.scope.cases' },
  { value: 'places', label: 'modals.scope.places' },
  { value: 'content', label: 'content.title' },
  { value: 'events', label: 'modals.scope.events' },
  { value: 'commands', label: 'modals.scope.commands' }
] satisfies { value: Scope; label: MessageKey }[]).filter((entry) => featureShown(entry.value))

export const GROUP_LIMIT = 4

const SCOPE_GROUPS: Record<Exclude<Scope, 'all'>, GroupKind[]> = {
  people: ['people'],
  chats: ['chats', 'messages'],
  mail: ['mail', 'messages'],
  actions: ['actions'],
  cases: ['cases'],
  places: ['places', 'tags'],
  content: ['content'],
  events: ['events'],
  commands: ['commands']
}

export function inScope(scope: Scope, kind: GroupKind): boolean {
  return featureShown(kind) && (scope === 'all' || SCOPE_GROUPS[scope].includes(kind))
}

export function scopeOf(kind: GroupKind): Scope {
  if (kind === 'messages') return 'all'
  if (kind === 'tags') return 'places'
  return kind as Scope
}

export function cycleScope(scope: Scope, step: 1 | -1): Scope {
  const index = SCOPES.findIndex((entry) => entry.value === scope)
  return SCOPES[(index + step + SCOPES.length) % SCOPES.length].value
}

export function operatorAt(text: string): Operator | null {
  const match = /(^|\s)([@#])([^\s@#]*)$/.exec(text)
  if (!match) return null
  return { op: match[2] as '@' | '#', term: match[3], start: match.index + match[1].length }
}

export function withoutOperator(text: string, operator: Operator): string {
  return text.slice(0, operator.start).replace(/\s+$/, '')
}

function fold(text: string): string {
  return text.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ø/g, 'o').replace(/đ/g, 'd').replace(/ł/g, 'l')
}

export function matchScore(text: string | null | undefined, query: string): number {
  const q = fold(query.trim())
  if (!q) return 1
  const value = fold(text ?? '')
  if (!value) return 0
  if (value === q) return 100
  if (value.startsWith(q)) return 80
  if (new RegExp(`(^|[\\s._@-])${q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`).test(value)) return 60
  if (value.includes(q)) return 40
  const words = q.split(/\s+/).filter(Boolean)
  if (words.length > 1 && words.every((word) => value.includes(word))) return 30
  return 0
}

export function rank<T>(items: T[], query: string, fields: (item: T) => (string | null | undefined)[], date?: (item: T) => string | null | undefined, now = Date.now()): T[] {
  const scored: { item: T; score: number }[] = []
  for (const item of items) {
    const best = Math.max(0, ...fields(item).map((field) => matchScore(field, query)))
    if (best === 0) continue
    const stamp = date?.(item)
    const days = stamp ? Math.max(0, (now - new Date(stamp).getTime()) / 86400000) : 365
    scored.push({ item, score: best + Math.max(0, 20 - days / 9) })
  }
  return scored.sort((a, b) => b.score - a.score).map((entry) => entry.item)
}

export function snippetParts(snippet: string, open = '\u0002', close = '\u0003'): SnippetPart[] {
  const parts: SnippetPart[] = []
  let rest = snippet.replace(/\s+/g, ' ')
  while (rest.length) {
    const start = rest.indexOf(open)
    if (start < 0) {
      parts.push({ text: rest, hit: false })
      break
    }
    if (start > 0) parts.push({ text: rest.slice(0, start), hit: false })
    const end = rest.indexOf(close, start + open.length)
    const stop = end < 0 ? rest.length : end
    parts.push({ text: rest.slice(start + open.length, stop), hit: true })
    rest = end < 0 ? '' : rest.slice(end + close.length)
  }
  return parts.filter((part) => part.text.length > 0)
}

export function highlightParts(text: string, query: string, radius = 70): SnippetPart[] {
  const plain = text.replace(/\s+/g, ' ').trim()
  const words = fold(query).split(/\s+/).filter(Boolean)
  const folded = fold(plain)
  const first = words.map((word) => folded.indexOf(word)).filter((at) => at >= 0).sort((a, b) => a - b)[0]
  if (first === undefined) return [{ text: plain.length > radius * 2 ? plain.slice(0, radius * 2) + '…' : plain, hit: false }]
  const from = Math.max(0, first - radius)
  const to = Math.min(plain.length, first + radius)
  const window = (from > 0 ? '…' : '') + plain.slice(from, to) + (to < plain.length ? '…' : '')
  const marks: [number, number][] = []
  const foldedWindow = fold(window)
  for (const word of words) {
    let at = foldedWindow.indexOf(word)
    while (at >= 0) {
      marks.push([at, at + word.length])
      at = foldedWindow.indexOf(word, at + word.length)
    }
  }
  marks.sort((a, b) => a[0] - b[0])
  const parts: SnippetPart[] = []
  let cursor = 0
  for (const [start, end] of marks) {
    if (start < cursor) continue
    if (start > cursor) parts.push({ text: window.slice(cursor, start), hit: false })
    parts.push({ text: window.slice(start, end), hit: true })
    cursor = end
  }
  if (cursor < window.length) parts.push({ text: window.slice(cursor), hit: false })
  return parts
}

export function visibleItems<T>(items: T[], scope: Scope, limit = GROUP_LIMIT): { shown: T[]; hidden: number } {
  if (scope !== 'all' || items.length <= limit) return { shown: items, hidden: 0 }
  return { shown: items.slice(0, limit), hidden: items.length - limit }
}

export function uniqueTokens(tokens: Token[], next: Token): Token[] {
  return tokens.some((token) => token.kind === next.kind && token.id === next.id) ? tokens : [...tokens, next]
}

export type FilterKey = 'from' | 'in' | 'has' | 'is' | 'before' | 'after'

export interface SearchFilter {
  key: FilterKey
  value: string
  raw: string
}

export interface FilterOperator {
  key: FilterKey
  term: string
  start: number
}

export const FILTER_KEYS: FilterKey[] = ['from', 'in', 'has', 'is', 'before', 'after']

export const FILTER_VALUES: Record<'has' | 'is', string[]> = {
  has: ['link', 'file', 'reaction', 'mention'],
  is: ['unread', 'saved', 'pinned', 'thread']
}

const FILTER_PATTERN = /^(from|in|has|is|before|after):(\S+)$/

function validFilter(key: FilterKey, value: string): boolean {
  if (key === 'has' || key === 'is') return FILTER_VALUES[key].includes(value)
  if (key === 'before' || key === 'after') return /^\d{4}-\d{2}-\d{2}$/.test(value)
  return value.length > 0
}

export function searchFilters(text: string): { filters: SearchFilter[]; words: string } {
  const filters: SearchFilter[] = []
  const words: string[] = []
  for (const part of text.split(/\s+/).filter(Boolean)) {
    const match = FILTER_PATTERN.exec(part)
    if (match && validFilter(match[1] as FilterKey, match[2].toLowerCase())) filters.push({ key: match[1] as FilterKey, value: match[2], raw: part })
    else words.push(part)
  }
  return { filters, words: words.join(' ') }
}

export function withoutFilter(text: string, filter: SearchFilter): string {
  return text
    .split(/\s+/)
    .filter((part) => part && part !== filter.raw)
    .join(' ')
}

export function filterAt(text: string): FilterOperator | null {
  const match = /(^|\s)(from|in|has|is|before|after):(\S*)$/.exec(text)
  if (!match) return null
  return { key: match[2] as FilterKey, term: match[3], start: match.index + match[1].length }
}

export function withFilter(text: string, operator: FilterOperator, value: string): string {
  return `${text.slice(0, operator.start)}${operator.key}:${value} `
}

function day(offset: number, now: number): string {
  const date = new Date(now - offset * 86400000)
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}

export function filterCompletions(operator: FilterOperator, names: string[] = [], now = Date.now()): string[] {
  const term = operator.term.toLowerCase()
  const values = (() => {
    switch (operator.key) {
      case 'has':
      case 'is':
        return FILTER_VALUES[operator.key]
      case 'before':
      case 'after':
        return [day(0, now), day(1, now), day(7, now), day(30, now)]
      default:
        return names.map((name) => name.trim().split(/\s+/)[0]?.toLowerCase() ?? '').filter(Boolean)
    }
  })()
  return [...new Set(values)].filter((value) => value.toLowerCase().startsWith(term)).slice(0, 8)
}
