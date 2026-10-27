import { getAction, getAppSettings, getCase, getConversation, getGroups, getInboxes, getPerson, getTags, putAppSetting } from '$lib/tauri/api'
import type { Group, Profile, Provider } from '$lib/tauri/types'
import type { InboxView } from '$lib/inbox'
import { featureShown, parseHash, sectionOf, SETTINGS_PATH } from '$lib/settings'
import { debounce } from '$lib/debounce'

export const LAST_ROUTE_SETTING = 'app.last-route'
export const STARTUP_PAGE_SETTING = 'app.startup-page'
export const STARTUP_LAST = 'last'
export const INBOX_ROUTE = '/main/conversation'
export const MAIN_ROUTE = '/main'
export const SAVE_DELAY_MS = 1500
export const SETUP_WELCOME = '/setup/welcome'
export const SETUP_PROVIDERS = '/setup/providers'
export const SETUP_DONE = 3

const LAST_KEY = 'chatnip.last-route'
const STARTUP_KEY = 'chatnip.startup-page'
const BASE = 'chatnip://app'
const VIEWS: InboxView[] = ['inbox', 'done', 'snoozed', 'deleted', 'notifications', 'pipeline']
const INBOX_PARAMS = ['inbox', 'view', 'group', 'tag']
const PAGES = ['/main/actions', '/main/cases', '/main/content', '/main/calendar', '/main/stats', '/main/threads', '/main/profile']
const KEPT: Record<string, string[]> = { '/main/actions': ['id'], '/main/cases': ['id'] }

export interface InboxState {
  inbox: string | null
  view: InboxView
  group: string | null
  tag: string | null
}

export interface StartupTarget extends Omit<InboxState, 'group'> {
  path: string
  group: Group | null
}

export interface StartupSettings {
  choice: string
  last: string | null
}

const EMPTY: InboxState = { inbox: null, view: 'inbox', group: null, tag: null }

function inboxPath(path: string): boolean {
  return path === INBOX_ROUTE || /^\/main\/conversation\/[^/]+$/.test(path) || /^\/main\/person\/[^/]+$/.test(path)
}

function normalPath(path: string): string | null {
  const trimmed = path.length > 1 ? path.replace(/\/+$/, '') : path
  if (!featureShown(trimmed)) return null
  const conversation = trimmed.match(/^\/main\/conversation\/([^/]+)(\/.*)?$/)
  if (conversation) return conversation[2] === '/error' ? null : `${INBOX_ROUTE}/${conversation[1]}`
  if (trimmed === INBOX_ROUTE || PAGES.includes(trimmed)) return trimmed
  if (trimmed.startsWith(`${SETTINGS_PATH}/`)) return sectionOf(trimmed.slice(SETTINGS_PATH.length + 1)) ? trimmed : SETTINGS_PATH
  if (/^\/main\/person\/[^/]+(\/profile)?$/.test(trimmed) || /^\/main\/case\/[^/]+$/.test(trimmed)) return trimmed
  return null
}

function parsed(route: string): URL | null {
  if (!route.startsWith('/')) return null
  try {
    return new URL(route, BASE)
  } catch {
    return null
  }
}

export function inboxRoute(name: 'inbox' | 'view' | 'group' | 'tag', value: string): string {
  return `${INBOX_ROUTE}?${new URLSearchParams({ [name]: value })}`
}

export function trackedRoute(url: { pathname: string; search: string; hash: string }, state: InboxState | null = null, defaultInbox: string | null = null): string | null {
  const path = normalPath(url.pathname)
  if (!path) return null
  const source = new URLSearchParams(url.search)
  const params = new URLSearchParams()
  for (const name of KEPT[path] ?? []) {
    const value = source.get(name)
    if (value) params.set(name, value)
  }
  if (path.startsWith('/main/person/') && path.endsWith('/profile') && source.get('identity')) params.set('identity', source.get('identity') as string)
  if (inboxPath(path) && state) {
    if (state.inbox && state.inbox !== defaultInbox) params.set('inbox', state.inbox)
    if (state.view !== 'inbox') params.set('view', state.view)
    if (state.group) params.set('group', state.group)
    if (state.tag) params.set('tag', state.tag)
  }
  const query = params.toString()
  const hash = path.startsWith(SETTINGS_PATH) && parseHash(url.hash) ? url.hash : ''
  return `${path}${query ? `?${query}` : ''}${hash}`
}

export function sanitizeRoute(route: string | null | undefined): string | null {
  const url = route ? parsed(route) : null
  if (!url) return null
  const view = url.searchParams.get('view')
  const state: InboxState = {
    inbox: url.searchParams.get('inbox'),
    view: view && (VIEWS as string[]).includes(view) ? (view as InboxView) : 'inbox',
    group: url.searchParams.get('group'),
    tag: url.searchParams.get('tag')
  }
  if (view && state.view !== view) return null
  return trackedRoute(url, state)
}

export function inboxStateOf(route: string): InboxState {
  const url = parsed(route)
  if (!url) return EMPTY
  const view = url.searchParams.get('view') as InboxView | null
  return { inbox: url.searchParams.get('inbox'), view: view ?? 'inbox', group: url.searchParams.get('group'), tag: url.searchParams.get('tag') }
}

