import { describe, expect, it, vi } from 'vitest'
import type { ContentGroup, ContentItem } from '$lib/tauri/types'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
  convertFileSrc: vi.fn((path: string) => `asset://localhost/${path}`)
}))

import type { GifResult } from '$lib/tauri/types'
import { addFiles, addContent, removeAttachment, readFileAsBase64, toOutgoing, pushRecent, loadRecent, saveRecent, sortLibrary, filterLibrary, parseTags, contentSrc, favouriteItems, gifSrc, appendResults, favouriteInput, savedUrls, attachmentInput, mostUsed, recentlyUsed, tagSuggestions, toggleWord, isVideo, isTemplate, isVisual, contentSections, placementIndex, sourceLink, composerVariables, nestValues, firstName, templateFromMessage, quickReactions, noteReaction, DEFAULT_REACTIONS } from './media'

function item(overrides: Partial<ContentItem> = {}): ContentItem {
  return {
    id: 'm1',
    profile_id: 'p1',
    kind: 'gif',
    name: 'dance',
    source_url: 'https://media.tenor.com/dance.gif',
    path: 'media/dance.gif',
    mime_type: 'image/gif',
    width: 200,
    height: 100,
    tags: ['party', 'dance'],
    is_favourite: false,
    used_count: 0,
    last_used_at: null,
    created_at: '2026-10-01T00:00:00Z',
    favourite_position: null,
    query: null,
    keywords: [],
    body: null,
    source_kind: null,
    source_message_id: null,
    source_conversation_id: null,
    source_provider_id: null,
    source_person_id: null,
    source_action_id: null,
    content_hash: null,
    size_bytes: null,
    duration_ms: null,
    group_ids: [],
    file_path: null,
    ...overrides
  }
}

describe('pending attachments', () => {
  it('adds files and content and removes by id', () => {
    const file = new File(['x'], 'a.png', { type: 'image/png' })
    let list = addFiles([], [file])
    list = addContent(list, item())
    expect(list).toHaveLength(2)
    expect(list[0].file).toBe(file)
    expect(list[1].content?.id).toBe('m1')
    list = removeAttachment(list, list[0].id)
    expect(list).toHaveLength(1)
    expect(list[0].content?.id).toBe('m1')
  })

  it('does not add the same content twice', () => {
    const list = addContent(addContent([], item()), item())
    expect(list).toHaveLength(1)
  })

  it('reads a file as bare base64', async () => {
    const file = new File(['hello'], 'hello.txt', { type: 'text/plain' })
    expect(await readFileAsBase64(file)).toBe('aGVsbG8=')
  })

  it('maps to outgoing attachments', async () => {
    const file = new File(['hi'], 'hi.txt', { type: 'text/plain' })
    const out = await toOutgoing(addContent(addFiles([], [file]), item()))
    expect(out).toEqual([
      { name: 'hi.txt', mime_type: 'text/plain', data: 'aGk=' },
      { content_id: 'm1' }
    ])
  })
})

describe('recent emoji', () => {
  it('keeps the newest first without duplicates and caps the length', () => {
    let list: string[] = []
    for (let i = 0; i < 30; i++) list = pushRecent(list, `e${i}`)
    expect(list).toHaveLength(24)
    expect(list[0]).toBe('e29')
    list = pushRecent(list, 'e10')
    expect(list[0]).toBe('e10')
    expect(list.filter((e) => e === 'e10')).toHaveLength(1)
  })

  it('round trips through localStorage and survives junk', () => {
    const store = new Map<string, string>()
    vi.stubGlobal('localStorage', { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => store.set(k, v), removeItem: (k: string) => store.delete(k) })
    saveRecent(['😀', '🎉'], 'test_recent')
    expect(loadRecent('test_recent')).toEqual(['😀', '🎉'])
    localStorage.setItem('test_recent', '{bad')
    expect(loadRecent('test_recent')).toEqual([])
    vi.unstubAllGlobals()
  })
})

