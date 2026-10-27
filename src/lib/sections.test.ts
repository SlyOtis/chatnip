import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'

vi.mock('$lib/tauri/api', () => ({
  getAppSettings: vi.fn(),
  putAppSetting: vi.fn(() => Promise.resolve()),
  getSidebarLayout: vi.fn(),
  putSidebarLayout: vi.fn(),
  getSectionSummaries: vi.fn(),
  getSettingRegistry: vi.fn(() => Promise.resolve({ defaults: { 'summaries.concurrency': '3' }, values: { 'summaries.concurrency': '3' }, features: [] })),
  postSectionSummary: vi.fn(),
  postContent: vi.fn()
}))

import { getSettingRegistry, postSectionSummary, putAppSetting } from '$lib/tauri/api'
import { layoutSettings } from '$lib/layoutSettings'
import type { Message, PostSectionSummaryInput, SectionSummaries, SectionSummary } from '$lib/tauri/types'
import {
  SUMMARY_CONCURRENCY_KEY,
  allSectionsCollapsed,
  chooseSummary,
  collapsedSections,
  deleteWarning,
  putSectionCollapsed,
  sections,
  sectionsKey,
  splitSections,
  summarizeAll,
  summaryKey,
  summarySnippet,
  toggleAllSections,
  toggleSection,
  type Timeline
} from './sections.svelte'

const file = { id: 'f1', name: 'notes.pdf', content_type: 'application/pdf', size: 1, is_inline: false, content_id: null }
const message = (id: string, timestamp: string, extra: Partial<Message> = {}): Message => ({ id, timestamp, conversation_id: 'c1', sender_id: 'u1', sender_name: 'Kari', is_read: true, attachments: [file], ...extra }) as Message

const day = [
  message('a', '2026-10-12T09:00:00'),
  message('b', '2026-10-12T09:05:00'),
  message('c', '2026-10-13T10:00:00'),
  message('d', '2026-10-13T12:30:00'),
  message('e', '2026-10-14T08:00:00')
]

function plan(list: Message[] = day, settings: Record<string, string> = {}) {
  return splitSections(settings, list, (m) => ({ key: m.id, first: m.timestamp, last: m.timestamp, messages: [m] }))
}

function timeline(list: Message[] = day): Timeline {
  return { scope: 'c1', source: { kind: 'conversation', id: 'c1' }, conversationId: 'c1', title: 'Avtale', sections: plan(list).sections }
}

function summary(input: PostSectionSummaryInput): SectionSummary {
  return { id: input.start_message_id, scope: input.scope, kind: input.kind, start_message_id: input.start_message_id, end_message_id: input.end_message_id, content_hash: 'h', message_count: input.message_ids.length, summary: `Sum ${input.start_message_id}`, action_items: [], connection: 'Claude', model: null, created_at: '', updated_at: '', stale: false }
}

function found(input: PostSectionSummaryInput): SectionSummaries {
  return { summaries: [summary(input)], requirements: [] }
}

beforeEach(() => {
  layoutSettings.set({})
  sections.timeline = null
  sections.summaries = {}
  sections.running = {}
  sections.failures = {}
  sections.requirements = []
  sections.warned = []
  sections.choosing = false
  vi.mocked(putAppSetting).mockClear()
  vi.mocked(postSectionSummary).mockReset()
})

afterEach(() => vi.useRealTimers())

describe('splitting a timeline', () => {
  it('opens a section at every day and gap break', () => {
    const { sections: found, owner, headed } = plan()
    expect(found.map((s) => [s.id, s.end, s.kind, s.messages.length])).toEqual([
      ['a', 'b', 'none', 2],
      ['c', 'c', 'day', 1],
      ['d', 'd', 'gap', 1],
      ['e', 'e', 'day', 1]
    ])
    expect(owner.get('b')?.id).toBe('a')
    expect(headed).toBe(true)
    expect(plan(day, { 'messages.section_gap': 'off' }).sections.map((s) => s.id)).toEqual(['a'])
    expect(plan(day, { 'messages.section_gap': 'off' }).headed).toBe(false)
  })
})

