import { writable } from 'svelte/store'
import type { MessageKey } from '$lib/i18n.svelte'
import type { Chat, ChatMode, ChatSource, ChatSourceKind, ChatView, Conversation, ViewSettings } from './types'
import { getChats, putConversation } from './api'
import { conversationStore, conversationsStore } from './conversations'

export const RECEIPTS_KEY = 'builtin:receipts'

export const chatsStore = writable<Chat[]>([])

export async function loadChats(): Promise<Chat[]> {
  const chats = await getChats()
  chatsStore.set(chats)
  return chats
}

export interface NewChatTarget {
  mode?: ChatMode
  sources?: ChatSource[]
  actionId?: string | null
}

export const newChatStore = writable<NewChatTarget | null>(null)

export function openNewChat(target: NewChatTarget = {}) {
  newChatStore.set(target)
}

export function closeNewChat() {
  newChatStore.set(null)
}

export const CHAT_MODES: { value: ChatMode; label: MessageKey; icon: string }[] = [
  { value: 'read', label: 'chats.mode.read', icon: 'lucide:book-open' },
  { value: 'write', label: 'chats.mode.write', icon: 'lucide:pen-line' }
]

export const NOTIFY_LEVELS: { value: string; label: MessageKey; icon: string }[] = [
  { value: 'all', label: 'conversation.notify.all', icon: 'lucide:bell' },
  { value: 'mentions', label: 'conversation.notify.mentions', icon: 'lucide:at-sign' },
  { value: 'nothing', label: 'conversation.notify.muted', icon: 'lucide:bell-off' }
]

export const CHAT_VIEWS: { value: ChatView; label: MessageKey; icon: string }[] = [
  { value: 'chat', label: 'chats.view.chat', icon: 'lucide:messages-square' },
  { value: 'grid', label: 'chats.view.grid', icon: 'lucide:layout-grid' }
]

export const SOURCE_KINDS: { value: ChatSourceKind | 'chat'; label: MessageKey; icon: string }[] = [
  { value: 'conversation', label: 'chats.source.chats', icon: 'lucide:message-square' },
  { value: 'chat', label: 'chats.source.readChats', icon: 'lucide:book-open' },
  { value: 'group', label: 'chats.source.groups', icon: 'lucide:folder' },
  { value: 'tag', label: 'chats.source.tags', icon: 'lucide:tag' },
  { value: 'person', label: 'chats.source.people', icon: 'lucide:user' },
  { value: 'query', label: 'chats.source.search', icon: 'lucide:search' }
]

export function sameSource(a: ChatSource, b: ChatSource): boolean {
  return a.kind === b.kind && (a.kind === 'query' ? JSON.stringify(a.query ?? null) === JSON.stringify(b.query ?? null) : (a.id ?? null) === (b.id ?? null))
}

export function toggleSource(sources: ChatSource[], source: ChatSource): ChatSource[] {
  return sources.some((s) => sameSource(s, source)) ? sources.filter((s) => !sameSource(s, source)) : [...sources, source]
}

function patch(id: string, change: Partial<Conversation>) {
  conversationStore.update((state) => (state.data?.id === id ? { ...state, data: { ...state.data, ...change } } : state))
  conversationsStore.update((state) => ({ ...state, data: state.data.map((c) => (c.id === id ? { ...c, ...change } : c)) }))
}

export async function putChatMode(id: string, mode: ChatMode): Promise<void> {
  await putConversation({ conversation_id: id, mode })
  patch(id, { mode })
}

export async function putChatView(id: string, view: ChatView): Promise<void> {
  await putConversation({ conversation_id: id, view })
  patch(id, { view })
}

export async function putChatViewSettings(id: string, view_settings: ViewSettings): Promise<void> {
  patch(id, { view_settings })
  await putConversation({ conversation_id: id, view_settings })
}

export function participants(conversation: Pick<Conversation, 'members'>, me: Set<string> = new Set()): string {
  return conversation.members
    .filter((m) => !me.has(m.id))
    .map((m) => m.display_name || m.email || '')
    .filter(Boolean)
    .join(', ')
}

export function chatChoices(conversations: Conversation[], query: string, local: Set<string>, me: Set<string> = new Set(), limit = 60): Conversation[] {
  const words = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean)
  return conversations
    .filter((c) => !local.has(c.id) && !c.deleted_at && c.type !== 'local')
    .filter((c) => {
      if (!words.length) return true
      const hay = [c.name, participants(c, me), ...c.members.filter((m) => !me.has(m.id)).map((m) => m.email ?? ''), ...c.providers.map((p) => `${p.name} ${p.provider_type}`)].join(' ').toLocaleLowerCase()
      return words.every((word) => hay.includes(word))
    })
    .sort((a, b) => b.last_activity.localeCompare(a.last_activity))
    .slice(0, limit)
}

export function viaStorageKey(chatId: string): string {
  return `chatnip.via.${chatId}`
}

export const chatDetailsStore = writable<string | null>(null)

export function openChatDetails(conversationId: string) {
  chatDetailsStore.set(conversationId)
}

export function closeChatDetails() {
  chatDetailsStore.set(null)
}
