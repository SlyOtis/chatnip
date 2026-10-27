import type { Stats, PersonStat, TimelinePoint, Origin, StatsRangeInput, Ranking, RankingEntry, GroupStat, ResponseBucket, WeekdayReply, KindStat, FileTypeStat } from '$lib/tauri/types'
import { formatDate, formatNumber, t, type MessageKey } from '$lib/i18n.svelte'
import { en } from '$lib/i18n/en'

export type RangePreset = '7d' | '30d' | '90d' | 'year' | 'custom'

export interface Range {
  start: string
  end: string
  preset: RangePreset
}

export function weekdayLabel(day: number, style: 'short' | 'long' = 'short'): string {
  return formatDate(new Date(2024, 0, 1 + day), { weekday: style })
}

export function segments(text: string): { text: string; slot: string | null }[] {
  return text.split(/(\{\w+\})/).filter(Boolean).map((part) => (/^\{\w+\}$/.test(part) ? { text: part, slot: part.slice(1, -1) } : { text: part, slot: null }))
}

function known(key: string): key is MessageKey {
  return key in en
}

function iso(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}

function local(day: string): Date {
  const [y, m, d] = day.split('-').map(Number)
  return new Date(y, m - 1, d)
}

export function presetRange(preset: RangePreset, now = new Date()): Range {
  const end = new Date(now)
  end.setHours(23, 59, 59, 999)
  const start = new Date(now)
  start.setHours(0, 0, 0, 0)
  switch (preset) {
    case '7d':
      start.setDate(start.getDate() - 6)
      break
    case '30d':
      start.setDate(start.getDate() - 29)
      break
    case '90d':
      start.setDate(start.getDate() - 89)
      break
    case 'year':
      start.setMonth(0, 1)
      break
    default:
      start.setDate(start.getDate() - 29)
  }
  return { start: iso(start), end: iso(end), preset }
}

export function previousRange(range: Range): Range {
  const start = local(range.start)
  const end = local(range.end)
  const days = Math.max(1, Math.round((end.getTime() - start.getTime()) / 86400000) + 1)
  const prevEnd = new Date(start)
  prevEnd.setDate(prevEnd.getDate() - 1)
  const prevStart = new Date(prevEnd)
  prevStart.setDate(prevStart.getDate() - days + 1)
  return { start: iso(prevStart), end: iso(prevEnd), preset: 'custom' }
}

export function rangeDays(range: { start: string; end: string }): number {
  return Math.max(1, Math.round((local(range.end).getTime() - local(range.start).getTime()) / 86400000) + 1)
}

export function rangeInput(range: Range, filters: Partial<StatsRangeInput> = {}): StatsRangeInput {
  return { start: range.start, end: range.end, ...filters }
}

export function compact(value: number): string {
  if (!Number.isFinite(value)) return '0'
  const abs = Math.abs(value)
  const fixed = (n: number, digits: number) => formatNumber(n, { minimumFractionDigits: digits, maximumFractionDigits: digits })
  if (abs >= 1_000_000) return t('stats.unit.million', { value: fixed(value / 1_000_000, abs >= 10_000_000 ? 0 : 1) })
  if (abs >= 10_000) return t('stats.unit.thousand', { value: fixed(Math.round(value / 1000), 0) })
  if (abs >= 1_000) return t('stats.unit.thousand', { value: fixed(value / 1000, 1) })
  return formatNumber(Math.round(value))
}

export function minutes(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return '—'
  if (value < 1) return t('stats.unit.lessThanMinute')
  const m = (count: number) => t('stats.unit.minutes', { count })
  const h = (count: number) => t('stats.unit.hours', { count })
  const d = (count: number) => t('stats.unit.days', { count })
  if (value < 60) return m(Math.round(value))
  if (value < 60 * 24) {
    const hours = Math.floor(value / 60)
    const rest = Math.round(value % 60)
    return rest ? `${h(hours)} ${m(rest)}` : h(hours)
  }
  const days = Math.floor(value / 1440)
  const hours = Math.round((value % 1440) / 60)
  return hours ? `${d(days)} ${h(hours)}` : d(days)
}

export function hourLabel(hour: number | null | undefined): string {
  if (hour === null || hour === undefined) return '—'
  return `${String(hour).padStart(2, '0')}:00`
}

