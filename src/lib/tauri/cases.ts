import { derived, writable } from 'svelte/store'
import type { CaseExpectation, CaseFile, CaseStatus, Message } from './types'
import { getCases } from './api'
import { formatDate, t, type MessageKey } from '$lib/i18n.svelte'
import { startOfDay } from '$lib/dates'

export interface CasePickerTarget {
  conversationIds: string[]
  messageIds: string[]
  create?: boolean
}

export const casesStore = writable<CaseFile[]>([])

export const casesById = derived(casesStore, ($cases) => new Map($cases.map((c) => [c.id, c])))

export const casePickerStore = writable<CasePickerTarget | null>(null)

export async function loadCases(): Promise<CaseFile[]> {
  const cases = await getCases()
  casesStore.set(cases)
  return cases
}

export function openCasePicker(target: CasePickerTarget) {
  casePickerStore.set(target)
}

export function closeCasePicker() {
  casePickerStore.set(null)
}

export const CASE_STATUSES: { value: CaseStatus; label: MessageKey }[] = [
  { value: 'open', label: 'cases.status.open' },
  { value: 'waiting', label: 'cases.status.waiting' },
  { value: 'done', label: 'cases.status.done' }
]

export const CASE_COLORS = ['#1f9d55', '#d6573a', '#2f9e6e', '#c9a227', '#3a8fd6', '#b84fb0', '#6b7280'] as const

export function waitingCount(c: CaseFile): number {
  return c.pending_count + c.overdue_count
}

export function caseSignal(c: CaseFile): boolean {
  return c.status !== 'done' && (c.overdue_count > 0 || c.status === 'waiting')
}

export function activeCases(cases: CaseFile[]): CaseFile[] {
  return cases.filter((c) => c.status !== 'done')
}

export function filterCases(cases: CaseFile[], query: string): CaseFile[] {
  const q = query.trim().toLowerCase()
  if (!q) return cases
  return cases.filter((c) => c.name.toLowerCase().includes(q) || c.description.toLowerCase().includes(q))
}

export function memberships(cases: CaseFile[], conversationIds: string[], messageIds: string[] = []): Set<string> {
  const out = new Set<string>()
  for (const c of cases) {
    const hit = c.items.some((item) => (item.conversation_id && conversationIds.includes(item.conversation_id)) || (item.message_id && messageIds.includes(item.message_id)))
    if (hit) out.add(c.id)
  }
  return out
}

export function shortDate(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return formatDate(date, { day: 'numeric', month: 'short' })
}

export function formatArrival(expectation: CaseExpectation): string {
  if (expectation.status !== 'arrived') return ''
  const date = expectation.arrived_at ? shortDate(expectation.arrived_at) : null
  const from = expectation.arrived_from
  if (date && from) return t('cases.arrival.dateFrom', { date, from })
  if (date) return t('cases.arrival.date', { date })
  if (from) return t('cases.arrival.from', { from })
  return t('cases.arrival.plain')
}

export function formatDue(expectation: CaseExpectation | CaseFile, now: Date = new Date()): string {
  if (!expectation.due_at) return ''
  const due = new Date(expectation.due_at)
  if (Number.isNaN(due.getTime())) return ''
  const days = Math.round((startOfDay(due) - startOfDay(now)) / 86_400_000)
  if (days === 0) return t('cases.due.today')
  if (days === 1) return t('cases.due.tomorrow')
  if (days === -1) return t('cases.due.yesterday')
  if (days < 0) return t('cases.due.overdue', { count: -days })
  return t('cases.due.date', { date: shortDate(expectation.due_at) })
}

export function describeExpectation(expectation: CaseExpectation): string {
  const parts: string[] = []
  if (expectation.filename_match) parts.push(expectation.filename_match)
  if (expectation.from_match) parts.push(t('cases.expectation.from', { from: expectation.from_match }))
  if (expectation.subject_match) parts.push(t('cases.expectation.subject', { subject: expectation.subject_match }))
  if (expectation.mime_match) parts.push(expectation.mime_match)
  return parts.join(' · ')
}

export function sortExpectations(expectations: CaseExpectation[]): CaseExpectation[] {
  const rank = { overdue: 0, pending: 1, arrived: 2 } as const
  return [...expectations].sort((a, b) => rank[a.status] - rank[b.status] || (a.due_at ?? '9999').localeCompare(b.due_at ?? '9999') || a.created_at.localeCompare(b.created_at))
}

export interface TimelineRun {
  conversationId: string
  conversationName: string
  messages: Message[]
}

export function timelineRuns(messages: Message[], names: Map<string, string>): TimelineRun[] {
  const runs: TimelineRun[] = []
  for (const message of messages) {
    const last = runs[runs.length - 1]
    if (last && last.conversationId === message.conversation_id) {
      last.messages.push(message)
    } else {
      runs.push({ conversationId: message.conversation_id, conversationName: names.get(message.conversation_id) ?? t('cases.timeline.conversation'), messages: [message] })
    }
  }
  return runs
}

export function dateValue(iso: string | null): string {
  if (!iso) return ''
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return ''
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

export function endOfDay(value: string): string | null {
  if (!value) return null
  const date = new Date(`${value}T23:59:59`)
  return Number.isNaN(date.getTime()) ? null : date.toISOString()
}
