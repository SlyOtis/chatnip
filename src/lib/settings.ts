import type { MessageKey } from '$lib/i18n.svelte'
import type { SettingRegistry } from '$lib/tauri/types'

export interface SettingEntry {
  readonly id: string
  readonly label: MessageKey
  readonly fallback?: string
  readonly keywords?: readonly string[]
}

export interface SettingSectionEntry {
  readonly id: string
  readonly label: MessageKey
  readonly icon: string
  readonly keywords: readonly string[]
  readonly settings: readonly SettingEntry[]
}

export const SETTING_SECTIONS = [
  {
    id: 'general',
    label: 'settings.section.general',
    icon: 'lucide:settings',
    keywords: ['general', 'profile', 'preferences'],
    settings: [
      { id: 'profile', label: 'settings.profile.title', keywords: ['language', 'startup'] },
      { id: 'language', label: 'settings.language.label', keywords: ['locale', 'norsk', 'english'] },
      { id: 'startup-page', label: 'settings.startup.label', keywords: ['startup', 'launch', 'open', 'last page', 'home', 'oppstart'] },
      { id: 'panels', label: 'settings.panels.title', keywords: ['sidebar', 'list', 'details', 'panel', 'collapse', 'hide'] },
      { id: 'shortcuts', label: 'settings.shortcuts.title', keywords: ['keys', 'keyboard', 'hotkey', 'rebind', 'shortcut'] }
    ]
  },
  {
    id: 'appearance',
    label: 'settings.section.appearance',
    icon: 'lucide:palette',
    keywords: ['theme', 'look', 'colours', 'colors'],
    settings: [
      { id: 'theme', label: 'settings.appearance.theme', keywords: ['dark', 'light', 'day', 'night'] },
      { id: 'background', label: 'settings.appearance.background', keywords: ['gradient', 'image', 'wallpaper'] },
      { id: 'surface', label: 'settings.appearance.surface', keywords: ['glass', 'solid'] },
      { id: 'frost', label: 'settings.appearance.frost', keywords: ['blur', 'tint'] },
      { id: 'accent', label: 'settings.appearance.accent', keywords: ['colour', 'color'] },
      { id: 'avatars', label: 'settings.appearance.avatars', keywords: ['avatar', 'photo', 'round', 'square', 'border', 'size'] },
      { id: 'presence', label: 'settings.appearance.motionFor', keywords: ['typing', 'presence', 'animation', 'entry'] }
    ]
  },
  {
    id: 'messages',
    label: 'settings.section.messages',
    icon: 'lucide:message-square',
    keywords: ['messages', 'chat', 'mail', 'thread'],
    settings: [
      { id: 'time-format', label: 'settings.messages.timeFormat', keywords: ['clock', '24h', '12h'] },
      { id: 'timestamp-interval', label: 'settings.messages.timestampInterval' },
      { id: 'breaks', label: 'settings.messages.breaks', keywords: ['days', 'separator'] },
      { id: 'save-eyes', label: 'settings.messages.saveEyes', keywords: ['invert', 'white', 'dark'] },
      { id: 'sending', label: 'settings.messages.sending', keywords: ['enter', 'send', 'send with', 'shift', 'cmd', 'new line'] },
      { id: 'collapse', label: 'settings.messages.collapse', keywords: ['long', 'fold'] },
      { id: 'emoji-size', label: 'settings.messages.emojiOnly' },
      { id: 'edited', label: 'settings.messages.edited' },
      { id: 'link-previews', label: 'settings.messages.linkPreviews', keywords: ['links', 'unfurl'] },
      { id: 'folded-mail', label: 'settings.messages.folded', keywords: ['summary', 'summaries'] },
      { id: 'model-threads', label: 'settings.models.feature.threads', keywords: ['model', 'ai', 'summary', 'summaries', 'thread', 'section'] },
      { id: 'default-reply', label: 'settings.messages.defaultReply', keywords: ['reply all'] },
      { id: 'mail-delay', label: 'settings.messages.mailDelay', keywords: ['undo', 'delay', 'countdown', 'seconds', 'send'] },
      { id: 'mail-notice', label: 'settings.messages.mailNotice', keywords: ['notification', 'template', 'countdown'] },
      { id: 'mail-draft-notice', label: 'settings.messages.mailDraftNotice', keywords: ['notification', 'template', 'draft', 'quit'] },
      { id: 'mail-delay-actions', label: 'settings.messages.mailDelayActions', keywords: ['actions', 'automation', 'delay'] },
      { id: 'mail-quit', label: 'settings.messages.mailQuit', keywords: ['quit', 'draft', 'send now'] }
    ]
  },
  {
    id: 'layouts',
    label: 'settings.section.layouts',
    icon: 'lucide:layout-template',
    keywords: ['layout', 'template', 'svelte', 'code', 'component', 'design', 'mail', 'chat', 'bot'],
    settings: [{ id: 'editor', label: 'settings.layouts.editor', keywords: ['code', 'svelte'] }]
  },
  {
    id: 'accounts',
    label: 'settings.section.accounts',
    icon: 'lucide:plug',
    keywords: ['connections', 'accounts', 'microsoft', 'google', 'slack', 'imap', 'teams', 'outlook', 'gmail'],
    settings: [
      { id: 'connections', label: 'settings.tab.connections', keywords: ['providers', 'tenants', 'workspaces', 'connect', 'new account'] },
      { id: 'test-clients', label: 'settings.connections.testClients', keywords: ['mock', 'fake', 'debug'] },
      { id: 'mock-state', label: 'settings.connections.mockState', keywords: ['mock', 'debug', 'error', 'offline', 'rate limit', 'expired'] },
      { id: 'mock-replies', label: 'settings.connections.mockReplies', keywords: ['mock', 'debug', 'simulate', 'reply'] },
      { id: 'mock-newsletters', label: 'settings.connections.mockNewsletters', keywords: ['mock', 'debug', 'simulate', 'newsletter', 'bot'] },
      { id: 'registrations', label: 'settings.connections.registrations', keywords: ['client id', 'secret', 'app'] },
      { id: 'sync-interval', label: 'settings.connections.syncInterval', keywords: ['sync', 'poll'] },
      { id: 'live-interval', label: 'settings.connections.liveInterval', keywords: ['sync', 'live', 'push', 'socket'] },
      { id: 'history-days', label: 'settings.connections.historyDays', keywords: ['sync', 'history'] },
      { id: 'emoji-interval', label: 'settings.connections.emojiInterval', keywords: ['emoji', 'custom', 'slack', 'sync'] },
      { id: 'directory-interval', label: 'settings.connections.directoryInterval', keywords: ['people', 'directory', 'avatar', 'contacts', 'sync'] }
    ]
  },
  {
    id: 'organise',
    label: 'settings.section.organise',
    icon: 'lucide:folder-tree',
    keywords: ['organise', 'organize', 'sorting', 'inboxes'],
    settings: [
      { id: 'splits', label: 'settings.sorting.tabs', keywords: ['split', 'main tabs'] },
      { id: 'tags', label: 'settings.inbox.tags', keywords: ['labels'] },
      { id: 'groups', label: 'settings.inbox.groups', keywords: ['folders', 'queries'] },
      { id: 'sidebar', label: 'settings.tab.sidebar', keywords: ['rail', 'order', 'sections'] }
    ]
  },
  {
    id: 'ai',
    label: 'settings.section.ai',
    icon: 'lucide:sparkles',
    keywords: ['intelligence', 'assistant', 'claude', 'ollama', 'replicate', 'models', 'providers'],
    settings: [
      { id: 'enabled', label: 'settings.assistant.use' },
      { id: 'claude', label: 'settings.ai.claude', keywords: ['claude code', 'cli'] },
      { id: 'ollama', label: 'settings.ai.ollama', keywords: ['local', 'pull'] },
      { id: 'connection', label: 'settings.origins.title', keywords: ['providers', 'openai', 'replicate', 'command'] },
      { id: 'default-model', label: 'settings.ai.defaultModel', fallback: 'connection', keywords: ['model', 'default'] },
      { id: 'claude-model', label: 'settings.ai.claudeModel', keywords: ['claude', 'model', 'default'] },
      { id: 'ollama-host', label: 'settings.ai.ollamaHost', keywords: ['ollama', 'host', 'address', 'url'] },
      { id: 'summaries', label: 'settings.summaries.title', keywords: ['summary', 'summaries', 'summarise', 'summarize', 'format', 'length', 'focus'] },
      { id: 'runs', label: 'settings.assistant.runs', keywords: ['cost', 'history'] }
    ]
  },
  {
    id: 'voice',
    label: 'settings.section.voice',
    icon: 'lucide:mic',
    keywords: ['voice', 'dictation', 'speech', 'whisper', 'microphone', 'tale', 'diktering'],
    settings: [
      { id: 'enabled', label: 'settings.voice.enabled', keywords: ['switch', 'dictate'] },
      { id: 'models', label: 'settings.voice.models', keywords: ['set up', 'download', 'whisper', 'silero'] },
      { id: 'language', label: 'settings.voice.language', keywords: ['norsk', 'english', 'detect'] },
      { id: 'cleanup', label: 'settings.voice.cleanup', keywords: ['correct', 'punctuation', 'claude', 'ollama'] },
      { id: 'model-voice', label: 'settings.models.feature.voice', keywords: ['model', 'ai', 'cleanup', 'dictation', 'claude', 'ollama'] },
      { id: 'hotkey', label: 'settings.voice.hotkey', keywords: ['shortcut', 'keys'] },
      { id: 'correction-delay', label: 'settings.voice.correctionDelay', keywords: ['debounce', 'pause', 'correct'] },
      { id: 'model-url', label: 'settings.voice.modelUrl', keywords: ['download', 'whisper', 'url'] },
      { id: 'vad-url', label: 'settings.voice.vadUrl', keywords: ['download', 'silero', 'url'] }
    ]
  },
  {
    id: 'actions',
    label: 'settings.section.actions',
    icon: 'lucide:zap',
    keywords: ['actions', 'automation', 'rules', 'workflow'],
    settings: [
      { id: 'enabled', label: 'settings.actions.enabled', keywords: ['switch', 'automation'] },
      { id: 'run', label: 'settings.sorting.title', keywords: ['resort', 'recent mail'] },
      { id: 'model-assistant', label: 'settings.models.feature.assistant', keywords: ['model', 'ai', 'assistant', 'template', 'write'] },
      { id: 'send-limit', label: 'settings.actions.sendLimit', keywords: ['hourly', 'limit', 'sends'] },
      { id: 'receipt-batch', label: 'settings.actions.receiptBatch', keywords: ['receipts', 'batch'] },
      { id: 'receipt-threshold', label: 'settings.actions.receiptThreshold', keywords: ['receipts', 'score', 'threshold'] }
    ]
  },
  {
    id: 'content',
    label: 'settings.section.content',
    icon: 'lucide:image',
    keywords: ['content', 'media', 'gif', 'emoji', 'tenor', 'giphy'],
    settings: [
      { id: 'gif-search', label: 'settings.media.gifTitle', keywords: ['tenor', 'giphy'] },
      { id: 'tenor-key', label: 'settings.media.tenorKey', keywords: ['tenor', 'api key'] },
      { id: 'giphy-key', label: 'settings.media.giphyKey', keywords: ['giphy', 'api key'] },
      { id: 'rating', label: 'settings.media.rating', keywords: ['safe', 'content rating'] },
      { id: 'locale', label: 'settings.media.language', keywords: ['locale', 'search language'] },
      { id: 'opens-on', label: 'settings.media.opensOn', keywords: ['picker', 'trending', 'favourites'] },
      { id: 'max-size', label: 'settings.media.largest', keywords: ['megabytes', 'size'] },
      { id: 'stickers', label: 'settings.media.stickers', keywords: ['transparent'] },
      { id: 'previews', label: 'settings.media.linksTitle', keywords: ['links'] },
      { id: 'library', label: 'settings.media.libraryTitle', keywords: ['emoji', 'images', 'upload'] }
    ]
  },
  {
    id: 'calendar',
    label: 'settings.section.calendar',
    icon: 'lucide:calendar',
    keywords: ['calendar', 'events', 'ics'],
    settings: [
      { id: 'export', label: 'settings.calendar.export', keywords: ['ics', 'feed'] },
      { id: 'proposed', label: 'settings.calendar.proposed', keywords: ['suggested', 'confirm'] }
    ]
  },
  {
    id: 'privacy',
    label: 'settings.section.privacy',
    icon: 'lucide:shield',
    keywords: ['security', 'lock', 'privacy', 'touch id'],
    settings: [
      { id: 'touch-id', label: 'settings.security.touchId', keywords: ['biometric'] },
      { id: 'idle', label: 'settings.security.idle', keywords: ['timeout'] },
      { id: 'lock-now', label: 'settings.security.lockNow' }
    ]
  },
  {
    id: 'developer',
    label: 'settings.section.developer',
    icon: 'lucide:square-terminal',
    keywords: ['developer', 'cli', 'mcp', 'plugin', 'terminal'],
    settings: [
      { id: 'plugin', label: 'settings.ai.plugin', keywords: ['claude code', 'mcp'] },
      { id: 'plugin-home', label: 'settings.ai.pluginHome', keywords: ['mcp', 'home', 'data', 'mock', 'folder'] },
      { id: 'plugin-mock', label: 'settings.ai.pluginMock', keywords: ['mcp', 'mock', 'debug', 'test'] },
      { id: 'cli', label: 'settings.ai.cli', keywords: ['path', 'terminal'] },
      { id: 'ai-timeout', label: 'settings.ai.timeout', keywords: ['timeout', 'seconds', 'ai'] },
      { id: 'summary-concurrency', label: 'settings.ai.summaryConcurrency', keywords: ['summaries', 'parallel', 'concurrency'] },
      { id: 'launch-card', label: 'settings.developer.launchCard', keywords: ['window', 'splash', 'launch', 'size'] },
      { id: 'mock-home', label: 'settings.developer.mockHome', keywords: ['mock', 'home', 'test', 'dev', 'chatnip-mock'] }
    ]
  }
] as const satisfies readonly SettingSectionEntry[]