export function percent(part: number, whole: number): string {
  return formatNumber(whole ? Math.round((part / whole) * 100) / 100 : 0, { style: 'percent' })
}

export function bucket<T>(items: T[], key: (item: T) => string): Map<string, T[]> {
  const out = new Map<string, T[]>()
  for (const item of items) {
    const k = key(item)
    const list = out.get(k) ?? []
    list.push(item)
    out.set(k, list)
  }
  return out
}

export type PeopleSortKey = 'messages' | 'characters' | 'avg_length' | 'median_response_minutes' | 'my_median_response_minutes' | 'busiest_hour' | 'name'

export function sortPeople(people: PersonStat[], key: PeopleSortKey, direction: 'asc' | 'desc'): PersonStat[] {
  const sign = direction === 'asc' ? 1 : -1
  return [...people].sort((a, b) => {
    if (key === 'name') return sign * a.name.localeCompare(b.name)
    const av = a[key]
    const bv = b[key]
    if (av === null || av === undefined) return 1
    if (bv === null || bv === undefined) return -1
    return sign * (av - bv)
  })
}

export interface Diff {
  before: number
  after: number
  delta: number
  ratio: number | null
}

export function diff(after: number, before: number): Diff {
  return { before, after, delta: after - before, ratio: before === 0 ? null : (after - before) / before }
}

export function deltaLabel(d: Diff): string {
  if (d.ratio === null) return d.after === 0 ? '±0' : t('stats.delta.new')
  return formatNumber(Math.round(d.ratio * 100) / 100, { style: 'percent', signDisplay: 'exceptZero' })
}

export function heatmapMax(grid: number[][]): number {
  return Math.max(1, ...grid.flat())
}

export function wordScale(words: { word: string; count: number }[]): (count: number) => number {
  const max = Math.max(1, ...words.map((w) => w.count))
  const min = Math.min(max, ...words.map((w) => w.count))
  return (count: number) => {
    if (max === min) return 15
    return 13 + ((count - min) / (max - min)) * 4
  }
}

export function originTotals(origins: Origin[]): { countries: number; cities: number; messages: number } {
  const countries = new Set(origins.map((o) => o.country).filter(Boolean))
  const cities = new Set(origins.map((o) => `${o.city}|${o.country}`))
  return { countries: countries.size, cities: cities.size, messages: origins.reduce((sum, o) => sum + o.count, 0) }
}

export function leadSentence(stats: Stats): { top: PersonStat | null; fastest: PersonStat | null; longest: PersonStat | null; peakHour: number; peakDay: number } {
  const humans = stats.by_person.filter((p) => p.messages > 0)
  const top = [...humans].sort((a, b) => b.messages - a.messages)[0] ?? null
  const fastest = [...humans].filter((p) => p.median_response_minutes !== null).sort((a, b) => (a.median_response_minutes ?? 0) - (b.median_response_minutes ?? 0))[0] ?? null
  const longest = [...humans].sort((a, b) => b.avg_length - a.avg_length)[0] ?? null
  const peakHour = stats.by_hour.indexOf(Math.max(...stats.by_hour))
  const peakDay = stats.by_weekday.indexOf(Math.max(...stats.by_weekday))
  return { top, fastest, longest, peakHour: Math.max(0, peakHour), peakDay: Math.max(0, peakDay) }
}

function seeded(seed: number): () => number {
  let s = seed >>> 0
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0
    return s / 4294967296
  }
}

