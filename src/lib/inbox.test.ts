import { describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import { dateSegments, dateSublabel, edgeFade, favouriteList, groupItems, inView, itemActivity, itemUnread, relativeTime, reorder, rowDate, rowModel, snoozeUntil, tabScrollTarget, type InboxItem } from './inbox'
import type { Conversation, Person } from './tauri/types'
import { directConversation } from '../test/fixtures/conversations'

function conversation(overrides: Partial<Conversation>): Conversation {
  return { ...directConversation, tags: [], threads: [], members: [], ...overrides }
}

function person(overrides: Partial<Person>): Person {
  return {
    id: 'p1',
    profile_id: 'profile',
    display_name: 'Ada Lovelace',
    provider_name: 'Ada Lovelace',
    primary_email: 'ada@example.com',
    avatar_url: null,
    avatar_data: null,
    overrides: {},
    nicknames: {},
    is_human: true,
    favourite_position: null,
    identities: [],
    conversation_ids: [],
    provider_types: ['microsoft'],
    last_activity: null,
    unread_count: 0,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides
  }
}

describe('relativeTime', () => {
  const now = new Date('2026-10-09T12:00:00Z').getTime()
  it('rounds to the nearest unit', () => {
    expect(relativeTime('2026-10-09T11:59:40Z', now)).toBe('now')
    expect(relativeTime('2026-10-09T11:15:00Z', now)).toBe('45m')
    expect(relativeTime('2026-10-09T03:00:00Z', now)).toBe('9h')
    expect(relativeTime('2026-10-06T12:00:00Z', now)).toBe('3d')
  })
  it('falls back to a date after a week', () => {
    expect(relativeTime('2026-09-01T12:00:00Z', now)).toMatch(/Sep/)
  })
})

describe('snoozeUntil', () => {
  const thursday = new Date('2026-10-08T14:30:00')
  it('later is three hours out', () => {
    expect(new Date(snoozeUntil('later', thursday)).getHours()).toBe(17)
  })
  it('tomorrow is 08:00 next day', () => {
    const at = new Date(snoozeUntil('tomorrow', thursday))
    expect(at.getDate()).toBe(9)
    expect(at.getHours()).toBe(8)
  })
  it('weekend lands on saturday', () => {
    expect(new Date(snoozeUntil('weekend', thursday)).getDay()).toBe(6)
  })
  it('week lands on monday', () => {
    expect(new Date(snoozeUntil('week', thursday)).getDay()).toBe(1)
  })
})

describe('inView', () => {
  const now = Date.now()
  it('hides deleted rows from every view but Deleted', () => {
    const gone = conversation({ id: 'x', deleted_at: '2026-10-01T00:00:00Z' })
    expect(inView(gone, 'inbox', now)).toBe(false)
    expect(inView(gone, 'done', now)).toBe(false)
    expect(inView(gone, 'notifications', now)).toBe(false)
    expect(inView(gone, 'deleted', now)).toBe(true)
  })
  it('keeps live rows out of Deleted', () => {
    expect(inView(conversation({ id: 'y', deleted_at: null }), 'deleted', now)).toBe(false)
  })
})

describe('groupItems', () => {
  const ada = person({ id: 'p-ada', display_name: 'Ada Lovelace', conversation_ids: ['teams-ada', 'mail-ada'], provider_types: ['microsoft', 'imap'] })
  const bot = person({ id: 'p-bot', display_name: 'Build Bot', is_human: false, conversation_ids: ['bot-dm'], primary_email: 'noreply@ci.example' })
  const teamsAda = conversation({ id: 'teams-ada', type: 'direct_message', person_id: 'p-ada', unread_count: 2, last_activity: '2026-10-01T10:00:00Z', name: 'Ada Lovelace' })
  const mailAda = conversation({ id: 'mail-ada', type: 'email', person_id: 'p-ada', unread_count: 1, last_activity: '2026-10-03T10:00:00Z', name: 'Re: launch' })
  const botDm = conversation({ id: 'bot-dm', type: 'direct_message', person_id: 'p-bot', unread_count: 0, last_activity: '2026-10-04T10:00:00Z', name: 'Build Bot' })
  const channel = conversation({ id: 'chan', type: 'channel', person_id: null, unread_count: 0, last_activity: '2026-10-05T10:00:00Z', name: 'general' })
  const newsletter = conversation({ id: 'news', type: 'email', person_id: null, unread_count: 3, last_activity: '2026-10-06T10:00:00Z', name: 'Weekly digest' })

  it('merges a person into one row and puts humans first', () => {
    const groups = groupItems([teamsAda, mailAda, botDm, channel, newsletter], [ada, bot], [])
    expect(groups.map((g) => g.key)).toEqual(['people', 'teams', 'services'])
    const people = groups[0]
    expect(people.rows).toHaveLength(1)
    const row = people.rows[0] as Extract<InboxItem, { kind: 'person' }>
    expect(row.kind).toBe('person')
    expect(row.conversations.map((c) => c.id).sort()).toEqual(['mail-ada', 'teams-ada'])
    expect(itemUnread(row)).toBe(3)
    expect(itemActivity(row)).toBe('2026-10-03T10:00:00Z')
  })

  it('sends non human people and orphan mail to Services', () => {
    const groups = groupItems([teamsAda, mailAda, botDm, channel, newsletter], [ada, bot], [])
    const services = groups.find((g) => g.key === 'services')!
    const ids = services.rows.map((r) => (r.kind === 'person' ? r.person.id : r.kind === 'merged' ? r.id : r.conversation.id)).sort()
    expect(ids).toEqual(['news', 'p-bot'])
  })

  it('keeps offsets contiguous across groups', () => {
    const groups = groupItems([teamsAda, mailAda, botDm, channel, newsletter], [ada, bot], [])
    expect(groups.map((g) => g.offset)).toEqual([0, 1, 2])
  })

  it('lists own sections before people', () => {
    const starred = { ...teamsAda, is_pinned: true }
    const section = { id: 's1', profile_id: 'profile', name: 'Clients', position: 0, is_collapsed: false, conversations: [channel], created_at: '' }
    const groups = groupItems([starred, mailAda, channel], [ada], [section])
    expect(groups.map((g) => g.key)).toEqual(['section:s1', 'people'])
  })

  it('falls back to a direct messages group when no person is known', () => {
    const groups = groupItems([teamsAda], [], [])
    expect(groups.map((g) => g.key)).toEqual(['direct'])
  })
})

describe('favourites', () => {
  it('orders people and conversations by position', () => {
    const list = favouriteList(
      [person({ id: 'p1', display_name: 'Ada', favourite_position: 2 }), person({ id: 'p2', display_name: 'Bob', favourite_position: null })],
      [conversation({ id: 'c1', name: 'general', favourite_position: 0 }), conversation({ id: 'c2', name: 'gone', favourite_position: 1, deleted_at: '2026-01-01T00:00:00Z' })]
    )
    expect(list.map((f) => f.id)).toEqual(['c1', 'p1'])
  })
  it('reorders by index', () => {
    expect(reorder(['a', 'b', 'c'], 0, 2)).toEqual(['b', 'c', 'a'])
    expect(reorder(['a', 'b', 'c'], 2, 0)).toEqual(['c', 'a', 'b'])
    expect(reorder(['a', 'b'], 5, 0)).toEqual(['a', 'b'])
  })
})

describe('row dates', () => {
  const now = new Date('2026-10-09T15:00:00')
  it('shows the time today and the day and month otherwise', () => {
    expect(rowDate('2026-10-09T09:21:00', now)).toBe('09:21')
    expect(rowDate('2026-07-18T09:21:00', now)).toBe('18 JUL')
    expect(rowDate('2025-02-14T09:21:00', now)).toBe('14 FEB 25')
  })
  it('names the date bucket', () => {
    expect(dateSublabel('2026-10-09T01:00:00', now)).toBe('Today')
    expect(dateSublabel('2026-10-08T23:00:00', now)).toBe('Yesterday')
    expect(dateSublabel('2026-10-05T12:00:00', now)).toBe('Last 7 days')
    expect(dateSublabel('2026-09-20T12:00:00', now)).toBe('Last 30 days')
    expect(dateSublabel('2026-03-02T12:00:00', now)).toBe('March')
    expect(dateSublabel('2025-03-02T12:00:00', now)).toBe('March 2025')
  })
  it('segments only long groups and keeps offsets', () => {
    const rows = Array.from({ length: 10 }, (_, i) => ({ kind: 'conversation' as const, conversation: conversation({ id: 'c' + i, name: 'c' + i, last_activity: i < 3 ? '2026-10-09T10:00:00' : '2026-10-05T10:00:00' }) }))
    const segments = dateSegments(rows, now)
    expect(segments.map((s) => [s.label, s.rows.length, s.offset])).toEqual([['Today', 3, 0], ['Last 7 days', 7, 3]])
    expect(dateSegments(rows.slice(0, 4), now)).toEqual([{ label: null, rows: rows.slice(0, 4), offset: 0 }])
  })
})

describe('split tab scrolling', () => {
  it('scrolls just enough to reveal the active tab with a margin', () => {
    expect(tabScrollTarget(0, 300, 100, 80)).toBe(0)
    expect(tabScrollTarget(0, 300, 280, 80)).toBe(84)
    expect(tabScrollTarget(200, 300, 150, 80)).toBe(126)
    expect(tabScrollTarget(200, 300, 10, 80)).toBe(0)
  })

  it('fades only the edges that hide tabs', () => {
    expect(edgeFade(0, 300, 300)).toBeUndefined()
    expect(edgeFade(0, 600, 300)).toContain('black calc(100% - 2.5rem), transparent')
    expect(edgeFade(300, 600, 300)).toContain('transparent, black 2rem')
    expect(edgeFade(100, 600, 300)).toMatch(/transparent, black 2rem.*transparent\)$/)
  })
})

