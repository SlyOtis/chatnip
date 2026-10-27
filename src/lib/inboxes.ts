import { derived, get, writable } from 'svelte/store'
import type { Conversation, Group, Inbox, InboxMember } from '$lib/tauri/types'
import { getInboxes, putInboxMembers } from '$lib/tauri/api'
import { descendants, groupConversationIds } from '$lib/tauri/groups'

export const inboxesStore = writable<Inbox[]>([])
export const activeInbox = writable<string | null>(null)

export const userInboxes = derived(inboxesStore, ($inboxes) => $inboxes.filter((inbox) => !inbox.is_default).sort((a, b) => a.position - b.position))
export const defaultInbox = derived(inboxesStore, ($inboxes) => $inboxes.find((inbox) => inbox.is_default) ?? null)

export async function loadInboxes(): Promise<Inbox[]> {
  const inboxes = await getInboxes()
  setInboxes(inboxes)
  return inboxes
}

export function setInboxes(inboxes: Inbox[]) {
  inboxesStore.set(inboxes)
  const active = get(activeInbox)
  if (!active || !inboxes.some((inbox) => inbox.id === active)) activeInbox.set(inboxes.find((inbox) => inbox.is_default)?.id ?? null)
}

export async function listenInboxes(): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event')
  return listen<Inbox[]>('inboxes-updated', (event) => setInboxes(event.payload))
}

export function memberOf(inbox: Inbox, conversation: Conversation, groups: Group[]): boolean {
  for (const member of inbox.members) {
    switch (member.kind) {
      case 'conversation':
        if (member.member_id === conversation.id) return true
        break
      case 'provider':
        if (conversation.providers.some((p) => p.id === member.member_id)) return true
        break
      case 'person':
        if (conversation.person_id === member.member_id) return true
        break
      case 'tag':
        if (conversation.tags.some((l) => l.tag_id === member.member_id)) return true
        break
      case 'group':
        if (groupConversationIds(groups, member.member_id).has(conversation.id)) return true
        break
    }
  }
  return false
}

export function inboxMembership(inboxes: Inbox[], conversations: Conversation[], groups: Group[]): Map<string, Set<string>> {
  const users = inboxes.filter((inbox) => !inbox.is_default)
  const fallback = inboxes.find((inbox) => inbox.is_default)
  const membership = new Map<string, Set<string>>()
  for (const inbox of inboxes) membership.set(inbox.id, new Set())
  for (const conversation of conversations) {
    let placed = false
    for (const inbox of users) {
      if (memberOf(inbox, conversation, groups)) {
        membership.get(inbox.id)!.add(conversation.id)
        placed = true
      }
    }
    if (!placed && fallback) membership.get(fallback.id)!.add(conversation.id)
  }
  return membership
}

export function inboxGroupIds(inbox: Inbox, groups: Group[]): Set<string> {
  const roots = inbox.members.filter((m) => m.kind === 'group').map((m) => m.member_id)
  return new Set(roots.flatMap((id) => [id, ...descendants(groups, id)]))
}

export function hasMember(inbox: Inbox, member: InboxMember): boolean {
  return inbox.members.some((m) => m.kind === member.kind && m.member_id === member.member_id)
}

export async function addMembers(inbox: Inbox, members: InboxMember[]): Promise<void> {
  if (!members.length || inbox.is_default) return
  const updated = await putInboxMembers({ inbox_id: inbox.id, add: members })
  inboxesStore.set(get(inboxesStore).map((candidate) => (candidate.id === updated.id ? updated : candidate)))
}