describe('library', () => {
  it('sorts favourites first then by last use', () => {
    const sorted = sortLibrary([
      item({ id: 'a', last_used_at: '2026-10-02T00:00:00Z' }),
      item({ id: 'b', is_favourite: true, last_used_at: '2026-09-01T00:00:00Z' }),
      item({ id: 'c', last_used_at: '2026-10-03T00:00:00Z' })
    ])
    expect(sorted.map((i) => i.id)).toEqual(['b', 'c', 'a'])
  })

  it('filters by name and tag and parses tags', () => {
    const items = [item({ id: 'a', name: 'dance' }), item({ id: 'b', name: 'cat', tags: ['pets'] })]
    expect(filterLibrary(items, 'pet').map((i) => i.id)).toEqual(['b'])
    expect(filterLibrary(items, '').map((i) => i.id)).toEqual(['a', 'b'])
    expect(parseTags('Party, dance  fun,party')).toEqual(['party', 'dance', 'fun'])
  })

  it('resolves a content source through the asset protocol', () => {
    expect(contentSrc(item())).toBe('asset://localhost/media/dance.gif')
    expect(contentSrc(item({ path: '' }))).toBe('https://media.tenor.com/dance.gif')
  })
})

function gif(overrides: Partial<GifResult> = {}): GifResult {
  return { id: 't1', source: 'tenor', kind: 'gif', title: 'Slow clap', url: 'https://t/1.gif', preview_url: 'https://t/1-tiny.gif', width: 10, height: 10, content_id: null, ...overrides }
}

function group(overrides: Partial<ContentGroup> = {}): ContentGroup {
  return { id: 'g1', profile_id: 'p1', parent_id: null, name: 'Team', kind: 'manual', rule: null, position: 0, is_collapsed: false, icon: null, content_ids: [], created_at: '2026-10-01T00:00:00Z', ...overrides }
}

describe('favourites and groups', () => {
  it('orders favourites by position and lists a group in its own order', () => {
    const items = [
      item({ id: 'a', is_favourite: true, favourite_position: 2 }),
      item({ id: 'b', is_favourite: true, favourite_position: -1 }),
      item({ id: 'c' }),
      item({ id: 'd', is_favourite: true, favourite_position: 0 })
    ]
    expect(favouriteItems(items).map((i) => i.id)).toEqual(['b', 'd', 'a'])
    expect(favouriteItems(items, group({ content_ids: ['c', 'b', 'gone'] })).map((i) => i.id)).toEqual(['c', 'b'])
    expect(favouriteItems(items, group({ content_ids: [] }))).toEqual([])
  })

  it('searches the saved query, keywords and template text too', () => {
    const items = [item({ id: 'a', name: 'x', tags: [], query: 'Slow clap' }), item({ id: 'b', name: 'y', tags: [], keywords: ['team spirit'] }), item({ id: 'c', name: 'z', tags: [], kind: 'reply', body: 'Takk for sist' })]
    expect(filterLibrary(items, 'clap').map((i) => i.id)).toEqual(['a'])
    expect(filterLibrary(items, 'team').map((i) => i.id)).toEqual(['b'])
    expect(filterLibrary(items, 'sist').map((i) => i.id)).toEqual(['c'])
  })
})

