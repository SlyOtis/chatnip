import { describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import { caseSignal, dateValue, describeExpectation, endOfDay, filterCases, formatArrival, formatDue, memberships, sortExpectations, timelineRuns, waitingCount } from './cases'
import type { CaseExpectation, CaseFile, Message } from './types'

function expectation(overrides: Partial<CaseExpectation>): CaseExpectation {
  return {
    id: 'e',
    case_id: 'c',
    title: 'Signed contract',
    kind: 'file',
    from_match: null,
    subject_match: null,
    filename_match: null,
    mime_match: null,
    due_at: null,
    status: 'pending',
    arrived_message_id: null,
    arrived_attachment_name: null,
    arrived_at: null,
    arrived_from: null,
    arrived_conversation_id: null,
    created_at: '2026-10-01T00:00:00Z',
    ...overrides
  }
}

function caseFile(overrides: Partial<CaseFile>): CaseFile {
  return {
    id: 'c',
    profile_id: 'p',
    name: 'Case',
    description: '',
    status: 'open',
    color: '#5a57d6',
    due_at: null,
    items: [],
    expectations: [],
    rules: [],
    conversation_count: 0,
    message_count: 0,
    pending_count: 0,
    arrived_count: 0,
    overdue_count: 0,
    last_activity: null,
    created_at: '2026-10-01T00:00:00Z',
    updated_at: '2026-10-01T00:00:00Z',
    ...overrides
  }
}

function message(id: string, conversation_id: string): Message {
  return { id, conversation_id, timestamp: '2026-10-01T00:00:00Z' } as Message
}

describe('formatArrival', () => {
  it('reads as arrived date from sender', () => {
    const arrived = expectation({ status: 'arrived', arrived_at: '2026-10-03T09:00:00Z', arrived_from: 'Anna Berg' })
    expect(formatArrival(arrived)).toBe('arrived 3 Oct from Anna Berg')
  })

  it('is empty while pending', () => {
    expect(formatArrival(expectation({}))).toBe('')
  })
})

describe('formatDue', () => {
  const now = new Date(2026, 9, 9, 12)
  it('names today, tomorrow and overdue days', () => {
    expect(formatDue(expectation({ due_at: new Date(2026, 9, 9, 18).toISOString() }), now)).toBe('due today')
    expect(formatDue(expectation({ due_at: new Date(2026, 9, 10, 9).toISOString() }), now)).toBe('due tomorrow')
    expect(formatDue(expectation({ due_at: new Date(2026, 9, 6, 9).toISOString() }), now)).toBe('3 days overdue')
    expect(formatDue(expectation({}), now)).toBe('')
  })
})

describe('case helpers', () => {
  it('counts pending and overdue as waiting', () => {
    expect(waitingCount(caseFile({ pending_count: 2, overdue_count: 1 }))).toBe(3)
  })

  it('signals overdue or waiting cases unless done', () => {
    expect(caseSignal(caseFile({ overdue_count: 1 }))).toBe(true)
    expect(caseSignal(caseFile({ status: 'waiting' }))).toBe(true)
    expect(caseSignal(caseFile({ status: 'done', overdue_count: 1 }))).toBe(false)
    expect(caseSignal(caseFile({}))).toBe(false)
  })

  it('filters by name and description', () => {
    const cases = [caseFile({ id: 'a', name: 'Kontrakt Kunde AS' }), caseFile({ id: 'b', name: 'Hiring', description: 'kunde onboarding' }), caseFile({ id: 'c', name: 'Other' })]
    expect(filterCases(cases, 'kunde').map((c) => c.id)).toEqual(['a', 'b'])
    expect(filterCases(cases, '  ').length).toBe(3)
  })

  it('finds every case a conversation or message lives in', () => {
    const item = (conversation_id: string | null, message_id: string | null) => ({ id: 'i', case_id: 'x', conversation_id, message_id, note: null, title: '', conversation_type: null, added_at: '' })
    const cases = [
      caseFile({ id: 'a', items: [item('c1', null)] }),
      caseFile({ id: 'b', items: [item('c1', null), item(null, 'm9')] }),
      caseFile({ id: 'c', items: [item(null, 'm9')] }),
      caseFile({ id: 'd', items: [item('c2', null)] })
    ]
    expect([...memberships(cases, ['c1'])].sort()).toEqual(['a', 'b'])
    expect([...memberships(cases, [], ['m9'])].sort()).toEqual(['b', 'c'])
  })

  it('orders overdue, pending, then arrived', () => {
    const list = [expectation({ id: '1', status: 'arrived' }), expectation({ id: '2', status: 'pending' }), expectation({ id: '3', status: 'overdue' })]
    expect(sortExpectations(list).map((e) => e.id)).toEqual(['3', '2', '1'])
  })

  it('describes matchers in one line', () => {
    expect(describeExpectation(expectation({ filename_match: 'contract*.pdf', from_match: 'client.example' }))).toBe('contract*.pdf · from client.example')
  })

  it('groups consecutive messages from one conversation into runs', () => {
    const runs = timelineRuns([message('1', 'a'), message('2', 'a'), message('3', 'b'), message('4', 'a')], new Map([['a', 'Outlook thread'], ['b', 'Gmail thread']]))
    expect(runs.map((r) => [r.conversationName, r.messages.length])).toEqual([['Outlook thread', 2], ['Gmail thread', 1], ['Outlook thread', 1]])
  })
})

describe('due dates', () => {
  it('stores the end of the picked local day and reads it back', () => {
    const stored = endOfDay('2026-10-12')!
    expect(new Date(stored).getHours()).toBe(23)
    expect(dateValue(stored)).toBe('2026-10-12')
    expect(endOfDay('')).toBeNull()
    expect(dateValue(null)).toBe('')
  })
})
