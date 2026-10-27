import { writable } from 'svelte/store'

export type PresenceStatus = 'online' | 'away' | 'dnd' | 'offline'

export interface UserPresence {
  user_id: string
  status: PresenceStatus
  status_text: string
  status_emoji: string
  status_expiration: string | null
  last_seen: string | null
}

export const presenceStore = writable<Map<string, UserPresence>>(new Map())

export function updatePresence(user_id: string, presence: Partial<UserPresence>) {
  presenceStore.update(map => {
    const existing = map.get(user_id) || {
      user_id,
      status: 'offline' as PresenceStatus,
      status_text: '',
      status_emoji: '',
      status_expiration: null,
      last_seen: null,
    }
    map.set(user_id, { ...existing, ...presence })
    return new Map(map)
  })
}

export function getPresence(user_id: string): UserPresence | undefined {
  let result: UserPresence | undefined
  presenceStore.subscribe(map => result = map.get(user_id))()
  return result
}

export interface Typer {
  conversation_id: string
  user_id: string | null
  user_name: string
  timestamp: number
}

export const typingStore = writable<Map<string, Typer>>(new Map())

export function putTyping(conversation_id: string, user_id: string | null, user_name: string) {
  typingStore.update((map) => {
    map.set(`${conversation_id}:${user_id ?? user_name}`, { conversation_id, user_id, user_name, timestamp: Date.now() })
    return new Map(map)
  })
}

export function deleteTyping(conversation_id: string, user_id: string | null, user_name: string) {
  typingStore.update((map) => {
    map.delete(`${conversation_id}:${user_id ?? user_name}`)
    return new Map(map)
  })
}

const PRESENCE_TTL_MS = 60_000
const asked = new Map<string, number>()

export async function requestPresence(user_ids: string[]) {
  const now = Date.now()
  const wanted = user_ids.filter((id) => id && now - (asked.get(id) ?? 0) > PRESENCE_TTL_MS)
  if (wanted.length === 0) return
  for (const id of wanted) asked.set(id, now)
  const { getPresences } = await import('./api')
  for (const found of await getPresences(wanted).catch(() => [])) {
    updatePresence(found.user_id, { status: found.status, ...(found.status_text != null ? { status_text: found.status_text } : {}), ...(found.status_emoji != null ? { status_emoji: found.status_emoji } : {}) })
  }
}