describe('gif results', () => {
  it('resolves library results locally and providers remotely', () => {
    expect(gifSrc(gif())).toBe('https://t/1-tiny.gif')
    expect(gifSrc(gif(), true)).toBe('https://t/1.gif')
    expect(gifSrc(gif({ source: 'library', url: 'media/x.gif', preview_url: 'media/x.gif' }))).toBe('asset://localhost/media/x.gif')
  })

  it('appends pages without repeats', () => {
    const first = [gif(), gif({ id: 'g1', source: 'giphy', url: 'https://g/1.gif' })]
    const next = appendResults(first, [gif(), gif({ id: 't2', url: 'https://g/1.gif' }), gif({ id: 't3', url: 'https://t/3.gif' })])
    expect(next.map((r) => r.id)).toEqual(['t1', 'g1', 't3'])
  })

  it('builds a favourite from a result and the query', () => {
    expect(favouriteInput(gif(), ' Clap ')).toEqual({ kind: 'gif', name: 'Slow clap', source_url: 'https://t/1.gif', query: 'Clap', tags: ['clap'], is_favourite: true, group_ids: undefined })
    expect(favouriteInput(gif({ title: '' }), '', false, ['Team'])).toEqual({ kind: 'gif', name: 'gif', source_url: 'https://t/1.gif', query: undefined, tags: [], is_favourite: false, group_ids: ['Team'] })
  })

  it('maps saved items by source url', () => {
    const saved = savedUrls([item({ id: 'a', source_url: 'https://t/1.gif' }), item({ id: 'b', source_url: null })])
    expect(saved.get('https://t/1.gif')?.id).toBe('a')
    expect(saved.size).toBe(1)
  })
})

describe('attachment favourites', () => {
  it('builds a favourite from a message attachment', () => {
    const attachment = { id: 'attachments/a1.gif', name: 'party.gif', content_type: 'image/gif', size: 10, is_inline: false, content_id: null }
    expect(attachmentInput(attachment, 'R0lG')).toEqual({ kind: 'gif', name: 'party', source_url: 'attachment:attachments/a1.gif', data: 'R0lG', mime_type: 'image/gif', is_favourite: true })
    expect(attachmentInput({ ...attachment, name: '.png', content_type: 'image/png' }, 'x').kind).toBe('image')
    expect(attachmentInput({ ...attachment, name: '.png', content_type: 'image/png' }, 'x').name).toBe('image')
  })
})

