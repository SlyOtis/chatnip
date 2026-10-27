import { derived, get, writable } from 'svelte/store'
import type { ConversationsResponse, ConversationResponse, MessagesResponse, PinnedMessage } from '$lib/tauri'
import { getPinnedMessages, postMessagePin, deleteMessagePin } from './api'

export const conversationsStore = writable<ConversationsResponse>({
  is_loading: false,
  data: []
});

export const conversationsById = derived(conversationsStore, ($conversations) => new Map($conversations.data.map((conversation) => [conversation.id, conversation])))

export const conversationStore = writable<ConversationResponse>({
  is_loading: false,
  data: null
});

export const messagesStore = writable<MessagesResponse>({
  is_loading: false,
  data: []
});

export const pinnedStore = writable<{ conversation_id: string | null; data: PinnedMessage[] }>({
  conversation_id: null,
  data: []
});

let pinnedLoading: { conversation_id: string; done: Promise<PinnedMessage[]> } | null = null

export function loadPinned(conversation_id: string): Promise<PinnedMessage[]> {
  if (pinnedLoading?.conversation_id === conversation_id) return pinnedLoading.done
  const done: Promise<PinnedMessage[]> = getPinnedMessages({ conversation_id })
    .catch(() => [])
    .then((data) => {
      if (pinnedLoading?.done !== done) return data
      pinnedLoading = null
      pinnedStore.set({ conversation_id, data })
      return data
    })
  pinnedLoading = { conversation_id, done }
  return done
}

export async function putPin(conversation_id: string, message_id: string, pinned: boolean): Promise<void> {
  await (pinned ? postMessagePin : deleteMessagePin)({ conversation_id, message_id })
  if (get(pinnedStore).conversation_id !== conversation_id) return
  pinnedLoading = null
  await loadPinned(conversation_id)
}

export function conversationHref(id: string): string {
  return `/main/conversation/${encodeURIComponent(id)}`
}

export function mergedHref(id: string): string {
  return `/main/merged/${encodeURIComponent(id)}`
}
