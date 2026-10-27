import type { MessageKey } from '$lib/i18n.svelte'
import { startOfDay } from '$lib/dates'
import type { GridGroup, GridItem, GridRows, GridSettings, GridShow, GridSort, ObjectType, ViewSettings } from '$lib/tauri/types'

export const DEFAULT_GRID: GridSettings = { group: 'type', sort: 'date', direction: 'desc', rows: 'scroll', tags: true, show: [] }

export const DEFAULT_VIEW: ViewSettings = { chat: { sources: false }, grid: DEFAULT_GRID }

export const GRID_GROUPS: { value: GridGroup; label: MessageKey; icon: string }[] = [
  { value: 'type', label: 'chats.grid.group.type', icon: 'lucide:shapes' },
  { value: 'kind', label: 'chats.grid.group.kind', icon: 'lucide:layers' },
  { value: 'modified', label: 'chats.grid.group.modified', icon: 'lucide:calendar' },
  { value: 'source', label: 'chats.grid.group.source', icon: 'lucide:messages-square' },
  { value: 'sender', label: 'chats.grid.group.sender', icon: 'lucide:user' },
  { value: 'tag', label: 'chats.grid.group.tag', icon: 'lucide:tag' },
  { value: 'group', label: 'chats.grid.group.group', icon: 'lucide:folder' },
  { value: 'name', label: 'chats.grid.group.name', icon: 'lucide:a-large-small' },
  { value: 'none', label: 'chats.grid.group.none', icon: 'lucide:layout-grid' }
]

export const GRID_SORTS: { value: GridSort; label: MessageKey }[] = [
  { value: 'date', label: 'chats.grid.sort.date' },
  { value: 'name', label: 'chats.grid.sort.name' },
  { value: 'size', label: 'chats.grid.sort.size' },
  { value: 'kind', label: 'chats.grid.sort.kind' },
  { value: 'sender', label: 'chats.grid.sort.sender' },
  { value: 'reactions', label: 'chats.grid.sort.reactions' }
]

export const GRID_SHOWS: { value: GridShow; label: MessageKey; icon: string }[] = [
  { value: 'messages', label: 'chats.grid.show.messages', icon: 'lucide:message-square' },
  { value: 'files', label: 'chats.grid.show.files', icon: 'lucide:file-text' },
  { value: 'media', label: 'chats.grid.show.media', icon: 'lucide:image' },
  { value: 'links', label: 'chats.grid.show.links', icon: 'lucide:link-2' },
  { value: 'records', label: 'chats.grid.show.records', icon: 'lucide:receipt' },
  { value: 'events', label: 'chats.grid.show.events', icon: 'lucide:calendar-check' }
]

export type Tile = 'message' | 'mail' | 'file' | 'media' | 'link' | 'record' | 'event' | 'content'

export function tileOf(item: Pick<GridItem, 'kind'>): Tile {
  switch (item.kind) {
    case 'image':
    case 'video':
      return 'media'
    case 'message':
    case 'mail':
    case 'file':
    case 'link':
    case 'record':
    case 'event':
    case 'content':
      return item.kind
    default:
      return 'file'
  }
}

export function showOf(item: Pick<GridItem, 'kind'>): GridShow {
  switch (tileOf(item)) {
    case 'message':
    case 'mail':
      return 'messages'
    case 'media':
      return 'media'
    case 'link':
      return 'links'
    case 'record':
      return 'records'
    case 'event':
      return 'events'
    default:
      return 'files'
  }
}

export function filterShow(items: GridItem[], show: GridShow[]): GridItem[] {
  if (!show.length) return items
  return items.filter((item) => show.includes(showOf(item)))
}

export function toggleShow(show: GridShow[], value: GridShow | 'all'): GridShow[] {
  if (value === 'all') return []
  const next = show.includes(value) ? show.filter((s) => s !== value) : [...show, value]
  return next.length === GRID_SHOWS.length ? [] : GRID_SHOWS.map((s) => s.value).filter((s) => next.includes(s))
}

