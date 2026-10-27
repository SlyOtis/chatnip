import { get } from 'svelte/store'
import { getSectionSummaries, getSettingRegistry, postContent, postSectionSummary } from '$lib/tauri/api'
import type { ContentItem, Message, PostContentInput, SectionInput, SectionKind, SectionSummary, SummaryOptions } from '$lib/tauri/types'
import { layoutSettings, messageSetting, putLayoutSetting, sectionBreak, slug, type SectionBreak } from '$lib/layoutSettings'
import { requirementsFor, type NodeRequirement } from '$lib/actions'
import { settingNumber } from '$lib/settings'
import { t } from '$lib/i18n.svelte'

export const SUMMARY_CONCURRENCY_KEY = 'summaries.concurrency'

export type SummaryChoice = 'each' | 'all'

export type TimelineSource = { kind: 'conversation'; id: string } | { kind: 'person'; id: string } | { kind: 'case'; id: string } | { kind: 'merged'; id: string }

export interface TimelineSection {
  id: string
  end: string
  kind: SectionBreak
  from: string
  to: string
  messages: Message[]
  keys: string[]
}

export interface SectionPlan {
  sections: TimelineSection[]
  owner: Map<string, TimelineSection>
  headed: boolean
}

export interface Timeline {
  scope: string
  source: TimelineSource
  conversationId: string | null
  title: string | null
  sections: TimelineSection[]
}

export interface SectionEntry {
  key: string
  first: string
  last: string
  messages: Message[]
}

class SectionState {
  timeline = $state.raw<Timeline | null>(null)
  summaries = $state<Record<string, SectionSummary>>({})
  running = $state<Record<string, boolean>>({})
  failures = $state<Record<string, string>>({})
  requirements = $state.raw<NodeRequirement[]>([])
  warned = $state.raw<NodeRequirement[]>([])
  choosing = $state(false)
  selection = $state.raw<TimelineSection | null>(null)
}

export const sections = new SectionState()

export function splitSections<T>(settings: Record<string, string>, items: T[], read: (item: T) => SectionEntry): SectionPlan {
  const out: TimelineSection[] = []
  const owner = new Map<string, TimelineSection>()
  let previous: string | null = null
  for (const item of items) {
    const entry = read(item)
    const kind = sectionBreak(settings, previous, entry.first)
    let current = out[out.length - 1]
    if (!current || kind !== 'none') {
      current = { id: '', end: '', kind: current ? kind : 'none', from: entry.first, to: entry.last, messages: [], keys: [] }
      out.push(current)
    }
    current.keys.push(entry.key)
    current.messages.push(...entry.messages)
    current.to = entry.last
    owner.set(entry.key, current)
    previous = entry.last
  }
  for (const section of out) {
    section.id = section.messages[0]?.id ?? section.keys[0]
    section.end = section.messages[section.messages.length - 1]?.id ?? section.keys[section.keys.length - 1]
  }
  return { sections: out, owner, headed: messageSetting(settings, 'messages.section_gap') !== 'off' }
}

export function sectionsKey(scope: string): string {
  return `messages.sections.${slug(scope)}.collapsed`
}

export function collapsedSections(settings: Record<string, string>, scope: string): Set<string> {
  try {
    const parsed: unknown = JSON.parse(settings[sectionsKey(scope)] ?? '[]')
    return new Set(Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === 'string') : [])
  } catch {
    return new Set()
  }
}

function putCollapsed(scope: string, ids: Set<string>): Promise<void> {
  return putLayoutSetting(sectionsKey(scope), ids.size ? JSON.stringify([...ids]) : null)
}

export function putSectionCollapsed(scope: string, id: string, collapsed: boolean): Promise<void> {
  const ids = collapsedSections(get(layoutSettings), scope)
  if (collapsed) ids.add(id)
  else ids.delete(id)
  return putCollapsed(scope, ids)
}

export function toggleSection(scope: string, id: string): Promise<void> {
  return putSectionCollapsed(scope, id, !collapsedSections(get(layoutSettings), scope).has(id))
}

export function allSectionsCollapsed(settings: Record<string, string>, scope: string, ids: string[]): boolean {
  const collapsed = collapsedSections(settings, scope)
  return ids.length > 0 && ids.every((id) => collapsed.has(id))
}

