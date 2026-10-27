import { derived, get, writable } from 'svelte/store'
import { conversationsStore } from '$lib/tauri/conversations'
import { objectTagsStore, splitsStore } from '$lib/tauri/tags'
import { sidebarSectionsStore, sortPreferenceStore, unreadOnlyStore } from '$lib/tauri/sidebar'
import { displayName, peopleStore, personOf, personProfileHref } from '$lib/tauri/people'
import { AVATAR_FACES, chatAvatar, personAvatar, userAvatar, type AvatarSource } from '$lib/avatar'
import { groupsStore, groupConversationIds } from '$lib/tauri/groups'
import type { Conversation, Group, GroupSort, MergeGroup, Tag, Person, SidebarSection } from '$lib/tauri/types'
import * as api from '$lib/tauri/api'
import { layoutSettings, putLayoutSetting, sectionCollapsedKey, sectionNameKey, sidebarLayout, slug, sortKey, sortOrder, toggleFlag } from '$lib/layoutSettings'
import type { SidebarEntry, SidebarLayout } from '$lib/tauri/types'
import { activeInbox, inboxGroupIds, inboxesStore, inboxMembership } from '$lib/inboxes'
import { mergeTree, type MergeTree } from '$lib/merge'
import { formatDate, t, type MessageKey } from '$lib/i18n.svelte'
import {  } from '$lib/tauri/conversations'
import { conversationHref, mergedHref } from '$lib/tauri/conversations'

export type InboxView = 'inbox' | 'done' | 'snoozed' | 'deleted' | 'notifications' | 'pipeline'

export const VIEW_TITLES: Record<InboxView, MessageKey> = { inbox: 'inbox.view.inbox', notifications: 'inbox.view.notifications', pipeline: 'inbox.view.pipeline', snoozed: 'inbox.view.snoozed', done: 'inbox.view.done', deleted: 'inbox.view.deleted' }

export const inboxView = writable<InboxView>('inbox')
export const activeSplit = writable<string>('')

splitsStore.subscribe((splits) => {
  if (splits.length > 0 && !splits.some((s) => s.id === get(activeSplit))) activeSplit.set(splits[0].id)
})
export const selectedIndex = writable<number>(0)
export const selectedIds = writable<Set<string>>(new Set())
export const activeGroup = writable<string | null>(null)
export const activeTag = writable<string | null>(null)
const INBOX_PREFIX = 'inbox:'
export const activeFolder = writable<string | null>(null)

export function sidebarFolder(layout: SidebarLayout, id: string): Extract<SidebarEntry, { kind: 'folder' }> | null {
  const walk = (entries: SidebarEntry[]): Extract<SidebarEntry, { kind: 'folder' }> | null => {
    for (const entry of entries) {
      if (entry.kind !== 'folder') continue
      if (entry.id === id) return entry
      const inner = walk(entry.items)
      if (inner) return inner
    }
    return null
  }
  return walk(layout.top) ?? walk(layout.footer)
}

export function openFolder(id: string | null) {
  activeFolder.set(id)
  activeGroup.set(null)
  activeTag.set(null)
  groupSort.set(null)
  inboxView.set('inbox')
  selectedIndex.set(0)
}
export const pipelineHolders = writable<Map<string, string>>(new Map())
export const mergeGroups = writable<MergeGroup[]>([])
export const mergeTreeStore = derived(mergeGroups, mergeTree)

export async function loadMergeGroups(): Promise<void> {
  mergeGroups.set(await api.getMergeGroups())
}

export async function loadPipeline(): Promise<void> {
  const [schedules, runs] = await Promise.all([
    api.getMessageSchedules().catch(() => []),
    api.getActionRuns(null, 200).catch(() => [])
  ])
  pipelineHolders.set(pipelineMap(schedules, runs))
}

export function pipelineMap(
  schedules: { conversation_id: string; action_name: string | null; status: string; at: string | null }[],
  runs: { action_name: string; status: string; trigger: unknown }[]
): Map<string, string> {
  const held = new Map<string, string>()
  for (const schedule of schedules) if (schedule.status === 'pending' && !held.has(schedule.conversation_id)) held.set(schedule.conversation_id, schedule.action_name ? t('inbox.pipeline.waiting', { action: schedule.action_name }) : t('inbox.pipeline.scheduledSend'))
  for (const run of runs) {
    const conversation = (run.trigger as { conversation_id?: unknown } | null)?.conversation_id
    if ((run.status === 'running' || run.status === 'waiting') && typeof conversation === 'string' && !held.has(conversation)) held.set(conversation, t(run.status === 'running' ? 'inbox.pipeline.running' : 'inbox.pipeline.waiting', { action: run.action_name }))
  }
  return held
}

export function inboxSplitKey(inbox: string): string {
  return `list.inbox.${slug(inbox)}.split`
}

export function openInbox(id: string | null) {
  activeInbox.set(id)
  activeFolder.set(null)
  activeGroup.set(null)
  activeTag.set(null)
  groupSort.set(null)
  inboxView.set('inbox')
  const split = id ? get(layoutSettings)[inboxSplitKey(id)] : undefined
  if (split && get(splitsStore).some((s) => s.id === split)) activeSplit.set(split)
  selectedIndex.set(0)
}