export type FileIcon = 'pdf' | 'svg' | 'image' | 'video' | 'audio' | 'font' | 'archive' | 'csv' | 'sheet' | 'slides' | 'word' | 'code' | 'markdown' | 'text' | 'receipt' | 'record' | 'file' | 'message' | 'mail' | 'link' | 'event' | 'content'

export const ICONS: Record<FileIcon, string> = {
  pdf: 'lucide:file-text',
  svg: 'lucide:file-image',
  image: 'lucide:file-image',
  video: 'lucide:file-video',
  audio: 'lucide:file-audio',
  font: 'lucide:file-type',
  archive: 'lucide:file-archive',
  csv: 'lucide:file-spreadsheet',
  sheet: 'lucide:file-spreadsheet',
  slides: 'lucide:presentation',
  word: 'lucide:file-pen-line',
  code: 'lucide:file-code',
  markdown: 'lucide:file-json',
  text: 'lucide:file-text',
  receipt: 'lucide:receipt',
  record: 'lucide:file-search',
  file: 'lucide:file',
  message: 'lucide:message-square',
  mail: 'lucide:mail',
  link: 'lucide:link-2',
  event: 'lucide:calendar-check',
  content: 'lucide:library'
}

const CODE = ['js', 'ts', 'tsx', 'jsx', 'json', 'html', 'htm', 'css', 'scss', 'less', 'go', 'py', 'rs', 'java', 'c', 'cpp', 'h', 'rb', 'php', 'sh', 'yml', 'yaml', 'xml', 'vue', 'svelte', 'sql', 'toml']
const ARCHIVE = ['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz']
const FONT = ['ttf', 'otf', 'woff', 'woff2', 'eot']
const IMAGE = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'avif', 'heic', 'tiff']
const VIDEO = ['mp4', 'mov', 'webm', 'mkv', 'avi', 'm4v']
const AUDIO = ['mp3', 'wav', 'ogg', 'flac', 'm4a', 'aac']

export function extensionOf(name: string): string {
  const i = name.lastIndexOf('.')
  return i >= 0 ? name.slice(i + 1).toLowerCase() : ''
}

function fromExtension(ext: string): FileIcon {
  if (ext === 'pdf') return 'pdf'
  if (ext === 'svg') return 'svg'
  if (IMAGE.includes(ext)) return 'image'
  if (VIDEO.includes(ext)) return 'video'
  if (AUDIO.includes(ext)) return 'audio'
  if (FONT.includes(ext)) return 'font'
  if (ARCHIVE.includes(ext)) return 'archive'
  if (ext === 'csv') return 'csv'
  if (['xls', 'xlsx', 'ods', 'numbers'].includes(ext)) return 'sheet'
  if (['ppt', 'pptx', 'odp', 'key'].includes(ext)) return 'slides'
  if (['doc', 'docx', 'odt', 'rtf', 'pages'].includes(ext)) return 'word'
  if (CODE.includes(ext)) return 'code'
  if (['md', 'markdown'].includes(ext)) return 'markdown'
  if (['txt', 'log', 'eml'].includes(ext)) return 'text'
  return 'file'
}

function fileIcon(name: string, mime: string): FileIcon {
  if (mime === '' || mime === 'application/octet-stream' || mime === 'text/plain') {
    const guess = fromExtension(extensionOf(name))
    if (guess !== 'file') return guess
    return mime === 'text/plain' ? 'text' : 'file'
  }
  if (mime === 'application/pdf') return 'pdf'
  if (mime === 'image/svg+xml') return 'svg'
  if (mime.startsWith('image/')) return 'image'
  if (mime.startsWith('video/')) return 'video'
  if (mime.startsWith('audio/')) return 'audio'
  if (mime.startsWith('font/') || mime.includes('font')) return 'font'
  if (['zip', 'compressed', 'tar', '7z', 'rar', 'gzip'].some((k) => mime.includes(k))) return 'archive'
  if (mime === 'text/csv') return 'csv'
  if (mime.includes('spreadsheet') || mime.includes('ms-excel')) return 'sheet'
  if (mime.includes('presentation') || mime.includes('powerpoint')) return 'slides'
  if (mime.includes('word') || mime === 'application/msword' || mime.includes('opendocument.text')) return 'word'
  if (mime === 'application/json' || mime.includes('javascript') || mime.includes('ecmascript') || mime.includes('xml') || mime.includes('yaml') || mime === 'text/html') return 'code'
  if (mime === 'text/markdown' || mime === 'text/x-markdown') return 'markdown'
  if (mime.startsWith('text/')) return 'text'
  return 'file'
}

