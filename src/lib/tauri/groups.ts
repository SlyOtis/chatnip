import { derived, writable } from 'svelte/store'
import type { Conversation, Group, GroupQuery, GroupSort, Inbox, Tag } from './types'
import { getGroups } from './api'
import { conversationsStore } from './conversations'
import { t, type MessageKey } from '$lib/i18n.svelte'

export const groupsStore = writable<Group[]>([])

export const groupsById = derived(groupsStore, ($groups) => new Map($groups.map((group) => [group.id, group])))

export interface GroupDialog {
  group: Group | null
  parentId: string | null
  inbox: Inbox | null
}

export const groupDialog = writable<GroupDialog | null>(null)

export async function loadGroups(): Promise<Group[]> {
  const groups = await getGroups()
  groupsStore.set(groups)
  return groups
}

export interface TreeGroup {
  id: string
  parent_id: string | null
  position: number
  name: string
}

export interface GroupNode<T extends TreeGroup = Group> {
  group: T
  depth: number
  children: GroupNode<T>[]
}

export const GROUP_SORTS: { value: GroupSort; label: MessageKey }[] = [
  { value: 'activity', label: 'inbox.groupSort.activity' },
  { value: 'name', label: 'common.name' },
  { value: 'unread', label: 'inbox.sort.unread' },
  { value: 'messages', label: 'inbox.groupSort.messages' },
  { value: 'tags', label: 'inbox.groupSort.tags' },
  { value: 'created', label: 'inbox.groupSort.created' }
]

export function emptyQuery(): GroupQuery {
  return { sort: 'activity', direction: 'desc' }
}

export function validRegex(pattern: string | null | undefined): string | null {
  if (!pattern) return null
  try {
    new RegExp(pattern)
    return null
  } catch (error) {
    return error instanceof Error ? error.message : String(error)
  }
}

export function buildTree<T extends TreeGroup>(groups: T[]): GroupNode<T>[] {
  const byId = new Map(groups.map((group) => [group.id, group]))
  const children = new Map<string | null, T[]>()
  for (const group of groups) {
    const parent = group.parent_id && byId.has(group.parent_id) && group.parent_id !== group.id ? group.parent_id : null
    if (!children.has(parent)) children.set(parent, [])
    children.get(parent)!.push(group)
  }
  const visited = new Set<string>()
  const walk = (parent: string | null, depth: number): GroupNode<T>[] =>
    (children.get(parent) ?? [])
      .filter((group) => !visited.has(group.id))
      .sort((a, b) => a.position - b.position || a.name.localeCompare(b.name))
      .map((group) => {
        visited.add(group.id)
        return { group, depth, children: walk(group.id, depth + 1) }
      })
  const tree = walk(null, 0)
  for (const group of groups) {
    if (!visited.has(group.id)) {
      visited.add(group.id)
      tree.push({ group, depth: 0, children: [] })
    }
  }
  return tree
}

export function flattenTree<T extends TreeGroup>(nodes: GroupNode<T>[]): GroupNode<T>[] {
  return nodes.flatMap((node) => [node, ...flattenTree(node.children)])
}

export function descendants(groups: TreeGroup[], id: string): Set<string> {
  const out = new Set<string>()
  const queue = [id]
  while (queue.length) {
    const current = queue.shift()!
    for (const group of groups) {
      if (group.parent_id === current && !out.has(group.id)) {
        out.add(group.id)
        queue.push(group.id)
      }
    }
  }
  return out
}

export function canNest(groups: TreeGroup[], id: string, parentId: string | null): boolean {
  if (parentId === null) return true
  if (parentId === id) return false
  if (!groups.some((group) => group.id === parentId)) return false
  return !descendants(groups, id).has(parentId)
}

export function blockedParents(groups: TreeGroup[], id: string | null): Set<string> {
  if (!id) return new Set()
  return new Set([id, ...descendants(groups, id)])
}

export interface ParentChoice<T extends TreeGroup = TreeGroup> {
  group: T
  depth: number
  disabled: boolean
}

export function parentChoices<T extends TreeGroup>(groups: T[], id: string | null, search = ''): ParentChoice<T>[] {
  const blocked = blockedParents(groups, id)
  const nodes = flattenTree(buildTree(groups))
  const needle = search.trim().toLowerCase()
  const shown = new Set<string>()
  if (needle) {
    const byId = new Map(groups.map((group) => [group.id, group]))
    for (const node of nodes) {
      if (!node.group.name.toLowerCase().includes(needle)) continue
      let cursor: T | undefined = node.group
      while (cursor && !shown.has(cursor.id)) {
        shown.add(cursor.id)
        cursor = cursor.parent_id ? byId.get(cursor.parent_id) : undefined
      }
    }
  }
  return nodes.filter((node) => !needle || shown.has(node.group.id)).map((node) => ({ group: node.group, depth: node.depth, disabled: blocked.has(node.group.id) }))
}