type Sections = (typeof SETTING_SECTIONS)[number]
type AnchorsOf<S> = S extends { readonly id: infer I extends string; readonly settings: readonly (infer R)[] } ? I | (R extends { readonly id: infer N extends string } ? `${I}.${N}` : never) : never

export type SettingSection = Sections['id']
export type SettingAnchor = AnchorsOf<Sections>

export const SETTINGS_PATH = '/main/profile'

export interface ParsedAnchor {
  anchor: SettingAnchor
  section: SettingSection
  setting: string | null
}

export function sectionOf(id: string): SettingSectionEntry | null {
  return (SETTING_SECTIONS as readonly SettingSectionEntry[]).find((s) => s.id === id) ?? null
}

export function settingOf(anchor: string): SettingEntry | null {
  const [section, ...rest] = anchor.split('.')
  return sectionOf(section)?.settings.find((s) => s.id === rest.join('.')) ?? null
}

export function isSettingAnchor(value: string): value is SettingAnchor {
  const [section, ...rest] = value.split('.')
  if (!sectionOf(section)) return false
  return rest.length === 0 || !!settingOf(value)
}

export function parseHash(hash: string): ParsedAnchor | null {
  let raw = hash.startsWith('#') ? hash.slice(1) : hash
  try {
    raw = decodeURIComponent(raw).trim()
  } catch {
    return null
  }
  if (!raw || !isSettingAnchor(raw)) return null
  const [section, ...rest] = raw.split('.')
  return { anchor: raw, section: section as SettingSection, setting: rest.length ? rest.join('.') : null }
}