describe('rowModel', () => {
  const me = new Set(['user-002'])
  const ada = person({ id: 'p-ada', display_name: 'Ada Lovelace', avatar_data: 'data:image/png;base64,AA', identities: [{ id: 'i1', person_id: 'p-ada', user_id: 'user-001', provider_type: 'microsoft', external_id: 'ada', email: null, display_name: 'Ada', manual: false }] })
  const dm = conversation({ id: 'dm', type: 'direct_message', person_id: 'p-ada' })
  const group = conversation({ id: 'group', name: 'Design', type: 'group_message', members: [] })
  const mail = conversation({ id: 'mail', name: 'Newsletter', type: 'email', members: [] })

  it('mixes one row per person with chat rows in one list', () => {
    const rows = groupItems([dm, group, mail], [{ ...ada, conversation_ids: ['dm'] }], []).flatMap((g) => g.rows)
    const models = rows.map((row) => rowModel(row, [ada], me))
    expect(models.map((m) => m.kind).sort()).toEqual(['chat', 'chat', 'person'])
    expect(models.every((m) => !!m.avatar)).toBe(true)
  })

  it('opens a person as their merged chat and their image as the profile page', () => {
    const model = rowModel({ kind: 'person', person: ada, conversations: [dm] }, [ada], me)
    expect(model).toMatchObject({ kind: 'person', title: 'Ada Lovelace', open: '/main/person/p-ada', image: { kind: 'profile', path: '/main/person/p-ada/profile' } })
    expect(model.avatar).toMatchObject({ shape: 'person', src: 'data:image/png;base64,AA' })
  })

  it('opens a chat by its row and its picture as the chat details', () => {
    const model = rowModel({ kind: 'conversation', conversation: group }, [ada], me)
    expect(model).toMatchObject({ kind: 'chat', title: 'Design', open: '/main/conversation/group', image: { kind: 'chat', conversationId: 'group' } })
    expect(model.avatar.shape).toBe('chat')
  })

  it('shows the other person on a loose direct message and sends the image to their profile', () => {
    const loose = conversation({ id: 'loose', type: 'direct_message', person_id: null, members: directConversation.members, nicknames: { 'user-001': 'Countess' } })
    const model = rowModel({ kind: 'conversation', conversation: loose }, [ada], me)
    expect(model.title).toBe('Countess')
    expect(model.avatar).toMatchObject({ shape: 'person', id: 'p-ada' })
    expect(model.image).toEqual({ kind: 'profile', path: '/main/person/p-ada/profile' })
    expect(model.open).toBe('/main/conversation/loose')
  })

  it('keeps the chat dialog for a direct message with nobody known', () => {
    const stranger = conversation({ id: 'stranger', type: 'direct_message', person_id: null, members: directConversation.members })
    expect(rowModel({ kind: 'conversation', conversation: stranger }, [], me).image).toEqual({ kind: 'chat', conversationId: 'stranger' })
  })
})
