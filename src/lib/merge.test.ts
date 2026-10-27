import { describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import type { Conversation, MergeGroup, Person } from '$lib/tauri/types'
import { groupItems, groupName } from './inbox'
import { mergeTree } from './merge'

const conversation = (id: string, type: Conversation['type'] = 'direct_message', last_activity = '2026-10-01T00:00:00Z', person_id: string | null = null) =>
  ({ id, name: id, type, providers: [], tags: [], members: [], threads: [], person_id, last_activity, unread_count: 0, overrides: {} }) as unknown as Conversation
const person = (id: string) => ({ id, display_name: id, is_human: true, conversation_ids: [], identities: [] }) as unknown as Person
const tree = (clusters: [string, string][], parents: [string, string][] = [], names: [string, string][] = []) => ({ clusters: new Map(clusters), parents: new Map(parents), names: new Map(names), ids: new Map<string, string>() })
const stored = (id: string, lead: string, conversation_ids: string[], parent_id: string | null = null, name: string | null = null): MergeGroup => ({ id, lead_conversation_id: lead, name, parent_id, conversation_ids, created_at: '', updated_at: '' })

describe('merge decisions', () => {
  it('reads every stored member into the cluster of its lead', () => {
    const { clusters, parents, names, ids } = mergeTree([stored('g1', 'a', ['a', 'b', 'c'])])
    expect([...clusters]).toEqual([['a', 'a'], ['b', 'a'], ['c', 'a']])
    expect(parents.size).toBe(0)
    expect(names.size).toBe(0)
    expect([...ids]).toEqual([['a', 'g1']])
  })

  it('nests a stored group under the lead of its parent group and keeps the stored name', () => {
    const { clusters, parents, names } = mergeTree([stored('g1', 'a', ['a', 'b']), stored('g2', 'c', ['c', 'd'], 'g1', 'Launch'), stored('g3', 'e', ['e'], 'g1')])
    expect([clusters.get('c'), clusters.get('d'), clusters.get('e')]).toEqual(['c', 'c', 'e'])
    expect([...parents]).toEqual([['c', 'a'], ['e', 'a']])
    expect(names.get('c')).toBe('Launch')
  })

  it('orders group members by their stored position', () => {
    const groups = groupItems([conversation('a'), conversation('b'), conversation('c')], [], [], 'recent', null, mergeTree([stored('g1', 'a', ['a', 'c', 'b'])]))
    const merged = groups.flatMap((g) => g.rows).find((r) => r.kind === 'merged')!
    expect(merged.kind === 'merged' && merged.conversations.map((c) => c.id)).toEqual(['a', 'c', 'b'])
    expect(merged.kind === 'merged' && merged.groupId).toBe('g1')
  })

  it('names a group by its stored name when it has one', () => {
    const groups = groupItems([conversation('a'), conversation('b')], [], [], 'recent', null, tree([['a', 'a'], ['b', 'a']], [], [['a', 'Launch']]))
    const merged = groups.flatMap((g) => g.rows).find((r) => r.kind === 'merged')!
    expect(merged.kind === 'merged' && merged.name).toBe('Launch')
  })
  it('folds a merged cluster into one inbox row placed by its lead', () => {
    const groups = groupItems([conversation('a'), conversation('b'), conversation('c')], [], [], 'recent', null, tree([['a', 'a'], ['b', 'a']]))
    const rows = groups.flatMap((g) => g.rows)
    expect(rows).toHaveLength(2)
    const merged = rows.find((r) => r.kind === 'merged')!
    expect(merged.kind === 'merged' && merged.conversations.map((c) => c.id)).toEqual(['a', 'b'])
    expect(groups.find((g) => g.rows.includes(merged))?.key).toBe('direct')
  })

  it('shows a nested group as a member of its parent and gives the parent every nested chat', () => {
    const groups = groupItems(['a', 'b', 'c', 'd', 'e'].map((id) => conversation(id)), [], [], 'recent', null, tree([['a', 'a'], ['b', 'a'], ['c', 'c'], ['d', 'c'], ['e', 'e']], [['c', 'a']]))
    const rows = groups.flatMap((g) => g.rows)
    expect(rows).toHaveLength(2)
    const parent = rows.find((r) => r.kind === 'merged')!
    expect(parent.kind === 'merged' && parent.id).toBe('a')
    expect(parent.kind === 'merged' && parent.conversations.map((c) => c.id)).toEqual(['a', 'b', 'c', 'd'])
    const members = parent.kind === 'merged' ? parent.members : []
    expect(members.map((m) => m.kind)).toEqual(['conversation', 'conversation', 'merged'])
    const child = members[2]
    expect(child.kind === 'merged' && child.id).toBe('c')
    expect(child.kind === 'merged' && child.members.map((m) => (m.kind === 'conversation' ? m.conversation.id : ''))).toEqual(['c', 'd'])
  })

  it('makes a chat a group when a group is nested under it', () => {
    const groups = groupItems(['a', 'b', 'c'].map((id) => conversation(id)), [], [], 'recent', null, tree([['a', 'a'], ['b', 'a'], ['c', 'c']], [['a', 'c']]))
    const rows = groups.flatMap((g) => g.rows)
    expect(rows).toHaveLength(1)
    const merged = rows[0]
    expect(merged.kind === 'merged' && merged.id).toBe('c')
    expect(merged.kind === 'merged' && merged.members.map((m) => m.kind)).toEqual(['conversation', 'merged'])
    expect(merged.kind === 'merged' && merged.conversations.map((c) => c.id)).toEqual(['c', 'a', 'b'])
  })

  it('groups a person row with a chat in the place of the row it was dropped onto', () => {
    const dm = conversation('dm', 'direct_message', '2026-10-02T00:00:00Z', 'p')
    const team = conversation('team', 'channel', '2026-10-09T00:00:00Z')
    const other = conversation('other', 'direct_message', '2026-10-05T00:00:00Z', 'q')
    const groups = groupItems([dm, team, other], [person('p'), person('q')], [], 'recent', null, tree([['dm', 'dm'], ['team', 'dm']]))
    expect(groups.map((g) => g.key)).toEqual(['people'])
    const rows = groups[0].rows
    expect(rows.map((r) => r.kind)).toEqual(['person', 'merged'])
    const merged = rows[1]
    expect(merged.kind === 'merged' && merged.conversations.map((c) => c.id)).toEqual(['dm', 'team'])
    expect(merged.kind === 'merged' && merged.name).toBe('p, team')
  })

  it('takes only the grouped chats of a person into a group and leaves the rest on the person row', () => {
    const dm = conversation('dm', 'direct_message', '2026-10-02T00:00:00Z', 'p')
    const mail = conversation('mail', 'email', '2026-10-03T00:00:00Z', 'p')
    const other = conversation('other', 'direct_message', '2026-10-05T00:00:00Z', 'q')
    const rows = groupItems([dm, mail, other], [person('p'), person('q')], [], 'recent', null, tree([['other', 'other'], ['dm', 'other']])).flatMap((g) => g.rows)
    const merged = rows.find((r) => r.kind === 'merged')!
    expect(merged.kind === 'merged' && merged.conversations.map((c) => c.id)).toEqual(['other', 'dm'])
    const rest = rows.find((r) => r.kind === 'person')!
    expect(rest.kind === 'person' && rest.conversations.map((c) => c.id)).toEqual(['mail'])
  })

  it('shows a person once when a nested group shrinks to that person', () => {
    const dm = conversation('dm', 'direct_message', '2026-10-02T00:00:00Z', 'p')
    const mail = conversation('mail', 'email', '2026-10-03T00:00:00Z', 'p')
    const other = conversation('other', 'direct_message', '2026-10-05T00:00:00Z', 'q')
    const rows = groupItems([dm, mail, other], [person('p'), person('q')], [], 'recent', null, tree([['other', 'other'], ['dm', 'other'], ['mail', 'mail']], [['mail', 'other']])).flatMap((g) => g.rows)
    expect(rows).toHaveLength(1)
    const merged = rows[0]
    expect(merged.kind === 'merged' && merged.members.map((m) => (m.kind === 'person' ? m.person.id : m.kind))).toEqual(['q', 'p'])
    expect(merged.kind === 'merged' && merged.conversations.map((c) => c.id).sort()).toEqual(['dm', 'mail', 'other'])
  })

  it('names a group from its members unless the lead was renamed', () => {
    expect(groupName(conversation('a'), ['Sam Hale', 'Anna Berg'])).toBe('Sam Hale, Anna Berg')
    expect(groupName(conversation('a'), ['Sam Hale', 'Anna Berg', 'Ola', 'Kari'])).toBe('Sam Hale +3')
    expect(groupName({ ...conversation('a'), name: 'Launch', overrides: { name: 'Launch' } }, ['Sam Hale'])).toBe('Launch')
  })
})