export type GroupScope = 'conversation' | 'action' | 'content'

export type GroupField = 'name' | 'icon' | 'parent' | 'colour' | 'read' | 'kind' | 'rule' | 'query' | 'sort' | 'count' | 'newGroup' | 'newAction' | 'delete'

export function groupFields(scope: GroupScope, editing: boolean, builtin = false): Set<GroupField> {
  const fields = new Set<GroupField>(['name', 'icon', 'parent'])
  const add = (...more: GroupField[]) => more.forEach((field) => fields.add(field))
  if (scope === 'conversation') add('colour', 'read', 'rule', 'query')
  if (scope === 'content') add('rule')
  if (scope !== 'action' && !editing) add('kind')
  if (!editing) return fields
  if (scope === 'conversation') add('sort', 'count')
  if (scope === 'action') add('newAction')
  add('newGroup')
  if (!builtin) add('delete')
  return fields
}

export function groupConversationIds(groups: Group[], id: string): Set<string> {
  const ids = new Set<string>()
  const include = (groupId: string) => {
    const group = groups.find((g) => g.id === groupId)
    if (!group) return
    for (const conversationId of group.conversation_ids) ids.add(conversationId)
  }
  include(id)
  for (const child of descendants(groups, id)) include(child)
  return ids
}

export function readIds(conversations: Pick<Conversation, 'id' | 'mode'>[], groups: Group[]): Set<string> {
  const ids = new Set<string>()
  for (const conversation of conversations) {
    if (conversation.mode === 'read') ids.add(conversation.id)
  }
  for (const group of groups) {
    if (group.mode !== 'read') continue
    for (const id of groupConversationIds(groups, group.id)) ids.add(id)
  }
  return ids
}

export function composerOpen(conversationIds: string[], read: Set<string>): boolean {
  return conversationIds.some((id) => !read.has(id))
}

export const readStore = derived([conversationsStore, groupsStore], ([$conversations, $groups]) => readIds($conversations.data, $groups))

export function groupUnread(groups: Group[], id: string, conversations: Conversation[]): number {
  const ids = groupConversationIds(groups, id)
  return conversations.filter((c) => ids.has(c.id) && !c.deleted_at && c.unread_count > 0).length
}

export function describeQuery(query: GroupQuery | null, labels: Tag[]): string {
  if (!query) return t('inbox.query.everything')
  const parts: string[] = []
  if (query.providers?.length) parts.push(query.providers.join(', '))
  if (query.types?.length) parts.push(query.types.map((type) => type.replace('_', ' ')).join(', '))
  if (query.tag_ids?.length) parts.push(query.tag_ids.map((id) => labels.find((l) => l.id === id)?.name ?? id).join(', '))
  if (query.humans_only) parts.push(t('inbox.query.humans'))
  if (query.unread) parts.push(t('inbox.query.unread'))
  if (query.has_attachments) parts.push(t('inbox.query.attachments'))
  if (query.sender) parts.push(t('inbox.query.from', { sender: query.sender }))
  if (query.text) parts.push(`"${query.text}"`)
  if (query.regex) parts.push(`/${query.regex}/`)
  if (query.from || query.to) parts.push([query.from ?? '…', query.to ?? '…'].join(' → '))
  const label = GROUP_SORTS.find((s) => s.value === query.sort)?.label
  const sort = label ? t(label).toLowerCase() : query.sort
  const what = parts.length ? parts.join(' · ') : t('inbox.query.everything')
  return query.limit ? t('inbox.query.sortedTop', { what, sort, limit: String(query.limit) }) : t('inbox.query.sorted', { what, sort })
}

export function serializeQuery(query: GroupQuery): GroupQuery {
  const clean = (value: string | null | undefined) => (value && value.trim().length ? value.trim() : null)
  const list = <T>(value: T[] | undefined) => (value && value.length ? value : undefined)
  return {
    providers: list(query.providers),
    connection_ids: list(query.connection_ids),
    types: list(query.types),
    tag_ids: list(query.tag_ids),
    person_ids: list(query.person_ids),
    unread: query.unread || undefined,
    has_attachments: query.has_attachments || undefined,
    humans_only: query.humans_only || undefined,
    from: clean(query.from),
    to: clean(query.to),
    sender: clean(query.sender),
    text: clean(query.text),
    regex: clean(query.regex),
    sort: query.sort,
    direction: query.direction,
    limit: query.limit && query.limit > 0 ? Math.floor(query.limit) : null
  }
}