export function openTag(id: string | null) {
  const tagged = id ? get(conversationsStore).data.filter((c) => c.tags.some((l) => l.tag_id === id)) : []
  activeTag.set(id)
  activeGroup.set(null)
  activeFolder.set(null)
  inboxView.set(tagged.length > 0 && tagged.every((c) => !!c.deleted_at) ? 'deleted' : 'inbox')
  selectedIndex.set(0)
}
export const groupSort = writable<{ sort: GroupSort; direction: 'asc' | 'desc' } | null>(null)

export const activeGroupRecord = derived([groupsStore, activeGroup], ([$groups, $active]) => ($active ? ($groups.find((g) => g.id === $active) ?? null) : null))

export function openGroup(group: Group | null) {
  activeGroup.set(group?.id ?? null)
  activeFolder.set(null)
  activeTag.set(null)
  const chosen = group ? sortOrder(get(layoutSettings)[sortKey(group.id)]) : null
  groupSort.set(chosen ?? (group?.query ? { sort: group.query.sort, direction: group.query.direction } : null))
  selectedIndex.set(0)
}

export function byGroupSort(sort: GroupSort, direction: 'asc' | 'desc') {
  const sign = direction === 'asc' ? 1 : -1
  return (a: InboxItem, b: InboxItem): number => {
    const value = (item: InboxItem): number | string => {
      const conversations = itemConversations(item)
      switch (sort) {
        case 'name':
          return itemName(item).toLowerCase()
        case 'unread':
          return itemUnread(item)
        case 'messages':
          return conversations.reduce((sum, c) => sum + c.threads.reduce((n, thread) => n + thread.message_count, 0), 0)
        case 'tags':
          return conversations.reduce((sum, c) => sum + c.tags.length, 0)
        case 'created':
          return conversations.reduce((earliest, c) => (c.created_at < earliest ? c.created_at : earliest), conversations[0]?.created_at ?? '')
        default:
          return itemActivity(item)
      }
    }
    const left = value(a)
    const right = value(b)
    if (left === right) return itemActivity(b).localeCompare(itemActivity(a))
    return (left < right ? -1 : 1) * sign
  }
}

export type InboxItem =
  | { kind: 'person'; person: Person; conversations: Conversation[] }
  | { kind: 'conversation'; conversation: Conversation }
  | { kind: 'merged'; id: string; groupId: string; name: string; conversations: Conversation[]; members: InboxItem[] }

export function itemId(item: InboxItem): string {
  if (item.kind === 'person') return 'person:' + item.person.id
  if (item.kind === 'merged') return 'merged:' + item.id
  return item.conversation.id
}

export function itemConversations(item: InboxItem): Conversation[] {
  return item.kind === 'conversation' ? [item.conversation] : item.conversations
}

export function itemActivity(item: InboxItem): string {
  if (item.kind === 'conversation') return item.conversation.last_activity
  return item.conversations.reduce((latest, c) => (c.last_activity > latest ? c.last_activity : latest), item.conversations[0]?.last_activity ?? '')
}

export function itemUnread(item: InboxItem): number {
  return itemConversations(item).reduce((sum, c) => sum + c.unread_count, 0)
}

export function itemName(item: InboxItem): string {
  if (item.kind === 'person') return item.person.display_name
  if (item.kind === 'merged') return item.name
  return item.conversation.name
}

export function groupName(lead: Conversation, names: string[]): string {
  if (lead.overrides?.name) return lead.name
  const unique = [...new Set(names.filter(Boolean))]
  return unique.length <= 2 ? unique.join(', ') : `${unique[0]} +${unique.length - 1}`
}

export function inView(conversation: Conversation, view: InboxView, now: number): boolean {
  if (view === 'deleted') return !!conversation.deleted_at
  if (conversation.deleted_at) return false
  const snoozed = conversation.snoozed_until ? new Date(conversation.snoozed_until).getTime() > now : false
  switch (view) {
    case 'done':
      return conversation.is_archived
    case 'snoozed':
      return snoozed
    case 'notifications':
      return conversation.unread_count > 0 && !conversation.is_archived && !snoozed
    case 'pipeline':
      return true
    default:
      return !conversation.is_archived && !snoozed
  }
}

function inSplit(conversation: Conversation, split: string, splits: Tag[]): boolean {
  const ids = new Set(conversation.tags.map((label) => label.tag_id))
  if (ids.has(split)) return true
  const last = splits[splits.length - 1]
  return !!last && last.id === split && !splits.some((label) => ids.has(label.id))
}

function byPreference(preference: 'recent' | 'unread' | 'alphabetical') {
  return (a: InboxItem, b: InboxItem): number => {
    if (preference === 'alphabetical') return itemName(a).localeCompare(itemName(b))
    if (preference === 'unread' && (itemUnread(a) > 0) !== (itemUnread(b) > 0)) return itemUnread(a) > 0 ? -1 : 1
    return itemActivity(b).localeCompare(itemActivity(a))
  }
}

export interface InboxGroup {
  key: string
  label: MessageKey | null
  rows: InboxItem[]
  offset: number
  section: SidebarSection | null
  name?: string
}

export function groupLabel(group: InboxGroup, settings: Record<string, string>): string {
  return group.name ?? group.section?.name ?? (settings[sectionNameKey(group.key)] || (group.label ? t(group.label) : ''))
}

export const collapsedGroups = derived(layoutSettings, ($settings) => new Set(Object.keys($settings).filter((key) => $settings[key] === 'true' && key.startsWith('list.section.') && key.endsWith('.collapsed')).map((key) => key.slice('list.section.'.length, -'.collapsed'.length))))

export function groupCollapsed(collapsed: Set<string>, key: string): boolean {
  return collapsed.has(slug(key))
}

