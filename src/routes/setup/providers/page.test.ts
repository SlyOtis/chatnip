import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import { invoke } from '@tauri-apps/api/core'
import { goto } from '$app/navigation'
import Page from './+page.svelte'
import { providersStore } from '$lib/tauri/providers'
import type { Provider } from '$lib/tauri/types'
import { t } from '$lib/i18n.svelte'

vi.mock('$app/navigation', () => ({ goto: vi.fn() }))

const teams = { id: 'p-teams', provider_type: 'mock', external_id: 'mock-teams-me', name: 'Teams', is_connected: true, missing_scopes: [], account: { user_id: 'u-me', display_name: 'Milo Oakes', email: 'milo@northwind.example', avatar_url: null, avatar_data: null }, last_used_at: null } as unknown as Provider

beforeEach(() => {
  providersStore.set({ is_loading: false, data: [] })
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === 'get_mock_mode') return Promise.resolve({ enabled: true, forced: true, states: {} })
    if (cmd === 'post_mock_connection') {
      providersStore.set({ is_loading: false, data: [teams] })
      return Promise.resolve(teams)
    }
    return Promise.reject(new Error(cmd))
  })
})

afterEach(() => {
  cleanup()
  vi.mocked(invoke).mockReset()
  vi.mocked(goto).mockReset()
})

describe('setup accounts step', () => {
  it('asks for no name and cannot continue until an account is connected', async () => {
    render(Page)
    expect(screen.queryByText(t('setup.profile.title'))).toBeNull()
    expect(screen.queryByPlaceholderText(t('setup.profile.namePlaceholder'))).toBeNull()
    expect(screen.queryByText(t('settings.registrations.intro'))).toBeNull()
    const next = screen.getByTestId('setup-providers-continue')
    expect(next).toBeDisabled()
    expect(screen.getByText(new RegExp(t('setup.providers.needsAccount')))).toBeInTheDocument()
    await fireEvent.click(next)
    expect(goto).not.toHaveBeenCalled()
  })

  it('counts a test client in debug mode and then moves on', async () => {
    render(Page)
    const add = await screen.findByTestId('setup-mock-teams')
    await fireEvent.click(add)
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('post_mock_connection', { kind: 'teams' }))
    const next = screen.getByTestId('setup-providers-continue')
    await waitFor(() => expect(next).not.toBeDisabled())
    await fireEvent.click(next)
    expect(goto).toHaveBeenCalledWith('/setup/intelligence')
  })

  it('hides test clients when debug mode is off', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => (cmd === 'get_mock_mode' ? Promise.resolve({ enabled: false, forced: null, states: {} }) : Promise.resolve([])))
    render(Page)
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('get_mock_mode'))
    expect(screen.queryByTestId('setup-test-clients')).toBeNull()
  })
})