describe('content sections', () => {
  const library = [
    item({ id: 'a', name: 'party', tags: ['party', 'monday'], used_count: 5, last_used_at: '2026-10-10T00:00:00Z', width: 100, height: 200 }),
    item({ id: 'b', name: 'cat', tags: ['pets'], used_count: 9, last_used_at: '2026-10-01T00:00:00Z', is_favourite: true, width: 100, height: 50 }),
    item({ id: 'c', name: 'clip', kind: 'video', mime_type: 'video/mp4', tags: ['monday'], width: 100, height: 100 }),
    item({ id: 'd', name: 'Thanks', kind: 'reply', mime_type: 'text/plain', path: '', body: 'Takk {{ .person.first_name }}' })
  ]
  const ids = (items: { id: string }[]) => items.map((i) => i.id)
  const titles = (sections: ReturnType<typeof contentSections>) => sections.map((s) => [s.group?.name ?? 'rest', s.depth, ids(s.items)])

  it('stacks the user tree depth first and keeps a manual group in its own order', () => {
    const groups = [
      group({ id: 'work', name: 'Work', position: 1, content_ids: ['c', 'a'] }),
      group({ id: 'fun', name: 'Fun', position: 0, content_ids: ['b'] }),
      group({ id: 'deep', name: 'Deep', parent_id: 'work', content_ids: ['a'] })
    ]
    expect(titles(contentSections(groups, library))).toEqual([
      ['Fun', 0, ['b']],
      ['Work', 0, ['c', 'a']],
      ['Deep', 1, ['a']],
      ['rest', 0, ['d']]
    ])
  })

  it('renders an item in two nested groups in both sections', () => {
    const groups = [group({ id: 'outer', name: 'Outer', content_ids: ['a', 'b'] }), group({ id: 'inner', name: 'Inner', parent_id: 'outer', content_ids: ['a'] }), group({ id: 'innermost', name: 'Innermost', parent_id: 'inner', content_ids: ['a', 'c'] })]
    const sections = contentSections(groups, library)
    expect(sections.filter((s) => s.items.some((i) => i.id === 'a')).map((s) => s.group?.name)).toEqual(['Outer', 'Inner', 'Innermost'])
    expect(sections.map((s) => s.depth)).toEqual([0, 1, 2, 0])
    expect(ids(sections[sections.length - 1].items)).toEqual(['d'])
  })

  it('fills automatic groups from kind and tag, drops empty ones and hides what a collapsed group holds', () => {
    const groups = [
      group({ id: 'k', name: 'Videos', kind: 'kind', rule: 'video', position: 0 }),
      group({ id: 't', name: 'Mondays', kind: 'tag', rule: 'Monday', position: 1 }),
      group({ id: 'e', name: 'Files', kind: 'kind', rule: 'file', position: 2 }),
      group({ id: 'm', name: 'Empty folder', position: 3 }),
      group({ id: 'shut', name: 'Shut', position: 4, is_collapsed: true, content_ids: ['b'] }),
      group({ id: 'child', name: 'Child', parent_id: 'shut', content_ids: ['d'] })
    ]
    expect(titles(contentSections(groups, library))).toEqual([
      ['Videos', 0, ['c']],
      ['Mondays', 0, ['a', 'c']],
      ['Empty folder', 0, []],
      ['Shut', 0, ['b']]
    ])
  })

  it('places a dropped item before its neighbour without counting the moved ones', () => {
    const target = group({ content_ids: ['a', 'b', 'c'] })
    expect(placementIndex(target, 'c', ['a'])).toBe(1)
    expect(placementIndex(target, null, ['b'])).toBe(2)
    expect(placementIndex(target, 'missing', [])).toBe(3)
  })

  it('lists most and recently used', () => {
    expect(ids(mostUsed(library))).toEqual(['b', 'a'])
    expect(ids(recentlyUsed(library))).toEqual(['a', 'b'])
    expect(mostUsed(library, 1)).toHaveLength(1)
  })

  it('suggests unused tags, prefix matches first', () => {
    const tags = [{ name: 'monday', count: 3 }, { name: 'party', count: 2 }, { name: 'sunday mood', count: 1 }]
    expect(tagSuggestions(tags, 'day', [])).toEqual(['monday', 'sunday mood'])
    expect(tagSuggestions(tags, 'mo', ['monday'])).toEqual(['sunday mood'])
    expect(tagSuggestions(tags, 'p', [])).toEqual(['party'])
    expect(toggleWord(['a'], ' A ')).toEqual([])
    expect(toggleWord(['a'], 'B')).toEqual(['a', 'b'])
  })

  it('tells templates, files and visuals apart', () => {
    expect(isVideo(library[2])).toBe(true)
    expect(isVideo(library[0])).toBe(false)
    expect(isTemplate(library[3])).toBe(true)
    expect(isVisual(library[3])).toBe(false)
    expect(isVisual(item({ kind: 'file', mime_type: 'application/pdf' }))).toBe(false)
    expect(isVisual(library[0])).toBe(true)
  })

  it('links the source it came from', () => {
    expect(sourceLink(item({ source_kind: 'message', source_conversation_id: 'c1' }))).toEqual({ href: '/main/conversation/c1', external: false })
    expect(sourceLink(item({ source_kind: 'person', source_person_id: 'p 1', source_url: null }))).toEqual({ href: '/main/person/p 1', external: false })
    expect(sourceLink(item({ source_kind: 'action', source_action_id: 'a/1', source_url: null }))).toEqual({ href: '/main/actions?id=a%2F1', external: false })
    expect(sourceLink(item({ source_kind: 'tenor' }))).toEqual({ href: 'https://media.tenor.com/dance.gif', external: true })
    expect(sourceLink(item({ source_kind: 'manual', source_url: null }))).toBeNull()
  })

  it('prefers the absolute file path for the source', () => {
    expect(contentSrc(item({ file_path: '/data/media/p/x.gif' }))).toBe('asset://localhost//data/media/p/x.gif')
  })
})

