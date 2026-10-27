import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const api = vi.hoisted(() => ({
  getAppSettings: vi.fn(),
  putAppSetting: vi.fn(),
  getConversation: vi.fn(),
  getPerson: vi.fn(),
  getCase: vi.fn(),
  getAction: vi.fn(),
  getInboxes: vi.fn(),
  getGroups: vi.fn(),
  getTags: vi.fn()
}))

vi.mock('$lib/tauri/api', () => api)

import { chosenRoute, flushLastRoute, setupReady, setupRoute, SETUP_PROVIDERS, SETUP_WELCOME, getStartupSettings, getStartupTarget, inboxRoute, INBOX_ROUTE, LAST_ROUTE_SETTING, MAIN_ROUTE, parseStartupChoice, putLastRoute, putStartupPage, SAVE_DELAY_MS, sanitizeRoute, STARTUP_PAGE_SETTING, trackedRoute, type InboxState } from './startup'

const plain: InboxState = { inbox: null, view: 'inbox', group: null, tag: null }

function url(route: string) {
  return new URL(route, 'chatnip://app')
}

beforeEach(() => {
  const store = new Map<string, string>()
  vi.stubGlobal('localStorage', { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => store.set(k, v), removeItem: (k: string) => store.delete(k) })
  for (const fn of Object.values(api)) fn.mockReset()
  api.putAppSetting.mockResolvedValue({})
})

describe('trackedRoute', () => {
  it('excludes setup, error, the root and unknown pages', () => {
    expect(trackedRoute(url('/'))).toBeNull()
    expect(trackedRoute(url('/setup/welcome'))).toBeNull()
    expect(trackedRoute(url('/error/boom'))).toBeNull()
    expect(trackedRoute(url('/main'))).toBeNull()
    expect(trackedRoute(url('/main/conversation/c1/error'))).toBeNull()
    expect(trackedRoute(url('/main/user/u1'))).toBeNull()
  })

  it('keeps pages and folds an open chat to its loader route', () => {
    expect(trackedRoute(url('/main/calendar'))).toBe('/main/calendar')
    expect(trackedRoute(url('/main/conversation/c1/chat'))).toBe('/main/conversation/c1')
    expect(trackedRoute(url('/main/person/p1/profile?identity=i1'))).toBe('/main/person/p1/profile?identity=i1')
  })

  it('drops transient search and hash state', () => {
    expect(trackedRoute(url('/main/actions?id=a1&assist=hello&new=1'))).toBe('/main/actions?id=a1')
    expect(trackedRoute(url('/main/actions?conversation=c1&name=x'))).toBe('/main/actions')
    expect(trackedRoute(url('/main/calendar#modal'))).toBe('/main/calendar')
    expect(trackedRoute(url('/main/profile#general.startup-page'))).toBe('/main/profile#general.startup-page')
    expect(trackedRoute(url('/main/profile#nope'))).toBe('/main/profile')
    expect(trackedRoute(url('/main/profile/ai#ai.claude'))).toBe('/main/profile/ai#ai.claude')
    expect(trackedRoute(url('/main/profile/nowhere'))).toBe('/main/profile')
  })

  it('adds the inbox state only on inbox routes and skips the default inbox', () => {
    expect(trackedRoute(url('/main/conversation'), { ...plain, inbox: 'default', view: 'done' }, 'default')).toBe('/main/conversation?view=done')
    expect(trackedRoute(url('/main/conversation/c1/chat'), { ...plain, inbox: 'work', group: 'g1' }, 'default')).toBe('/main/conversation/c1?inbox=work&group=g1')
    expect(trackedRoute(url('/main/calendar'), { ...plain, tag: 't1' })).toBe('/main/calendar')
  })
})

describe('sanitizeRoute and choices', () => {
  it('accepts known routes and refuses the rest', () => {
    expect(sanitizeRoute(inboxRoute('view', 'notifications'))).toBe('/main/conversation?view=notifications')
    expect(sanitizeRoute('/main/conversation?view=starred')).toBeNull()
    expect(sanitizeRoute('https://example.com/main/calendar')).toBeNull()
    expect(sanitizeRoute('/setup/profile')).toBeNull()
    expect(sanitizeRoute(null)).toBeNull()
  })

  it('reads last as the default choice', () => {
    expect(parseStartupChoice(undefined)).toBe('last')
    expect(parseStartupChoice('garbage')).toBe('last')
    expect(parseStartupChoice('/main/stats')).toBe('/main/stats')
    expect(chosenRoute({ choice: 'last', last: '/main/calendar' })).toBe('/main/calendar')
    expect(chosenRoute({ choice: '/main/stats', last: '/main/calendar' })).toBe('/main/stats')
  })
})

