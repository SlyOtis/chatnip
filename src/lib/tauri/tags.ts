import { derived, get, writable } from 'svelte/store'
import type { ObjectTag, ObjectType, Tag, TaggedObject } from './types'
import { deleteObjectTag, getObjectTags, getTags, putObjectTag } from './api'

export const tagRecordsStore = writable<Tag[]>([])

export const objectTagsStore = writable<ObjectTag[]>([])

export const splitsStore = derived(tagRecordsStore, ($labels) =>
  $labels.filter((label) => label.kind === 'split').sort((a, b) => a.position - b.position)
)

export const tagsStore = derived(tagRecordsStore, ($labels) =>
  $labels.filter((label) => label.kind === 'tag').sort((a, b) => a.position - b.position)
)

export const TAG_COLOURS = ['#1f9d55', '#4f9e8f', '#7a8f3a', '#d9a400', '#e8743b', '#c4553f', '#d0587e', '#a0703c', '#5a8fb8', '#8c93a3']

const RECENT_COLOURS_KEY = 'chatnip.recent-colours'
const RECENT_COLOURS_LIMIT = 8

function storedColours(): string[] {
  try {
    const list: unknown = JSON.parse(globalThis.localStorage?.getItem(RECENT_COLOURS_KEY) ?? '[]')
    return Array.isArray(list) ? list.filter((colour): colour is string => typeof colour === 'string') : []
  } catch {
    return []
  }
}

export const recentColours = writable<string[]>(storedColours())

export function rememberColour(colour: string | null | undefined) {
  const hex = colour?.trim().toLowerCase()
  if (!hex || !/^#[0-9a-f]{6}$/.test(hex)) return
  recentColours.update((list) => {
    const next = [hex, ...list.filter((known) => known !== hex)].slice(0, RECENT_COLOURS_LIMIT)
    globalThis.localStorage?.setItem(RECENT_COLOURS_KEY, JSON.stringify(next))
    return next
  })
}

const PAPER = { day: '#faf8f4', night: '#211e1b' }
const INK = { day: '#1d1a17', night: '#f4f1ec' }
const FLOOR = 3

export async function loadTags(): Promise<Tag[]> {
  const labels = await getTags()
  tagRecordsStore.set(labels)
  return labels
}

export async function loadObjectTags(): Promise<ObjectTag[]> {
  const all = await getObjectTags()
  objectTagsStore.set(all)
  return all
}

export function tagColor(label: Tag | undefined): string {
  return label?.color || 'var(--accent)'
}

export function tagIdsOn(all: ObjectTag[], object_type: ObjectType, object_id: string): string[] {
  return all.filter((tag) => tag.object_type === object_type && tag.object_id === object_id).map((tag) => tag.tag_id)
}

export async function toggleObjectTag(object_type: ObjectType, ids: string[], tag_id: string, current: ObjectTag[] = get(objectTagsStore)): Promise<void> {
  const tagged = (id: string) => current.some((tag) => tag.object_type === object_type && tag.object_id === id && tag.tag_id === tag_id)
  const removing = ids.length > 0 && ids.every(tagged)
  await Promise.all(
    ids
      .filter((id) => removing || !tagged(id))
      .map((object_id) => (removing ? deleteObjectTag({ object_type, object_id, tag_id }) : putObjectTag({ object_type, object_id, tag_id })))
  )
  await loadObjectTags()
}

function channels(hex: string): [number, number, number] | null {
  const match = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex.trim())
  if (!match) return null
  const raw = match[1].length === 3 ? [...match[1]].map((c) => c + c).join('') : match[1]
  return [0, 2, 4].map((i) => parseInt(raw.slice(i, i + 2), 16)) as [number, number, number]
}

function luminance([r, g, b]: [number, number, number]): number {
  const linear = (v: number) => {
    const c = v / 255
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

export function contrast(a: string, b: string): number {
  const x = channels(a)
  const y = channels(b)
  if (!x || !y) return 21
  const [hi, lo] = [luminance(x), luminance(y)].sort((m, n) => n - m)
  return (hi + 0.05) / (lo + 0.05)
}

function mix(a: [number, number, number], b: [number, number, number], share: number): string {
  return '#' + a.map((v, i) => Math.round(v * share + b[i] * (1 - share)).toString(16).padStart(2, '0')).join('')
}

export function tagTone(color: string, theme: 'day' | 'night'): string {
  const own = channels(color)
  const ink = channels(INK[theme])
  if (!own || !ink || contrast(color, PAPER[theme]) >= FLOOR) return color
  for (let share = 95; share >= 0; share -= 5) {
    if (contrast(mix(own, ink, share / 100), PAPER[theme]) >= FLOOR) return `color-mix(in srgb, ${color} ${share}%, var(--ink))`
  }
  return 'var(--ink)'
}

export const OBJECT_ORDER: ObjectType[] = ['conversation', 'message', 'attachment', 'person', 'identity', 'content', 'event', 'action', 'action_group', 'group', 'inbox', 'merge_group']

export const OBJECT_ICONS: Record<ObjectType, string> = {
  conversation: 'lucide:message-circle',
  message: 'lucide:message-square-text',
  attachment: 'lucide:paperclip',
  person: 'lucide:user',
  identity: 'lucide:at-sign',
  content: 'lucide:library',
  event: 'lucide:calendar',
  action: 'lucide:workflow',
  action_group: 'lucide:folder-cog',
  group: 'lucide:folder',
  inbox: 'lucide:inbox',
  merge_group: 'lucide:merge'
}

export function groupTagged(objects: TaggedObject[], skip: ObjectType[] = []): { type: ObjectType; objects: TaggedObject[] }[] {
  return OBJECT_ORDER.filter((type) => !skip.includes(type))
    .map((type) => ({ type, objects: objects.filter((object) => object.object_type === type).sort((a, b) => (b.timestamp ?? b.created_at).localeCompare(a.timestamp ?? a.created_at)) }))
    .filter((group) => group.objects.length > 0)
}
