import { beforeEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { formatHotkey, hotkeyMatches, insertText, joinSpans, parseHotkey, reduceVoice, replaceText, voice, VOICE_EVENT, VOICE_IDLE, voicePreview, type VoiceSnapshot } from '$lib/voice.svelte'
import type { VoiceEvent, VoiceSetup } from '$lib/tauri/types'

const ready: VoiceSnapshot = { ...VOICE_IDLE, ready: true, phase: 'idle' }

const run = (state: VoiceSnapshot, ...events: Parameters<typeof reduceVoice>[1][]) => events.reduce(reduceVoice, state)

const partial = (id: string, committed: string, tentative: string): VoiceEvent => ({ event: 'partial', id, state: 'recording', committed, tentative, language: 'no', ms_first_word: 300, ms_decode: 80 })
const final = (id: string, segment: number, text: string, committed: string): VoiceEvent => ({ event: 'final', id, state: 'recording', segment, text, committed, tentative: '', language: 'no', ms_first_word: 300, ms_final: 120, ms_decode: 90 })
const done = (id: string, text: string, cleaning: boolean): VoiceEvent => ({ event: 'done', id, state: 'done', text, raw: text.toLowerCase(), cleaning, language: 'no', ms_first_word: 300, ms_final: 120 })
const cleaned = (id: string, text: string, raw: string): VoiceEvent => ({ event: 'cleaned', id, state: 'cleaned', text, raw, model: 'claude-haiku-4-5-20251001', language: 'no', ms_first_word: 300, ms_final: 120, ms_cleaned: 900 })

describe('reduceVoice', () => {
  it('is off until voice is enabled and set up', () => {
    expect(VOICE_IDLE.phase).toBe('off')
    expect(run(VOICE_IDLE, { event: 'setup', ready: true }).phase).toBe('idle')
    expect(run(ready, { event: 'setup', ready: false }).phase).toBe('off')
  })

  it('walks start, started, partials, finals, stop, done and cleaned', () => {
    const starting = run(ready, { event: 'start' })
    expect(starting.phase).toBe('starting')
    const recording = run(starting, { event: 'started', id: 'v-1' })
    expect([recording.phase, recording.id]).toEqual(['recording', 'v-1'])
    const heard = run(recording, { event: 'level', id: 'v-1', level: 0.6, speech: true }, partial('v-1', 'hei', 'der'))
    expect([heard.level, heard.speech, heard.committed, heard.tentative]).toEqual([0.6, true, 'hei', 'der'])
    const closed = run(heard, final('v-1', 0, 'hei der', 'hei der'))
    expect([closed.committed, closed.tentative]).toEqual(['hei der', ''])
    const stopping = run(closed, { event: 'stop' })
    expect(stopping.phase).toBe('stopping')
    const finished = run(stopping, done('v-1', 'Hei der', true))
    expect([finished.phase, finished.text, finished.level, finished.speech]).toEqual(['cleaning', 'Hei der', 0, false])
    const tidy = run(finished, cleaned('v-1', 'Hei der.', 'Hei der'))
    expect([tidy.phase, tidy.text]).toEqual(['idle', 'Hei der.'])
  })

  it('goes idle at done when nothing is left to clean', () => {
    const state = run(ready, { event: 'start' }, { event: 'started', id: 'v-2' }, done('v-2', 'Hei', false))
    expect([state.phase, state.text]).toEqual(['idle', 'Hei'])
  })

  it('ignores events from another session and adopts the first id while starting', () => {
    const recording = run(ready, { event: 'start' }, { event: 'started', id: 'v-1' })
    expect(run(recording, partial('v-9', 'other', ''))).toBe(recording)
    const adopted = run(ready, { event: 'start' }, { event: 'level', id: 'v-3', level: 0.2, speech: false })
    expect([adopted.id, adopted.level]).toEqual(['v-3', 0.2])
  })

  it('keeps the id when stopped before the session id arrived', () => {
    const state = run(ready, { event: 'start' }, { event: 'stop' }, { event: 'started', id: 'v-4' })
    expect([state.phase, state.id]).toEqual(['stopping', 'v-4'])
  })

  it('fails with the reason the backend gives, and when the start is refused', () => {
    const denied = run(ready, { event: 'start' }, { event: 'failed', id: 'v-5', state: 'failed', error: 'denied', reason: 'microphone' })
    expect([denied.phase, denied.error, denied.reason]).toEqual(['failed', 'denied', 'microphone'])
    const refused = run(ready, { event: 'start' }, { event: 'refused', error: 'Voice is switched off' })
    expect([refused.phase, refused.error]).toEqual(['failed', 'Voice is switched off'])
    expect(run(refused, { event: 'start' }).error).toBeNull()
  })

  it('shows corrected spans followed by the rest of the raw text', () => {
    const state = run(ready, { event: 'start' }, { event: 'started', id: 'v-6' }, final('v-6', 0, 'hei der', 'hei der'), final('v-6', 1, 'hvordan går det', 'hei der hvordan går det'))
    const fixed = run(state, { event: 'corrected', id: 'v-6', from: 0, upto: 0, raw: 'hei der', text: 'Hei der.', ms_corrected: 700 }, partial('v-6', 'hei der hvordan går det og', 'så'))
    expect(voicePreview(fixed)).toBe('Hei der. hvordan går det og så')
  })
})

describe('text helpers', () => {
  it('joins spans with a space or a line break before a list item', () => {
    expect(joinSpans('', ' Hei.')).toBe('Hei.')
    expect(joinSpans('Todo:\n1. Fiks', '2. Test')).toBe('Todo:\n1. Fiks\n2. Test')
    expect(joinSpans('Vi trenger', '25 folk.')).toBe('Vi trenger 25 folk.')
  })

  it('inserts at the cursor with the spaces the neighbours need', () => {
    expect(insertText('Hei', 3, 3, 'der.')).toEqual({ value: 'Hei der.', caret: 8 })
    expect(insertText('Hei !', 4, 4, 'der')).toEqual({ value: 'Hei der!', caret: 7 })
    expect(insertText('Hei du', 3, 3, 'der')).toEqual({ value: 'Hei der du', caret: 7 })
    expect(insertText('', 0, 0, 'Hei.')).toEqual({ value: 'Hei.', caret: 4 })
    expect(replaceText('Før. hei der etter', 'hei der', 'Hei der.')).toBe('Før. Hei der. etter')
    expect(replaceText('Endret', 'hei der', 'Hei der.')).toBeNull()
  })

  it('reads hotkeys by physical key', () => {
    expect(parseHotkey('alt+space')).toEqual({ code: 'Space', meta: false, shift: false, alt: true, ctrl: false })
    expect(parseHotkey('off')).toBeNull()
    expect(formatHotkey('meta+shift+period')).toBe('⌘⇧.')
    expect(hotkeyMatches({ code: 'Space', metaKey: false, shiftKey: false, altKey: true, ctrlKey: false }, 'alt+space')).toBe(true)
    expect(hotkeyMatches({ code: 'Space', metaKey: true, shiftKey: false, altKey: true, ctrlKey: false }, 'alt+space')).toBe(false)
  })
})

describe('voice store', () => {
  const handlers: Record<string, (..._args: [{ payload: unknown }]) => void> = {}
  const setup = (ready: boolean): VoiceSetup => ({
    settings: { enabled: ready, model: '/m.bin', vad: '/v.bin', language: 'auto', cleanup: 'live', hotkey: 'alt+space', model_url: 'https://example.org/m.bin', vad_url: 'https://example.org/v.bin', debounce_ms: 1200 },
    models: [],
    unmet: ready ? [] : [{ key: 'voice.enabled', label: 'Voice switched on' }],
    ready,
    microphone: 'authorized',
    cleanup_available: true,
    setting_up: false,
    sessions: []
  })
  let enabled = true

  beforeEach(() => {
    vi.mocked(listen).mockImplementation((name, handler) => {
      handlers[name as string] = handler as (..._args: [{ payload: unknown }]) => void
      return Promise.resolve(() => {})
    })
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'get_voice_setup') return setup(enabled)
      if (command === 'post_voice') return 'v-10'
      if (command === 'delete_voice') return 'v-10'
      throw new Error(`Unmocked invoke call: ${command}`)
    })
  })

  it('links to the setting voice needs while switched off', async () => {
    enabled = false
    await voice.load()
    expect([voice.phase, voice.ready, voice.requirement, voice.setupHref]).toEqual(['off', false, 'voice.enabled', '/main/profile/voice#voice.enabled'])
    await voice.start()
    expect(voice.phase).toBe('off')
  })

  it('starts, streams, stops and hands the text to the composer', async () => {
    enabled = true
    const inserted: string[] = []
    const replaced: [string, string][] = []
    const target = { insert: (text: string) => inserted.push(text), replace: (a: string, b: string) => (replaced.push([a, b]), true) }
    await voice.load()
    await voice.start(target, 'c-1')
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('post_voice', { conversationId: 'c-1' })
    expect([voice.phase, voice.recording]).toEqual(['recording', true])
    handlers[VOICE_EVENT]({ payload: { event: 'level', id: 'v-10', level: 0.5, speech: true } })
    handlers[VOICE_EVENT]({ payload: partial('v-10', 'hei', 'der') })
    expect([voice.level, voice.preview]).toEqual([0.5, 'hei der'])
    await voice.stop()
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('delete_voice', { voiceId: 'v-10' })
    expect(voice.phase).toBe('stopping')
    handlers[VOICE_EVENT]({ payload: done('v-10', 'hei der', true) })
    expect([voice.phase, inserted]).toEqual(['cleaning', ['hei der']])
    handlers[VOICE_EVENT]({ payload: cleaned('v-10', 'Hei der.', 'hei der') })
    expect([voice.phase, voice.text, replaced]).toEqual(['idle', 'Hei der.', [['hei der', 'Hei der.']]])
  })

  it('reports a refused start as failed', async () => {
    enabled = true
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'get_voice_setup') return setup(true)
      throw 'Chatnip may not use the microphone'
    })
    await voice.load()
    await voice.start({ insert: () => {} })
    expect([voice.phase, voice.error]).toEqual(['failed', 'Chatnip may not use the microphone'])
  })
})