export function demoStats(range: { start: string; end: string }): Stats {
  const rand = seeded(42)
  const days = rangeDays(range)
  const names = ['Lea Lindqvist', 'Eva Ortega', 'Sam Hale', 'Mike Vance', 'Nora Gale', 'Max Sutton', 'Oscar Strand', 'Hugo Brandt', 'Ray Dalton', 'Toby Stone']
  const providers = ['microsoft', 'slack', 'google', 'imap']
  const by_person = names.map((name, i) => {
    const messages = Math.round(20 + rand() * 180 * (1 - i / names.length))
    const avg_length = Math.round(40 + rand() * 260)
    return {
      person_id: `demo-${i}`,
      name,
      messages,
      characters: messages * avg_length,
      avg_length,
      to_me: Math.round(messages * 0.55),
      from_me: Math.round(messages * 0.45),
      median_response_minutes: Math.round(5 + rand() * 600),
      my_median_response_minutes: Math.round(5 + rand() * 300),
      first_seen: range.start,
      last_seen: range.end,
      provider_types: providers.slice(0, 1 + Math.floor(rand() * 3)),
      busiest_hour: 8 + Math.floor(rand() * 10),
      busiest_weekday: Math.floor(rand() * 5)
    }
  })
  const heatmap = Array.from({ length: 7 }, (_, d) =>
    Array.from({ length: 24 }, (_, h) => {
      const work = d < 5 && h >= 7 && h <= 18 ? 1 : 0.15
      const peak = Math.exp(-Math.pow((h - 10.5) / 3.2, 2)) + 0.6 * Math.exp(-Math.pow((h - 14.5) / 2.5, 2))
      return Math.round(rand() * 3 + work * peak * 24)
    })
  )
  const by_hour = Array.from({ length: 24 }, (_, h) => heatmap.reduce((s, row) => s + row[h], 0))
  const by_weekday = heatmap.map((row) => row.reduce((s, v) => s + v, 0))
  const timeline: TimelinePoint[] = Array.from({ length: days }, (_, i) => {
    const date = local(range.start)
    date.setDate(date.getDate() + i)
    const weekend = date.getDay() === 0 || date.getDay() === 6
    const base = weekend ? 4 : 22
    const received = Math.round(base * 1.4 * (0.4 + rand() * 0.6))
    const age = days - 1 - i
    return { date: iso(date), sent: Math.round(base * (0.4 + rand() * 0.6)), received, unread: age < 10 ? Math.round(received * rand() * (1 - age / 10) * 0.5) : rand() > 0.85 ? 1 : 0 }
  })
  const messages = timeline.reduce((s, p) => s + p.sent + p.received, 0)
  const origins: Origin[] = [
    { ip: '185.12.0.1', lat: 59.91, lon: 10.75, city: 'Oslo', country: 'Norway', count: 148, person_name: 'Lea Lindqvist', provider_type: 'microsoft', last_seen: range.end },
    { ip: '52.96.0.1', lat: 47.61, lon: -122.33, city: 'Seattle', country: 'United States', count: 62, person_name: null, provider_type: 'microsoft', last_seen: range.end },
    { ip: '140.82.0.1', lat: 37.77, lon: -122.42, city: 'San Francisco', country: 'United States', count: 41, person_name: 'GitHub', provider_type: 'imap', last_seen: range.end },
    { ip: '13.69.0.1', lat: 52.37, lon: 4.9, city: 'Amsterdam', country: 'Netherlands', count: 27, person_name: null, provider_type: 'google', last_seen: range.end },
    { ip: '91.108.0.1', lat: 55.68, lon: 12.57, city: 'Copenhagen', country: 'Denmark', count: 19, person_name: 'Toby Stone', provider_type: 'slack', last_seen: range.end },
    { ip: '103.21.0.1', lat: 1.35, lon: 103.82, city: 'Singapore', country: 'Singapore', count: 9, person_name: null, provider_type: 'imap', last_seen: range.end },
    { ip: '200.3.0.1', lat: -23.55, lon: -46.63, city: 'São Paulo', country: 'Brazil', count: 5, person_name: null, provider_type: 'google', last_seen: range.end }
  ]
  return {
    range,
    totals: {
      messages,
      sent: timeline.reduce((s, p) => s + p.sent, 0),
      received: timeline.reduce((s, p) => s + p.received, 0),
      conversations: 23,
      people: by_person.length,
      services: 6,
      attachments: 31,
      characters: by_person.reduce((s, p) => s + p.characters, 0),
      human_messages: Math.round(timeline.reduce((s, p) => s + p.received, 0) * 0.82),
      service_messages: timeline.reduce((s, p) => s + p.received, 0) - Math.round(timeline.reduce((s, p) => s + p.received, 0) * 0.82),
      median_response_minutes: 47,
      my_median_response_minutes: 23
    },
    by_person,
    by_provider: [
      { provider_id: 'p1', provider_type: 'microsoft', name: 'Northwind', messages: Math.round(messages * 0.52), conversations: 11, people: 7, unread: 3 },
      { provider_id: 'p2', provider_type: 'slack', name: 'Sundial', messages: Math.round(messages * 0.27), conversations: 6, people: 5, unread: 1 },
      { provider_id: 'p3', provider_type: 'google', name: 'Gmail', messages: Math.round(messages * 0.14), conversations: 4, people: 4, unread: 0 },
      { provider_id: 'p4', provider_type: 'imap', name: 'Mail', messages: Math.round(messages * 0.07), conversations: 2, people: 2, unread: 0 }
    ],
    by_conversation: [
      { conversation_id: 'c1', name: 'Start of Week Sync', type: 'group_message', messages: 142, people: 6, characters: 18400, last_activity: range.end },
      { conversation_id: 'c2', name: 'Website relaunch', type: 'group_message', messages: 98, people: 4, characters: 22100, last_activity: range.end },
      { conversation_id: 'c3', name: 'Sundial Design', type: 'channel', messages: 77, people: 5, characters: 9800, last_activity: range.end },
      { conversation_id: 'c4', name: 'Lea Lindqvist', type: 'direct_message', messages: 61, people: 2, characters: 7100, last_activity: range.end },
      { conversation_id: 'c5', name: 'C-level', type: 'group_message', messages: 44, people: 3, characters: 5300, last_activity: range.end }
    ],
    by_label: [
      { tag_id: 'l1', name: 'Important', color: '#f5b84b', conversations: 8, messages: Math.round(messages * 0.5) },
      { tag_id: 'l2', name: 'Team', color: '#4f8fd6', conversations: 9, messages: Math.round(messages * 0.38) },
      { tag_id: 'l3', name: 'Newsletters', color: '#8bc38a', conversations: 3, messages: Math.round(messages * 0.07) },
      { tag_id: 'l4', name: 'Notifications', color: '#9b8ee8', conversations: 3, messages: Math.round(messages * 0.05) }
    ],
    by_hour,
    by_weekday,
    heatmap,
    timeline,
    response_times: [
      { label: 'under 5m', minutes: 5, count: 62, theirs: 24, mine: 38 },
      { label: '15m', minutes: 15, count: 71, theirs: 33, mine: 38 },
      { label: '1h', minutes: 60, count: 58, theirs: 31, mine: 27 },
      { label: '4h', minutes: 240, count: 37, theirs: 22, mine: 15 },
      { label: '1d', minutes: 1440, count: 21, theirs: 14, mine: 7 },
      { label: '3d', minutes: 4320, count: 9, theirs: 7, mine: 2 },
      { label: 'longer', minutes: Number.MAX_SAFE_INTEGER, count: 4, theirs: 3, mine: 1 }
    ],
    rankings: demoRankings(by_person),
    groups: demoGroups(by_person, heatmap),
    by_kind: [
      { kind: 'chat', messages: Math.round(messages * 0.41), sent: Math.round(messages * 0.2), received: Math.round(messages * 0.21) },
      { kind: 'channel', messages: Math.round(messages * 0.27), sent: Math.round(messages * 0.09), received: Math.round(messages * 0.18) },
      { kind: 'mail', messages: Math.round(messages * 0.17), sent: Math.round(messages * 0.05), received: Math.round(messages * 0.12) },
      { kind: 'notification', messages: Math.round(messages * 0.09), sent: 0, received: Math.round(messages * 0.09) },
      { kind: 'bot', messages: Math.round(messages * 0.04), sent: 0, received: Math.round(messages * 0.04) },
      { kind: 'invite', messages: Math.round(messages * 0.02), sent: 0, received: Math.round(messages * 0.02) }
    ],
    by_file_type: [
      { kind: 'image', files: 14, bytes: 9_400_000 },
      { kind: 'pdf', files: 9, bytes: 6_100_000 },
      { kind: 'document', files: 5, bytes: 1_200_000 },
      { kind: 'spreadsheet', files: 3, bytes: 640_000 },
      { kind: 'calendar', files: 2, bytes: 8_000 }
    ],
    reply_by_weekday: [58, 41, 37, 44, 96, 240, 310].map((theirs, weekday) => ({ weekday, theirs, mine: Math.round(theirs * (weekday < 5 ? 0.45 : 0.8)), replies: weekday < 5 ? 30 + weekday * 3 : 6 })),
    words: [
      ['møte', 61], ['design', 48], ['deploy', 41], ['kunde', 37], ['pr', 35], ['review', 31], ['release', 28], ['demo', 26], ['fredag', 24], ['sprint', 22], ['faktura', 19], ['api', 18], ['kalender', 16], ['tilbud', 14], ['ferie', 12]
    ].map(([word, count]) => ({ word: String(word), count: Number(count) })),
    origins
  }
}