export function toggleGroup(key: string) {
  toggleFlag(sectionCollapsedKey(key)).catch(() => {})
}

function isTeam(conversation: Conversation): boolean {
  return conversation.type === 'channel' || conversation.type === 'group_message' || conversation.group_type === 'team'
}

export function groupItems(
  conversations: Conversation[],
  people: Person[],
  sections: SidebarSection[],
  preference: 'recent' | 'unread' | 'alphabetical' = 'recent',
  order: ((a: InboxItem, b: InboxItem) => number) | null = null,
  tree: MergeTree = { clusters: new Map(), parents: new Map(), names: new Map(), ids: new Map() }
): InboxGroup[] {
  const sort = order ?? byPreference(preference)
  const ownerOf = new Map<string, Person>()
  for (const person of people) {
    for (const id of person.conversation_ids) ownerOf.set(id, person)
  }
  const byId = new Map(people.map((person) => [person.id, person]))
  for (const c of conversations) {
    const person = c.person_id ? byId.get(c.person_id) : undefined
    if (person) ownerOf.set(c.id, person)
  }
  const personItems: InboxItem[] = []
  const grouped = new Set<string>()
  for (const person of people) {
    const owned = conversations.filter((c) => ownerOf.get(c.id)?.id === person.id && !isTeam(c))
    if (owned.length === 0) continue
    for (const c of owned) grouped.add(c.id)
    personItems.push({ kind: 'person', person, conversations: owned })
  }
  const single = (c: Conversation): InboxItem => ({ kind: 'conversation', conversation: c })
  const placed = new Set<string>()
  const groups: InboxGroup[] = []
  const place = (item: InboxItem): InboxItem => (item.kind === 'merged' ? place(item.members[0]) : item)
  const push = (key: string, label: MessageKey | null, rows: InboxItem[], section: SidebarSection | null = null) => {
    const fresh = rows.filter((row) => !placed.has(itemId(row))).sort((a, b) => sort(place(a), place(b)))
    if (fresh.length === 0) return
    for (const row of fresh) placed.add(itemId(row))
    groups.push({ key, label, rows: fresh, offset: 0, section })
  }
  const loose = conversations.filter((c) => !grouped.has(c.id))
  const bundles = new Map<string, InboxItem[]>()
  const alone: InboxItem[] = []
  for (const item of [...personItems, ...loose.map(single)]) {
    const parts = new Map<string, Conversation[]>()
    for (const c of itemConversations(item)) {
      const anchor = tree.clusters.get(c.id) ?? ''
      parts.set(anchor, [...(parts.get(anchor) ?? []), c])
    }
    for (const [anchor, owned] of parts) {
      const part: InboxItem = item.kind === 'person' ? { ...item, conversations: owned } : item
      if (anchor) bundles.set(anchor, [...(bundles.get(anchor) ?? []), part])
      else alone.push(part)
    }
  }
  const fold = (items: InboxItem[]): InboxItem[] => {
    const seen = new Map<string, Conversation[]>()
    return items.flatMap((item): InboxItem[] => {
      if (item.kind !== 'person') return [item]
      const owned = seen.get(item.person.id)
      if (!owned) {
        seen.set(item.person.id, item.conversations)
        return [item]
      }
      owned.push(...item.conversations)
      return []
    })
  }
  const children = new Map<string, string[]>()
  for (const [child, above] of tree.parents) children.set(above, [...(children.get(above) ?? []), child])
  const position = new Map([...tree.clusters.keys()].map((id, index) => [id, index]))
  const rank = (item: InboxItem): number => Math.min(...itemConversations(item).map((c) => position.get(c.id) ?? Infinity))
  const build = (anchor: string): InboxItem[] => {
    const members = bundles.get(anchor) ?? []
    const people = new Set(members.filter((item) => item.kind === 'person').flatMap((item) => itemConversations(item).map((c) => c.id)))
    const distinct = members.filter((item) => item.kind !== 'conversation' || !people.has(item.conversation.id)).sort((a, b) => rank(a) - rank(b))
    const head = distinct.find((item) => itemConversations(item).some((c) => c.id === anchor)) ?? distinct[0]
    const ordered = fold([...(head ? [head, ...distinct.filter((item) => item !== head)] : []), ...(children.get(anchor) ?? []).flatMap(build)])
    if (ordered.length < 2) return ordered
    const owned = [...new Map(ordered.flatMap(itemConversations).map((c) => [c.id, c])).values()]
    const lead = owned.find((c) => c.id === anchor) ?? owned[0]
    return [{ kind: 'merged', id: anchor, groupId: tree.ids.get(anchor) ?? '', name: tree.names.get(anchor) ?? groupName(lead, ordered.map(itemName)), conversations: [lead, ...owned.filter((c) => c.id !== lead.id)], members: ordered }]
  }
  const merged = [...new Set(tree.clusters.values())].filter((anchor) => !tree.parents.has(anchor)).flatMap(build)
  const lead = (item: InboxItem): Conversation | null => { const at = place(item); return at.kind === 'conversation' ? at.conversation : null }
  const owner = (item: InboxItem): Person | null => { const at = place(item); return at.kind === 'person' ? at.person : null }
  const all: InboxItem[] = fold([...merged, ...alone])
  for (const section of sections) {
    const ids = new Set(section.conversations.map((c) => c.id))
    push('section:' + section.id, null, all.filter((item) => itemConversations(place(item)).some((c) => ids.has(c.id))), section)
  }
  push('people', 'inbox.group.people', all.filter((item) => !!owner(item)?.is_human))
  push('teams', 'inbox.group.teams', all.filter((item) => { const c = lead(item); return !!c && isTeam(c) }))
  push(
    'services',
    'inbox.group.services',
    all.filter((item) => owner(item)?.is_human === false || (() => { const c = lead(item); return !!c && c.type === 'email' && !c.person_id })())
  )
  push('mail', 'inbox.group.mail', all.filter((item) => lead(item)?.type === 'email'))
  push('direct', 'inbox.group.direct', all.filter((item) => { const c = lead(item); return !!c && c.type !== 'email' }))
  push('threads', 'inbox.group.threads', conversations.filter((c) => c.threads.some((thread) => !thread.is_resolved)).map(single))
  return numbered(groups)
}

