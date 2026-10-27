import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import * as api from '$lib/tauri/api'
import type { VoiceEvent, VoiceSetup, VoiceSetupProgress } from '$lib/tauri/types'
import { requirementAnchor, settingHref, type SettingAnchor } from '$lib/settings'

export const VOICE_EVENT = 'voice'
export const VOICE_SETUP_EVENT = 'voice-setup'
export const HOTKEYS = ['alt+space', 'meta+shift+period', 'off'] as const

export type VoicePhase = 'off' | 'idle' | 'starting' | 'recording' | 'stopping' | 'cleaning' | 'failed'

export type VoiceAction =
  | { event: 'setup'; ready: boolean }
  | { event: 'start' }
  | { event: 'started'; id: string }
  | { event: 'stop' }
  | { event: 'refused'; error: string }

export interface VoiceSnapshot {
  phase: VoicePhase
  ready: boolean
  id: string | null
  level: number
  speech: boolean
  committed: string
  tentative: string
  fixed: string
  fixedRaw: string
  text: string
  raw: string
  language: string | null
  error: string | null
  reason: 'microphone' | null
}

export interface VoiceTarget {
  insert: (text: string) => void
  replace?: (previous: string, next: string) => boolean
}

export interface Hotkey {
  code: string
  meta: boolean
  shift: boolean
  alt: boolean
  ctrl: boolean
}

export const VOICE_IDLE: VoiceSnapshot = {
  phase: 'off',
  ready: false,
  id: null,
  level: 0,
  speech: false,
  committed: '',
  tentative: '',
  fixed: '',
  fixedRaw: '',
  text: '',
  raw: '',
  language: null,
  error: null,
  reason: null
}

const LIST_ITEM = /^(?:[-*•] |\d+[.)] )/

export function joinSpans(head: string, tail: string): string {
  if (!head || !tail) return `${head}${tail.trimStart()}`
  if (head.endsWith('\n') || tail.startsWith('\n')) return `${head}${tail}`
  return LIST_ITEM.test(tail) ? `${head}\n${tail}` : `${head} ${tail}`
}

export function voicePreview(snapshot: VoiceSnapshot): string {
  const rest = snapshot.fixedRaw && snapshot.committed.startsWith(snapshot.fixedRaw) ? snapshot.committed.slice(snapshot.fixedRaw.length).trim() : snapshot.fixedRaw ? '' : snapshot.committed
  return joinSpans(joinSpans(snapshot.fixed, rest), snapshot.tentative).trim()
}

const listening = (phase: VoicePhase) => phase === 'starting' || phase === 'recording' || phase === 'stopping'

export function reduceVoice(state: VoiceSnapshot, event: VoiceEvent | VoiceAction): VoiceSnapshot {
  switch (event.event) {
    case 'setup':
      if (listening(state.phase) || state.phase === 'cleaning') return { ...state, ready: event.ready }
      return { ...state, ready: event.ready, phase: event.ready ? 'idle' : 'off' }
    case 'start':
      return { ...VOICE_IDLE, ready: state.ready, language: state.language, phase: 'starting' }
    case 'started':
      if (state.phase === 'stopping') return { ...state, id: event.id }
      return state.phase === 'starting' ? { ...state, id: event.id, phase: 'recording' } : state
    case 'stop':
      return state.phase === 'recording' || state.phase === 'starting' ? { ...state, phase: 'stopping' } : state
    case 'refused':
      return { ...state, phase: 'failed', error: event.error, level: 0, speech: false }
  }
  const adopting = state.phase === 'starting' && state.id === null
  if (!adopting && event.id !== state.id) return state
  const owned = adopting ? { ...state, id: event.id } : state
  switch (event.event) {
    case 'level':
      return listening(owned.phase) ? { ...owned, level: event.level, speech: event.speech } : owned
    case 'partial':
      return { ...owned, committed: event.committed, tentative: event.tentative, language: event.language ?? owned.language }
    case 'final':
      return { ...owned, committed: event.committed, tentative: '', language: event.language ?? owned.language }
    case 'corrected':
      return { ...owned, fixed: joinSpans(owned.fixed, event.text), fixedRaw: owned.fixedRaw ? `${owned.fixedRaw} ${event.raw}` : event.raw }
    case 'done':
      return { ...owned, phase: event.cleaning ? 'cleaning' : owned.ready ? 'idle' : 'off', text: event.text, raw: event.raw, tentative: '', level: 0, speech: false, language: event.language ?? owned.language }
    case 'cleaned':
      return { ...owned, phase: owned.ready ? 'idle' : 'off', text: event.text, raw: event.raw }
    case 'failed':
      return { ...owned, phase: 'failed', error: event.error, reason: event.reason, level: 0, speech: false }
  }
}

export function insertText(value: string, start: number, end: number, text: string): { value: string; caret: number } {
  const head = value.slice(0, start)
  const tail = value.slice(end)
  const lead = head && !/\s$/.test(head) ? ' ' : ''
  const trail = tail && !/^[\s.,!?;:]/.test(tail) ? ' ' : ''
  const placed = `${lead}${text}${trail}`
  return { value: `${head}${placed}${tail}`, caret: head.length + lead.length + text.length }
}

export function replaceText(value: string, previous: string, next: string): string | null {
  const at = value.lastIndexOf(previous)
  return previous && at >= 0 ? `${value.slice(0, at)}${next}${value.slice(at + previous.length)}` : null
}

const CODES: Record<string, string> = { space: 'Space', period: 'Period', comma: 'Comma', slash: 'Slash' }
const GLYPHS: Record<string, string> = { meta: '⌘', ctrl: '⌃', alt: '⌥', shift: '⇧', space: 'Space', period: '.', comma: ',', slash: '/' }