describe('section collapse state', () => {
  it('persists per conversation and section start', async () => {
    await putSectionCollapsed('c1', 'c', true)
    expect(putAppSetting).toHaveBeenLastCalledWith(sectionsKey('c1'), JSON.stringify(['c']))
    expect(sectionsKey('person:P 1')).toBe('messages.sections.person-p-1.collapsed')
    await toggleSection('c1', 'a')
    expect(collapsedSections(get(layoutSettings), 'c1')).toEqual(new Set(['c', 'a']))
    expect(collapsedSections(get(layoutSettings), 'c2')).toEqual(new Set())
    await toggleSection('c1', 'c')
    await toggleSection('c1', 'a')
    expect(putAppSetting).toHaveBeenLastCalledWith(sectionsKey('c1'), '')
    expect(get(layoutSettings)[sectionsKey('c1')]).toBeUndefined()
  })

  it('reads a broken value as nothing collapsed', () => {
    expect(collapsedSections({ [sectionsKey('c1')]: 'not json' }, 'c1')).toEqual(new Set())
  })

  it('collapses all then expands all', async () => {
    sections.timeline = timeline()
    const ids = sections.timeline.sections.map((s) => s.id)
    await putSectionCollapsed('c1', 'c', true)
    expect(allSectionsCollapsed(get(layoutSettings), 'c1', ids)).toBe(false)
    await toggleAllSections()
    expect(collapsedSections(get(layoutSettings), 'c1')).toEqual(new Set(ids))
    expect(allSectionsCollapsed(get(layoutSettings), 'c1', ids)).toBe(true)
    await toggleAllSections()
    expect(collapsedSections(get(layoutSettings), 'c1')).toEqual(new Set())
    expect(allSectionsCollapsed(get(layoutSettings), 'c1', [])).toBe(false)
  })
})