export function settingHref(anchor: SettingAnchor): string {
  const [section] = anchor.split('.')
  return anchor === section ? `${SETTINGS_PATH}/${section}` : `${SETTINGS_PATH}/${section}#${anchor}`
}

const SECTION_KEY = 'chatnip.settings-section'

export function getLastSection(): SettingSection {
  const stored = typeof localStorage === 'undefined' ? null : localStorage.getItem(SECTION_KEY)
  return stored && sectionOf(stored) ? (stored as SettingSection) : 'general'
}

export function putLastSection(section: SettingSection) {
  if (typeof localStorage !== 'undefined') localStorage.setItem(SECTION_KEY, section)
}

export const ACTIONS_PATH = '/main/actions'

export type ActionField = 'ai'

export function actionHref(action: string, field?: ActionField): string {
  const params = new URLSearchParams({ id: action })
  if (field) params.set('field', field)
  return `${ACTIONS_PATH}?${params}`
}

export const MOVED_ANCHORS: Record<string, string> = {
  'ai.models': ACTIONS_PATH,
  'ai.model-sorting': actionHref('sort_splits', 'ai'),
  'ai.model-summaries': actionHref('summarise_mail', 'ai'),
  'ai.model-events': actionHref('find_appointments', 'ai'),
  'ai.model-drafting': actionHref('draft_reply', 'ai'),
  'ai.model-scripts': ACTIONS_PATH,
  'ai.model-receipts': actionHref('builtin:receipts', 'ai'),
  'ai.model-assistant': settingHref('actions.model-assistant'),
  'ai.model-voice': settingHref('voice.model-voice')
}

