import { describe, expect, it } from 'vitest'
import type { GridItem } from '$lib/tauri/types'
import { arrange, DEFAULT_GRID, eventDate, filterShow, groupItems, iconFor, iconName, opensInApp, rowsFor, searchItems, selectItems, showOf, sortItems, tileOf, toggleShow, TOOLBAR, withGrid } from './grid'

const now = new Date('2026-10-15T12:00:00')

function item(id: string, patch: Partial<GridItem> = {}): GridItem {
  return {
    id,
    kind: 'file',
    name: `${id}.pdf`,
    content_type: 'application/pdf',
    size: 1000,
    at: '2026-10-15T09:00:00',
    message_id: `m-${id}`,
    attachment_id: `a-${id}`,
    conversation_id: 'c1',
    conversation_name: 'Elkjøp',
    provider_type: 'mock',
    sender: 'Elkjøp',
    sender_id: null,
    excerpt: '',
    reactions: 0,
    path: null,
    url: null,
    tags: [],
    record: null,
    presentation: null,
    ...patch
  }
}

const items = [
  item('kvittering', { name: 'Kvittering-5521093.pdf', size: 52_000, at: '2026-10-15T08:00:00' }),
  item('cover', { kind: 'image', name: 'Cover photo.png', content_type: 'image/png', size: 900_000, at: '2026-10-12T08:00:00', sender: 'Mia Sund', conversation_id: 'c2', conversation_name: 'Design', url: 'https://example.org/cover.png' }),
  item('hello', { kind: 'message', name: 'Lunsj?', content_type: 'text', size: 0, attachment_id: null, at: '2026-10-14T10:00:00', sender: 'Mia Sund', conversation_id: 'c2', conversation_name: 'Design', excerpt: 'Lunsj på **fredag**?', reactions: 3 }),
  item('mail', { kind: 'mail', name: 'Kontrakt Kunde AS', content_type: 'html', size: 0, attachment_id: null, at: '2026-09-25T08:00:00', sender: 'Anna Berg', conversation_id: 'c3', conversation_name: 'Kunde AS', excerpt: 'Her er kontrakten' }),
  item('link', { kind: 'link', name: 'northwind.example/agenda', content_type: '', size: 0, attachment_id: null, at: '2026-10-13T08:00:00', sender: 'Mia Sund', conversation_id: 'c2', conversation_name: 'Design', url: 'https://northwind.example/agenda', reactions: 1 }),
  item('receipt', { kind: 'record', name: 'Nordlys Elektro · 11916.00 NOK', content_type: '', size: 0, attachment_id: null, at: '2026-10-05T15:41:07', record: { kind: 'receipt', vendor: 'Nordlys Elektro' }, tags: ['Nordlys Elektro', '11916 NOK'] }),
  item('invite', { kind: 'event', name: 'Sprintdemo', content_type: 'html', size: 0, attachment_id: null, at: '2026-10-10T08:00:00', sender: 'Jonas Lie', conversation_id: 'c3', conversation_name: 'Kunde AS', presentation: { kind: 'invite', invite: { starts_at: '2026-10-16T14:00:00' } } }),
  item('gif', { kind: 'content', name: 'Party parrot', content_type: 'image/gif', size: 0, attachment_id: null, at: '2026-10-01T08:00:00', sender: '' })
]

const ids = (list: GridItem[]) => list.map((i) => i.id)
const keys = (groups: ReturnType<typeof groupItems>) => groups?.map((g) => [g.key, ids(g.items)])

describe('tile kinds', () => {
  it('resolves each grid item to the tile it renders as and the Show bucket it belongs to', () => {
    expect(items.map((i) => tileOf(i))).toEqual(['file', 'media', 'message', 'mail', 'link', 'record', 'event', 'content'])
    expect(tileOf(item('v', { kind: 'video' }))).toBe('media')
    expect(items.map((i) => showOf(i))).toEqual(['files', 'media', 'messages', 'messages', 'links', 'records', 'events', 'files'])
    expect(iconName(items[0])).toBe('pdf')
    expect(iconName(items[5])).toBe('receipt')
    expect(iconFor(items[2])).toBe('lucide:message-square')
    expect(iconFor(items[7])).toBe('lucide:library')
    expect(opensInApp(items[0])).toBe(true)
    expect(opensInApp(items[2])).toBe(false)
    expect(opensInApp(item('r', { kind: 'record', attachment_id: null, path: '/cache/r.pdf' }))).toBe(true)
    expect(eventDate(items[6])).toBe('2026-10-16T14:00:00')
    expect(eventDate(items[0])).toBe(items[0].at)
  })
})

describe('the Show filter', () => {
  it('shows everything when empty and any mix of kinds otherwise', () => {
    expect(filterShow(items, [])).toHaveLength(items.length)
    expect(ids(filterShow(items, ['messages']))).toEqual(['hello', 'mail'])
    expect(ids(filterShow(items, ['files', 'records']))).toEqual(['kvittering', 'receipt', 'gif'])
    expect(ids(filterShow(items, ['media', 'links', 'events']))).toEqual(['cover', 'link', 'invite'])
  })

  it('toggles kinds, keeps their order and folds back to all', () => {
    expect(toggleShow([], 'records')).toEqual(['records'])
    expect(toggleShow(['records'], 'messages')).toEqual(['messages', 'records'])
    expect(toggleShow(['messages', 'records'], 'records')).toEqual(['messages'])
    expect(toggleShow(['messages'], 'all')).toEqual([])
    expect(toggleShow(['messages', 'files', 'media', 'links', 'records'], 'events')).toEqual([])
  })
})

