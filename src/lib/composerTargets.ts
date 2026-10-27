import { t } from '$lib/i18n.svelte'
import type { Conversation, ConversationType, Message, Person, Provider } from '$lib/tauri/types'
import { viaStorageKey } from '$lib/tauri/chats'
import { accountName } from '$lib/accounts'
import { mockKind } from '$lib/providerIcons'
import { personEmail } from '$lib/tauri/people'

export type Channel = 'mail' | 'teams' | 'slack' | 'google_chat' | 'chat'

export interface ViaOption {
  key: string
  label: string
  detail: string
  channel: Channel
  conversation_type: ConversationType
  conversation_id: string | null
  provider_type: string
  external_id: string | null
  to: string | null
  member_ids: string[]
}

export interface FromOption {
  key: string
  provider: Provider
  label: string
}

const MAIL_KINDS = ['outlook', 'gmail', 'imap']

export function channelOf(source: { provider_type: string; external_id?: string | null }, type: ConversationType | string): Channel {
  if (type === 'email') return 'mail'
  const kind = mockKind(source)
  if (kind) return kind === 'teams' ? 'teams' : kind === 'slack' ? 'slack' : kind === 'google_chat' ? 'google_chat' : MAIL_KINDS.includes(kind) ? 'mail' : 'chat'
  if (source.provider_type === 'microsoft') return 'teams'
  if (source.provider_type === 'slack') return 'slack'
  if (source.provider_type === 'google_chat') return 'google_chat'
  if (source.provider_type === 'google') return 'mail'
  if (source.provider_type === 'imap') return 'mail'
  return 'chat'
}

export function channelLabel(channel: Channel, type: ConversationType | string): string {
  const group = type === 'group_message' || type === 'channel'
  switch (channel) {
    case 'mail':
      return t('composer.channel.mail')
    case 'teams':
      return t(group ? 'composer.channel.teamsGroup' : 'composer.channel.teamsChat')
    case 'slack':
      return t(group ? 'composer.channel.slackChannel' : 'composer.channel.slackDm')
    case 'google_chat':
      return 'Google Chat'
    default:
      return t('composer.channel.chat')
  }
}

export function canSendVia(provider: Provider, channel: Channel): boolean {
  const kind = mockKind(provider)
  if (channel === 'mail') return kind ? MAIL_KINDS.includes(kind) : ['microsoft', 'google', 'imap'].includes(provider.provider_type)
  return channelOf(provider, 'direct_message') === channel
}

export function personFor(conversation: Conversation | null, people: Person[], personId: string | null = null): Person | null {
  if (personId) return people.find((p) => p.id === personId) ?? null
  if (!conversation) return null
  return people.find((p) => p.is_human && p.conversation_ids.includes(conversation.id)) ?? null
}

export function viaOptions(current: Conversation | null, person: Person | null, conversations: Conversation[]): ViaOption[] {
  const options: ViaOption[] = []
  const seen = new Set<string>()
  const add = (conversation: Conversation) => {
    if (seen.has(conversation.id)) return
    seen.add(conversation.id)
    const provider = conversation.providers[0]
    const channel = provider ? channelOf(provider, conversation.type) : conversation.type === 'email' ? 'mail' : 'chat'
    options.push({
      key: 'conversation:' + conversation.id,
      label: channelLabel(channel, conversation.type),
      detail: conversation.type === 'direct_message' || conversation.type === 'email' ? (provider?.name ?? '') : conversation.name,
      channel,
      conversation_type: conversation.type,
      conversation_id: conversation.id,
      provider_type: provider?.provider_type ?? '',
      external_id: provider?.external_id ?? null,
      to: null,
      member_ids: conversation.members.map((m) => m.id)
    })
  }
  if (current) add(current)
  if (person) {
    const own = conversations.filter((c) => person.conversation_ids.includes(c.id)).sort((a, b) => b.last_activity.localeCompare(a.last_activity))
    for (const conversation of own) add(conversation)
    const hasMail = options.some((o) => o.channel === 'mail')
    const mail = personEmail(person)
    if (!hasMail && mail) {
      options.push({ key: 'mail:' + mail, label: t('composer.channel.mail'), detail: t('composer.via.newMail', { address: mail }), channel: 'mail', conversation_type: 'email', conversation_id: null, provider_type: '', external_id: null, to: mail, member_ids: [] })
    }
    for (const identity of person.identities) {
      if (!identity.user_id) continue
      const channel = channelOf(identity, 'direct_message')
      if (channel === 'mail' || channel === 'chat' || options.some((o) => o.channel === channel && o.conversation_type === 'direct_message')) continue
      options.push({
        key: 'identity:' + identity.id,
        label: channelLabel(channel, 'direct_message'),
        detail: t('composer.via.newChat', { name: identity.display_name ?? person.display_name }),
        channel,
        conversation_type: 'direct_message',
        conversation_id: null,
        provider_type: identity.provider_type,
        external_id: identity.external_id,
        to: null,
        member_ids: [identity.user_id]
      })
    }
  }
  return options
}

