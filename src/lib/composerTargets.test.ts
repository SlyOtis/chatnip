import { describe, expect, it, vi } from 'vitest'
import type { Conversation, Message, Person, Provider } from '$lib/tauri/types'
import { canSendVia, channelOf, defaultSendOption, fromOptions, getSendChoice, personFor, putSendChoice, replyActivity, sendOptions, viaOptions } from './composerTargets'

const provider = (id: string, provider_type: string, external_id: string | null = null, is_connected = true) => ({ id, provider_type, external_id, name: id, is_connected }) as unknown as Provider

const conversation = (id: string, type: Conversation['type'], providers: Provider[], last_activity = '2026-10-01T00:00:00Z') =>
  ({ id, name: id, type, providers, members: [{ id: 'u-' + id }], last_activity }) as unknown as Conversation

const outlook = provider('outlook', 'mock', 'mock-outlook-me')
const teams = provider('teams', 'mock', 'mock-teams-me')
const slack = provider('slack', 'slack')
const gmail = provider('gmail', 'google')

const mail = conversation('mail', 'email', [outlook], '2026-10-03T00:00:00Z')
const chat = conversation('chat', 'direct_message', [teams], '2026-10-05T00:00:00Z')

const anna = {
  id: 'p1',
  display_name: 'Anna',
  is_human: true,
  primary_email: 'anna@client.example',
  conversation_ids: ['mail', 'chat'],
  identities: [
    { id: 'i1', person_id: 'p1', user_id: 'su', provider_type: 'slack', external_id: 'U1', email: null, display_name: 'anna', manual: false },
    { id: 'i2', person_id: 'p1', user_id: 'tu', provider_type: 'mock', external_id: 'mock-teams-u1', email: 'anna@client.example', display_name: 'Anna', manual: false }
  ]
} as unknown as Person

describe('composer channels', () => {
  it('names the channel of a provider and conversation', () => {
    expect(channelOf(outlook, 'email')).toBe('mail')
    expect(channelOf(teams, 'direct_message')).toBe('teams')
    expect(channelOf(provider('m', 'microsoft'), 'channel')).toBe('teams')
    expect(channelOf(slack, 'direct_message')).toBe('slack')
    expect(canSendVia(gmail, 'mail')).toBe(true)
    expect(canSendVia(teams, 'mail')).toBe(false)
    expect(canSendVia(teams, 'teams')).toBe(true)
  })

  it('finds the person behind a conversation', () => {
    expect(personFor(mail, [anna])?.id).toBe('p1')
    expect(personFor(null, [anna], 'p1')?.id).toBe('p1')
    expect(personFor(conversation('x', 'email', []), [anna])).toBeNull()
  })

  it('offers the current conversation first, then the person’s other chats, then new channels', () => {
    const options = viaOptions(mail, anna, [mail, chat])
    expect(options.map((o) => o.key)).toEqual(['conversation:mail', 'conversation:chat', 'identity:i1'])
    expect(options[1].label).toBe('Teams chat')
    expect(options[2]).toMatchObject({ label: 'Slack DM', conversation_id: null, member_ids: ['su'] })
  })

  it('offers a new mail when the person has no mail conversation', () => {
    const options = viaOptions(chat, { ...anna, conversation_ids: ['chat'] }, [chat])
    expect(options.find((o) => o.channel === 'mail')).toMatchObject({ to: 'anna@client.example', conversation_id: null })
  })

  it('lists sending accounts that fit the channel', () => {
    const options = viaOptions(mail, anna, [mail, chat])
    expect(fromOptions(options[0], [outlook, teams, gmail, slack], mail).map((o) => o.key)).toEqual(['outlook'])
    expect(fromOptions(options[2], [outlook, teams, gmail, slack], mail).map((o) => o.key)).toEqual(['slack'])
    expect(fromOptions({ ...options[0], conversation_id: null }, [outlook, teams, gmail, provider('off', 'google', null, false)], mail).map((o) => o.key)).toEqual(['outlook', 'gmail'])
  })
})

describe('send as', () => {
  const message = (conversation_id: string, provider_id: string, sender_id: string, timestamp: string) => ({ id: conversation_id + timestamp, conversation_id, provider_id, sender_id, timestamp }) as unknown as Message
  const merged = conversation('merged', 'direct_message', [teams, slack], '2026-10-06T00:00:00Z')
  const vias = viaOptions(mail, anna, [mail, chat])
  const options = sendOptions(vias, [outlook, teams, gmail, slack], [mail, chat])

  it('lists every account and chat combination that can carry the message', () => {
    expect(options.map((o) => o.key)).toEqual(['conversation:mail|outlook', 'conversation:chat|teams', 'identity:i1|slack'])
    const both = sendOptions(viaOptions(merged, null, [merged]), [outlook, teams, slack], [merged])
    expect(both.map((o) => `${o.via.conversation_id}|${o.from.provider.id}`)).toEqual(['merged|teams', 'merged|slack'])
    expect(sendOptions([], [outlook], [mail])).toEqual([])
  })

  it('defaults to the chat the latest message came in on, else the current conversation', () => {
    expect(defaultSendOption(options, {}, null)?.key).toBe('conversation:mail|outlook')
    const activity = replyActivity([message('mail', 'outlook', 'anna', '2026-10-01T10:00:00Z'), message('chat', 'teams', 'anna', '2026-10-02T10:00:00Z'), message('mail', 'outlook', 'me', '2026-10-09T10:00:00Z')], new Set(['me']))
    expect(activity).toEqual({ mail: '2026-10-01T10:00:00Z', 'mail|outlook': '2026-10-01T10:00:00Z', chat: '2026-10-02T10:00:00Z', 'chat|teams': '2026-10-02T10:00:00Z' })
    expect(defaultSendOption(options, activity, null)?.key).toBe('conversation:chat|teams')
    expect(defaultSendOption([], activity, null)).toBeNull()
  })

  it('picks the account the latest message came in on inside a merged chat', () => {
    const both = sendOptions(viaOptions(merged, null, [merged]), [teams, slack], [merged])
    const activity = replyActivity([message('merged', 'teams', 'anna', '2026-10-01T10:00:00Z'), message('merged', 'slack', 'anna', '2026-10-03T10:00:00Z')], new Set(['me']))
    expect(defaultSendOption(both, activity, null)?.from.provider.id).toBe('slack')
  })

  it('keeps your choice until you interact in another chat', () => {
    const activity = { chat: '2026-10-02T10:00:00Z' }
    expect(defaultSendOption(options, activity, { key: 'identity:i1|slack', at: '2026-10-03T00:00:00Z' })?.key).toBe('identity:i1|slack')
    expect(defaultSendOption(options, { ...activity, chat: '2026-10-04T00:00:00Z' }, { key: 'identity:i1|slack', at: '2026-10-03T00:00:00Z' })?.key).toBe('conversation:chat|teams')
    expect(defaultSendOption(options, activity, { key: 'gone|x', at: '2026-10-03T00:00:00Z' })?.key).toBe('conversation:chat|teams')
  })

  it('remembers the choice across composers', () => {
    const store = new Map<string, string>()
    vi.stubGlobal('localStorage', { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => store.set(k, v), removeItem: (k: string) => store.delete(k) })
    expect(getSendChoice('mail')).toBeNull()
    putSendChoice('mail', 'identity:i1|slack', '2026-10-03T00:00:00Z')
    expect(getSendChoice('mail')).toEqual({ key: 'identity:i1|slack', at: '2026-10-03T00:00:00Z' })
    localStorage.setItem('chatnip.via.mail', 'conversation:mail')
    expect(getSendChoice('mail')).toBeNull()
  })
})