export function chosenRoute(settings: StartupSettings): string | null {
  return settings.choice === STARTUP_LAST ? sanitizeRoute(settings.last) : sanitizeRoute(settings.choice)
}

export function parseStartupChoice(value: string | null | undefined): string {
  return sanitizeRoute(value) ?? STARTUP_LAST
}

function storage(): Storage | null {
  return typeof localStorage === 'undefined' ? null : localStorage
}

export function getCachedStartup(): StartupSettings | null {
  const store = storage()
  const choice = store?.getItem(STARTUP_KEY) ?? null
  const last = store?.getItem(LAST_KEY) ?? null
  if (choice === null && last === null) return null
  return { choice: parseStartupChoice(choice), last }
}

function cache(settings: StartupSettings) {
  const store = storage()
  if (!store) return
  store.setItem(STARTUP_KEY, settings.choice)
  if (settings.last) store.setItem(LAST_KEY, settings.last)
  else store.removeItem(LAST_KEY)
}

export async function getStartupSettings(): Promise<StartupSettings> {
  const cached = getCachedStartup()
  if (cached) return cached
  const all = await getAppSettings().catch(() => null)
  const settings = { choice: parseStartupChoice(all?.[STARTUP_PAGE_SETTING]), last: sanitizeRoute(all?.[LAST_ROUTE_SETTING]) }
  if (all) cache(settings)
  return settings
}

export async function loadStartupSettings(): Promise<StartupSettings> {
  const all = await getAppSettings()
  const settings = { choice: parseStartupChoice(all[STARTUP_PAGE_SETTING]), last: sanitizeRoute(all[LAST_ROUTE_SETTING]) ?? getCachedStartup()?.last ?? null }
  cache(settings)
  return settings
}

export async function putStartupPage(choice: string): Promise<string> {
  const next = parseStartupChoice(choice)
  storage()?.setItem(STARTUP_KEY, next)
  await putAppSetting(STARTUP_PAGE_SETTING, next)
  return next
}

let pending: string | null = null
let saved: string | null = null
const saveSoon = debounce(() => void flushLastRoute(), SAVE_DELAY_MS)

export function putLastRoute(route: string | null) {
  if (!route || route === (pending ?? saved)) return
  storage()?.setItem(LAST_KEY, route)
  pending = route
  saveSoon()
}

export async function flushLastRoute(): Promise<void> {
  const route = pending
  if (!route) return
  pending = null
  saved = route
  await putAppSetting(LAST_ROUTE_SETTING, route).catch(() => {
    if (saved === route) saved = null
  })
}

function inbox(path: string): StartupTarget {
  return { ...EMPTY, path, group: null }
}

async function present(check: () => Promise<boolean>): Promise<boolean> {
  try {
    return await check()
  } catch {
    return false
  }
}

export async function getStartupTarget(settings: StartupSettings): Promise<StartupTarget> {
  const route = chosenRoute(settings)
  if (!route) return inbox(settings.choice === STARTUP_LAST && !settings.last ? MAIN_ROUTE : INBOX_ROUTE)
  const url = parsed(route) as URL
  const state = inboxStateOf(route)
  const path = url.pathname
  const segment = path.split('/')[3] ?? ''
  const checks: Promise<boolean>[] = []
  let group: Group | null = null
  if (path.startsWith(`${INBOX_ROUTE}/`)) checks.push(present(async () => !(await getConversation(segment)).deleted_at))
  if (path.startsWith('/main/person/')) checks.push(present(async () => !!(await getPerson(segment))))
  if (path.startsWith('/main/merged/')) checks.push(present(async () => !(await getConversation(segment)).deleted_at))
  if (path.startsWith('/main/case/')) checks.push(present(async () => !!(await getCase(segment))))
  const id = url.searchParams.get('id')
  if (path === '/main/actions' && id) checks.push(present(async () => !!(await getAction(id))))
  if (path === '/main/cases' && id) checks.push(present(async () => !!(await getCase(id))))
  if (state.inbox) checks.push(present(async () => (await getInboxes()).some((row) => row.id === state.inbox)))
  if (state.tag) checks.push(present(async () => (await getTags()).some((row) => row.id === state.tag)))
  if (state.group) {
    checks.push(
      present(async () => {
        group = (await getGroups()).find((row) => row.id === state.group) ?? null
        return !!group
      })
    )
  }
  if (!(await Promise.all(checks)).every(Boolean)) return inbox(INBOX_ROUTE)
  const kept = new URLSearchParams(url.search)
  for (const name of INBOX_PARAMS) kept.delete(name)
  const query = kept.toString()
  return { path: `${path}${query ? `?${query}` : ''}${url.hash}`, inbox: state.inbox, view: state.view, group, tag: state.tag }
}

export function setupReady(providers: Pick<Provider, 'is_connected'>[]): boolean {
  return providers.some((provider) => provider.is_connected)
}

export function setupRoute(profile: Pick<Profile, 'setup_step'> | null): string {
  if (!profile) return SETUP_WELCOME
  return profile.setup_step < SETUP_DONE ? SETUP_PROVIDERS : MAIN_ROUTE
}