export function iconName(item: Pick<GridItem, 'name' | 'content_type' | 'kind' | 'record'>): FileIcon {
  switch (item.kind) {
    case 'message':
    case 'mail':
    case 'link':
    case 'event':
    case 'content':
      return item.kind
    case 'record':
      return item.record?.kind === 'receipt' ? 'receipt' : 'record'
    default:
      return fileIcon(item.name, item.content_type ?? '')
  }
}

export function iconFor(item: Pick<GridItem, 'name' | 'content_type' | 'kind' | 'record'>): string {
  return ICONS[iconName(item)]
}

export type Title = { key: MessageKey } | { text: string }

export interface Bucket {
  key: string
  title: Title
  order: number
}

export interface GridBucket extends Bucket {
  items: GridItem[]
}

export interface GridContext {
  tagsOf: (item: GridItem) => string[]
  groupsOf: (conversationId: string) => string[]
}

const NO_CONTEXT: GridContext = { tagsOf: () => [], groupsOf: () => [] }

const TYPES: Record<string, Bucket> = {
  message: { key: 'message', title: { key: 'chats.grid.type.messages' }, order: 0 },
  mail: { key: 'mail', title: { key: 'chats.grid.type.mail' }, order: 1 },
  record: { key: 'record', title: { key: 'chats.grid.type.records' }, order: 2 },
  event: { key: 'event', title: { key: 'chats.grid.type.events' }, order: 3 },
  link: { key: 'link', title: { key: 'chats.grid.type.links' }, order: 4 },
  content: { key: 'content', title: { key: 'chats.grid.type.content' }, order: 5 },
  image: { key: 'image', title: { key: 'chats.grid.type.images' }, order: 6 },
  video: { key: 'video', title: { key: 'chats.grid.type.videos' }, order: 7 },
  audio: { key: 'audio', title: { key: 'chats.grid.type.audio' }, order: 8 },
  doc: { key: 'doc', title: { key: 'chats.grid.type.documents' }, order: 9 },
  sheet: { key: 'sheet', title: { key: 'chats.grid.type.sheets' }, order: 10 },
  slides: { key: 'slides', title: { key: 'chats.grid.type.slides' }, order: 11 },
  archive: { key: 'archive', title: { key: 'chats.grid.type.archives' }, order: 12 },
  code: { key: 'code', title: { key: 'chats.grid.type.code' }, order: 13 },
  font: { key: 'font', title: { key: 'chats.grid.type.fonts' }, order: 14 },
  other: { key: 'other', title: { key: 'chats.grid.type.other' }, order: 15 }
}

export function typeBucket(item: GridItem): Bucket {
  switch (iconName(item)) {
    case 'message':
      return TYPES.message
    case 'mail':
      return TYPES.mail
    case 'receipt':
    case 'record':
      return TYPES.record
    case 'event':
      return TYPES.event
    case 'link':
      return TYPES.link
    case 'content':
      return TYPES.content
    case 'image':
    case 'svg':
      return TYPES.image
    case 'video':
      return TYPES.video
    case 'audio':
      return TYPES.audio
    case 'pdf':
    case 'word':
    case 'text':
    case 'markdown':
      return TYPES.doc
    case 'sheet':
    case 'csv':
      return TYPES.sheet
    case 'slides':
      return TYPES.slides
    case 'archive':
      return TYPES.archive
    case 'code':
      return TYPES.code
    case 'font':
      return TYPES.font
    default:
      return TYPES.other
  }
}

