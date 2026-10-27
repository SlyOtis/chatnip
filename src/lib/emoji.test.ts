import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
  convertFileSrc: vi.fn((path: string) => `asset://localhost/${path}`)
}))

import {
  canonicalEmoji,
  getEmojiGroups,
  getShortcode,
  getUnicode,
  groupWorkspaceEmoji,
  putWorkspaceEmoji,
  renderEmojiHtml,
  renderEmojiText,
  searchEmoji,
  shortcodeQuery,
  STANDARD_EMOJI,
  withSkinTone
} from './emoji'
import type { WorkspaceEmoji } from '$lib/tauri/types'

function custom(name: string, provider = 'slack-a', extra: Partial<WorkspaceEmoji> = {}): WorkspaceEmoji {
  return { provider_id: provider, provider_name: provider === 'slack-a' ? 'Northwind' : 'Sundial', provider_type: 'slack', name, alias_for: null, url: `https://emoji.slack-edge.com/${name}.gif`, file_path: null, mime_type: 'image/gif', updated_at: '2026-10-15T10:00:00Z', ...extra }
}

beforeEach(() => putWorkspaceEmoji([]))

describe('standard shortcodes', () => {
  it('resolves iamcal and github names to one emoji and back to the primary name', () => {
    expect(getUnicode(':+1:')).toBe('👍')
    expect(getUnicode('thumbsup')).toBe('👍')
    expect(getShortcode('👍')).toBe(':+1:')
    expect(getShortcode('❤️')).toBe(':heart:')
    expect(getShortcode('❤')).toBe(':heart:')
    expect(canonicalEmoji('🎉')).toBe(':tada:')
    expect(canonicalEmoji(':tada:')).toBe(':tada:')
    expect(canonicalEmoji('nope')).toBe('nope')
  })

  it('carries skin tones as slack suffixes in both directions', () => {
    expect(getUnicode(':+1::skin-tone-3:')).toBe('👍🏼')
    expect(getShortcode('👍🏿')).toBe(':+1::skin-tone-6:')
    expect(renderEmojiText('ok :wave::skin-tone-2:')).toBe('ok 👋🏻')
    const wave = STANDARD_EMOJI.find((e) => e.name === 'wave')!
    expect(withSkinTone(wave, 2)).toEqual({ unicode: '👋🏼', code: ':wave::skin-tone-3:' })
    expect(withSkinTone(wave, 0)).toEqual({ unicode: '👋', code: ':wave:' })
  })

  it('groups the picker by the emojibase groups without components', () => {
    const groups = getEmojiGroups()
    expect(groups[0].key).toBe('smileys-emotion')
    expect(groups.some((g) => g.key === 'component')).toBe(false)
    expect(groups.find((g) => g.key === 'flags')!.emojis.length).toBeGreaterThan(200)
  })
})

describe('rendering', () => {
  it('renders known shortcodes in text and leaves unknown ones and times alone', () => {
    expect(renderEmojiText('Shipped :rocket: :tada: :unknown: at 10:30:00')).toBe('Shipped 🚀 🎉 :unknown: at 10:30:00')
  })

  it('renders html text nodes but not tags, attributes or code', () => {
    const html = '<p title=":tada:">Hei :tada:</p><code>:tada:</code><pre>x :rocket:</pre><p>:rocket:</p>'
    expect(renderEmojiHtml(html)).toBe('<p title=":tada:">Hei 🎉</p><code>:tada:</code><pre>x :rocket:</pre><p>🚀</p>')
  })

  it('renders custom emoji from the message workspace as images and others as text', () => {
    putWorkspaceEmoji([custom('party_parrot'), custom('shipit', 'slack-b')])
    const html = renderEmojiHtml('<p>:party_parrot: :shipit:</p>', 'slack-a')
    expect(html).toContain('<img class="emoji-custom" src="https://emoji.slack-edge.com/party_parrot.gif" alt=":party_parrot:"')
    expect(html).toContain(' :shipit:</p>')
    expect(renderEmojiHtml('<p>:shipit:</p>')).toContain('alt=":shipit:"')
  })

  it('prefers the cached file and follows aliases', () => {
    putWorkspaceEmoji([custom('party_parrot', 'slack-a', { file_path: '/data/assets/emoji/a/party_parrot.gif' }), custom('partyparrot', 'slack-a', { alias_for: 'alias:party_parrot', url: null }), custom('yes', 'slack-a', { alias_for: 'alias:+1', url: null })])
    expect(renderEmojiHtml(':partyparrot:', 'slack-a')).toContain('src="asset://localhost//data/assets/emoji/a/party_parrot.gif"')
    expect(renderEmojiHtml(':yes:', 'slack-a')).toBe('👍')
  })
})

describe('autocomplete', () => {
  it('opens after a colon and two characters', () => {
    expect(shortcodeQuery('hei :ta')).toEqual({ query: 'ta', start: 4 })
    expect(shortcodeQuery('hei :t')).toBeNull()
    expect(shortcodeQuery('klokka 10:30')).toBeNull()
  })

  it('ranks exact and prefix matches before substring matches, standard and custom', () => {
    putWorkspaceEmoji([custom('tadaa_parrot'), custom('ultratada')])
    const names = searchEmoji('tada', { limit: 20 }).map((m) => m.name)
    expect(names[0]).toBe('tada')
    expect(names.indexOf('tadaa_parrot')).toBeLessThan(names.indexOf('ultratada'))
    const parrot = searchEmoji('tadaa', { limit: 5 }).find((m) => m.name === 'tadaa_parrot')!
    expect(parrot.custom?.provider_id).toBe('slack-a')
    expect(searchEmoji('thu').map((m) => m.name)).toContain('+1')
  })

  it('groups custom emoji per workspace', () => {
    const groups = groupWorkspaceEmoji([custom('b'), custom('a'), custom('c', 'slack-b')])
    expect(groups.map((g) => [g.provider_name, g.emojis.map((e) => e.name)])).toEqual([['Northwind', ['a', 'b']], ['Sundial', ['c']]])
  })
})
