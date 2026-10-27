import { describe, expect, it } from 'vitest'
import type { Conversation } from './types'
import { chatChoices, participants } from './chats'
import { directConversation } from '../../test/fixtures/conversations'

const chat = (id: string, name: string, at: string, patch: Partial<Conversation> = {}): Conversation => ({ ...directConversation, id, name, last_activity: at, ...patch })

describe('the chat picker search', () => {
  const me = new Set(['me'])
  const mia = { ...directConversation.members[0], id: 'mia', display_name: 'Mia Sund', email: 'mia@client.example' }
  const anna = { ...directConversation.members[0], id: 'anna', display_name: 'Anna Berg', email: 'anna@client.example' }
  const self = { ...directConversation.members[0], id: 'me', display_name: 'Milo', email: 'milo@northwind.example' }
  const conversations = [
    chat('t1', 'Mia Sund', '2026-10-14T10:00:00Z', { members: [self, mia], providers: [{ ...directConversation.providers[0], name: 'Mock Teams', provider_type: 'mock' }] }),
    chat('s1', 'design', '2026-10-15T10:00:00Z', { type: 'channel', members: [self, mia, anna], providers: [{ ...directConversation.providers[0], name: 'Mock Slack', provider_type: 'mock' }] }),
    chat('m1', 'Kontrakt Kunde AS', '2026-10-12T10:00:00Z', { type: 'email', members: [self, anna] }),
    chat('gone', 'Old', '2026-10-15T11:00:00Z', { deleted_at: '2026-10-15T11:00:00Z' }),
    chat('local', 'Receipts', '2026-10-15T12:00:00Z', { type: 'local' })
  ]

  it('lists live chats newest first, without chats made in Chatnip', () => {
    expect(chatChoices(conversations, '', new Set(), me).map((c) => c.id)).toEqual(['s1', 't1', 'm1'])
    expect(chatChoices(conversations, '', new Set(['s1']), me).map((c) => c.id)).toEqual(['t1', 'm1'])
  })

  it('searches names, participants, addresses and connections across providers', () => {
    expect(chatChoices(conversations, 'mia', new Set(), me).map((c) => c.id)).toEqual(['s1', 't1'])
    expect(chatChoices(conversations, 'anna client', new Set(), me).map((c) => c.id)).toEqual(['s1', 'm1'])
    expect(chatChoices(conversations, 'slack', new Set(), me).map((c) => c.id)).toEqual(['s1'])
    expect(chatChoices(conversations, 'milo', new Set(), me).map((c) => c.id)).toEqual([])
    expect(participants(conversations[1], me)).toBe('Mia Sund, Anna Berg')
  })
})