function numbered(groups: InboxGroup[]): InboxGroup[] {
  let offset = 0
  for (const group of groups) {
    group.offset = offset
    offset += group.rows.length
  }
  return groups
}

export const inboxGroups = derived(
  [conversationsStore, inboxView, sidebarSectionsStore, sortPreferenceStore, unreadOnlyStore, peopleStore, groupsStore, activeGroup, groupSort, activeTag, pipelineHolders, inboxesStore, activeInbox, layoutSettings, activeFolder, sidebarLayout, mergeTreeStore, objectTagsStore],
  ([$conversations, $view, $sections, $preference, $unreadOnly, $people, $groups, $group, $groupSort, $label, $pipeline, $inboxes, $inbox, $layout, $folder, $sidebar, $tree, $objectTags]) => {
    const now = Date.now()
    const members = $group ? groupConversationIds($groups, $group) : null
    const taggedGroups = new Set($label ? $objectTags.filter((tag) => tag.object_type === 'merge_group' && tag.tag_id === $label).map((tag) => tag.object_id) : [])
    const groupTagged = (id: string): boolean => {
      for (let lead = $tree.clusters.get(id); lead; lead = $tree.parents.get(lead)) if (taggedGroups.has($tree.ids.get(lead) ?? '')) return true
      return false
    }
    const membership = inboxMembership($inboxes, $conversations.data, $groups)
    const liveFor = (view: InboxView | null, inbox: string | null, label: string | null) => {
      const inboxMembers = inbox ? (membership.get(inbox) ?? new Set<string>()) : null
      return $conversations.data.filter(
        (c) =>
          (view ? inView(c, view, now) : !c.deleted_at) &&
          (!$unreadOnly || c.unread_count > 0) &&
          (!members || members.has(c.id)) &&
          (!label || c.tags.some((l) => l.tag_id === label) || groupTagged(c.id)) &&
          (view !== 'pipeline' || $pipeline.has(c.id)) &&
          (!inboxMembers || inboxMembers.has(c.id))
      )
    }
    const folder = $folder ? sidebarFolder($sidebar, $folder) : null
    if (folder) {
      const out: InboxGroup[] = []
      const flat = (list: Conversation[]): InboxItem[] => {
        const seen = new Set<string>()
        return groupItems(list, $people, [], $preference, null, $tree).flatMap((g) => g.rows).filter((item) => !seen.has(itemId(item)) && seen.add(itemId(item)))
      }
      const own = (groupId: string) => {
        const all = groupConversationIds($groups, groupId)
        for (const child of $groups.filter((g) => g.parent_id === groupId)) for (const c of groupConversationIds($groups, child.id)) all.delete(c)
        return all
      }
      const walk = (ids: string[], live: Conversation[]) => {
        for (const group of ids.map((gid) => $groups.find((g) => g.id === gid)).filter((g): g is Group => !!g)) {
          const mine = own(group.id)
          out.push({ key: `folder:group:${group.id}`, label: null, rows: flat(live.filter((c) => mine.has(c.id))), offset: 0, section: null, name: group.name })
          walk($groups.filter((g) => g.parent_id === group.id).map((g) => g.id), live)
        }
      }
      for (const child of folder.items) {
        if (child.kind !== 'item') continue
        const key = child.key
        if (key.startsWith(INBOX_PREFIX)) {
          const id = key.slice(INBOX_PREFIX.length)
          const named = $inboxes.find((i) => i.id === id)
          if (!named) continue
          const live = liveFor('inbox', id, null)
          const grouped = new Set<string>()
          for (const groupId of inboxGroupIds(named, $groups)) for (const c of groupConversationIds($groups, groupId)) grouped.add(c)
          out.push({ key: `folder:${key}`, label: null, rows: flat(live.filter((c) => !grouped.has(c.id))), offset: 0, section: null, name: named.name })
          walk(named.members.filter((m) => m.kind === 'group').map((m) => m.member_id), live)
        } else if (key === 'groups') {
          const linked = new Set($inboxes.filter((i) => !i.is_default).flatMap((i) => [...inboxGroupIds(i, $groups)]))
          walk($groups.filter((g) => !g.parent_id && !linked.has(g.id)).map((g) => g.id), liveFor('inbox', null, null))
        } else if (key === 'inbox') {
          const fallback = $inboxes.find((i) => i.is_default)
          out.push({ key: `folder:${key}`, label: null, rows: flat(liveFor('inbox', fallback?.id ?? null, null)), offset: 0, section: null, name: fallback?.name ?? t(VIEW_TITLES.inbox) })
        } else if (key in VIEW_TITLES) {
          out.push({ key: `folder:${key}`, label: VIEW_TITLES[key as InboxView], rows: flat(liveFor(key as InboxView, null, null)), offset: 0, section: null })
        }
      }
      return numbered(out)
    }
    const inbox = $label ? null : $inbox
    const view = $label && $view === 'inbox' ? null : $view
    const live = liveFor(view, inbox, $label)
    const inboxSort = inbox && !$group ? sortOrder($layout[sortKey(inbox)]) : null
    const order = $group && $groupSort ? byGroupSort($groupSort.sort, $groupSort.direction) : inboxSort ? byGroupSort(inboxSort.sort, inboxSort.direction) : null
    return groupItems(live, $people, $sections, $preference, order, $tree)
  }
)

