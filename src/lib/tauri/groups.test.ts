import { describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import { blockedParents, buildTree, canNest, composerOpen, describeQuery, emptyQuery, flattenTree, groupConversationIds, groupFields, groupUnread, parentChoices, readIds, serializeQuery, validRegex } from './groups'
import type { ChatMode, Conversation, Group } from './types'
import { directConversation } from '../../test/fixtures/conversations'

function group(overrides: Partial<Group>): Group {
  return {
    id: 'g',
    profile_id: 'profile',
    parent_id: null,
    name: 'Group',
    kind: 'manual',
    tag_id: null,
    query: null,
    position: 0,
    is_collapsed: false,
    icon: null,
    color: null,
    mode: 'write',
    conversation_ids: [],
    created_at: '2026-01-01T00:00:00Z',
    ...overrides
  }
}

const groups: Group[] = [
  group({ id: 'work', name: 'Work', position: 0, conversation_ids: ['c1'] }),
  group({ id: 'clients', name: 'Clients', parent_id: 'work', position: 1, conversation_ids: ['c2'] }),
  group({ id: 'acme', name: 'Acme', parent_id: 'clients', position: 0, conversation_ids: ['c3'] }),
  group({ id: 'home', name: 'Home', position: 1 }),
  group({ id: 'orphan', name: 'Orphan', parent_id: 'missing', position: 5 })
]

describe('buildTree', () => {
  it('nests children under parents in position order', () => {
    const tree = buildTree(groups)
    expect(tree.map((n) => n.group.id)).toEqual(['work', 'home', 'orphan'])
    expect(tree[0].children.map((n) => n.group.id)).toEqual(['clients'])
    expect(tree[0].children[0].children[0].group.id).toBe('acme')
    expect(tree[0].children[0].children[0].depth).toBe(2)
  })

  it('flattens with depth', () => {
    const flat = flattenTree(buildTree(groups))
    expect(flat.map((n) => `${n.group.id}:${n.depth}`)).toEqual(['work:0', 'clients:1', 'acme:2', 'home:0', 'orphan:0'])
  })

  it('does not loop on a cycle', () => {
    const cyclic = [group({ id: 'a', parent_id: 'b' }), group({ id: 'b', parent_id: 'a' })]
    const flat = flattenTree(buildTree(cyclic))
    expect(flat).toHaveLength(2)
  })
})

describe('canNest', () => {
  it('refuses self, descendants and unknown parents', () => {
    expect(canNest(groups, 'work', 'work')).toBe(false)
    expect(canNest(groups, 'work', 'acme')).toBe(false)
    expect(canNest(groups, 'work', 'nowhere')).toBe(false)
    expect(canNest(groups, 'acme', 'home')).toBe(true)
    expect(canNest(groups, 'acme', null)).toBe(true)
  })
})

describe('group membership', () => {
  it('includes descendants and counts unread', () => {
    expect([...groupConversationIds(groups, 'work')].sort()).toEqual(['c1', 'c2', 'c3'])
    const conversations: Conversation[] = [
      { ...directConversation, id: 'c1', unread_count: 2 },
      { ...directConversation, id: 'c3', unread_count: 1, deleted_at: '2026-01-01T00:00:00Z' },
      { ...directConversation, id: 'c9', unread_count: 4 }
    ]
    expect(groupUnread(groups, 'work', conversations)).toBe(1)
  })
})

describe('query builder', () => {
  it('serialises only what is set', () => {
    const query = serializeQuery({ ...emptyQuery(), providers: [], text: '  invoice ', regex: '', limit: 0, unread: false })
    expect(query).toEqual({
      providers: undefined,
      types: undefined,
      tag_ids: undefined,
      person_ids: undefined,
      unread: undefined,
      has_attachments: undefined,
      humans_only: undefined,
      from: null,
      to: null,
      sender: null,
      text: 'invoice',
      regex: null,
      sort: 'activity',
      direction: 'desc',
      limit: null
    })
  })

  it('validates regex live', () => {
    expect(validRegex('^inv(oice)?$')).toBeNull()
    expect(validRegex('(')).toMatch(/Invalid regular expression/)
    expect(validRegex('')).toBeNull()
  })

  it('describes a query in words', () => {
    const text = describeQuery({ ...emptyQuery(), providers: ['slack'], unread: true, regex: 'acme', sort: 'tags', limit: 10 }, [])
    expect(text).toBe('slack · unread · /acme/ by tags, top 10')
  })
})

describe('read chats', () => {
  const chat = (id: string, mode: ChatMode = 'write') => ({ id, mode })

  it('collects read chats and every chat under a read group', () => {
    const groups = [
      group({ id: 'root', mode: 'read', conversation_ids: ['a'] }),
      group({ id: 'child', parent_id: 'root', conversation_ids: ['b'] }),
      group({ id: 'open', conversation_ids: ['c'] })
    ]
    const ids = readIds([chat('a'), chat('b'), chat('c'), chat('d', 'read'), chat('f', 'read')], groups)
    expect([...ids].sort()).toEqual(['a', 'b', 'd', 'f'])
  })

  it('keeps the composer while any of the chats can still be written to', () => {
    const locked = new Set(['a', 'b'])
    expect(composerOpen(['a', 'b'], locked)).toBe(false)
    expect(composerOpen(['a', 'c'], locked)).toBe(true)
    expect(composerOpen([], locked)).toBe(false)
  })
})

describe('parent picker', () => {
  it('blocks the group itself and everything below it', () => {
    expect([...blockedParents(groups, 'work')].sort()).toEqual(['acme', 'clients', 'work'])
    expect([...blockedParents(groups, 'clients')].sort()).toEqual(['acme', 'clients'])
    expect([...blockedParents(groups, 'home')]).toEqual(['home'])
    expect(blockedParents(groups, null).size).toBe(0)
  })

  it('lists the tree in order with the blocked rows disabled', () => {
    const choices = parentChoices(groups, 'clients')
    expect(choices.map((c) => [c.group.id, c.depth, c.disabled])).toEqual([
      ['work', 0, false],
      ['clients', 1, true],
      ['acme', 2, true],
      ['home', 0, false],
      ['orphan', 0, false]
    ])
  })

  it('disables nothing for a new group', () => {
    expect(parentChoices(groups, null).every((c) => !c.disabled)).toBe(true)
  })

  it('keeps the ancestors of a match when searching', () => {
    expect(parentChoices(groups, 'home', 'acm').map((c) => c.group.id)).toEqual(['work', 'clients', 'acme'])
    expect(parentChoices(groups, null, 'HOME').map((c) => c.group.id)).toEqual(['home'])
    expect(parentChoices(groups, null, 'nothing')).toEqual([])
  })
})

describe('group dialog fields', () => {
  const sorted = (fields: Set<string>) => [...fields].sort()

  it('gives conversation groups sort order, count and read only', () => {
    expect(sorted(groupFields('conversation', true))).toEqual(['colour', 'count', 'delete', 'icon', 'name', 'newGroup', 'parent', 'query', 'read', 'rule', 'sort'])
    expect(sorted(groupFields('conversation', false))).toEqual(['colour', 'icon', 'kind', 'name', 'parent', 'query', 'read', 'rule'])
  })

  it('gives action groups new action inside and hides delete on the built in group', () => {
    expect(sorted(groupFields('action', true))).toEqual(['delete', 'icon', 'name', 'newAction', 'newGroup', 'parent'])
    expect(groupFields('action', true, true).has('delete')).toBe(false)
    expect(sorted(groupFields('action', false))).toEqual(['icon', 'name', 'parent'])
  })

  it('gives content groups a rule and a kind only when new', () => {
    expect(sorted(groupFields('content', true))).toEqual(['delete', 'icon', 'name', 'newGroup', 'parent', 'rule'])
    expect(sorted(groupFields('content', false))).toEqual(['icon', 'kind', 'name', 'parent', 'rule'])
  })

  it('keeps sort order and read only to conversation groups', () => {
    for (const scope of ['action', 'content'] as const) {
      const fields = groupFields(scope, true)
      expect(fields.has('sort') || fields.has('count') || fields.has('read')).toBe(false)
    }
  })
})
