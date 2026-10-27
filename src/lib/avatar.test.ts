import { describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import { AVATAR_FACES, AVATAR_HUES, avatarColor, avatarHue, avatarInitials, avatarSeed, chatAvatar, imageSrc, personAvatar, userAvatar } from './avatar'
import type { Conversation, Person, User } from './tauri/types'
import { channelConversation, directConversation, groupConversation } from '../test/fixtures/conversations'

function person(id: string, name: string, user_id: string, extra: Partial<Person> = {}): Person {
  return {
    id,
    profile_id: 'profile',
    display_name: name,
    provider_name: name,
    primary_email: null,
    avatar_url: null,
    avatar_data: null,
    overrides: {},
    nicknames: {},
    is_human: true,
    favourite_position: null,
    identities: [{ id: `${id}-i`, person_id: id, user_id, provider_type: 'microsoft', external_id: user_id, email: null, display_name: name, manual: false }],
    conversation_ids: [],
    provider_types: ['microsoft'],
    last_activity: null,
    unread_count: 0,
    created_at: '',
    updated_at: '',
    ...extra
  }
}

function user(id: string, display_name: string): User {
  return { id, email: null, phone: null, display_name, first_name: null, last_name: null, avatar_url: null, avatar_data: null, job_title: null, department: null, company: null, office_location: null, first_seen: '', last_updated: '' }
}

function chat(extra: Partial<Conversation>): Conversation {
  return { ...groupConversation, image: null, overrides: {}, ...extra }
}

describe('generated avatars', () => {
  it('give the same seed, hue and colour for the same id every time', () => {
    expect(avatarSeed('person-42')).toBe(avatarSeed('person-42'))
    expect(avatarHue('person-42')).toBe(avatarHue('person-42'))
    expect(avatarColor({ id: 'person-42', color: null })).toBe(avatarColor({ id: 'person-42', color: null }))
  })

  it('spread different ids over the palette', () => {
    const hues = new Set(Array.from({ length: 64 }, (_, i) => avatarHue(`person-${i}`)))
    expect(hues.size).toBeGreaterThan(6)
  })

  it('never land on purple or blue violet', () => {
    for (let i = 0; i < 500; i++) {
      const hue = avatarHue(`seed-${i}`)
      expect(AVATAR_HUES).toContain(hue)
      expect(hue < 245 || hue > 340).toBe(true)
    }
  })

  it('take a chosen colour over the generated one', () => {
    expect(avatarColor({ id: 'p', color: '#e07b39' })).toBe('#e07b39')
    expect(avatarColor({ id: 'p', color: '  ' })).toMatch(/^oklch\(/)
  })

  it('reduce names to two initials', () => {
    expect(avatarInitials('Eva Ortega')).toBe('EO')
    expect(avatarInitials('kari.nordmann@tidewater.example')).toBe('KN')
    expect(avatarInitials('Øyvind')).toBe('Ø')
  })
})

describe('image sources', () => {
  it('keep urls and data urls and wrap bare base64', () => {
    expect(imageSrc('data:image/png;base64,AAA')).toBe('data:image/png;base64,AAA')
    expect(imageSrc('AAA')).toBe('data:image/png;base64,AAA')
    expect(imageSrc(null, 'https://x/a.png')).toBe('https://x/a.png')
    expect(imageSrc(null, null)).toBeNull()
  })
})

describe('person and chat avatars', () => {
  const alice = person('p-alice', 'Alice', 'user-001', { avatar_data: 'data:image/svg+xml;base64,AA' })
  const bob = person('p-bob', 'Bob', 'user-002', { overrides: { color: '#1f9d55' } })

  it('use the synced photo, else a monogram with the local colour', () => {
    expect(personAvatar(alice)).toMatchObject({ shape: 'person', src: 'data:image/svg+xml;base64,AA' })
    expect(personAvatar(bob)).toMatchObject({ shape: 'person', src: null, color: '#1f9d55', name: 'Bob' })
  })

  it('show the other person on a direct message', () => {
    const avatar = chatAvatar(directConversation, [alice, bob], new Set(['user-002']))
    expect(avatar).toMatchObject({ shape: 'person', id: 'p-alice', src: 'data:image/svg+xml;base64,AA' })
  })

  it('prefer the chat picture, then the members, then a generated chat avatar', () => {
    const me = new Set(['me'])
    const members = [user('me', 'Me'), user('user-001', 'Alice'), user('user-002', 'Bob'), user('u3', 'Cleo'), user('u4', 'Dag'), user('u5', 'Eva')]
    expect(chatAvatar(chat({ image: 'https://x/team.png', members }), [alice], me)).toMatchObject({ shape: 'chat', src: 'https://x/team.png', faces: [] })
    const composed = chatAvatar(chat({ members }), [alice, bob], me)
    expect(composed.src).toBeNull()
    expect(composed.faces).toHaveLength(AVATAR_FACES)
    expect(composed.faces.map((f) => f.name)).toEqual(['Alice', 'Bob', 'Cleo', 'Dag'])
    expect(composed.faces[0].src).toBe('data:image/svg+xml;base64,AA')
    const empty = chatAvatar({ ...channelConversation, image: null, overrides: {}, members: [] }, [], me)
    expect(empty).toMatchObject({ shape: 'chat', src: null, faces: [], icon: 'lucide:hash', id: channelConversation.id })
  })

  it('name a member by the given name and fall back to the person', () => {
    expect(userAvatar(user('user-001', 'Alice Johnson'), [alice], 'Ali').name).toBe('Ali')
    expect(userAvatar(user('nobody', 'Nobody'), [alice]).shape).toBe('person')
  })
})