export type MergedItem = Extract<InboxItem, { kind: 'merged' }>

const mergedEntries = (item: InboxItem): [string, MergedItem][] => (item.kind === 'merged' ? [[item.id, item], ...item.members.flatMap(mergedEntries)] : [])

export const mergedItems = derived([conversationsStore, peopleStore, mergeTreeStore], ([$conversations, $people, $tree]) =>
  new Map(
    groupItems($conversations.data.filter((c) => !c.deleted_at), $people, [], 'recent', null, $tree)
      .flatMap((group) => group.rows)
      .flatMap(mergedEntries)
  )
)

export const inboxRows = derived(inboxGroups, ($groups) => $groups.flatMap((group) => group.rows))

export const splitCounts = derived([conversationsStore, inboxView, splitsStore], ([$conversations, $view, $splits]) => {
  const now = Date.now()
  const live = $conversations.data.filter((c) => inView(c, $view === 'deleted' ? 'inbox' : $view, now))
  const counts = new Map<string, number>()
  for (const split of $splits) {
    counts.set(split.id, live.filter((c) => c.unread_count > 0 && inSplit(c, split.id, $splits)).length)
  }
  return counts
})

export const viewCounts = derived([conversationsStore, pipelineHolders], ([$conversations, $pipeline]) => {
  const now = Date.now()
  const counts = new Map<InboxView, number>()
  for (const view of ['inbox', 'snoozed', 'done', 'deleted', 'notifications'] as InboxView[]) {
    counts.set(view, $conversations.data.filter((c) => inView(c, view, now) && c.unread_count > 0).length)
  }
  counts.set('pipeline', $conversations.data.filter((c) => !c.deleted_at && $pipeline.has(c.id)).length)
  return counts
})

export const selectedItem = derived([inboxRows, selectedIndex], ([$rows, $index]) => $rows[Math.min($index, $rows.length - 1)] ?? null)

export function focusedItem(): InboxItem | null {
  const rows = get(inboxRows)
  return rows[Math.min(get(selectedIndex), rows.length - 1)] ?? null
}

export function moveSelection(delta: number) {
  const rows = get(inboxRows)
  if (rows.length === 0) return
  selectedIndex.update((index) => Math.max(0, Math.min(rows.length - 1, index + delta)))
}

export function selectConversation(id: string) {
  const index = get(inboxRows).findIndex((item) => itemConversations(item).some((c) => c.id === id))
  if (index >= 0) selectedIndex.set(index)
}

export function selectPerson(id: string) {
  const index = get(inboxRows).findIndex((item) => item.kind === 'person' && item.person.id === id)
  if (index >= 0) selectedIndex.set(index)
}

export function itemPath(item: InboxItem): string {
  if (item.kind === 'person') return '/main/person/' + item.person.id
  if (item.kind === 'merged') return mergedHref(item.id)
  return conversationHref(item.conversation.id)
}

export type RowImage = { kind: 'profile'; path: string } | { kind: 'chat'; conversationId: string }

export interface RowModel {
  kind: 'person' | 'chat'
  title: string
  avatar: AvatarSource
  open: string
  image: RowImage
}

export function counterpartOf(conversation: Conversation, me: Set<string>) {
  const others = conversation.members.filter((member) => !me.has(member.id))
  if (conversation.type === 'direct_message') return others[0] ?? null
  if (conversation.type === 'email' && others.length === 1) return others[0]
  return null
}

export function rowModel(item: InboxItem, people: Person[], me: Set<string>): RowModel {
  const open = itemPath(item)
  if (item.kind === 'person') return { kind: 'person', title: item.person.display_name, avatar: personAvatar(item.person), open, image: { kind: 'profile', path: personProfileHref(item.person.id) } }
  const lead = item.kind === 'conversation' ? item.conversation : item.conversations[0]
  const chat: RowImage = { kind: 'chat', conversationId: lead.id }
  if (item.kind === 'merged') {
    const members = item.members.map((member) => rowModel(member, people, me))
    const title = groupName(lead, members.map((member) => member.title))
    if (lead.overrides?.avatar_data) return { kind: 'chat', title, avatar: { ...chatAvatar(lead, people, me), name: title }, open, image: chat }
    const faces = [...new Map(members.flatMap((member) => (member.avatar.src || member.avatar.faces.length === 0 ? [member.avatar] : member.avatar.faces)).map((face) => [face.id, { id: face.id, name: face.name, src: face.src, color: face.color }])).values()]
    const avatar: AvatarSource = faces.length < 2 ? { ...members[0].avatar, name: title } : { shape: 'chat', id: lead.id, name: title, src: null, color: lead.overrides?.color ?? null, faces: faces.slice(0, AVATAR_FACES), icon: null }
    return { kind: 'chat', title, avatar, open, image: chat }
  }
  const counterpart = counterpartOf(lead, me)
  if (counterpart) {
    const owner = (lead.person_id ? people.find((person) => person.id === lead.person_id) : null) ?? personOf(people, { user_id: counterpart.id })
    const title = displayName(people, lead, counterpart.id, counterpart.display_name ?? counterpart.email ?? lead.name)
    return { kind: 'chat', title, avatar: lead.image ? chatAvatar(lead, people, me) : userAvatar(counterpart, people, title), open, image: owner ? { kind: 'profile', path: personProfileHref(owner.id) } : chat }
  }
  return { kind: 'chat', title: lead.name, avatar: chatAvatar(lead, people, me), open, image: chat }
}