describe('summarize all', () => {
  it('runs each section separately within the concurrency limit', async () => {
    const list = Array.from({ length: 7 }, (_, i) => message(`m${i}`, `2026-10-${String(10 + i).padStart(2, '0')}T09:00:00`))
    sections.timeline = timeline(list)
    let open = 0
    let peak = 0
    vi.mocked(postSectionSummary).mockImplementation(async (input) => {
      open++
      peak = Math.max(peak, open)
      await new Promise((resolve) => setTimeout(resolve, 5))
      open--
      return found(input)
    })
    chooseSummary()
    expect(sections.choosing).toBe(true)
    await summarizeAll('each')
    expect(sections.choosing).toBe(false)
    const calls = vi.mocked(postSectionSummary).mock.calls.map(([input]) => input)
    expect(calls.map((c) => [c.kind, c.start_message_id, c.message_ids])).toEqual(list.map((m) => ['section', m.id, [m.id]]))
    expect(calls.every((c) => c.scope === 'c1' && c.conversation_id === 'c1' && !c.force)).toBe(true)
    expect(peak).toBeLessThanOrEqual(3)
    expect(peak).toBeGreaterThan(1)
    expect(getSettingRegistry).toHaveBeenCalled()
    expect(sections.summaries[summaryKey('c1', 'section', 'm3')]?.summary).toBe('Sum m3')
    expect(Object.keys(sections.running)).toEqual([])
  })

  it('takes the concurrency limit from the settings registry', async () => {
    sections.timeline = timeline(Array.from({ length: 4 }, (_, i) => message(`n${i}`, `2026-10-${String(10 + i).padStart(2, '0')}T09:00:00`)))
    vi.mocked(getSettingRegistry).mockResolvedValueOnce({ defaults: { [SUMMARY_CONCURRENCY_KEY]: '3' }, values: { [SUMMARY_CONCURRENCY_KEY]: '1' }, features: [] })
    let open = 0
    let peak = 0
    vi.mocked(postSectionSummary).mockImplementation(async (input) => {
      open++
      peak = Math.max(peak, open)
      await new Promise((resolve) => setTimeout(resolve, 2))
      open--
      return found(input)
    })
    await summarizeAll('each', true)
    expect(peak).toBe(1)
  })

  it('skips sections with a fresh summary unless forced', async () => {
    sections.timeline = timeline()
    sections.summaries[summaryKey('c1', 'section', 'a')] = summary({ scope: 'c1', kind: 'section', start_message_id: 'a', end_message_id: 'b', message_ids: ['a', 'b'], conversation_id: 'c1', title: null, force: false })
    sections.summaries[summaryKey('c1', 'section', 'c')] = { ...summary({ scope: 'c1', kind: 'section', start_message_id: 'c', end_message_id: 'c', message_ids: ['c'], conversation_id: 'c1', title: null, force: false }), stale: true }
    vi.mocked(postSectionSummary).mockImplementation(async (input) => found(input))
    await summarizeAll('each')
    expect(vi.mocked(postSectionSummary).mock.calls.map(([input]) => input.start_message_id)).toEqual(['c', 'd', 'e'])
    vi.mocked(postSectionSummary).mockClear()
    await summarizeAll('each', true)
    expect(vi.mocked(postSectionSummary).mock.calls.map(([input]) => [input.start_message_id, input.force])).toEqual([['a', true], ['c', true], ['d', true], ['e', true]])
  })

  it('runs one summary over everything shown', async () => {
    sections.timeline = timeline()
    vi.mocked(postSectionSummary).mockImplementation(async (input) => found(input))
    await summarizeAll('all')
    expect(vi.mocked(postSectionSummary).mock.calls.map(([input]) => [input.kind, input.start_message_id, input.end_message_id, input.message_ids])).toEqual([['all', 'a', 'e', ['a', 'b', 'c', 'd', 'e']]])
    expect(sections.summaries[summaryKey('c1', 'all', 'a')]?.kind).toBe('all')
  })

  it('leaves out a single message without a file', async () => {
    sections.timeline = timeline([message('p1', '2026-10-10T09:00:00', { attachments: [] }), message('p2', '2026-10-12T09:00:00')])
    vi.mocked(postSectionSummary).mockImplementation(async (input) => found(input))
    await summarizeAll('each')
    expect(vi.mocked(postSectionSummary).mock.calls.map(([input]) => input.start_message_id)).toEqual(['p2'])
  })

  it('keeps the failure on the section it belongs to', async () => {
    sections.timeline = timeline(day.slice(0, 2))
    vi.mocked(postSectionSummary).mockRejectedValue('Claude answered with something unreadable: <html>')
    await summarizeAll('each')
    expect(sections.failures[summaryKey('c1', 'section', 'a')]).toBe('That did not work. Try again in a moment.')
    expect(sections.warned).toEqual([])
  })

  it('turns a failure a setting fixes into a warning instead of an error', async () => {
    sections.timeline = timeline()
    vi.mocked(postSectionSummary).mockResolvedValue({ summaries: [], requirements: [{ severity: 'requirement', message: 'Claude Code signed in', node: null, edge: null, line: null, column: null, setting: 'ai.claude' }] })
    await summarizeAll('each')
    expect(vi.mocked(postSectionSummary).mock.calls.length).toBeLessThanOrEqual(3)
    expect(sections.warned).toEqual([{ setting: 'ai.claude', label: 'Claude Code signed in', href: '/main/profile/ai#ai.claude' }])
    expect(sections.failures).toEqual({})
    expect(deleteWarning()).toBe('/main/profile/ai#ai.claude')
    expect(sections.warned).toEqual([])
  })

  it('warns on the first click when a requirement is known up front', async () => {
    sections.timeline = timeline()
    sections.requirements = [{ setting: 'ai.connection', label: 'An AI connection', href: '/main/profile/ai#ai.connection' }]
    chooseSummary()
    expect(sections.choosing).toBe(false)
    expect(sections.warned).toEqual(sections.requirements)
    await summarizeAll('all')
    expect(postSectionSummary).not.toHaveBeenCalled()
  })

  it('does nothing without a timeline', async () => {
    chooseSummary()
    expect(sections.choosing).toBe(false)
    await summarizeAll('all')
    expect(postSectionSummary).not.toHaveBeenCalled()
  })
})

describe('saving a summary to Content', () => {
  it('keeps the conversation and the section start as provenance', () => {
    const tl = timeline()
    const saved = summarySnippet(tl, { ...summary({ scope: 'c1', kind: 'section', start_message_id: 'c', end_message_id: 'c', message_ids: ['c'], conversation_id: 'c1', title: null, force: false }), summary: 'Pris {{ fast }}', action_items: ['Svar Kari'] }, 'Avtale · Tuesday 13 October and more')
    expect(saved).toEqual({ kind: 'snippet', name: 'Avtale · Tuesday 13 October and more'.slice(0, 48), body: 'Pris {{ "{{" }} fast }}\n- Svar Kari', keywords: ['summary', 'section'], source_message_id: 'c', source_kind: 'conversation', source_conversation_id: 'c1' })
    expect(summarySnippet({ ...tl, source: { kind: 'person', id: 'p1' } }, summary({ scope: 'person:p1', kind: 'all', start_message_id: 'a', end_message_id: 'e', message_ids: [], conversation_id: null, title: null, force: false }), 'Kari')).toMatchObject({ source_kind: 'person', source_person_id: 'p1', source_conversation_id: 'c1', source_message_id: 'a' })
  })
})
