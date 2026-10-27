import { describe, expect, it, vi } from 'vitest'
import { isRedirect } from '@sveltejs/kit'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import { actionHref, anchorChain, featureShown, FEATURES, getLastSection, isSettingAnchor, parseHash, putLastSection, REQUIREMENT_SETTINGS, requirementAnchor, resolveRow, searchSettings, sectionOf, SETTING_SECTIONS, settingHref, settingRedirect, MOVED_ANCHORS, type SettingAnchor } from '$lib/settings'
import { cycleScope, inScope, SCOPES } from '$lib/palette'
import { chosenRoute, getStartupTarget, INBOX_ROUTE, parseStartupChoice, sanitizeRoute, trackedRoute } from '$lib/startup'
import { getActionCatalog, getActions } from '$lib/tauri/api'
import { load as casesLoad } from '../routes/main/cases/+page'
import { load as caseLoad } from '../routes/main/case/[id]/+page'
import rail from '$lib/components/rail/Rail.svelte?raw'
import sidebarSettings from '$lib/components/settings/SidebarSettings.svelte?raw'
import startupPage from '$lib/components/settings/StartupPage.svelte?raw'
import searchModal from '$lib/components/modals/SearchModal.svelte?raw'
import titleBar from '$lib/components/conversation/TitleBar.svelte?raw'
import messageMenu from '$lib/components/conversation/messages/MessageContextMenu.svelte?raw'
import inboxList from '$lib/components/inbox/InboxList.svelte?raw'
import layout from '../routes/main/+layout.svelte?raw'
import { t } from '$lib/i18n.svelte'
import { en } from '$lib/i18n/en'
import rustScript from '../../src-tauri/src/services/action_script.rs?raw'

describe('settings registry', () => {
  it('gives every section and setting a unique, hash safe id', () => {
    const anchors = SETTING_SECTIONS.flatMap((s) => [s.id, ...s.settings.map((r) => `${s.id}.${r.id}`)])
    expect(new Set(anchors).size).toBe(anchors.length)
    for (const anchor of anchors) expect(anchor).toMatch(/^[a-z]+(\.[a-z][a-z-]*)?$/)
  })

  it('labels everything with an existing English message', () => {
    for (const section of SETTING_SECTIONS) {
      expect(en[section.label]).toBeTruthy()
      for (const setting of section.settings) expect([setting.label, !!en[setting.label]]).toEqual([setting.label, true])
    }
  })

  it('points fallbacks at settings in the same section', () => {
    for (const section of SETTING_SECTIONS) {
      for (const setting of section.settings) {
        if ('fallback' in setting) expect(isSettingAnchor(`${section.id}.${setting.fallback}`)).toBe(true)
      }
    }
  })

  it('covers every setting the backend requirements link to', () => {
    const declared = [...rustScript.matchAll(/pub const REQUIRES_\w+: &str = "([^"]+)";/g)].map((m) => m[1]).sort()
    expect(declared.length).toBeGreaterThan(0)
    expect([...REQUIREMENT_SETTINGS].sort()).toEqual(declared)
    for (const key of declared) expect(isSettingAnchor(key)).toBe(true)
  })
})

describe('parseHash', () => {
  it('reads a section or a section and setting', () => {
    expect(parseHash('#ai')).toEqual({ anchor: 'ai', section: 'ai', setting: null })
    expect(parseHash('#ai.default-model')).toEqual({ anchor: 'ai.default-model', section: 'ai', setting: 'default-model' })
    expect(parseHash('messages.time-format')?.setting).toBe('time-format')
    expect(parseHash('#ai%2Edefault-model')?.anchor).toBe('ai.default-model')
  })

  it('refuses what the registry does not know', () => {
    expect(parseHash('')).toBeNull()
    expect(parseHash('#')).toBeNull()
    expect(parseHash('#nowhere')).toBeNull()
    expect(parseHash('#ai.nothing')).toBeNull()
    expect(parseHash('#%E0%A4%A')).toBeNull()
  })
})