export function rankingValue(entry: RankingEntry): string {
  switch (entry.unit) {
    case 'minutes':
      return minutes(entry.value)
    case 'percent':
      return formatNumber(Math.round(entry.value) / 100, { style: 'percent' })
    case 'characters':
      return compact(entry.value)
    default:
      return compact(entry.value)
  }
}

export interface RankingPosition {
  key: string
  title: string
  position: number
  of: number
  value: string
}

export function rankingPositions(rankings: Ranking[], personId: string): RankingPosition[] {
  const out: RankingPosition[] = []
  for (const ranking of rankings) {
    const index = ranking.entries.findIndex((e) => e.person_id === personId)
    if (index < 0) continue
    out.push({ key: ranking.key, title: rankingTitle(ranking), position: index + 1, of: ranking.entries.length, value: rankingValue(ranking.entries[index]) })
  }
  return out.sort((a, b) => a.position - b.position || a.title.localeCompare(b.title))
}

const BUCKET_ORDER = ['people', 'teams', 'services', 'mail']
const KIND_ORDER: GroupStat['kind'][] = ['bucket', 'split', 'tag', 'group']

export function orderGroups(groups: GroupStat[]): GroupStat[] {
  return [...groups].sort((a, b) => {
    const kind = KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind)
    if (kind !== 0) return kind
    if (a.kind === 'bucket') {
      const ai = BUCKET_ORDER.indexOf(a.key.replace(/^bucket:/, ''))
      const bi = BUCKET_ORDER.indexOf(b.key.replace(/^bucket:/, ''))
      if (ai !== bi) return (ai < 0 ? 99 : ai) - (bi < 0 ? 99 : bi)
    }
    return b.messages - a.messages || a.name.localeCompare(b.name)
  })
}