const KINDS: Record<GridShow, Bucket> = {
  messages: { key: 'messages', title: { key: 'chats.grid.show.messages' }, order: 0 },
  records: { key: 'records', title: { key: 'chats.grid.show.records' }, order: 1 },
  events: { key: 'events', title: { key: 'chats.grid.show.events' }, order: 2 },
  links: { key: 'links', title: { key: 'chats.grid.show.links' }, order: 3 },
  media: { key: 'media', title: { key: 'chats.grid.show.media' }, order: 4 },
  files: { key: 'files', title: { key: 'chats.grid.show.files' }, order: 5 }
}

export function kindBucket(item: GridItem): Bucket {
  if (item.kind === 'record' && item.record?.kind === 'receipt') return { key: 'receipts', title: { key: 'chats.grid.kind.receipts' }, order: 1 }
  return KINDS[showOf(item)]
}

const DAY = 86_400_000

export function modifiedBucket(item: GridItem, now: Date = new Date()): Bucket {
  const at = item.at ? new Date(item.at).getTime() : NaN
  if (Number.isNaN(at)) return { key: 'nodate', title: { key: 'chats.grid.modified.none' }, order: 5 }
  const today = startOfDay(now)
  if (at >= today) return { key: 'today', title: { key: 'chats.grid.modified.today' }, order: 0 }
  if (at >= today - 7 * DAY) return { key: 'week', title: { key: 'chats.grid.modified.week' }, order: 1 }
  if (at >= today - 30 * DAY) return { key: 'month', title: { key: 'chats.grid.modified.month' }, order: 2 }
  return { key: 'earlier', title: { key: 'chats.grid.modified.earlier' }, order: 3 }
}

function textBucket(value: string, empty: MessageKey): Bucket {
  const text = value.trim()
  return text ? { key: text.toLowerCase(), title: { text }, order: 1 } : { key: '', title: { key: empty }, order: 2 }
}

function nameBucket(item: GridItem): Bucket {
  const first = (item.name.trim()[0] ?? '#').toLocaleUpperCase()
  const letter = first.toLocaleLowerCase() !== first
  return { key: letter ? first : '#', title: { text: letter ? first : '#' }, order: letter ? (first.codePointAt(0) ?? 0) : 1_000_000 }
}

export function bucketsOf(item: GridItem, by: GridGroup, now: Date = new Date(), context: GridContext = NO_CONTEXT): Bucket[] {
  switch (by) {
    case 'kind':
      return [kindBucket(item)]
    case 'type':
      return [typeBucket(item)]
    case 'modified':
      return [modifiedBucket(item, now)]
    case 'source':
      return [textBucket(item.conversation_name || item.provider_type, 'chats.grid.source.unknown')]
    case 'sender':
      return [textBucket(item.sender, 'chats.grid.sender.unknown')]
    case 'tag': {
      const tags = context.tagsOf(item)
      return tags.length ? tags.map((tag) => textBucket(tag, 'chats.grid.tag.none')) : [textBucket('', 'chats.grid.tag.none')]
    }
    case 'group': {
      const groups = context.groupsOf(item.conversation_id)
      return groups.length ? groups.map((group) => textBucket(group, 'chats.grid.groups.none')) : [textBucket('', 'chats.grid.groups.none')]
    }
    default:
      return [nameBucket(item)]
  }
}

export function groupItems(items: GridItem[], by: GridGroup, now: Date = new Date(), context: GridContext = NO_CONTEXT): GridBucket[] | null {
  if (by === 'none') return null
  const map = new Map<string, GridBucket>()
  for (const item of items) {
    for (const bucket of bucketsOf(item, by, now, context)) {
      let group = map.get(bucket.key)
      if (!group) {
        group = { ...bucket, items: [] }
        map.set(bucket.key, group)
      }
      if (!group.items.includes(item)) group.items.push(item)
    }
  }
  const label = (b: GridBucket) => ('text' in b.title ? b.title.text : b.title.key)
  return [...map.values()].sort((a, b) => a.order - b.order || label(a).localeCompare(label(b)))
}