describe('resolveRow', () => {
  const rows = (present: string[]) => (id: SettingAnchor) => (present.includes(id) ? { id } : null)

  it('finds the exact row first', () => {
    expect(resolveRow('ai.default-model', rows(['ai.default-model', 'ai.connection', 'ai']))?.id).toBe('ai.default-model')
  })

  it('falls back to the declared row, then the section', () => {
    expect(anchorChain('ai.default-model')).toEqual(['ai.default-model', 'ai.connection', 'ai'])
    expect(resolveRow('ai.default-model', rows(['ai.connection', 'ai']))?.id).toBe('ai.connection')
    expect(resolveRow('privacy.lock-now', rows(['privacy']))?.id).toBe('privacy')
    expect(resolveRow('privacy', rows(['privacy']))?.id).toBe('privacy')
    expect(resolveRow('privacy.idle', rows([]))).toBeNull()
  })
})

describe('settings search and navigation', () => {
  it('matches labels, keywords and sections', () => {
    expect(searchSettings('touch', t).map((h) => h.anchor)).toContain('privacy.touch-id')
    expect(searchSettings('ollama', t).map((h) => h.anchor)).toContain('ai.ollama')
    expect(searchSettings('time format', t).map((h) => h.anchor)).toEqual(['messages.time-format'])
    expect(searchSettings('   ', t)).toEqual([])
  })

  it('sends a bare or mismatched settings path to the page of its anchor', () => {
    const store = new Map<string, string>()
    vi.stubGlobal('localStorage', { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => store.set(k, v), removeItem: (k: string) => store.delete(k) })
    expect(settingRedirect(undefined, '')).toBe('/main/profile/general')
    expect(settingRedirect(undefined, '#ai.default-model')).toBe('/main/profile/ai#ai.default-model')
    expect(settingRedirect('nowhere', '')).toBe('/main/profile/general')
    expect(settingRedirect('ai', '#voice.enabled')).toBe('/main/profile/voice#voice.enabled')
    expect(settingRedirect('ai', '#ai.ollama')).toBeNull()
    expect(settingRedirect('voice', '')).toBeNull()
    putLastSection('calendar')
    expect(getLastSection()).toBe('calendar')
    expect(settingRedirect(undefined, '')).toBe('/main/profile/calendar')
    vi.unstubAllGlobals()
  })

  it('links requirements to settings', () => {
    expect(settingHref('ai.default-model')).toBe('/main/profile/ai#ai.default-model')
    expect(settingHref('voice')).toBe('/main/profile/voice')
    expect(requirementAnchor('accounts.connections')).toBe('accounts.connections')
    expect(requirementAnchor('ai.unknown')).toBeNull()
    expect(requirementAnchor(null)).toBeNull()
    expect(sectionOf('developer')?.settings.length).toBeGreaterThan(0)
  })

  it('sends the old per feature AI anchors to the action or the settings page that now holds the choice', () => {
    expect(actionHref('builtin:receipts', 'ai')).toBe('/main/actions?id=builtin%3Areceipts&field=ai')
    expect(actionHref('sort_splits')).toBe('/main/actions?id=sort_splits')
    expect(settingRedirect('ai', '#ai.model-sorting')).toBe('/main/actions?id=sort_splits&field=ai')
    expect(settingRedirect('ai', '#ai.model-summaries')).toBe('/main/actions?id=summarise_mail&field=ai')
    expect(settingRedirect('ai', '#ai.model-voice')).toBe('/main/profile/voice#voice.model-voice')
    expect(settingRedirect('ai', '#ai.model-assistant')).toBe('/main/profile/actions#actions.model-assistant')
    expect(settingRedirect(undefined, '#ai.models')).toBe('/main/actions')
    for (const [anchor, href] of Object.entries(MOVED_ANCHORS)) {
      expect(isSettingAnchor(anchor)).toBe(false)
      if (href.startsWith('/main/profile')) expect(isSettingAnchor(href.split('#')[1])).toBe(true)
    }
  })
})

