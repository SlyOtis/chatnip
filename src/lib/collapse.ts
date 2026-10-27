import { get, writable } from 'svelte/store'
import { collapsedEmails } from './messageKind'
import { t } from '$lib/i18n.svelte'

export type FoldKey = 'open' | 'quote' | 'full' | 'mail'

export interface FoldState {
  all: 'open' | 'closed' | null
  items: Record<string, Partial<Record<FoldKey, boolean>>>
  focused: string | null
}

export const folds = writable<FoldState>({ all: null, items: {}, focused: null })

const defaults = new Map<string, boolean>()

export function isOpen(state: FoldState, id: string, key: FoldKey, fallback: boolean): boolean {
  const own = state.items[id]?.[key]
  if (own !== undefined) return own
  if (state.all) return state.all === 'open'
  return fallback
}

export function registerFold(id: string, fallback: boolean) {
  defaults.set(id, fallback)
}

export function setFold(id: string, key: FoldKey, value: boolean) {
  folds.update((state) => ({ ...state, items: { ...state.items, [id]: { ...state.items[id], [key]: value } } }))
}

export function toggleFold(id: string, key: FoldKey, fallback: boolean) {
  folds.update((state) => ({ ...state, items: { ...state.items, [id]: { ...state.items[id], [key]: !isOpen(state, id, key, fallback) } } }))
}

export function focusFold(id: string | null) {
  folds.update((state) => (state.focused === id ? state : { ...state, focused: id }))
}

export function toggleFocused() {
  const id = get(folds).focused
  if (id) toggleFold(id, 'open', defaults.get(id) ?? true)
}

export function foldAll(mode: 'open' | 'closed') {
  folds.update((state) => ({ ...state, all: mode, items: {} }))
}

export function resetFolds() {
  folds.update((state) => ({ all: null, items: {}, focused: state.focused }))
}

export interface FoldGroup {
  id: string
  conversationId: string
  email: boolean
  mine: boolean
  chat: boolean
  size: number
  section?: string
}

export interface FoldMarker {
  key: string
  kind: 'earlier' | 'run'
  count: number
}

export interface FoldPlan {
  folded: Set<string>
  summary: Set<string>
  hidden: Map<string, string>
  markers: Map<string, FoldMarker>
}

export function chatRuns(groups: FoldGroup[], limit = 12, edge = 3): { key: string; hidden: string[]; count: number }[] {
  const runs: FoldGroup[][] = []
  let current: FoldGroup[] = []
  for (const group of groups) {
    const continues = group.chat && !group.mine
    const sameSection = !current.length || current[current.length - 1].section === group.section
    if (continues && sameSection) {
      current.push(group)
      continue
    }
    if (current.length) {
      runs.push(current)
      current = []
    }
    if (continues) current.push(group)
  }
  if (current.length) runs.push(current)
  const out: { key: string; hidden: string[]; count: number }[] = []
  for (const run of runs) {
    const total = run.reduce((sum, g) => sum + g.size, 0)
    if (total <= limit) continue
    let head = 0
    let kept = 0
    while (head < run.length && kept < edge) kept += run[head++].size
    let tail = run.length
    kept = 0
    while (tail > head && kept < edge) kept += run[--tail].size
    const hidden = run.slice(head, tail)
    if (hidden.length === 0) continue
    out.push({ key: `run:${hidden[0].id}`, hidden: hidden.map((g) => g.id), count: hidden.reduce((sum, g) => sum + g.size, 0) })
  }
  return out
}

export function foldPlan(groups: FoldGroup[], keep = 2, limit = 12, edge = 3): FoldPlan {
  const loose = groups.filter((g) => !g.section)
  const folded = collapsedEmails(loose.map((g) => ({ id: g.id, kind: g.email ? ('email' as const) : ('chat' as const), conversationId: g.conversationId })), keep)
  const hidden = new Map<string, string>()
  const kinds = new Map<string, FoldMarker['kind']>()
  const markers = new Map<string, FoldMarker>()
  for (const group of groups) {
    if (!folded.has(group.id)) continue
    hidden.set(group.id, `earlier:${group.conversationId}`)
    kinds.set(`earlier:${group.conversationId}`, 'earlier')
  }
  for (const run of chatRuns(loose, limit, edge)) {
    kinds.set(run.key, 'run')
    for (const id of run.hidden) hidden.set(id, run.key)
  }
  const segments = new Map<string, FoldMarker>()
  for (const group of groups) {
    const key = hidden.get(group.id)
    if (!key) continue
    const kind = kinds.get(key) ?? 'earlier'
    const count = kind === 'run' ? group.size : 1
    const segment = `${key}|${group.section ?? ''}`
    const marker = segments.get(segment)
    if (marker) {
      marker.count += count
      continue
    }
    const opened: FoldMarker = { key, kind, count }
    segments.set(segment, opened)
    markers.set(group.id, opened)
  }
  const last = groups[groups.length - 1]?.id
  const summary = new Set(loose.filter((g) => g.email && g.id !== last).map((g) => g.id))
  return { folded, summary, hidden, markers }
}

export function markerLabel(marker: FoldMarker): string {
  if (marker.kind === 'run') return t('conversation.fold.showMore', { count: marker.count })
  return t('conversation.fold.earlier', { count: marker.count })
}
