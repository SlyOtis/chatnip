import {listen} from '@tauri-apps/api/event'
import {get} from 'svelte/store'
import type {
  ConversationsResponse,
  ConversationResponse,
  MessagesResponse,
  ProfileResponse,
  ProvidersResponse,
  UsersResponse,
  CaseFile,
  WorkspaceEmoji,
} from './types'
import {loadWorkspaceEmoji, putWorkspaceEmoji} from '$lib/emoji'

import {
  conversationsStore,
  conversationStore,
  messagesStore,
  conversationHref
} from './conversations'
import {providersStore, connectionConnected, connectionFailed} from './providers'
import {usersStore} from './users'
import {profileStore} from './profile'
import {loadTags, loadObjectTags} from './tags'
import {sidebarSectionsStore} from './sidebar'
import {loadPeople} from './people'
import {loadGroups} from './groups'
import {casesStore, loadCases} from './cases'
import {loadChats} from './chats'
import * as api from './api'
import {putTyping, typingStore, updatePresence} from './presence'
import type {PresenceUpdate, TypingUpdate} from './types'

export {
  conversationsStore, conversationsById, conversationStore, messagesStore, pinnedStore, loadPinned, putPin, conversationHref, mergedHref
} from './conversations'
export {providersStore, connectionStore, connectionWaiting, connectionConnected, connectionFailed, connectionReset, putAccountUsed} from './providers'
export type {ConnectionState, ConnectionPhase} from './providers'
export {usersStore} from './users'
export {presenceStore, updatePresence, getPresence, typingStore, requestPresence} from './presence'
export * from './types'
export {profileStore, profileSettingsStore, profileThemeStore} from './profile'
export {threadPanelStore, openThread, closeThread} from './threads'
export {sortPreferenceStore, unreadOnlyStore, sidebarSectionsStore} from './sidebar'
export {tagRecordsStore, splitsStore, tagsStore, loadTags, tagColor, objectTagsStore, loadObjectTags, tagIdsOn, toggleObjectTag, tagTone, TAG_COLOURS, OBJECT_ORDER, OBJECT_ICONS, groupTagged} from './tags'
export {peopleStore, peopleById, loadPeople, personInitials, myUserIds, isMine, identityLabel, identityInput, mergeCandidates} from './people'
export {groupsStore, groupsById, loadGroups, readStore, readIds, composerOpen} from './groups'
export {casesStore, casesById, loadCases, casePickerStore, openCasePicker, closeCasePicker} from './cases'
export {chatsStore, loadChats, newChatStore, openNewChat, closeNewChat, putChatMode, putChatView, putChatViewSettings, chatDetailsStore, openChatDetails, closeChatDetails} from './chats'
export type {SortPreference} from './sidebar'
export {api}