describe('postponed cases', () => {
  it('ships with cases switched off and hides every case key and route', () => {
    expect(FEATURES.cases).toBe(false)
    for (const key of ['cases', 'case', 'new-case', 'add_to_case', 'builtin:add_to_case', 'app.shortcut.addToCase', '/main/cases', '/main/cases?id=1', '/main/case/1']) expect(featureShown(key)).toBe(false)
    for (const key of ['actions', 'builtin:tag_add', '/main/calendar', '/main/conversation', '/main/casework', 'new-chat']) expect(featureShown(key)).toBe(true)
  })

  it('shows them again once the flag is on', () => {
    FEATURES.cases = true
    try {
      expect(featureShown('cases')).toBe(true)
      expect(featureShown('/main/case/1')).toBe(true)
    } finally {
      FEATURES.cases = false
    }
  })

  it('keeps cases out of the palette scopes, groups and commands', () => {
    expect(SCOPES.map((s) => s.value)).not.toContain('cases')
    expect(inScope('all', 'cases')).toBe(false)
    expect(inScope('all', 'people')).toBe(true)
    expect(cycleScope('actions', 1)).toBe('places')
    expect([{ id: 'new-chat' }, { id: 'new-case' }].filter((command) => featureShown(command.id)).map((c) => c.id)).toEqual(['new-chat'])
  })

  it('has no settings anchor for cases', () => {
    const anchors = SETTING_SECTIONS.flatMap((s) => [s.id, ...s.settings.map((r) => `${s.id}.${r.id}`)])
    expect(anchors.filter((a) => /case/.test(a))).toEqual([])
    expect(searchSettings('cases', t)).toEqual([])
  })

  it('offers no startup choice and keeps no last route for cases', () => {
    expect(parseStartupChoice('/main/cases')).toBe('last')
    expect(parseStartupChoice('/main/case/1')).toBe('last')
    expect(sanitizeRoute('/main/cases?id=1')).toBeNull()
    expect(trackedRoute(new URL('/main/case/1', 'chatnip://app'))).toBeNull()
    expect(chosenRoute({ choice: 'last', last: '/main/cases' })).toBeNull()
  })

  it('falls back to the inbox when the startup page or last route is a case', async () => {
    expect((await getStartupTarget({ choice: '/main/cases', last: null })).path).toBe(INBOX_ROUTE)
    expect((await getStartupTarget({ choice: 'last', last: '/main/case/1' })).path).toBe(INBOX_ROUTE)
  })

  it('drops the add to case picker item and built in from the actions presentation', async () => {
    const entries = [{ id: 'a', key: 'builtin:add_to_case' }, { id: 'b', key: 'builtin:tag_add' }, { id: 'c', key: null }]
    invoke.mockResolvedValueOnce({ actions: entries, builtins: [{ key: 'add_to_case' }] })
    const catalog = await getActionCatalog()
    expect(catalog.actions.map((a) => a.id)).toEqual(['b', 'c'])
    expect(catalog.builtins).toHaveLength(1)
    invoke.mockResolvedValueOnce(entries)
    expect((await getActions()).map((a) => a.id)).toEqual(['b', 'c'])
  })

  it('redirects the cases routes to the inbox', async () => {
    const parent = vi.fn(async () => ({}))
    const list = await (async () => casesLoad({ url: new URL('/main/cases', 'chatnip://app') } as never))().catch((e: unknown) => e)
    expect(isRedirect(list) && [list.status, list.location]).toEqual([307, INBOX_ROUTE])
    const one = await (async () => caseLoad({ params: { id: '1' }, parent } as never))().catch((e: unknown) => e)
    expect(isRedirect(one) && [one.status, one.location]).toEqual([307, INBOX_ROUTE])
    expect(parent).not.toHaveBeenCalled()
  })

  it('gates every case entry point in the components', () => {
    expect(rail).toContain('.filter((item) => featureShown(item.key))')
    expect(sidebarSettings).toContain("entry.kind !== 'item' || featureShown(entry.key)")
    expect(startupPage).toContain('.filter((item) => featureShown(item.route))')
    expect(searchModal).toContain('list.filter((command) => featureShown(command.id))')
    for (const source of [titleBar, messageMenu, inboxList]) expect(source).toMatch(/\{#if FEATURES\.cases\}\s*<(IconButton|MenuItem) icon="lucide:briefcase"/)
    expect(layout).toContain('if (FEATURES.cases) registerShortcut({')
    expect(layout).toMatch(/\{#if FEATURES\.cases\}\s*<CasePicker \/>/)
  })
})
