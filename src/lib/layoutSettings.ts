import { get, writable } from 'svelte/store'
import { getAppSettings, getSidebarLayout, putAppSetting, putSidebarLayout } from '$lib/tauri/api'
import type { SidebarEntry, SidebarLayout } from '$lib/tauri/types'
import { t, type MessageKey as I18nKey } from '$lib/i18n.svelte'

export const LAYOUT_PREFIXES = ['sidebar.', 'list.', 'messages.', 'shortcuts.'] as const

export const layoutSettings = writable<Record<string, string>>({})

export function isLayoutKey(key: string): boolean {
  return LAYOUT_PREFIXES.some((prefix) => key.startsWith(prefix) && key.length > prefix.length)
}

export function slug(value: string): string {
  return value
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

export function viewIconKey(view: string): string {
  return `sidebar.icon.${slug(view)}`
}

export function viewLabelKey(view: string): string {
  return `sidebar.label.${slug(view)}`
}

export function messageLayoutKey(kind: string): string {
  return `messages.template.${slug(kind)}`
}

export function calendarCollapsedKey(providerId: string): string {
  return `sidebar.calendars.${slug(providerId)}.collapsed`
}

export function sectionIconKey(section: string): string {
  return `list.section.${slug(section)}.icon`
}

export function sectionBackgroundKey(section: string): string {
  return `list.section.${slug(section)}.background`
}

export function sectionFitKey(section: string): string {
  return `list.section.${slug(section)}.fit`
}

export function sectionBlendKey(section: string): string {
  return `list.section.${slug(section)}.blend`
}

export function sectionCollapsedKey(section: string): string {
  return `list.section.${slug(section)}.collapsed`
}

export function sectionNameKey(section: string): string {
  return `list.section.${slug(section)}.name`
}

export function groupExpandedKey(id: string): string {
  return `list.group.${slug(id)}.expanded`
}

export function segmentCollapsedKey(group: string, label: string): string {
  return `list.segment.${slug(group)}.${slug(label)}.collapsed`
}

export function flag(settings: Record<string, string>, key: string): boolean {
  return settings[key] === 'true'
}

export async function loadLayoutSettings(): Promise<void> {
  const all = await getAppSettings()
  layoutSettings.set(Object.fromEntries(Object.entries(all).filter(([key]) => isLayoutKey(key))))
}

export async function putLayoutSetting(key: string, value: string | null): Promise<void> {
  const next = { ...get(layoutSettings) }
  if (value) next[key] = value
  else delete next[key]
  layoutSettings.set(next)
  await putAppSetting(key, value ?? '')
}

export function toggleFlag(key: string): Promise<void> {
  return putLayoutSetting(key, flag(get(layoutSettings), key) ? null : 'true')
}

export type SortChoice = 'manual' | 'newest' | 'oldest' | 'unread' | 'sender'

export const SORT_CHOICES: { value: SortChoice; label: I18nKey }[] = [
  { value: 'manual', label: 'inbox.sort.manual' },
  { value: 'newest', label: 'inbox.sort.newest' },
  { value: 'oldest', label: 'inbox.sort.oldest' },
  { value: 'unread', label: 'inbox.sort.unread' },
  { value: 'sender', label: 'inbox.sort.sender' }
]

export function sortKey(key: string): string {
  return `sidebar.sort.${slug(key)}`
}

export function countKey(key: string): string {
  return `sidebar.count.${slug(key)}`
}

export function sidebarCollapsedKey(key: string): string {
  return `sidebar.section.${slug(key)}.collapsed`
}

export function sortOrder(choice: string | undefined): { sort: 'activity' | 'unread' | 'name'; direction: 'asc' | 'desc' } | null {
  switch (choice) {
    case 'newest':
      return { sort: 'activity', direction: 'desc' }
    case 'oldest':
      return { sort: 'activity', direction: 'asc' }
    case 'unread':
      return { sort: 'unread', direction: 'desc' }
    case 'sender':
      return { sort: 'name', direction: 'asc' }
    default:
      return null
  }
}

export const MESSAGE_DEFAULTS = {
  'messages.enter_sends': 'enter',
  'messages.collapse_lines': '16',
  'messages.emoji_size': 'large',
  'messages.show_edited': 'on',
  'messages.mail_summaries': 'ai',
  'messages.default_reply': 'reply',
  'messages.section_gap': '60',
  'messages.save_eyes': 'off'
} as const

export type MessageKey = keyof typeof MESSAGE_DEFAULTS

export function messageSetting(settings: Record<string, string>, key: MessageKey): string {
  return settings[key] || MESSAGE_DEFAULTS[key]
}

export function collapseLimitRem(settings: Record<string, string>): number {
  const lines = Number(messageSetting(settings, 'messages.collapse_lines'))
  return Number.isFinite(lines) && lines > 0 ? lines * 1.375 : 0
}

export type SidebarZone = 'top' | 'footer'

export interface SidebarSlot {
  zone: SidebarZone
  index: number
  folder?: number
}

export const sidebarLayout = writable<SidebarLayout>({ top: [], footer: [] })

export async function loadSidebarLayout(): Promise<SidebarLayout> {
  const layout = await getSidebarLayout()
  sidebarLayout.set(layout)
  return layout
}

export async function saveSidebarLayout(layout: SidebarLayout): Promise<SidebarLayout> {
  sidebarLayout.set(layout)
  const saved = await putSidebarLayout(layout)
  sidebarLayout.set(saved)
  return saved
}

export async function listenSidebarLayout(): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event')
  return listen<SidebarLayout>('sidebar-layout-updated', (event) => sidebarLayout.set(event.payload))
}