export function toggleSelected(id: string) {
  selectedIds.update((set) => {
    const next = new Set(set)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    return next
  })
}

export function clearSelected() {
  selectedIds.set(new Set())
}

export function cycleSplit(delta: number) {
  const splits = get(splitsStore)
  const order = splits.map((s) => s.id)
  if (order.length === 0) return
  const current = order.indexOf(get(activeSplit))
  const next = (current + delta + order.length) % order.length
  activeSplit.set(order[next])
  selectedIndex.set(0)
}

export function relativeTime(iso: string, now = Date.now()): string {
  const date = new Date(iso)
  const diff = now - date.getTime()
  const minutes = Math.floor(diff / 60000)
  if (minutes < 1) return t('inbox.relative.now')
  if (minutes < 60) return t('inbox.relative.minutes', { count: minutes })
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return t('inbox.relative.hours', { count: hours })
  const days = Math.floor(hours / 24)
  if (days < 7) return t('inbox.relative.days', { count: days })
  return formatDate(date, { day: 'numeric', month: 'short' })
}

export function snoozeUntil(choice: 'later' | 'tomorrow' | 'weekend' | 'week', now = new Date()): string {
  const at = new Date(now)
  at.setSeconds(0, 0)
  switch (choice) {
    case 'later':
      at.setHours(at.getHours() + 3)
      break
    case 'tomorrow':
      at.setDate(at.getDate() + 1)
      at.setHours(8, 0)
      break
    case 'weekend': {
      const day = at.getDay()
      at.setDate(at.getDate() + ((6 - day + 7) % 7 || 7))
      at.setHours(9, 0)
      break
    }
    case 'week': {
      const day = at.getDay()
      at.setDate(at.getDate() + ((8 - day) % 7 || 7))
      at.setHours(8, 0)
      break
    }
  }
  return at.toISOString()
}

export function targetIds(focused: InboxItem | null): string[] {
  const picked = get(selectedIds)
  const rows = get(inboxRows)
  if (picked.size > 0) {
    return rows.filter((item) => picked.has(itemId(item))).flatMap((item) => itemConversations(item).map((c) => c.id))
  }
  return focused ? itemConversations(focused).map((c) => c.id) : []
}

export async function markDone(ids: string[], done = true) {
  await Promise.all(ids.map((conversation_id) => api.putConversation({ conversation_id, is_archived: done, snoozed_until: null })))
  clearSelected()
}

export async function snooze(ids: string[], until: string | null) {
  await Promise.all(ids.map((conversation_id) => api.putConversation({ conversation_id, snoozed_until: until, is_archived: false })))
  clearSelected()
}

export async function remove(ids: string[], conversations: Conversation[]) {
  const now = new Date().toISOString()
  await Promise.all(
    ids.map((conversation_id) => {
      const current = conversations.find((c) => c.id === conversation_id)
      return api.putConversation({ conversation_id, deleted_at: current?.deleted_at ? null : now })
    })
  )
  clearSelected()
}

export async function toggleUnread(ids: string[], conversations: Conversation[]) {
  await Promise.all(
    ids.map((conversation_id) => {
      const current = conversations.find((c) => c.id === conversation_id)
      if (!current) return Promise.resolve()
      if (current.unread_count > 0) return api.putConversationRead({ conversation_id })
      return current.last_id ? api.putMessageUnread({ message_id: current.last_id }) : Promise.resolve()
    })
  )
  clearSelected()
}

export async function toggleTag(ids: string[], tag_id: string, conversations: Conversation[]) {
  await Promise.all(
    ids.map((conversation_id) => {
      const current = conversations.find((c) => c.id === conversation_id)
      const has = current?.tags.some((l) => l.tag_id === tag_id)
      return has ? api.deleteObjectTag({ object_type: 'conversation', object_id: conversation_id, tag_id }) : api.putObjectTag({ object_type: 'conversation', object_id: conversation_id, tag_id })
    })
  )
}

export const inboxSheet = writable<'snooze' | 'tag' | null>(null)

export async function moveToGroup(ids: string[], group: Group) {
  const members = new Set(group.conversation_ids)
  const toAdd = ids.filter((id) => !members.has(id))
  if (toAdd.length) await api.putGroupItems({ group_id: group.id, conversation_ids: toAdd })
  else await api.deleteGroupItems({ group_id: group.id, conversation_ids: ids })
  groupsStore.set(await api.getGroups())
  clearSelected()
}

export async function toggleSection(ids: string[], section: SidebarSection) {
  const members = new Set(section.conversations.map((c) => c.id))
  await Promise.all(
    ids.map((conversation_id) =>
      members.has(conversation_id)
        ? api.deleteSidebarSectionItem({ section_id: section.id, conversation_id })
        : api.postSidebarSectionItem({ section_id: section.id, conversation_id })
    )
  )
  sidebarSectionsStore.set(await api.getSidebarSections())
}

