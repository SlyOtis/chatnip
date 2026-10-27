import type { SystemEvent, SystemEventKind, SystemPerson } from '$lib/tauri/types'
import { getLocale, intlLocale, translate, type Locale, type MessageKey } from '$lib/i18n.svelte'
import { parseMetadata } from '$lib/messageKind'

export type SystemPart = { text: string } | { person: SystemPerson; name: string }

const KINDS: SystemEventKind[] = ['member_joined', 'member_left', 'member_added', 'member_removed', 'renamed', 'topic_changed', 'call_started', 'call_ended', 'pinned', 'unpinned', 'chat_created']

function peopleOf(value: unknown): SystemPerson[] {
  if (!Array.isArray(value)) return []
  return value
    .filter((item): item is Record<string, unknown> => !!item && typeof item === 'object')
    .map((item) => ({
      user_id: typeof item.user_id === 'string' && item.user_id ? item.user_id : null,
      external_id: typeof item.external_id === 'string' && item.external_id ? item.external_id : null,
      name: typeof item.name === 'string' ? item.name : ''
    }))
}

export function systemEventOf(metadata: string | null | undefined): SystemEvent | null {
  const raw = parseMetadata(metadata).system
  if (!raw || typeof raw !== 'object') return null
  const system = raw as Record<string, unknown>
  const event = KINDS.find((kind) => kind === system.event)
  if (!event) return null
  const detail = typeof system.detail === 'string' && system.detail.trim() ? system.detail.trim() : null
  return { event, actors: peopleOf(system.actors), targets: peopleOf(system.targets), detail }
}

function templateOf(event: SystemEvent): { key: MessageKey; actors: SystemPerson[] } {
  const alone = event.actors.length === 0
  switch (event.event) {
    case 'member_joined':
      return { key: 'conversation.system.memberJoined', actors: event.actors }
    case 'member_left':
      return { key: 'conversation.system.memberLeft', actors: event.actors }
    case 'member_added':
      return alone ? { key: 'conversation.system.memberJoined', actors: event.targets } : { key: 'conversation.system.memberAdded', actors: event.actors }
    case 'member_removed':
      return alone ? { key: 'conversation.system.memberLeft', actors: event.targets } : { key: 'conversation.system.memberRemoved', actors: event.actors }
    case 'renamed':
      return { key: event.detail ? 'conversation.system.renamedTo' : 'conversation.system.renamed', actors: event.actors }
    case 'topic_changed':
      return { key: event.detail ? 'conversation.system.topicChanged' : 'conversation.system.topicCleared', actors: event.actors }
    case 'call_started':
      return { key: 'conversation.system.callStarted', actors: event.actors }
    case 'call_ended':
      return { key: alone ? 'conversation.system.callEnded' : 'conversation.system.callEndedBy', actors: event.actors }
    case 'pinned':
      return { key: 'conversation.system.pinned', actors: event.actors }
    case 'unpinned':
      return { key: 'conversation.system.unpinned', actors: event.actors }
    case 'chat_created':
      return { key: 'conversation.system.chatCreated', actors: event.actors }
  }
}

function listParts(people: SystemPerson[], locale: Locale): SystemPart[] {
  const someone = translate(locale, 'conversation.system.someone')
  if (!people.length) return [{ text: someone }]
  const names = people.map((person) => person.name.trim() || someone)
  let index = 0
  return new Intl.ListFormat(intlLocale(locale), { style: 'long', type: 'conjunction' })
    .formatToParts(names)
    .map((part) => {
      if (part.type !== 'element') return { text: part.value }
      const at = index++
      return { person: people[at], name: names[at] }
    })
}

export function systemParts(event: SystemEvent, locale: Locale = getLocale()): SystemPart[] {
  const { key, actors } = templateOf(event)
  const parts: SystemPart[] = []
  for (const piece of translate(locale, key).split(/(\{(?:actor|targets|detail)\})/)) {
    if (!piece) continue
    if (piece === '{actor}') parts.push(...listParts(actors, locale))
    else if (piece === '{targets}') parts.push(...listParts(event.targets, locale))
    else if (piece === '{detail}') parts.push({ text: event.detail ?? '' })
    else parts.push({ text: piece })
  }
  return parts
}