describe('getStartupSettings', () => {
  it('reads the cache without waiting on the backend', async () => {
    localStorage.setItem('chatnip.last-route', '/main/calendar')
    expect(await getStartupSettings()).toEqual({ choice: 'last', last: '/main/calendar' })
    expect(api.getAppSettings).not.toHaveBeenCalled()
  })

  it('falls back to the backend and caches what it finds', async () => {
    api.getAppSettings.mockResolvedValue({ [STARTUP_PAGE_SETTING]: '/main/stats', [LAST_ROUTE_SETTING]: '/main/calendar' })
    expect(await getStartupSettings()).toEqual({ choice: '/main/stats', last: '/main/calendar' })
    expect(localStorage.getItem('chatnip.startup-page')).toBe('/main/stats')
    expect(localStorage.getItem('chatnip.last-route')).toBe('/main/calendar')
  })

  it('stores the startup page in both places', async () => {
    expect(await putStartupPage('/main/calendar')).toBe('/main/calendar')
    expect(localStorage.getItem('chatnip.startup-page')).toBe('/main/calendar')
    expect(api.putAppSetting).toHaveBeenCalledWith(STARTUP_PAGE_SETTING, '/main/calendar')
  })
})

describe('getStartupTarget', () => {
  it('opens the main route when nothing was ever stored', async () => {
    expect((await getStartupTarget({ choice: 'last', last: null })).path).toBe(MAIN_ROUTE)
  })

  it('opens a stored chat that still exists', async () => {
    api.getConversation.mockResolvedValue({ id: 'c1', deleted_at: null })
    expect(await getStartupTarget({ choice: 'last', last: '/main/conversation/c1' })).toEqual({ path: '/main/conversation/c1', inbox: null, view: 'inbox', group: null, tag: null })
  })

  it('falls back to the inbox when the chat is gone or deleted', async () => {
    api.getConversation.mockRejectedValueOnce(new Error('not found'))
    expect((await getStartupTarget({ choice: 'last', last: '/main/conversation/c1' })).path).toBe(INBOX_ROUTE)
    api.getConversation.mockResolvedValueOnce({ id: 'c1', deleted_at: '2026-01-01' })
    expect((await getStartupTarget({ choice: 'last', last: '/main/conversation/c1' })).path).toBe(INBOX_ROUTE)
  })

  it('resolves a group into inbox state and drops it from the path', async () => {
    api.getGroups.mockResolvedValue([{ id: 'g1', name: 'Team' }])
    const target = await getStartupTarget({ choice: inboxRoute('group', 'g1'), last: '/main/calendar' })
    expect(target.path).toBe(INBOX_ROUTE)
    expect(target.group).toEqual({ id: 'g1', name: 'Team' })
  })

  it('falls back when a group, tag, inbox or action no longer exists', async () => {
    api.getGroups.mockResolvedValue([])
    api.getTags.mockResolvedValue([])
    api.getInboxes.mockResolvedValue([{ id: 'other' }])
    api.getAction.mockRejectedValue(new Error('gone'))
    for (const route of [inboxRoute('group', 'g1'), inboxRoute('tag', 't1'), inboxRoute('inbox', 'work'), '/main/actions?id=a1']) {
      expect(await getStartupTarget({ choice: route, last: null })).toEqual({ path: INBOX_ROUTE, inbox: null, view: 'inbox', group: null, tag: null })
    }
  })

  it('keeps a view and a settings anchor', async () => {
    expect(await getStartupTarget({ choice: 'last', last: '/main/conversation?view=done' })).toMatchObject({ path: INBOX_ROUTE, view: 'done' })
    expect((await getStartupTarget({ choice: '/main/profile#general.language', last: null })).path).toBe('/main/profile#general.language')
  })
})

describe('putLastRoute', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  it('caches at once and saves to the backend debounced', async () => {
    putLastRoute('/main/stats')
    putLastRoute('/main/calendar')
    expect(localStorage.getItem('chatnip.last-route')).toBe('/main/calendar')
    expect(api.putAppSetting).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS)
    expect(api.putAppSetting).toHaveBeenCalledTimes(1)
    expect(api.putAppSetting).toHaveBeenCalledWith(LAST_ROUTE_SETTING, '/main/calendar')
  })

  it('flushes on close and ignores excluded routes', async () => {
    putLastRoute(null)
    putLastRoute('/main/cases')
    await flushLastRoute()
    expect(api.putAppSetting).toHaveBeenCalledWith(LAST_ROUTE_SETTING, '/main/cases')
    putLastRoute('/main/cases')
    await vi.advanceTimersByTimeAsync(SAVE_DELAY_MS)
    expect(api.putAppSetting).toHaveBeenCalledTimes(1)
  })
})

describe('setup gating', () => {
  it('starts at the welcome screen without a profile and never at a name step', () => {
    expect(setupRoute(null)).toBe(SETUP_WELCOME)
    expect(SETUP_WELCOME).not.toContain('profile')
    expect(SETUP_PROVIDERS).toBe('/setup/providers')
  })

  it('keeps a profile without a connected account on the account step', () => {
    expect(setupRoute({ setup_step: 0 })).toBe(SETUP_PROVIDERS)
    expect(setupRoute({ setup_step: 1 })).toBe(SETUP_PROVIDERS)
    expect(setupRoute({ setup_step: 2 })).toBe(SETUP_PROVIDERS)
    expect(setupRoute({ setup_step: 3 })).toBe(MAIN_ROUTE)
  })

  it('cannot finish without one connected account, a test client included', () => {
    expect(setupReady([])).toBe(false)
    expect(setupReady([{ is_connected: false }])).toBe(false)
    expect(setupReady([{ is_connected: false }, { is_connected: true }])).toBe(true)
  })
})