describe('grouping and sorting across kinds', () => {
  const context = {
    tagsOf: (item: { conversation_id: string }) => (item.conversation_id === 'c2' ? ['Design', 'Team'] : item.conversation_id === 'c3' ? ['Kunde AS'] : []),
    groupsOf: (id: string) => (id === 'c3' ? ['Clients'] : [])
  }

  it('groups by type with messages first, and by kind, modified, source, sender, tag and group', () => {
    expect(keys(groupItems(items, 'type', now))).toEqual([
      ['message', ['hello']],
      ['mail', ['mail']],
      ['record', ['receipt']],
      ['event', ['invite']],
      ['link', ['link']],
      ['content', ['gif']],
      ['image', ['cover']],
      ['doc', ['kvittering']]
    ])
    expect(keys(groupItems(items, 'kind', now))).toEqual([
      ['messages', ['hello', 'mail']],
      ['receipts', ['receipt']],
      ['events', ['invite']],
      ['links', ['link']],
      ['media', ['cover']],
      ['files', ['kvittering', 'gif']]
    ])
    expect(keys(groupItems(items, 'modified', now))?.map(([k]) => k)).toEqual(['today', 'week', 'month'])
    expect(keys(groupItems(items, 'source', now))?.map(([k]) => k)).toEqual(['design', 'elkjøp', 'kunde as'])
    expect(keys(groupItems(items, 'sender', now))?.at(-1)).toEqual(['', ['gif']])
    expect(keys(groupItems(items, 'tag', now, context))).toEqual([
      ['design', ['cover', 'hello', 'link']],
      ['kunde as', ['mail', 'invite']],
      ['team', ['cover', 'hello', 'link']],
      ['', ['kvittering', 'receipt', 'gif']]
    ])
    expect(keys(groupItems(items, 'group', now, context))).toEqual([
      ['clients', ['mail', 'invite']],
      ['', ['kvittering', 'cover', 'hello', 'link', 'receipt', 'gif']]
    ])
    expect(groupItems(items, 'none', now)).toBeNull()
  })

  it('sorts mixed kinds by date, name, size, kind, sender and reactions', () => {
    expect(ids(sortItems(items, 'date', 'desc'))).toEqual(['kvittering', 'hello', 'link', 'cover', 'invite', 'receipt', 'gif', 'mail'])
    expect(ids(sortItems(items, 'reactions', 'desc')).slice(0, 2)).toEqual(['hello', 'link'])
    expect(ids(sortItems(items, 'size', 'desc')).slice(0, 2)).toEqual(['cover', 'kvittering'])
    expect(ids(sortItems(items, 'kind', 'asc')).slice(0, 4)).toEqual(['hello', 'mail', 'receipt', 'invite'])
    expect(ids(sortItems(items, 'name', 'asc')).slice(0, 3)).toEqual(['cover', 'mail', 'kvittering'])
    expect(ids(sortItems(items, 'sender', 'asc'))[0]).toBe('gif')
  })

  it('searches excerpts, links and tags and arranges what the Show filter lets through', () => {
    expect(ids(searchItems(items, 'fredag'))).toEqual(['hello'])
    expect(ids(searchItems(items, 'agenda'))).toEqual(['link'])
    expect(ids(searchItems(items, 'nordlys nok'))).toEqual(['receipt'])
    const arranged = arrange(items, { ...DEFAULT_GRID, show: ['messages', 'links'], group: 'source', sort: 'date', direction: 'desc' }, '', now)
    expect(ids(arranged.flat)).toEqual(['hello', 'link', 'mail'])
    expect(keys(arranged.groups)).toEqual([
      ['design', ['hello', 'link']],
      ['kunde as', ['mail']]
    ])
  })
})

describe('rows, selection, settings and the toolbar', () => {
  it('scrolls a row per group, wraps when asked, and wraps a flat grid', () => {
    expect(rowsFor({ group: 'type', rows: 'scroll' })).toBe('scroll')
    expect(rowsFor({ group: 'type', rows: 'wrap' })).toBe('wrap')
    expect(rowsFor({ group: 'none', rows: 'scroll' })).toBe('wrap')
  })

  it('selects one, toggles and extends a range', () => {
    const order = ids(items)
    let selected = selectItems(new Set(), 'cover', 'only', order, null)
    selected = selectItems(selected, 'mail', 'range', order, 'cover')
    expect([...selected].sort()).toEqual(['cover', 'hello', 'mail'])
    expect([...selectItems(selected, 'hello', 'toggle', order, 'cover')].sort()).toEqual(['cover', 'mail'])
  })

  it('patches the grid settings and keeps the rest of the view', () => {
    const view = withGrid({ chat: { sources: true }, grid: DEFAULT_GRID }, { show: ['records'] })
    expect(view.chat.sources).toBe(true)
    expect(view.grid).toEqual({ ...DEFAULT_GRID, show: ['records'] })
    expect(withGrid(null, { group: 'tag' }).grid.group).toBe('tag')
  })

  it('hides values before controls and shows the more menu only when controls are hidden', () => {
    const at = (cls: string) => Number(/@min-\[(\d+)rem\]/.exec(cls)?.[1])
    expect(at(TOOLBAR.value)).toBeGreaterThan(at(TOOLBAR.control))
    expect(at(TOOLBAR.control)).toBe(at(TOOLBAR.more))
    expect(TOOLBAR.value.startsWith('hidden ')).toBe(true)
    expect(TOOLBAR.control.startsWith('hidden ')).toBe(true)
    expect(TOOLBAR.more).toMatch(/^flex .*:hidden$/)
  })
})