describe('composer variables', () => {
  const conversation = { id: 'c1', name: 'Faktura oktober', type: 'email' as const }
  const message = { id: 'm1', subject: 'Faktura oktober', sender_name: 'Kari Nordmann', sender_email: 'kari@acme.no' }

  it('fills me, the person, the conversation and the message being answered', () => {
    const values = composerVariables({ conversation, person: { id: 'p1', display_name: 'Kari Nordmann', primary_email: 'kari@acme.no' }, me: { name: 'Milo Oakes', email: 'milo@northwind.example' }, message, subject: 'Re: Faktura oktober' })
    expect(values).toEqual({
      me: { name: 'Milo Oakes', first_name: 'Milo', email: 'milo@northwind.example' },
      person: { id: 'p1', name: 'Kari Nordmann', first_name: 'Kari', email: 'kari@acme.no' },
      conversation: { id: 'c1', name: 'Faktura oktober', type: 'email' },
      message: { id: 'm1', subject: 'Re: Faktura oktober', from_name: 'Kari Nordmann', from_email: 'kari@acme.no' }
    })
  })

  it('takes the person from the sender when the thread has none, and leaves out what it does not know', () => {
    expect(composerVariables({ conversation: null, person: null, me: { name: '  ' }, message: { ...message, sender_name: null } })).toEqual({
      person: { name: 'kari@acme.no', first_name: 'kari', email: 'kari@acme.no' },
      message: { id: 'm1', subject: 'Faktura oktober', from_name: '', from_email: 'kari@acme.no' }
    })
    expect(composerVariables({ conversation: null, person: null, me: null, message: null })).toEqual({})
    expect(firstName('  Ola  Nordmann ')).toBe('Ola')
  })

  it('turns a message into a reply template that keeps where it came from', () => {
    const mail = { id: 'm1', conversation_id: 'c1', type: 'email' as const, subject: 'Faktura oktober', content: '<p>Hei Kari,</p><p>Takk {{ for }} sist<br>Milo</p>', content_type: 'html' }
    expect(templateFromMessage(mail)).toEqual({ kind: 'email', name: 'Faktura oktober', body: 'Hei Kari,\n\nTakk {{ "{{" }} for }} sist\nMilo', source_kind: 'message', source_message_id: 'm1', source_conversation_id: 'c1' })
    expect(templateFromMessage({ ...mail, type: 'chat', subject: null, content: '  On my way  ', content_type: 'text' })).toMatchObject({ kind: 'reply', name: 'On my way', body: 'On my way' })
  })

  it('nests flat sample values by their dotted path', () => {
    expect(nestValues({ 'person.first_name': 'Kari', 'person.name': 'Kari N', today: 'Monday' })).toEqual({ person: { first_name: 'Kari', name: 'Kari N' }, today: 'Monday' })
  })
})

describe('reaction ordering', () => {
  const use = (emoji: string, used_count: number, last_used_at: string) => ({ emoji, used_count, last_used_at })

  it('puts the most used first, breaks ties by recency, and fills with the defaults', () => {
    const uses = [use('🙏', 2, '2026-10-01'), use('🎉', 5, '2026-09-01'), use('👀', 2, '2026-10-05')]
    expect(quickReactions(uses)).toEqual([':tada:', ':eyes:', ':pray:', ':+1:', ':heart:', ':joy:'])
    expect(quickReactions([])).toEqual(DEFAULT_REACTIONS)
    expect(quickReactions(uses, [], 2)).toEqual([':tada:', ':eyes:'])
  })

  it('never repeats an emoji that is both used and a default', () => {
    expect(quickReactions([use('👍', 1, '2026-10-01')])).toEqual([':+1:', ':heart:', ':joy:', ':open_mouth:', ':cry:', ':tada:'])
    expect(quickReactions([use(':+1:', 3, '2026-10-02'), use('👍', 1, '2026-10-01')], [], 3)).toEqual([':+1:'])
  })

  it('counts a sent reaction locally', () => {
    const once = noteReaction([], '🎉', '2026-10-14T10:00:00Z')
    expect(once).toEqual([use(':tada:', 1, '2026-10-14T10:00:00Z')])
    expect(noteReaction(once, ':tada:', '2026-10-14T11:00:00Z')).toEqual([use(':tada:', 2, '2026-10-14T11:00:00Z')])
  })
})