export function groupSentence(group: GroupStat): string {
  const parts = [
    t('stats.count.messages', { count: group.messages, value: compact(group.messages) }),
    t('stats.count.conversations', { count: group.conversations, value: compact(group.conversations) }),
    t('stats.count.people', { count: group.people, value: compact(group.people) })
  ]
  if (group.median_response_minutes !== null) parts.push(t('stats.groups.repliesIn', { time: minutes(group.median_response_minutes) }))
  if (group.busiest_weekday !== null && group.busiest_hour !== null) parts.push(t('stats.groups.busiest', { weekday: weekdayLabel(group.busiest_weekday), hour: hourLabel(group.busiest_hour) }))
  return parts.join(' · ')
}

export function rankingTitle(ranking: { key: string; title: string }): string {
  const key = `stats.ranking.${ranking.key}.title`
  return known(key) ? t(key) : ranking.title
}

export function rankingDescription(ranking: { key: string; description: string }): string {
  const key = `stats.ranking.${ranking.key}.description`
  return known(key) ? t(key) : ranking.description
}

export function groupName(group: GroupStat): string {
  const key = `stats.groups.bucket.${group.key.replace(/^bucket:/, '')}`
  return group.kind === 'bucket' && known(key) ? t(key) : group.name
}

export function conversationTypeLabel(type: string): string {
  const key = `stats.conversation.type.${type}`
  return known(key) ? t(key) : type.replace('_', ' ')
}

