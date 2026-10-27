import { derived, writable } from 'svelte/store'
import type { Chat, Conversation, Person, PersonIdentity, PostIdentityInput } from './types'
import { getPeople, getMyUserIds, postChat } from './api'
import { loadChats } from './chats'
import { providerLabel } from '$lib/providerIcons'
import { t } from '$lib/i18n.svelte'
import {  } from '$lib/tauri/conversations'
import { conversationHref } from '$lib/tauri/conversations'

export const peopleStore = writable<Person[]>([])

export const peopleById = derived(peopleStore, ($people) => new Map($people.map((person) => [person.id, person])))

export const myUserIds = writable<Set<string>>(new Set())

export function isMine(ids: Set<string>, userId: string | null | undefined): boolean {
  return !!userId && ids.has(userId)
}

export async function loadPeople(): Promise<Person[]> {
  const [people, mine] = await Promise.all([getPeople(), getMyUserIds()])
  peopleStore.set(people)
  myUserIds.set(new Set(mine))
  return people
}

export const PERSON_LINK = 'rounded-[0.2rem] font-semibold text-inherit underline-offset-[0.18em] outline-none hover:underline focus-visible:underline focus-visible:ring-2 focus-visible:ring-accent'

export const PERSON_NAME = 'font-semibold text-inherit'

export function personHref(personId: string): string {
  return `/main/person/${encodeURIComponent(personId)}`
}

export function personOf(people: Person[], ref: { user_id?: string | null; external_id?: string | null }): Person | null {
  const { user_id, external_id } = ref
  if (!user_id && !external_id) return null
  return people.find((person) => person.identities.some((identity) => (!!user_id && identity.user_id === user_id) || (!!external_id && identity.external_id === external_id))) ?? null
}

export function personEmail(person: Pick<Person, 'primary_email' | 'identities'>): string | null {
  return person.primary_email ?? person.identities.find((identity) => identity.email)?.email ?? null
}

export function personInitials(name: string): string {
  return (
    name
      .split(/[\s@._|-]+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((word) => word[0]?.toUpperCase() ?? '')
      .join('') || '?'
  )
}

export function identityLabel(identity: PersonIdentity): { primary: string; secondary: string | null; source: string } {
  const primary = identity.email ?? identity.display_name ?? identity.external_id ?? identity.provider_type
  const secondary = identity.display_name && identity.display_name !== primary ? identity.display_name : identity.user_id ? null : t('people.identity.waiting')
  const source = providerLabel(identity)
  return { primary, secondary, source }
}

export function identityInput(personId: string, providerType: string, value: string, displayName = ''): PostIdentityInput | null {
  const text = value.trim()
  if (!text) return null
  const name = displayName.trim() || null
  if (text.includes('@') && !text.startsWith('@')) return { person_id: personId, provider_type: providerType, email: text.toLowerCase(), external_id: null, display_name: name }
  return { person_id: personId, provider_type: providerType, email: null, external_id: text.replace(/^@/, ''), display_name: name }
}

export function mergeCandidates(people: Person[], personId: string, query: string, max = 8): Person[] {
  const needle = query.trim().toLowerCase()
  if (!needle) return []
  return people
    .filter((person) => person.id !== personId)
    .filter((person) => [person.display_name, person.primary_email ?? '', ...person.identities.flatMap((i) => [i.email ?? '', i.display_name ?? ''])].some((text) => text.toLowerCase().includes(needle)))
    .slice(0, max)
}

export function personProfileHref(personId: string): string {
  return `${personHref(personId)}/profile`
}

export function displayName(people: Person[], conversation: Pick<Conversation, 'nicknames'> | null | undefined, userId: string | null | undefined, fallback: string): string {
  if (!userId) return fallback
  const nickname = conversation?.nicknames?.[userId]?.trim()
  if (nickname) return nickname
  return personOf(people, { user_id: userId })?.overrides.display_name?.trim() || fallback
}

export type PersonTarget = { kind: 'thread' | 'chat'; path: string } | { kind: 'create' }

export function personTarget(person: Pick<Person, 'id' | 'conversation_ids'>, chats: Pick<Chat, 'conversation_id' | 'sources' | 'mode'>[]): PersonTarget {
  if (person.conversation_ids.length > 0) return { kind: 'thread', path: personHref(person.id) }
  const chat = chats.find((candidate) => candidate.mode === 'write' && candidate.sources.some((source) => source.kind === 'person' && source.id === person.id))
  if (chat) return { kind: 'chat', path: conversationHref(chat.conversation_id) }
  return { kind: 'create' }
}

export async function openPerson(person: Pick<Person, 'id' | 'display_name' | 'conversation_ids'>, chats: Chat[], go: (path: string) => unknown): Promise<string> {
  const target = personTarget(person, chats)
  if (target.kind !== 'create') {
    await go(target.path)
    return target.path
  }
  const chat = await postChat({ name: person.display_name, mode: 'write', sources: [{ kind: 'person', id: person.id }] })
  await loadChats()
  const path = conversationHref(chat.conversation_id)
  await go(path)
  return path
}