export function putSectionsCollapsed(scope: string, ids: string[], collapsed: boolean): Promise<void> {
  const current = collapsedSections(get(layoutSettings), scope)
  for (const id of ids) {
    if (collapsed) current.add(id)
    else current.delete(id)
  }
  return putCollapsed(scope, current)
}

export function toggleAllSections(): Promise<void> {
  const timeline = sections.timeline
  if (!timeline || !timeline.sections.length) return Promise.resolve()
  const ids = timeline.sections.map((s) => s.id)
  return putSectionsCollapsed(timeline.scope, ids, !allSectionsCollapsed(get(layoutSettings), timeline.scope, ids))
}

export function summaryKey(scope: string, kind: SectionKind, id: string): string {
  return `${scope}|${kind}|${id}`
}

export function summarisable(messages: Message[]): boolean {
  return messages.length >= 2 || messages.some((m) => m.attachments?.some((a) => !a.is_inline))
}

export function chosen(options: SummaryOptions): boolean {
  return !!(options.focus?.trim() || options.include?.trim() || options.exclude?.trim() || options.without_files)
}

export function sectionInput(kind: SectionKind, list: TimelineSection[]): SectionInput | null {
  const messages = list.flatMap((s) => s.messages)
  if (!messages.length) return null
  return { kind, start_message_id: messages[0].id, end_message_id: messages[messages.length - 1].id, message_ids: messages.map((m) => m.id) }
}

export function timelineInputs(timeline: Timeline): SectionInput[] {
  const own = timeline.sections.map((s) => sectionInput('section', [s])).filter((i): i is SectionInput => !!i)
  const all = sectionInput('all', timeline.sections)
  return all ? [...own, all] : own
}

export function summaryOf(timeline: Timeline | null, kind: SectionKind, section: TimelineSection | null): SectionSummary | null {
  if (!timeline) return null
  const input = kind === 'all' ? sectionInput('all', timeline.sections) : section ? sectionInput('section', [section]) : null
  return input ? sections.summaries[summaryKey(timeline.scope, kind, input.start_message_id)] ?? null : null
}

export async function getTimelineSummaries(timeline: Timeline): Promise<void> {
  const inputs = timelineInputs(timeline)
  if (!inputs.length) return
  const found = await getSectionSummaries(timeline.scope, inputs)
  for (const input of inputs) delete sections.summaries[summaryKey(timeline.scope, input.kind, input.start_message_id)]
  for (const summary of found.summaries) sections.summaries[summaryKey(summary.scope, summary.kind, summary.start_message_id)] = summary
  sections.requirements = requirementsFor(found.requirements)
}

export async function postTimelineSummary(timeline: Timeline, kind: SectionKind, list: TimelineSection[], force = false, options: SummaryOptions = {}): Promise<SectionSummary | null> {
  const input = sectionInput(kind, list)
  if (!input) return null
  const key = summaryKey(timeline.scope, kind, input.start_message_id)
  if (sections.running[key]) return null
  if (sections.requirements.length || sections.warned.length) {
    sections.warned = sections.warned.length ? sections.warned : sections.requirements
    return null
  }
  sections.running[key] = true
  delete sections.failures[key]
  try {
    const found = await postSectionSummary({ ...input, scope: timeline.scope, conversation_id: timeline.conversationId, title: timeline.title, force: force || chosen(options), options })
    const needs = requirementsFor(found.requirements)
    if (needs.length) {
      sections.warned = needs
      return null
    }
    const summary = found.summaries[0] ?? null
    if (summary) sections.summaries[key] = summary
    return summary
  } catch (error) {
    const said = typeof error === 'string' ? error : ''
    sections.failures[key] = said.startsWith('Nothing in that part') ? t('conversation.summarize.nothing') : said.startsWith('A summary needs') ? t('conversation.section.tooSmall') : t('actions.requirement.failed')
    return null
  } finally {
    delete sections.running[key]
  }
}