export function demoRankings(people: PersonStat[]): Ranking[] {
  const entry = (p: PersonStat, value: number, unit: RankingEntry['unit'], detail: string | null = null): RankingEntry => ({ person_id: p.person_id, name: p.name, value, unit, detail })
  const top = (pick: (p: PersonStat) => number | null, unit: RankingEntry['unit'], asc = false, detail?: (p: PersonStat) => string) =>
    people
      .map((p) => ({ p, v: pick(p) }))
      .filter((x): x is { p: PersonStat; v: number } => x.v !== null)
      .sort((a, b) => (asc ? a.v - b.v : b.v - a.v))
      .slice(0, 10)
      .map((x) => entry(x.p, x.v, unit, detail ? detail(x.p) : null))
  return [
    { key: 'most_messages', title: 'Writes most often', description: 'Messages in the range', entries: top((p) => p.messages, 'messages') },
    { key: 'most_text', title: 'Writes the most text', description: 'Characters in the range', entries: top((p) => p.characters, 'characters') },
    { key: 'longest', title: 'Longest messages', description: 'Average characters per message', entries: top((p) => p.avg_length, 'characters') },
    { key: 'shortest', title: 'Shortest messages', description: 'Average characters per message', entries: top((p) => p.avg_length, 'characters', true) },
    { key: 'fastest_reply', title: 'Fastest reply', description: 'Median time until they answer', entries: top((p) => p.median_response_minutes, 'minutes', true) },
    { key: 'slowest_reply', title: 'Slowest reply', description: 'Median time until they answer', entries: top((p) => p.median_response_minutes, 'minutes') },
    { key: 'i_answer_fastest', title: 'You answer fastest', description: 'Median time until you reply', entries: top((p) => p.my_median_response_minutes, 'minutes', true) },
    { key: 'most_to_me', title: 'Sends you the most', description: 'Messages addressed to you', entries: top((p) => p.to_me, 'messages') },
    { key: 'you_write_most', title: 'You write the most to', description: 'Messages from you', entries: top((p) => p.from_me, 'messages') },
    { key: 'share', title: 'Share of all traffic', description: 'Percent of messages', entries: top((p) => Math.round((p.messages / Math.max(1, people.reduce((s, q) => s + q.messages, 0))) * 100), 'percent') },
    { key: 'early_birds', title: 'Earliest in the day', description: 'Busiest hour', entries: top((p) => p.busiest_hour, 'count', true, (p) => hourLabel(p.busiest_hour)) },
    { key: 'night_owls', title: 'Latest in the day', description: 'Busiest hour', entries: top((p) => p.busiest_hour, 'count', false, (p) => hourLabel(p.busiest_hour)) }
  ]
}

export function demoGroups(people: PersonStat[], heatmap: number[][]): GroupStat[] {
  const by_hour = Array.from({ length: 24 }, (_, h) => heatmap.reduce((s, row) => s + row[h], 0))
  const by_weekday = heatmap.map((row) => row.reduce((s, v) => s + v, 0))
  const scale = (factor: number) => ({ by_hour: by_hour.map((v) => Math.round(v * factor)), by_weekday: by_weekday.map((v) => Math.round(v * factor)) })
  const top = (list: PersonStat[]): RankingEntry[] => list.slice(0, 5).map((p) => ({ person_id: p.person_id, name: p.name, value: p.messages, unit: 'messages', detail: null }))
  const sorted = [...people].sort((a, b) => b.messages - a.messages)
  const group = (key: string, kind: GroupStat['kind'], name: string, color: string | null, factor: number, list: PersonStat[], hour: number, day: number): GroupStat => ({
    key,
    kind,
    name,
    color,
    messages: list.reduce((s, p) => s + p.messages, 0),
    conversations: Math.max(1, Math.round(list.length * 1.4)),
    people: list.length,
    characters: list.reduce((s, p) => s + p.characters, 0),
    median_response_minutes: list.length ? Math.round(list.reduce((s, p) => s + (p.median_response_minutes ?? 0), 0) / list.length) : null,
    busiest_hour: hour,
    busiest_weekday: day,
    top: top(list),
    ...scale(factor)
  })
  return [
    group('bucket:people', 'bucket', 'People', null, 0.6, sorted, 10, 1),
    group('bucket:teams', 'bucket', 'Teams and channels', null, 0.3, sorted.slice(0, 6), 14, 2),
    group('bucket:services', 'bucket', 'Services', null, 0.05, sorted.slice(7), 9, 0),
    group('bucket:mail', 'bucket', 'Mail', null, 0.15, sorted.slice(2, 8), 8, 3),
    group('split:l1', 'split', 'Important', '#f5b84b', 0.5, sorted.slice(0, 5), 11, 1),
    group('split:l2', 'split', 'Team', '#4f8fd6', 0.38, sorted.slice(3, 9), 15, 2),
    group('tag:l3', 'tag', 'Newsletters', '#8bc38a', 0.07, sorted.slice(8), 7, 4),
    group('group:g1', 'group', 'Kunde: Tidewater', '#5a57d6', 0.2, sorted.slice(1, 4), 13, 3)
  ]
}

export interface Side {
  label: string
  stats: Stats
  range: { start: string; end: string }
}

export function dayList(range: { start: string; end: string }): string[] {
  const out: string[] = []
  const day = local(range.start)
  const end = local(range.end)
  while (day.getTime() <= end.getTime() && out.length < 3700) {
    out.push(iso(day))
    day.setDate(day.getDate() + 1)
  }
  return out
}

