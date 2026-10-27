import { describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import type { Conversation, Group, Inbox } from '$lib/tauri/types'
import { hasMember, inboxGroupIds, inboxMembership, memberOf } from './inboxes'

const conversation = (id: string, extra: Partial<Conversation> = {}) =>
  ({ id, providers: [{ id: 'work-outlook' }], tags: [], person_id: null, ...extra }) as unknown as Conversation

const inbox = (id: string, members: Inbox['members'], is_default = false) => ({ id, name: id, members, is_default, position: 0 }) as unknown as Inbox

const group = (id: string, parent_id: string | null, conversation_ids: string[] = []) => ({ id, parent_id, kind: 'manual', conversation_ids, tag_id: null, query: null }) as unknown as Group

const gmail = conversation('a', { providers: [{ id: 'gmail' }] as Conversation['providers'] })
const anna = conversation('b', { person_id: 'anna' })
const labelled = conversation('c', { tags: [{ object_type: 'conversation', object_id: 'c', tag_id: 'team' }] as Conversation['tags'] })
const grouped = conversation('d')
const loose = conversation('e')

const groups = [group('g1', null, ['d']), group('g2', 'g1')]
const inboxes = [
  inbox('private', [{ kind: 'provider', member_id: 'gmail' }, { kind: 'person', member_id: 'anna' }]),
  inbox('work', [{ kind: 'tag', member_id: 'team' }, { kind: 'person', member_id: 'anna' }, { kind: 'group', member_id: 'g1' }]),
  inbox('inbox', [], true)
]

describe('inbox membership', () => {
  it('matches every kind of member', () => {
    expect(memberOf(inboxes[0], gmail, groups)).toBe(true)
    expect(memberOf(inboxes[0], anna, groups)).toBe(true)
    expect(memberOf(inboxes[1], labelled, groups)).toBe(true)
    expect(memberOf(inboxes[1], grouped, groups)).toBe(true)
    expect(memberOf(inboxes[1], loose, groups)).toBe(false)
  })

  it('lets one conversation live in several inboxes and sends the rest to the default', () => {
    const membership = inboxMembership(inboxes, [gmail, anna, labelled, grouped, loose], groups)
    expect([...membership.get('private')!]).toEqual(['a', 'b'])
    expect([...membership.get('work')!]).toEqual(['b', 'c', 'd'])
    expect([...membership.get('inbox')!]).toEqual(['e'])
  })

  it('includes nested groups of a linked group', () => {
    expect([...inboxGroupIds(inboxes[1], groups)].sort()).toEqual(['g1', 'g2'])
    expect(hasMember(inboxes[1], { kind: 'tag', member_id: 'team' })).toBe(true)
    expect(hasMember(inboxes[0], { kind: 'tag', member_id: 'team' })).toBe(false)
  })
})