export async function runLimited<T>(items: T[], limit: number, task: (item: T) => Promise<unknown>): Promise<void> {
  let next = 0
  const worker = async () => {
    while (next < items.length) await task(items[next++])
  }
  await Promise.all(Array.from({ length: Math.max(1, Math.min(limit, items.length)) }, worker))
}

export async function summarizeAll(choice: SummaryChoice, force = false, options: SummaryOptions = {}): Promise<void> {
  const timeline = sections.timeline
  sections.choosing = false
  if (!timeline) return
  if (choice === 'all') {
    await postTimelineSummary(timeline, 'all', timeline.sections, force, options)
    return
  }
  const pending = timeline.sections.filter((s) => summarisable(s.messages) && (force || !summaryOf(timeline, 'section', s) || summaryOf(timeline, 'section', s)?.stale))
  const limit = settingNumber(await getSettingRegistry().catch(() => null), SUMMARY_CONCURRENCY_KEY) || 1
  await runLimited(pending, limit, (s) => (sections.warned.length ? Promise.resolve(null) : postTimelineSummary(timeline, 'section', [s], force, options)))
}

export async function summarizeSelection(messages: Message[], options: SummaryOptions = {}): Promise<SectionSummary | null> {
  const timeline = sections.timeline
  const ordered = [...messages].sort((a, b) => a.timestamp.localeCompare(b.timestamp))
  if (!timeline || !ordered.length) return null
  const section: TimelineSection = { id: `selection:${ordered[0].id}`, end: ordered[ordered.length - 1].id, kind: 'day' as SectionBreak, from: ordered[0].timestamp, to: ordered[ordered.length - 1].timestamp, messages: ordered, keys: ordered.map((m) => m.id) }
  sections.selection = section
  return postTimelineSummary(timeline, 'selection', [section], true, options)
}

export function chooseSummary() {
  if (sections.requirements.length) {
    sections.choosing = false
    sections.warned = sections.requirements
    return
  }
  if (sections.timeline?.sections.length) sections.choosing = !sections.choosing
}

export function deleteWarning(): string | null {
  const href = sections.warned[0]?.href ?? null
  sections.warned = []
  sections.choosing = false
  return href
}

export function summaryText(summary: Pick<SectionSummary, 'summary' | 'action_items'>): string {
  return [summary.summary.trim(), ...summary.action_items.map((item) => `- ${item}`)].join('\n')
}

export function summarySnippet(timeline: Timeline, summary: SectionSummary, name: string): PostContentInput {
  const start = timeline.sections.flatMap((s) => s.messages).find((m) => m.id === summary.start_message_id)
  const origin: Partial<PostContentInput> =
    timeline.source.kind === 'person'
      ? { source_kind: 'person', source_person_id: timeline.source.id, source_conversation_id: start?.conversation_id }
      : timeline.source.kind === 'case' || timeline.source.kind === 'merged'
        ? { source_kind: 'message', source_conversation_id: start?.conversation_id }
        : { source_kind: 'conversation', source_conversation_id: timeline.source.id }
  return {
    kind: 'snippet',
    name: name.slice(0, 48),
    body: summaryText(summary).replace(/\{\{/g, '{{ "{{" }}'),
    keywords: ['summary', summary.kind],
    source_message_id: summary.start_message_id,
    ...origin
  }
}

export function postSummaryContent(timeline: Timeline, summary: SectionSummary, name: string): Promise<ContentItem> {
  return postContent(summarySnippet(timeline, summary, name))
}

export function holdTimeline(read: () => Timeline | null) {
  let scope: string | null = null
  let signature = ''
  $effect(() => {
    const timeline = read()
    scope = timeline?.scope ?? null
    sections.timeline = timeline
    if (!timeline) return
    const next = `${timeline.scope}:${timelineInputs(timeline).map((i) => `${i.kind}.${i.start_message_id}.${i.end_message_id}.${i.message_ids.length}`).join(',')}`
    if (next === signature) return
    const timer = setTimeout(() => {
      signature = next
      getTimelineSummaries(timeline).catch(() => {})
    }, 250)
    return () => clearTimeout(timer)
  })
  $effect(() => () => {
    if (scope && sections.timeline?.scope === scope) {
      sections.timeline = null
      sections.choosing = false
    }
  })
}