export function parseHotkey(value: string | null | undefined): Hotkey | null {
  const parts = (value ?? '').toLowerCase().split('+').map((p) => p.trim()).filter(Boolean)
  const key = parts.at(-1)
  if (!key || key === 'off' || ['meta', 'shift', 'alt', 'ctrl'].includes(key)) return null
  const code = CODES[key] ?? (/^[a-z]$/.test(key) ? `Key${key.toUpperCase()}` : /^\d$/.test(key) ? `Digit${key}` : null)
  if (!code) return null
  return { code, meta: parts.includes('meta'), shift: parts.includes('shift'), alt: parts.includes('alt'), ctrl: parts.includes('ctrl') }
}

export function hotkeyMatches(event: Pick<KeyboardEvent, 'code' | 'metaKey' | 'shiftKey' | 'altKey' | 'ctrlKey'>, value: string | null | undefined): boolean {
  const hotkey = parseHotkey(value)
  return !!hotkey && event.code === hotkey.code && event.metaKey === hotkey.meta && event.shiftKey === hotkey.shift && event.altKey === hotkey.alt && event.ctrlKey === hotkey.ctrl
}

export function formatHotkey(value: string | null | undefined): string {
  if (!parseHotkey(value)) return ''
  return (value ?? '')
    .toLowerCase()
    .split('+')
    .map((p) => GLYPHS[p.trim()] ?? p.trim().toUpperCase())
    .join('')
}

class Voice {
  snapshot = $state<VoiceSnapshot>(VOICE_IDLE)
  setup = $state<VoiceSetup | null>(null)
  progress = $state<Record<string, VoiceSetupProgress>>({})
  #target: VoiceTarget | null = null
  #inserted: string | null = null
  #subscribed: Promise<UnlistenFn[]> | null = null

  get phase() {
    return this.snapshot.phase
  }
  get ready() {
    return this.snapshot.ready
  }
  get level() {
    return this.snapshot.level
  }
  get speech() {
    return this.snapshot.speech
  }
  get preview() {
    return voicePreview(this.snapshot)
  }
  get text() {
    return this.snapshot.text
  }
  get error() {
    return this.snapshot.error
  }
  get recording() {
    return listening(this.snapshot.phase)
  }
  get hotkey() {
    return this.setup?.settings.hotkey ?? HOTKEYS[0]
  }
  get requirement(): SettingAnchor | null {
    return this.setup?.unmet.map((r) => requirementAnchor(r.key)).find((a): a is SettingAnchor => !!a) ?? null
  }
  get setupHref(): string | null {
    return this.requirement ? settingHref(this.requirement) : null
  }
  get settingUp() {
    return this.setup?.setting_up || Object.values(this.progress).some((p) => !p.finished && !p.done && !p.error)
  }

  dispatch(event: VoiceEvent | VoiceAction) {
    const before = this.snapshot
    const after = reduceVoice(before, event)
    this.snapshot = after
    if (after === before) return
    if (event.event === 'done') {
      if (event.text && this.#target) {
        this.#target.insert(event.text)
        this.#inserted = event.text
      }
      if (!event.cleaning) this.#target = null
    }
    if (event.event === 'cleaned') {
      if (this.#target && this.#inserted && event.text !== this.#inserted) this.#target.replace?.(this.#inserted, event.text)
      this.#target = null
      this.#inserted = null
    }
    if (event.event === 'failed') this.#target = null
  }

  subscribe(): Promise<UnlistenFn[]> {
    this.#subscribed ??= Promise.all([
      listen<VoiceEvent>(VOICE_EVENT, (message) => this.dispatch(message.payload)),
      listen<VoiceSetupProgress>(VOICE_SETUP_EVENT, (message) => {
        const progress = message.payload
        if (progress.finished) {
          this.progress = {}
          this.load().catch(() => {})
          if (progress.error) this.dispatch({ event: 'refused', error: progress.error })
          return
        }
        this.progress = { ...this.progress, [progress.model]: progress }
      })
    ])
    return this.#subscribed
  }

  async load(): Promise<VoiceSetup> {
    await this.subscribe()
    const setup = await api.getVoiceSetup()
    this.setup = setup
    this.dispatch({ event: 'setup', ready: setup.ready })
    return setup
  }

  async start(target?: VoiceTarget, conversationId?: string | null) {
    if (this.recording || this.phase === 'cleaning') return
    await this.subscribe()
    if (!this.ready) await this.load()
    if (!this.ready) return
    this.#target = target ?? null
    this.#inserted = null
    this.dispatch({ event: 'start' })
    try {
      const id = await api.postVoice(conversationId)
      this.dispatch({ event: 'started', id })
      if (this.phase === 'stopping') await api.deleteVoice(id).catch(() => {})
    } catch (error) {
      this.#target = null
      if (this.phase === 'starting') this.dispatch({ event: 'refused', error: String(error) })
    }
  }

  async stop() {
    const id = this.snapshot.id
    if (!this.recording) return
    this.dispatch({ event: 'stop' })
    if (id) await api.deleteVoice(id).catch(() => {})
  }

  toggle(target?: VoiceTarget, conversationId?: string | null) {
    return this.recording ? this.stop() : this.start(target, conversationId)
  }

  async setUp(force = false) {
    await this.subscribe()
    await api.postVoiceSetup(force)
    await this.load()
  }

  async put(key: 'voice.enabled' | 'voice.language' | 'voice.cleanup' | 'voice.hotkey', value: string) {
    await api.putAppSetting(key, value)
    return this.load()
  }
}

export const voice = new Voice()