function newId(): string {
  return typeof crypto !== 'undefined' && 'randomUUID' in crypto ? crypto.randomUUID() : `${Date.now()}-${Math.random().toString(36).slice(2)}`
}

function listAt(layout: SidebarLayout, slot: SidebarSlot): SidebarEntry[] {
  const zone = layout[slot.zone]
  if (slot.folder === undefined) return zone
  const folder = zone[slot.folder]
  return folder && folder.kind === 'folder' ? folder.items : zone
}

function cloneLayout(layout: SidebarLayout): SidebarLayout {
  return JSON.parse(JSON.stringify(layout)) as SidebarLayout
}

export function entryAt(layout: SidebarLayout, slot: SidebarSlot): SidebarEntry | undefined {
  return listAt(layout, slot)[slot.index]
}

export function moveSidebarEntry(layout: SidebarLayout, from: SidebarSlot, to: SidebarSlot): SidebarLayout {
  const moving = entryAt(layout, from)
  if (!moving || ((moving.kind === 'folder' || moving.kind === 'heading') && to.folder !== undefined)) return layout
  const next = cloneLayout(layout)
  const source = listAt(next, from)
  const target = listAt(next, to)
  const [entry] = source.splice(from.index, 1)
  const sameList = from.zone === to.zone && from.folder === to.folder
  const index = sameList && to.index > from.index ? to.index - 1 : to.index
  target.splice(Math.max(0, Math.min(index, target.length)), 0, entry)
  return next
}

export function insertSidebarDivider(layout: SidebarLayout, at: SidebarSlot): SidebarLayout {
  const next = cloneLayout(layout)
  listAt(next, at).splice(at.index, 0, { kind: 'divider', id: newId() })
  return next
}

export function insertSidebarHeading(layout: SidebarLayout, zone: SidebarZone, name: string): SidebarLayout {
  const next = cloneLayout(layout)
  next[zone].push({ kind: 'heading', id: newId(), name: name.trim() || t('settings.sidebar.newHeading') })
  return next
}

export function insertSidebarFolder(layout: SidebarLayout, zone: SidebarZone, name: string, icon: string | null = null): SidebarLayout {
  const next = cloneLayout(layout)
  next[zone].push({ kind: 'folder', id: newId(), name: name.trim() || t('inbox.folder.default'), icon, collapsed: false, items: [] })
  return next
}

export function removeSidebarEntry(layout: SidebarLayout, slot: SidebarSlot): SidebarLayout {
  const next = cloneLayout(layout)
  const list = listAt(next, slot)
  const [entry] = list.splice(slot.index, 1)
  if (entry && entry.kind === 'folder') list.splice(slot.index, 0, ...entry.items)
  else if (entry && entry.kind === 'item') return layout
  return next
}

export function putSidebarEntry(layout: SidebarLayout, slot: SidebarSlot, patch: Partial<{ name: string; icon: string | null; collapsed: boolean }>): SidebarLayout {
  const next = cloneLayout(layout)
  const entry = listAt(next, slot)[slot.index]
  if (entry?.kind === 'folder') Object.assign(entry, patch)
  else if (entry?.kind === 'heading' && patch.name !== undefined) entry.name = patch.name
  else return layout
  return next
}

export type SectionBreak = 'none' | 'day' | 'gap'

export function sectionBreak(settings: Record<string, string>, previous: string | null | undefined, next: string): SectionBreak {
  if (!previous) return 'none'
  const value = messageSetting(settings, 'messages.section_gap')
  if (value === 'off') return 'none'
  const before = new Date(previous)
  const after = new Date(next)
  if (before.toDateString() !== after.toDateString()) return 'day'
  const minutes = Number(value)
  if (!Number.isFinite(minutes) || minutes <= 0) return 'none'
  return after.getTime() - before.getTime() >= minutes * 60000 ? 'gap' : 'none'
}