export function movedAnchor(hash: string): string | null {
  const raw = hash.startsWith('#') ? hash.slice(1) : hash
  return MOVED_ANCHORS[raw] ?? null
}

export function settingRedirect(section: string | undefined, hash: string): string | null {
  const moved = movedAnchor(hash)
  if (moved) return moved
  const parsed = parseHash(hash)
  const wanted = parsed?.section ?? (section && sectionOf(section) ? (section as SettingSection) : getLastSection())
  return section === wanted ? null : settingHref(parsed?.anchor ?? wanted)
}

export function anchorChain(anchor: SettingAnchor): SettingAnchor[] {
  const [section] = anchor.split('.')
  const entry = settingOf(anchor)
  const chain: string[] = [anchor]
  if (entry?.fallback) chain.push(`${section}.${entry.fallback}`)
  if (section !== anchor) chain.push(section)
  return chain.filter((id): id is SettingAnchor => isSettingAnchor(id))
}

export function resolveRow<T>(anchor: SettingAnchor, lookup: (id: SettingAnchor) => T | null | undefined): { id: SettingAnchor; row: T } | null {
  for (const id of anchorChain(anchor)) {
    const row = lookup(id)
    if (row) return { id, row }
  }
  return null
}

export interface SettingHit {
  anchor: SettingAnchor
  section: SettingSectionEntry
  label: string
  sectionLabel: string
}

