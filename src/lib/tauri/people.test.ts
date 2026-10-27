import { describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import { displayName, identityInput, identityLabel, mergeCandidates, personHref, personProfileHref, personTarget } from './people'
import type { Chat, Person, PersonIdentity } from './types'

function identity(overrides: Partial<PersonIdentity>): PersonIdentity {
  return { id: 'i', person_id: 'p', user_id: 'u', provider_type: 'microsoft', external_id: 'ext', email: null, display_name: null, manual: false, ...overrides }
}

function person(id: string, name: string, identities: Partial<PersonIdentity>[] = [], email: string | null = null): Person {
  return {
    id,
    profile_id: 'profile',
    display_name: name,
    provider_name: name,
    primary_email: email,
    avatar_url: null,
    avatar_data: null,
    overrides: {},
    nicknames: {},
    is_human: true,
    favourite_position: null,
    identities: identities.map((i) => identity({ person_id: id, ...i })),
    conversation_ids: [],
    provider_types: [],
    last_activity: null,
    unread_count: 0,
    created_at: '',
    updated_at: ''
  }
}

describe('identityLabel', () => {
  it('shows the address first and the name when it adds something', () => {
    expect(identityLabel(identity({ email: 'kari@tidewater.example', display_name: 'Kari Nordmann' }))).toEqual({ primary: 'kari@tidewater.example', secondary: 'Kari Nordmann', source: 'Microsoft 365' })
    expect(identityLabel(identity({ display_name: 'kari.n' }))).toEqual({ primary: 'kari.n', secondary: null, source: 'Microsoft 365' })
  })

  it('marks an alias that has not been seen yet', () => {
    expect(identityLabel(identity({ user_id: null, external_id: 'U9', provider_type: 'slack' }))).toEqual({ primary: 'U9', secondary: 'Waiting for first message', source: 'Slack' })
  })

  it('names the test client an identity came from', () => {
    expect(identityLabel(identity({ provider_type: 'mock', external_id: 'mock-gmail-u5', email: 'a@b.no' })).source).toBe('Mock Gmail')
    expect(identityLabel(identity({ provider_type: 'mock', external_id: 'mock-google_chat-u5' })).source).toBe('Mock Google Chat')
  })
})

describe('identityInput', () => {
  it('sends an address as a lowercased email', () => {
    expect(identityInput('p', 'email', '  Kari@Tidewater.example ', 'Kari privat')).toEqual({ person_id: 'p', provider_type: 'email', email: 'kari@tidewater.example', external_id: null, display_name: 'Kari privat' })
  })

  it('sends a handle as an external id without the at sign', () => {
    expect(identityInput('p', 'slack', '@kari.n')).toEqual({ person_id: 'p', provider_type: 'slack', email: null, external_id: 'kari.n', display_name: null })
  })

  it('refuses an empty value', () => {
    expect(identityInput('p', 'slack', '   ')).toBeNull()
  })
})

describe('mergeCandidates', () => {
  const people = [
    person('a', 'Kari Nordmann', [{ email: 'kari@tidewater.example' }]),
    person('b', 'kn', [{ provider_type: 'slack', display_name: 'kari.n' }]),
    person('c', 'Anna Berg', [], 'anna@client.example')
  ]

  it('matches names, addresses and aliases and skips the open person', () => {
    expect(mergeCandidates(people, 'a', 'kari').map((p) => p.id)).toEqual(['b'])
    expect(mergeCandidates(people, 'b', 'CLIENT').map((p) => p.id)).toEqual(['c'])
  })

  it('returns nothing for an empty query and respects the limit', () => {
    expect(mergeCandidates(people, 'x', ' ')).toEqual([])
    expect(mergeCandidates(people, 'x', 'a', 2)).toHaveLength(2)
  })
})

describe('person routing', () => {
  const chat = (id: string, sources: Chat['sources'], mode: Chat['mode'] = 'write'): Chat => ({ id, profile_id: 'profile', conversation_id: `conv-${id}`, name: id, icon: null, mode, sources, attachments: [], created_at: '' })

  it('sends the image to the profile page and the name to the chat', () => {
    expect(personProfileHref('p 1')).toBe('/main/person/p%201/profile')
    expect(personHref('p 1')).toBe('/main/person/p%201')
  })

  it('opens the merged direct messages when the person has any', () => {
    const ada = { ...person('ada', 'Ada'), conversation_ids: ['dm-1'] }
    expect(personTarget(ada, [chat('notes', [{ kind: 'person', id: 'ada' }])])).toEqual({ kind: 'thread', path: '/main/person/ada' })
  })

  it('opens the local write chat for the person when there are no direct messages', () => {
    const ada = person('ada', 'Ada')
    const chats = [chat('read', [{ kind: 'person', id: 'ada' }], 'read'), chat('other', [{ kind: 'person', id: 'bob' }]), chat('mine', [{ kind: 'tag', id: 't' }, { kind: 'person', id: 'ada' }])]
    expect(personTarget(ada, chats)).toEqual({ kind: 'chat', path: '/main/conversation/conv-mine' })
  })

  it('asks for a new chat when nothing exists yet', () => {
    expect(personTarget(person('ada', 'Ada'), [])).toEqual({ kind: 'create' })
  })
})

describe('displayName', () => {
  const ada = { ...person('ada', 'Ada Lovelace', [{ user_id: 'u-ada' }]), overrides: { display_name: 'Ada' } }

  it('prefers the chat nickname, then the local name, then the provider name', () => {
    expect(displayName([ada], { nicknames: { 'u-ada': 'Countess' } }, 'u-ada', 'Ada King')).toBe('Countess')
    expect(displayName([ada], { nicknames: {} }, 'u-ada', 'Ada King')).toBe('Ada')
    expect(displayName([], { nicknames: {} }, 'u-ada', 'Ada King')).toBe('Ada King')
    expect(displayName([ada], null, null, 'Ada King')).toBe('Ada King')
  })
})