export function fillDays(points: TimelinePoint[], range: { start: string; end: string }): TimelinePoint[] {
  const known = new Map(points.map((p) => [p.date, p]))
  return dayList(range).map((date) => known.get(date) ?? { date, sent: 0, received: 0, unread: 0 })
}

export interface VolumePoint {
  date: string
  other: string | null
  values: [number, number]
}

export interface Span {
  points: TimelinePoint[]
  range: { start: string; end: string }
}

export function volumeSeries(a: Span, b: Span | null): VolumePoint[] {
  const left = fillDays(a.points, a.range)
  if (!b) return left.map((p) => ({ date: p.date, other: null, values: [p.received, p.sent] }))
  const right = fillDays(b.points, b.range)
  return left.map((p, i) => ({ date: p.date, other: right[i]?.date ?? null, values: [p.received + p.sent, right[i] ? right[i].received + right[i].sent : 0] }))
}

export function volumeTotal(p: VolumePoint): number {
  return p.other === null ? p.values[0] + p.values[1] : p.values[0]
}

export function volumePeak(points: VolumePoint[]): VolumePoint | null {
  const best = points.reduce<VolumePoint | null>((top, p) => (!top || volumeTotal(p) > volumeTotal(top) ? p : top), null)
  return best && volumeTotal(best) > 0 ? best : null
}

export function bucketLabel(buckets: ResponseBucket[], index: number): string {
  const current = buckets[index]?.minutes ?? 0
  const previous = index > 0 ? buckets[index - 1].minutes : null
  if (previous === null) return t('stats.bucket.under', { time: minutes(current) })
  if (index === buckets.length - 1 && current >= 1e9) return t('stats.bucket.over', { time: minutes(previous) })
  return t('stats.bucket.between', { from: minutes(previous), to: minutes(current) })
}

export interface ReplyRow {
  key: string
  label: string
  counts: [number, number]
  shares: [number, number]
}

export function replySeries(a: ResponseBucket[], b: ResponseBucket[] | null): ReplyRow[] {
  const counts: [number, number][] = a.map((bucket, i) => (b ? [bucket.count, b[i]?.count ?? 0] : [bucket.theirs, bucket.mine]))
  const totals = [0, 1].map((side) => counts.reduce((s, c) => s + c[side], 0))
  return a.map((bucket, i) => ({
    key: String(bucket.minutes),
    label: bucketLabel(a, i),
    counts: counts[i],
    shares: [totals[0] ? counts[i][0] / totals[0] : 0, totals[1] ? counts[i][1] / totals[1] : 0]
  }))
}

export function withinShare(rows: ReplyRow[], limit: number, side: 0 | 1): number {
  const total = rows.reduce((s, r) => s + r.counts[side], 0)
  if (!total) return 0
  return rows.filter((r) => Number(r.key) <= limit).reduce((s, r) => s + r.counts[side], 0) / total
}

export interface Habits {
  days: number
  active: number
  streak: number
  quiet: number
  afterHours: number
  weekend: number
  perActiveDay: number
}

export function habits(stats: Stats, range: { start: string; end: string }): Habits {
  const days = fillDays(stats.timeline, range)
  let active = 0
  let streak = 0
  let quiet = 0
  let run = 0
  let gap = 0
  let total = 0
  for (const day of days) {
    const count = day.sent + day.received
    total += count
    if (count > 0) {
      active += 1
      run += 1
      gap = 0
    } else {
      gap += 1
      run = 0
    }
    streak = Math.max(streak, run)
    quiet = Math.max(quiet, gap)
  }
  let all = 0
  let outside = 0
  let weekend = 0
  stats.heatmap.forEach((row, day) =>
    row.forEach((value, hour) => {
      all += value
      if (day >= 5) weekend += value
      if (day >= 5 || hour < 8 || hour >= 17) outside += value
    })
  )
  return { days: days.length, active, streak, quiet, afterHours: all ? outside / all : 0, weekend: all ? weekend / all : 0, perActiveDay: active ? total / active : 0 }
}

export const BACKLOG_AGES = [
  { key: 'today', max: 0 },
  { key: 'days', max: 2 },
  { key: 'week', max: 7 },
  { key: 'month', max: 30 },
  { key: 'older', max: Infinity }
] as const