export function searchSettings(query: string, translate: (key: MessageKey) => string): SettingHit[] {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean)
  if (!words.length) return []
  const hits: SettingHit[] = []
  for (const section of SETTING_SECTIONS as readonly SettingSectionEntry[]) {
    const sectionLabel = translate(section.label)
    for (const setting of section.settings) {
      const label = translate(setting.label)
      const text = [label, sectionLabel, setting.id, ...(setting.keywords ?? []), ...section.keywords].join(' ').toLowerCase()
      if (words.every((w) => text.includes(w))) hits.push({ anchor: `${section.id}.${setting.id}` as SettingAnchor, section, label, sectionLabel })
    }
  }
  return hits
}

export const REQUIREMENT_SETTINGS = ['actions.enabled', 'ai.enabled', 'ai.connection', 'ai.default-model', 'ai.claude', 'ai.ollama', 'accounts.connections', 'voice.enabled', 'voice.models'] as const satisfies readonly SettingAnchor[]

export type RequirementSetting = (typeof REQUIREMENT_SETTINGS)[number]

export function requirementAnchor(setting: string | null | undefined): SettingAnchor | null {
  return setting && (REQUIREMENT_SETTINGS as readonly string[]).includes(setting) ? (setting as RequirementSetting) : null
}

export type Feature = 'cases'

export const FEATURES: Record<Feature, boolean> = { cases: false }

const GATED: Record<Feature, { keys: readonly string[]; routes: RegExp }> = {
  cases: { keys: ['cases', 'case', 'new-case', 'add_to_case', 'builtin:add_to_case', 'app.shortcut.addToCase'], routes: /^\/main\/cases?(?:[/?#]|$)/ }
}

export function featureShown(key: string): boolean {
  return (Object.keys(GATED) as Feature[]).every((feature) => FEATURES[feature] || !(GATED[feature].keys.includes(key) || GATED[feature].routes.test(key)))
}

export function settingValue(registry: Pick<SettingRegistry, 'defaults' | 'values'> | null | undefined, key: string): string {
  return registry?.values[key] ?? registry?.defaults[key] ?? ''
}

export function settingNumber(registry: Pick<SettingRegistry, 'defaults' | 'values'> | null | undefined, key: string): number {
  const stored = Number(registry?.values[key])
  return Number.isFinite(stored) && stored > 0 ? stored : Number(registry?.defaults[key] ?? 0)
}

export interface SettingField {
  key: string
  anchor: SettingAnchor
  label: MessageKey
  hint: MessageKey
  wide?: boolean
}