export type Favourite = { kind: 'person'; id: string; name: string; person: Person } | { kind: 'conversation'; id: string; name: string; conversation: Conversation }

export function favouriteList(people: Person[], conversations: Conversation[]): Favourite[] {
  const list: { position: number; entry: Favourite }[] = []
  for (const person of people) {
    if (person.favourite_position === null) continue
    list.push({ position: person.favourite_position, entry: { kind: 'person', id: person.id, name: person.display_name, person } })
  }
  for (const conversation of conversations) {
    if (conversation.favourite_position === null || conversation.deleted_at) continue
    list.push({ position: conversation.favourite_position, entry: { kind: 'conversation', id: conversation.id, name: conversation.name, conversation } })
  }
  return list.sort((a, b) => a.position - b.position || a.entry.name.localeCompare(b.entry.name)).map((item) => item.entry)
}

export const favourites = derived([peopleStore, conversationsStore], ([$people, $conversations]) => favouriteList($people, $conversations.data))

export function reorder<T>(list: T[], from: number, to: number): T[] {
  if (from === to || from < 0 || to < 0 || from >= list.length || to >= list.length) return list
  const next = [...list]
  const [moved] = next.splice(from, 1)
  next.splice(to, 0, moved)
  return next
}

function stored(key: string, fallback: string): string {
  if (typeof localStorage === 'undefined') return fallback
  return localStorage.getItem(key) ?? fallback
}

export const LIST_COLLAPSED_KEY = 'list.collapsed'

export const listCollapsed = derived(layoutSettings, ($settings) => $settings[LIST_COLLAPSED_KEY] === 'true')

export function toggleList() {
  putLayoutSetting(LIST_COLLAPSED_KEY, get(listCollapsed) ? null : 'true').catch(() => {})
}

export const UI_SCALES = [90, 100, 110, 125] as const
export type UiScale = (typeof UI_SCALES)[number]

export const uiScale = writable<UiScale>((Number(stored('chatnip.ui-scale', '100')) as UiScale) || 100)

export function applyUiScale(scale: UiScale) {
  if (typeof document === 'undefined') return
  document.documentElement.style.fontSize = `${(15 * scale) / 100}px`
}

export function setUiScale(scale: UiScale) {
  const next = UI_SCALES.includes(scale) ? scale : 100
  if (typeof localStorage !== 'undefined') localStorage.setItem('chatnip.ui-scale', String(next))
  uiScale.set(next)
  applyUiScale(next)
}

export function stepUiScale(delta: 1 | -1 | 0) {
  const current = get(uiScale)
  if (delta === 0) return setUiScale(100)
  const index = UI_SCALES.indexOf(current)
  const next = UI_SCALES[Math.max(0, Math.min(UI_SCALES.length - 1, index + delta))]
  setUiScale(next)
}

export type PaneKey = `chatnip.${string}`

const RELATIVE = /^-?\d+(\.\d+)?(vw|vh|rem)$/

export function paneSize(key: PaneKey, fallback: string): string {
  const value = stored(key, fallback)
  return value === 'auto' || RELATIVE.test(value) ? value : fallback
}

export function relativeSize(pixels: number, axis: 'x' | 'y', viewport = { width: window.innerWidth, height: window.innerHeight }): string {
  const extent = axis === 'x' ? viewport.width : viewport.height
  return `${Math.round((pixels / extent) * 10000) / 100}${axis === 'x' ? 'vw' : 'vh'}`
}

export function holdPane(event: PointerEvent, key: PaneKey, property: string, onSaved?: (_value: string) => void) {
  const handle = event.currentTarget as HTMLElement | null
  const grid = handle?.closest<HTMLElement>('[data-pane-grid]')
  if (!grid || !handle) return
  const axis: 'x' | 'y' = handle.dataset.axis === 'y' ? 'y' : 'x'
  const pane = [...grid.children].find((child) => child.contains(handle)) as HTMLElement | undefined
  if (!pane) return
  const box = pane.getBoundingClientRect()
  grid.style.setProperty(property, `${Math.round(axis === 'x' ? box.width : box.height)}px`)
  grid.classList.add('pane-dragging')
  const end = () => {
    window.removeEventListener('pointerup', end)
    window.removeEventListener('pointercancel', end)
    grid.classList.remove('pane-dragging')
    const pixels = parseFloat(grid.style.getPropertyValue(property))
    if (!Number.isFinite(pixels) || pixels <= 0) return
    const value = relativeSize(pixels, axis)
    grid.style.setProperty(property, value)
    grid.style.removeProperty(`${property}_calc`)
    if (typeof localStorage !== 'undefined') localStorage.setItem(key, value)
    onSaved?.(value)
  }
  window.addEventListener('pointerup', end)
  window.addEventListener('pointercancel', end)
}

const storedComposerSize = paneSize('chatnip.composer-size', '18vh')

export const composerSize = writable(storedComposerSize === 'auto' ? '18vh' : storedComposerSize)

export function rowDate(iso: string, now = new Date()): string {
  const date = new Date(iso)
  const sameDay = date.toDateString() === now.toDateString()
  if (sameDay) return formatDate(date, { hour: '2-digit', minute: '2-digit' })
  const day = date.getDate()
  const month = formatDate(date, { month: 'short' }).replace('.', '').toUpperCase()
  if (date.getFullYear() !== now.getFullYear()) return `${day} ${month} ${String(date.getFullYear()).slice(2)}`
  return `${day} ${month}`
}