export async function initStores() {
  profileStore.update(state => ({...state, is_loading: true}))
  api.getProfile()
    .then(response => profileStore.set(response))
    .catch(error => console.error(error.message || 'Failed to load profile'))

  providersStore.update(state => ({...state, is_loading: true}))
  api.getProviders()
    .then(response => providersStore.set(response))
    .catch(error => console.error(error.message || 'Failed to load providers'))

  usersStore.update(state => ({...state, is_loading: true}))
  api.getUsers()
    .then(response => usersStore.set(response))
    .catch(error => console.error(error.message || 'Failed to load users'))

  conversationStore.update(state => ({...state, is_loading: true}))
  api.getCurrentConversation()
    .then(response => conversationStore.set(response))
    .catch(error => console.error(error.message || 'Failed to load conversation'))

  messagesStore.update(state => ({...state, is_loading: true}))
  api.getMessages()
    .then(response => messagesStore.set(response))
    .catch(error => console.error(error.message || 'Failed to load messages'))

  conversationsStore.update(state => ({...state, is_loading: true}))
  api.getConversations()
    .then(response => conversationsStore.set(response))
    .catch(error => console.error(error.message || 'Failed to load conversations'))

  providersStore.update(state => ({...state, is_loading: true}))
  api.getProviders()
    .then(response => providersStore.set(response))
    .catch(error => console.error(error.message || 'Failed to reload providers'))

  const unsubscribeProfile = await listen<ProfileResponse>('profile-updated', (event) => {
    profileStore.set(event.payload)
  })

  let arrived = ''
  const unsubscribeConversations = await listen<ConversationsResponse>('conversations-updated', (event) => {
    conversationsStore.set(event.payload)
    const open = get(conversationStore).data
    const current = open ? (event.payload.data ?? []).find((c) => c.id === open.id) : undefined
    const latest = current ? `${current.id}|${current.last_id ?? ''}|${current.last_activity}` : ''
    if (latest && latest !== arrived) {
      arrived = latest
      api.getMessages().then((response) => messagesStore.set(response)).catch(() => {})
    }
    loadPeople().catch(() => {})
    loadChats().catch(() => {})
  })

  const unsubscribePeople = await listen<unknown>('people-updated', () => {
    loadPeople().catch(() => {})
  })

  const unsubscribeGroups = await listen<unknown>('groups-updated', () => {
    loadGroups().catch(() => {})
  })

  const unsubscribeCases = await listen<CaseFile[]>('cases-updated', (event) => {
    casesStore.set(event.payload)
  })

  const unsubscribeConversation = await listen<ConversationResponse>('conversation-updated', (event) => {
    conversationStore.set(event.payload)
  })

  const unsubscribeMessages = await listen<MessagesResponse>('messages-updated', (event) => {
    messagesStore.set(event.payload)
  })

  const unsubscribeConversationSynced = await listen<{ conversation_id: string }>('conversation:sync:completed', (event) => {
    if (get(conversationStore).data?.id !== event.payload.conversation_id) return
    api.getMessages().then((response) => messagesStore.set(response)).catch(() => {})
  })

  const unsubscribeConversationChanged = await listen<string>('conversation-changed', (event) => {
    typingStore.update((map) => new Map([...map].filter(([, typer]) => typer.conversation_id !== event.payload)))
    if (get(conversationStore).data?.id !== event.payload) return
    api.getMessages().then((response) => messagesStore.set(response)).catch(() => {})
  })

  const unsubscribePresence = await listen<PresenceUpdate>('presence-updated', (event) => {
    const { user_id, status, status_text, status_emoji } = event.payload
    updatePresence(user_id, { status, ...(status_text != null ? { status_text } : {}), ...(status_emoji != null ? { status_emoji } : {}) })
  })

  const unsubscribeTyping = await listen<TypingUpdate>('typing-updated', (event) => {
    putTyping(event.payload.conversation_id, event.payload.user_id, event.payload.user_name)
  })

  const unsubscribeOpen = await listen<string>('conversation-open', (event) => {
    import('$app/navigation').then(({ goto }) => goto(conversationHref(event.payload))).catch(() => {})
  })

  const unsubscribePresentation = await listen<unknown>('presentation-updated', () => {
    api.getMessages().then((response) => messagesStore.set(response)).catch(() => {})
    api.getCurrentConversation().then((response) => conversationStore.set(response)).catch(() => {})
  })

  const unsubscribeAppearance = await listen<unknown>('appearance-updated', () => {
    import('$lib/appearance').then(({ loadAppearance }) => loadAppearance()).catch(() => {})
  })

  const unsubscribeProviders = await listen<ProvidersResponse>('providers-updated', (event) => {
    providersStore.set(event.payload)
  })

  const unsubscribeUsers = await listen<UsersResponse>('users-updated', (event) => {
    usersStore.set(event.payload)
  })

  const unsubscribeAuthComplete = await listen<{name: string}>('auth-complete', (event) => {
    connectionConnected(event.payload.name)
  })

  const unsubscribeAuthFailed = await listen<string>('auth-failed', (event) => {
    connectionFailed(event.payload)
  })

  const unsubscribeSorted = await listen<string>('sorted', () => {
    api.getConversations().then((conversations) => conversationsStore.set(conversations)).catch(() => {})
  })

  const unsubscribeObjectTags = await listen<string>('object-tags-updated', () => {
    loadTags().catch(() => {})
    loadObjectTags().catch(() => {})
  })

  const unsubscribeEmoji = await listen<WorkspaceEmoji[]>('workspace-emoji-updated', (event) => {
    putWorkspaceEmoji(event.payload ?? [])
  })

  loadTags().catch(() => {})
  loadObjectTags().catch(() => {})
  loadPeople().catch(() => {})
  loadGroups().catch(() => {})
  loadCases().catch(() => {})
  loadChats().catch(() => {})
  loadWorkspaceEmoji().catch(() => {})
  api.getSidebarSections().then((sections) => sidebarSectionsStore.set(sections)).catch(() => {})

  return () => {
    unsubscribeProfile()
    unsubscribeConversations()
    unsubscribeConversation()
    unsubscribeMessages()
    unsubscribeConversationSynced()
    unsubscribeConversationChanged()
    unsubscribePresence()
    unsubscribeTyping()
    unsubscribeOpen()
    unsubscribePresentation()
    unsubscribeAppearance()
    unsubscribeProviders()
    unsubscribeUsers()
    unsubscribeAuthComplete()
    unsubscribeAuthFailed()
    unsubscribeSorted()
    unsubscribeObjectTags()
    unsubscribeEmoji()
    unsubscribePeople()
    unsubscribeGroups()
    unsubscribeCases()
  }
}