export type BacklogAge = (typeof BACKLOG_AGES)[number]['key']

export function backlog(points: TimelinePoint[], today = new Date()): { key: BacklogAge; value: number }[] {
  const now = local(iso(today)).getTime()
  const out = BACKLOG_AGES.map((age) => ({ key: age.key, value: 0 }))
  for (const p of points) {
    if (!p.unread) continue
    const age = Math.max(0, Math.round((now - local(p.date).getTime()) / 86400000))
    const slot = BACKLOG_AGES.findIndex((a) => age <= a.max)
    out[slot < 0 ? out.length - 1 : slot].value += p.unread
  }
  return out
}

export type MixKey = 'mine' | 'people' | 'services'

export function senderMix(stats: Stats): { key: MixKey; value: number; share: number }[] {
  const parts: [MixKey, number][] = [
    ['mine', stats.totals.sent],
    ['people', stats.totals.human_messages],
    ['services', stats.totals.service_messages]
  ]
  const total = parts.reduce((s, [, v]) => s + v, 0)
  return parts.map(([key, value]) => ({ key, value, share: total ? value / total : 0 }))
}

export interface PairRow {
  key: string
  value: number
  other?: number
}

export function pairRows(a: { key: string; value: number }[], b: { key: string; value: number }[] | null): PairRow[] {
  const keys = [...a.map((r) => r.key), ...(b ?? []).map((r) => r.key).filter((k) => !a.some((r) => r.key === k))]
  return keys
    .map((key) => {
      const value = a.find((r) => r.key === key)?.value ?? 0
      return b ? { key, value, other: b.find((r) => r.key === key)?.value ?? 0 } : { key, value }
    })
    .sort((x, y) => y.value - x.value || (y.other ?? 0) - (x.other ?? 0) || x.key.localeCompare(y.key))
}

export function kindRows(a: KindStat[], b: KindStat[] | null): PairRow[] {
  return pairRows(a.map((k) => ({ key: k.kind, value: k.messages })), b ? b.map((k) => ({ key: k.kind, value: k.messages })) : null)
}

export function fileRows(a: FileTypeStat[], b: FileTypeStat[] | null): PairRow[] {
  return pairRows(a.map((f) => ({ key: f.kind, value: f.files })), b ? b.map((f) => ({ key: f.kind, value: f.files })) : null)
}

export interface WeekdayReplyRow {
  key: string
  weekday: number
  value: number
  other: number
}

export function weekdayReplyRows(a: WeekdayReply[], b: WeekdayReply[] | null): WeekdayReplyRow[] {
  return a.map((day, i) => ({ key: String(day.weekday), weekday: day.weekday, value: (b ? day.mine : day.theirs) ?? 0, other: (b ? b[i]?.mine : day.mine) ?? 0 }))
}

export function fastestWeekday(rows: WeekdayReply[]): WeekdayReply | null {
  return rows.filter((r) => r.mine !== null).sort((x, y) => (x.mine ?? 0) - (y.mine ?? 0))[0] ?? null
}

export function bytes(value: number): string {
  if (value >= 1_000_000_000) return t('stats.unit.gigabytes', { value: formatNumber(value / 1_000_000_000, { maximumFractionDigits: 1 }) })
  if (value >= 1_000_000) return t('stats.unit.megabytes', { value: formatNumber(value / 1_000_000, { maximumFractionDigits: 1 }) })
  if (value >= 1_000) return t('stats.unit.kilobytes', { value: formatNumber(Math.round(value / 1_000)) })
  return t('stats.unit.bytes', { value: formatNumber(value) })
}

export function rampColor(value: number, max: number): string {
  if (value <= 0 || max <= 0) return 'var(--paper-2)'
  const level = Math.min(1, value / max)
  if (level <= 0.5) return `color-mix(in oklab, var(--ramp-mid) ${Math.round(level * 200)}%, var(--ramp-low))`
  return `color-mix(in oklab, var(--ramp-high) ${Math.round((level - 0.5) * 200)}%, var(--ramp-mid))`
}

export function kindLabel(kind: string): string {
  const key = `stats.kind.${kind}`
  return known(key) ? t(key) : kind
}

export function fileLabel(kind: string): string {
  const key = `stats.file.${kind}`
  return known(key) ? t(key) : kind
}