function sortKey(item: GridItem, sort: GridSort): string | number {
  switch (sort) {
    case 'name':
      return item.name.toLocaleLowerCase()
    case 'size':
      return item.size
    case 'kind':
      return `${String(typeBucket(item).order).padStart(2, '0')}${item.name.toLocaleLowerCase()}`
    case 'sender':
      return item.sender.toLocaleLowerCase()
    case 'reactions':
      return item.reactions
    default:
      return item.at
  }
}

export function sortItems(items: GridItem[], sort: GridSort, direction: 'asc' | 'desc'): GridItem[] {
  const sign = direction === 'asc' ? 1 : -1
  return [...items].sort((a, b) => {
    const x = sortKey(a, sort)
    const y = sortKey(b, sort)
    const primary = typeof x === 'number' && typeof y === 'number' ? x - y : String(x).localeCompare(String(y))
    return sign * primary || a.name.localeCompare(b.name) || a.id.localeCompare(b.id)
  })
}

export function searchItems(items: GridItem[], query: string): GridItem[] {
  const words = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean)
  if (!words.length) return items
  return items.filter((item) => {
    const hay = [item.name, item.excerpt, item.sender, item.conversation_name, item.provider_type, item.content_type, item.url ?? '', extensionOf(item.name), ...item.tags].join(' ').toLocaleLowerCase()
    return words.every((word) => hay.includes(word))
  })
}

export function rowsFor(settings: Pick<GridSettings, 'group' | 'rows'>): GridRows {
  return settings.group === 'none' ? 'wrap' : settings.rows
}

export function arrange(items: GridItem[], settings: GridSettings, query: string, now: Date = new Date(), context: GridContext = NO_CONTEXT): { flat: GridItem[]; groups: GridBucket[] | null; rows: GridRows } {
  const flat = sortItems(searchItems(filterShow(items, settings.show ?? []), query), settings.sort, settings.direction)
  return { flat, groups: groupItems(flat, settings.group, now, context), rows: rowsFor(settings) }
}

export function selectItems(selected: Set<string>, id: string, how: 'only' | 'toggle' | 'range', order: string[], anchor: string | null): Set<string> {
  if (how === 'toggle') {
    const next = new Set(selected)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    return next
  }
  if (how === 'range' && anchor && order.includes(anchor) && order.includes(id)) {
    const [from, to] = [order.indexOf(anchor), order.indexOf(id)].sort((a, b) => a - b)
    return new Set([...selected, ...order.slice(from, to + 1)])
  }
  return new Set([id])
}

export function withGrid(view: ViewSettings | null | undefined, patch: Partial<GridSettings>): ViewSettings {
  const base = view ?? DEFAULT_VIEW
  return { ...base, grid: { ...DEFAULT_GRID, ...base.grid, ...patch } }
}

export function opensInApp(item: Pick<GridItem, 'kind' | 'attachment_id' | 'path'>): boolean {
  return (item.kind === 'file' || item.kind === 'image' || item.kind === 'video' || item.kind === 'record') && (!!item.attachment_id || !!item.path)
}

export function eventDate(item: Pick<GridItem, 'at' | 'presentation' | 'record'>): string {
  const invite = (item.presentation?.invite ?? null) as { starts_at?: unknown } | null
  const starts = typeof invite?.starts_at === 'string' ? invite.starts_at : typeof item.record?.starts_at === 'string' ? item.record.starts_at : null
  return starts ?? item.at
}

export const TOOLBAR = {
  value: 'hidden @min-[38rem]:inline',
  control: 'hidden @min-[26rem]:flex',
  more: 'flex @min-[26rem]:hidden'
}

export function gridObject(item: Pick<GridItem, 'message_id' | 'attachment_id'>): { objectType: ObjectType; objectId: string } | null {
  if (item.message_id && item.attachment_id) return { objectType: 'attachment', objectId: `${item.message_id}/${item.attachment_id}` }
  if (item.message_id) return { objectType: 'message', objectId: item.message_id }
  return null
}
