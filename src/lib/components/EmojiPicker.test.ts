import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { render, screen, fireEvent, cleanup, waitFor } from '@testing-library/svelte'
import { invoke } from '@tauri-apps/api/core'
import EmojiPicker from './EmojiPicker.svelte'
import { putWorkspaceEmoji } from '$lib/emoji'
import type { WorkspaceEmoji } from '$lib/tauri/types'

vi.mock('$app/navigation', () => ({ goto: vi.fn() }))

function emoji(provider_id: string, provider_name: string, name: string): WorkspaceEmoji {
  return { provider_id, provider_name, provider_type: 'slack', name, alias_for: null, url: `https://emoji.slack-edge.com/${provider_id}/${name}.png`, file_path: null, mime_type: 'image/png', updated_at: '2026-10-15T10:00:00Z' }
}

const WORKSPACES = [emoji('slack-a', 'Northwind', 'party_parrot'), emoji('slack-a', 'Northwind', 'shipit'), emoji('slack-b', 'Sundial', 'sundial_logo')]

beforeEach(() => {
  const store = new Map<string, string>()
  vi.stubGlobal('localStorage', { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v), removeItem: (k: string) => void store.delete(k), clear: () => store.clear() })
  putWorkspaceEmoji([])
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === 'get_workspace_emoji') return Promise.resolve(WORKSPACES)
    if (cmd === 'get_reaction_uses' || cmd === 'get_content') return Promise.resolve([])
    return Promise.reject(new Error(cmd))
  })
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.mocked(invoke).mockReset()
})

function category(key: string): HTMLElement | null {
  return document.querySelector(`[data-category="${key}"]`)
}

describe('EmojiPicker', () => {
  it('groups custom emoji per connected workspace next to the standard groups', async () => {
    putWorkspaceEmoji(WORKSPACES)
    render(EmojiPicker, { onselect: () => {}, onclose: () => {} })
    await waitFor(() => expect(category('workspace-slack-a')).not.toBeNull())
    expect(category('smileys-emotion')).not.toBeNull()
    expect(category('component')).toBeNull()
    const northwind = category('workspace-slack-a')!
    expect(northwind.textContent).toContain('Northwind')
    expect([...northwind.querySelectorAll('[data-emoji]')].map((b) => b.getAttribute('data-emoji'))).toEqual([':party_parrot:', ':shipit:'])
    expect(category('workspace-slack-b')!.querySelector('img')?.getAttribute('src')).toBe('https://emoji.slack-edge.com/slack-b/sundial_logo.png')
  })

  it('shows only the message workspace when reacting', async () => {
    putWorkspaceEmoji(WORKSPACES)
    render(EmojiPicker, { onselect: () => {}, onclose: () => {}, providerId: 'slack-b' })
    await waitFor(() => expect(category('workspace-slack-b')).not.toBeNull())
    expect(category('workspace-slack-a')).toBeNull()
  })

  it('emits canonical shortcodes, with the stored skin tone, from search and groups', async () => {
    localStorage.setItem('chatnip_skin_tone', '3')
    putWorkspaceEmoji(WORKSPACES)
    const picked: string[] = []
    render(EmojiPicker, { onselect: (code: string) => picked.push(code), onclose: () => {} })
    expect(screen.getByRole('button', { name: 'Skin tone' }).textContent).toBe('👋🏽')
    await waitFor(() => expect(category('people-body')).not.toBeNull())
    const wave = [...category('people-body')!.querySelectorAll('button')].find((b) => b.getAttribute('title') === ':wave:')!
    await fireEvent.click(wave)
    expect(picked).toEqual([':wave::skin-tone-4:'])
    await fireEvent.click(category('workspace-slack-a')!.querySelector('[data-emoji=":shipit:"]')!)
    expect(picked.at(-1)).toBe(':shipit:')
    const field = screen.getByPlaceholderText(/emoji/i)
    await fireEvent.input(field, { target: { value: 'parrot' } })
    await waitFor(() => expect(document.querySelector('[data-emoji=":party_parrot:"]')).not.toBeNull())
    expect(document.querySelector('[data-emoji=":parrot:"]')).not.toBeNull()
  }, 20000)
})
