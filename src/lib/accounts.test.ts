import { describe, expect, it } from 'vitest'
import type { Conversation, Provider } from '$lib/tauri/types'
import { accountAvatar, accountHealth, accountName, lastUsedAccount, ownAuthor } from './accounts'
import { fromOptions, type ViaOption } from './composerTargets'

function provider(id: string, name: string, account: Provider['account'], extra: Partial<Provider> = {}): Provider {
  return { id, name, provider_type: 'microsoft', external_id: null, is_connected: true, missing_scopes: [], account, last_used_at: null, ...extra } as unknown as Provider
}

const work = provider('p-work', 'Northwind', { user_id: 'u-work', display_name: 'Milo Oakes', email: 'milo@northwind.example', avatar_url: 'https://graph/me.png', avatar_data: null })
const chat = provider('p-chat', 'Acme Slack', { user_id: 'u-chat', display_name: 'milo.o', email: 'milo@acme.io', avatar_url: null, avatar_data: null }, { provider_type: 'slack' })
const bare = provider('p-mail', 'milo@mail.no', null, { provider_type: 'imap' })

describe('own message authors', () => {
  it('names and pictures my message by the account it went through', () => {
    const author = ownAuthor({ provider_id: 'p-work', sender_id: 'u-work' }, [work, chat], 'You')
    expect(author.name).toBe('Milo Oakes')
    expect(author.face.src).toBe('https://graph/me.png')
    expect(author.provider?.id).toBe('p-work')
    expect(ownAuthor({ provider_id: 'p-chat', sender_id: 'u-chat' }, [work, chat], 'You').name).toBe('milo.o')
  })

  it('falls back to the connection name and then to the pronoun', () => {
    expect(ownAuthor({ provider_id: 'p-mail', sender_id: 'x' }, [bare], 'You').name).toBe('milo@mail.no')
    const unknown = ownAuthor({ provider_id: 'gone', sender_id: 'x' }, [work], 'You')
    expect(unknown.name).toBe('You')
    expect(unknown.provider).toBeNull()
    expect(accountName(provider('p', 'Tenant', { user_id: 'u', display_name: ' ', email: 'a@b.no', avatar_url: null, avatar_data: null }))).toBe('a@b.no')
    expect(accountAvatar(bare).id).toBe('p-mail')
  })

  it('labels each send as choice with the account that speaks', () => {
    const via = { key: 'conversation:c', channel: 'teams', conversation_id: 'c', conversation_type: 'group_message' } as unknown as ViaOption
    const conversation = { id: 'c', providers: [{ ...work, account: null }, bare] } as unknown as Conversation
    const labels = fromOptions(via, [work, chat, bare], conversation).map((option) => option.label)
    expect(labels).toEqual(['Milo Oakes · Northwind', 'milo@mail.no'])
  })
})

describe('the last used account', () => {
  it('is the connected account acted through most recently', () => {
    const older = { ...work, last_used_at: '2026-10-14T08:00:00Z' }
    const newer = { ...chat, last_used_at: '2026-10-15T08:00:00Z' }
    expect(lastUsedAccount([older, newer])?.id).toBe('p-chat')
    expect(lastUsedAccount([older, { ...newer, is_connected: false }])?.id).toBe('p-work')
    expect(lastUsedAccount([work, chat])?.id).toBe('p-work')
    expect(lastUsedAccount([])).toBeNull()
  })

  it('reports health from the connection and its consent', () => {
    expect(accountHealth(work)).toBe('ok')
    expect(accountHealth({ ...work, missing_scopes: ['Presence.ReadWrite'] })).toBe('reconsent')
    expect(accountHealth({ ...work, is_connected: false })).toBe('disconnected')
  })
})