export function fromOptions(via: ViaOption | null, providers: Provider[], conversation: Conversation | null): FromOption[] {
  if (!via) return []
  const existing = via.conversation_id && conversation?.id === via.conversation_id ? conversation.providers : []
  const candidates = existing.length ? existing : providers.filter((provider) => provider.is_connected && canSendVia(provider, via.channel))
  return candidates.map((provider) => {
    const full = providers.find((p) => p.id === provider.id)
    const name = full?.account ? accountName(full) : provider.name
    return { key: provider.id, provider, label: name === provider.name ? name : `${name} · ${provider.name}` }
  })
}

export interface SendOption {
  key: string
  via: ViaOption
  from: FromOption
}

export interface SendChoice {
  key: string
  at: string
}

export function sendOptions(vias: ViaOption[], providers: Provider[], conversations: Conversation[]): SendOption[] {
  return vias.flatMap((via) => {
    const conversation = via.conversation_id ? (conversations.find((c) => c.id === via.conversation_id) ?? null) : null
    return fromOptions(via, providers, conversation).map((from) => ({ key: `${via.key}|${from.key}`, via, from }))
  })
}

function later(map: Record<string, string>, key: string, at: string) {
  if (!map[key] || map[key] < at) map[key] = at
}

export function replyActivity(messages: Message[], mine: Set<string>): Record<string, string> {
  const out: Record<string, string> = {}
  for (const message of messages) {
    if (!message.conversation_id || mine.has(message.sender_id)) continue
    later(out, message.conversation_id, message.timestamp)
    if (message.provider_id) later(out, `${message.conversation_id}|${message.provider_id}`, message.timestamp)
  }
  return out
}

function activityOf(option: SendOption, activity: Record<string, string>): string {
  const id = option.via.conversation_id
  if (!id) return ''
  return activity[`${id}|${option.from.provider.id}`] ?? activity[id] ?? ''
}

export function defaultSendOption(options: SendOption[], activity: Record<string, string>, choice: SendChoice | null): SendOption | null {
  if (!options.length) return null
  const kept = choice ? options.find((o) => o.key === choice.key) : undefined
  if (kept && choice) {
    const elsewhere = options.filter((o) => o.via.conversation_id !== kept.via.conversation_id).reduce((at, o) => (activityOf(o, activity) > at ? activityOf(o, activity) : at), '')
    if (choice.at >= elsewhere) return kept
  }
  return options.reduce((best, o) => (activityOf(o, activity) > activityOf(best, activity) ? o : best), options[0])
}

export function getSendChoice(conversationId: string): SendChoice | null {
  try {
    const value = JSON.parse(localStorage.getItem(viaStorageKey(conversationId)) ?? 'null')
    return value && typeof value.key === 'string' && typeof value.at === 'string' ? value : null
  } catch {
    return null
  }
}

export function putSendChoice(conversationId: string, key: string, at = new Date().toISOString()): SendChoice {
  const choice = { key, at }
  localStorage.setItem(viaStorageKey(conversationId), JSON.stringify(choice))
  return choice
}


