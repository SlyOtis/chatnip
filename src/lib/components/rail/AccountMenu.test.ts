import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte'
import { get } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'
import { goto } from '$app/navigation'
import AccountMenu from './AccountMenu.svelte'
import { providersStore, putAccountUsed } from '$lib/tauri/providers'
import { modalsClose, modalsStore } from '$lib/modals'
import type { Provider } from '$lib/tauri/types'
import { t } from '$lib/i18n.svelte'

vi.mock('$app/navigation', () => ({ goto: vi.fn() }))

function provider(id: string, name: string, display: string, extra: Partial<Provider> = {}): Provider {
  return {
    id,
    name,
    provider_type: 'microsoft',
    external_id: null,
    is_connected: true,
    missing_scopes: [],
    capabilities: { supports_presence: true },
    account: { user_id: `u-${id}`, display_name: display, email: null, avatar_url: null, avatar_data: null },
    last_used_at: null,
    ...extra
  } as unknown as Provider
}

const work = provider('work', 'Northwind', 'Milo Oakes', { last_used_at: '2026-10-14T08:00:00Z' })
const slack = provider('slack', 'Acme', 'milo.o', { provider_type: 'slack', last_used_at: '2026-10-15T08:00:00Z' })
const mail = provider('mail', 'milo@mail.no', 'Milo', { provider_type: 'imap', capabilities: { supports_presence: false } as Provider['capabilities'], missing_scopes: ['Mail.Send'] })

beforeEach(() => {
  providersStore.set({ is_loading: false, data: [work, slack, mail] })
  vi.mocked(invoke).mockImplementation((cmd: string, args?: unknown) => {
    if (cmd === 'get_user_status') {
      const id = (args as { input: { user_id: string } }).input.user_id
      return Promise.resolve({ user_id: id, status: 'offline', status_text: id === 'u-slack' ? 'Heads down' : '', status_emoji: '', status_expiration: null, updated_at: '' })
    }
    return Promise.reject(new Error(cmd))
  })
})

afterEach(() => {
  cleanup()
  modalsClose()
  vi.mocked(invoke).mockReset()
  vi.mocked(goto).mockReset()
})

describe('the sidebar account', () => {
  it('shows the account used last with its own name', () => {
    render(AccountMenu)
    expect(screen.getByTestId('rail-account-name')).toHaveTextContent('milo.o')
  })

  it('follows the account acted through as it changes', async () => {
    render(AccountMenu)
    putAccountUsed('work', '2026-10-16T08:00:00Z')
    await waitFor(() => expect(screen.getByTestId('rail-account-name')).toHaveTextContent('Milo Oakes'))
  })

  it('lists every account with its health and status, last used first', async () => {
    render(AccountMenu)
    await fireEvent.click(screen.getByTestId('rail-account'))
    const menu = screen.getByTestId('rail-account-menu')
    const items = within(menu).getAllByTestId('rail-account-item')
    expect(items.map((item) => item.dataset.provider)).toEqual(['slack', 'work', 'mail'])
    expect(items[0]).toHaveTextContent(t('app.rail.lastUsed'))
    expect(items[2].dataset.health).toBe('reconsent')
    expect(items[2]).toHaveTextContent(t('settings.connections.reconsent'))
    await waitFor(() => expect(items[0]).toHaveTextContent('Heads down'))
    expect(within(items[2]).queryByTestId('rail-account-status')).toBeNull()
  })

  it('sets status on one account', async () => {
    render(AccountMenu)
    await fireEvent.click(screen.getByTestId('rail-account'))
    const items = within(screen.getByTestId('rail-account-menu')).getAllByTestId('rail-account-item')
    await fireEvent.click(within(items[1]).getByTestId('rail-account-status'))
    expect(get(modalsStore).setStatus?.provider_id).toBe('work')
  })
})