export function dateSublabel(iso: string, now = new Date()): string {
  const date = new Date(iso)
  const start = new Date(now)
  start.setHours(0, 0, 0, 0)
  const days = Math.floor((start.getTime() - new Date(date).setHours(0, 0, 0, 0)) / 86400000)
  if (days <= 0) return t('common.today')
  if (days === 1) return t('common.yesterday')
  if (days < 7) return t('inbox.date.lastWeek')
  if (days < 30) return t('inbox.date.lastMonth')
  const month = formatDate(date, date.getFullYear() === now.getFullYear() ? { month: 'long' } : { month: 'long', year: 'numeric' })
  return month.charAt(0).toUpperCase() + month.slice(1)
}

export const DATE_SUBLABEL_MIN = 8

export function dateSegments(rows: InboxItem[], now = new Date()): { label: string | null; rows: InboxItem[]; offset: number }[] {
  if (rows.length <= DATE_SUBLABEL_MIN) return [{ label: null, rows, offset: 0 }]
  const segments: { label: string | null; rows: InboxItem[]; offset: number }[] = []
  rows.forEach((row, index) => {
    const label = dateSublabel(itemActivity(row), now)
    const last = segments[segments.length - 1]
    if (last && last.label === label) last.rows.push(row)
    else segments.push({ label, rows: [row], offset: index })
  })
  return segments
}

export function tabScrollTarget(scrollLeft: number, viewWidth: number, tabStart: number, tabWidth: number, margin = 24): number {
  if (tabStart - margin < scrollLeft) return Math.max(0, tabStart - margin)
  if (tabStart + tabWidth + margin > scrollLeft + viewWidth) return tabStart + tabWidth + margin - viewWidth
  return scrollLeft
}

export function edgeFade(scrollLeft: number, scrollWidth: number, clientWidth: number): string | undefined {
  const start = scrollLeft > 1
  const end = scrollLeft + clientWidth < scrollWidth - 1
  if (!start && !end) return undefined
  return `linear-gradient(to right, ${start ? 'transparent, black 2rem' : 'black'}, ${end ? 'black calc(100% - 2.5rem), transparent' : 'black'})`
}

export async function mergeItems(items: InboxItem[]): Promise<(() => Promise<void>) | null> {
  if (items.length < 2) return null
  const [head, ...sources] = items
  const anchor = head.kind === 'merged' ? head.id : itemConversations(head)[0]?.id
  if (!anchor) return null
  const conversation_ids = [...new Set([...(head.kind === 'merged' ? [] : [head]), ...sources.filter((item) => item.kind !== 'merged')].flatMap((item) => itemConversations(item).map((c) => c.id)))].filter((id) => id !== anchor)
  const group_lead_ids = sources.flatMap((item) => (item.kind === 'merged' && item.id !== anchor ? [item.id] : []))
  const input = { lead_conversation_id: anchor, conversation_ids, group_lead_ids }
  mergeGroups.set(await api.postMergeGroup(input))
  clearSelected()
  return async () => {
    mergeGroups.set(await api.deleteMergeGroupItems(input))
  }
}

export async function unmergeItem(item: InboxItem, parent: MergedItem): Promise<void> {
  const nested = item.kind === 'merged'
  mergeGroups.set(await api.deleteMergeGroupItems({ lead_conversation_id: parent.id, conversation_ids: nested ? [] : itemConversations(item).map((c) => c.id), group_lead_ids: nested ? [item.id] : [] }))
}

export type MessageView = 'saved' | 'activity' | 'reminders'

export const MESSAGE_VIEWS: { view: MessageView; path: string; title: MessageKey; icon: string }[] = [
  { view: 'activity', path: '/main/activity', title: 'inbox.view.activity', icon: 'lucide:at-sign' },
  { view: 'saved', path: '/main/saved', title: 'inbox.view.saved', icon: 'lucide:bookmark' },
  { view: 'reminders', path: '/main/reminders', title: 'inbox.view.reminders', icon: 'lucide:alarm-clock' }
]

const JUMP_TRIES = 30
const JUMP_DELAY_MS = 100

export async function openMessage(conversationId: string, messageId: string | null = null): Promise<void> {
  const [{ goto }, { scrollToMessage }] = await Promise.all([import('$app/navigation'), import('$lib/reply.svelte')])
  await api.putCurrentConversation(conversationId).catch(() => {})
  await goto(conversationHref(conversationId))
  if (!messageId) return
  for (let i = 0; i < JUMP_TRIES; i++) {
    if (scrollToMessage(messageId)) return
    await new Promise((resolve) => setTimeout(resolve, JUMP_DELAY_MS))
  }
}

export async function openAdjacent(delta: 1 | -1, unread = false): Promise<boolean> {
  const rows = get(inboxRows)
  if (rows.length === 0) return false
  const current = get(conversationsStore).data.find((c) => c.is_current)
  const at = current ? rows.findIndex((item) => itemConversations(item).some((c) => c.id === current.id)) : get(selectedIndex)
  let index = at < 0 ? (delta > 0 ? -1 : rows.length) : at
  do {
    index += delta
    if (index < 0 || index >= rows.length) return false
  } while (unread && itemUnread(rows[index]) === 0)
  selectedIndex.set(index)
  const { goto } = await import('$app/navigation')
  await goto(itemPath(rows[index]))
  return true
}
