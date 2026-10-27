import { describe, expect, it } from 'vitest'
import { backlog, bucket, bucketLabel, compact, deltaLabel, demoStats, diff, fastestWeekday, fileRows, fillDays, groupSentence, habits, heatmapMax, kindLabel, kindRows, leadSentence, minutes, orderGroups, pairRows, presetRange, previousRange, rampColor, rangeDays, rankingPositions, rankingValue, replySeries, senderMix, sortPeople, volumePeak, volumeSeries, weekdayReplyRows, withinShare, wordScale } from './stats'
import type { ResponseBucket, Stats, TimelinePoint } from '$lib/tauri/types'

const now = new Date('2026-10-09T12:00:00')

describe('ranges', () => {
  it('presets end today and start the right number of days back', () => {
    expect(presetRange('7d', now)).toMatchObject({ start: '2026-10-03', end: '2026-10-09' })
    expect(presetRange('30d', now).start).toBe('2026-09-10')
    expect(presetRange('90d', now).start).toBe('2026-07-12')
    expect(presetRange('year', now).start).toBe('2026-01-01')
  })
  it('previous range has the same length and ends the day before', () => {
    const range = presetRange('7d', now)
    const previous = previousRange(range)
    expect(rangeDays(previous)).toBe(7)
    expect(previous.end).toBe('2026-10-02')
    expect(previous.start).toBe('2026-09-26')
  })
})

describe('formatting', () => {
  it('compacts numbers', () => {
    expect(compact(950)).toBe('950')
    expect(compact(1250)).toBe('1.3K')
    expect(compact(12500)).toBe('13K')
    expect(compact(2_400_000)).toBe('2.4M')
  })
  it('formats minutes as durations', () => {
    expect(minutes(null)).toBe('—')
    expect(minutes(0.4)).toBe('<1m')
    expect(minutes(42)).toBe('42m')
    expect(minutes(135)).toBe('2h 15m')
    expect(minutes(1500)).toBe('1d 1h')
  })
})

describe('series', () => {
  it('buckets by key', () => {
    const groups = bucket([{ k: 'a' }, { k: 'b' }, { k: 'a' }], (i) => i.k)
    expect(groups.get('a')?.length).toBe(2)
    expect(groups.get('b')?.length).toBe(1)
  })
  it('sorts people with nulls last', () => {
    const people = demoStats({ start: '2026-09-10', end: '2026-10-09' }).by_person
    people[0].median_response_minutes = null
    const sorted = sortPeople(people, 'median_response_minutes', 'asc')
    expect(sorted[sorted.length - 1].median_response_minutes).toBeNull()
    expect(sortPeople(people, 'messages', 'desc')[0].messages).toBe(Math.max(...people.map((p) => p.messages)))
  })
  it('diffs and labels deltas', () => {
    expect(deltaLabel(diff(120, 100))).toBe('+20%')
    expect(deltaLabel(diff(80, 100))).toBe('-20%')
    expect(deltaLabel(diff(5, 0))).toBe('new')
    expect(deltaLabel(diff(0, 0))).toBe('±0')
  })
  it('scales words between 13 and 17 px', () => {
    const scale = wordScale([{ word: 'a', count: 1 }, { word: 'b', count: 11 }])
    expect(scale(1)).toBe(13)
    expect(scale(11)).toBe(17)
  })
})

describe('demo stats', () => {
  it('is consistent across its own totals', () => {
    const stats = demoStats({ start: '2026-09-10', end: '2026-10-09' })
    expect(stats.timeline.length).toBe(30)
    expect(stats.totals.sent + stats.totals.received).toBe(stats.totals.messages)
    expect(stats.heatmap.length).toBe(7)
    expect(stats.heatmap[0].length).toBe(24)
    expect(heatmapMax(stats.heatmap)).toBeGreaterThan(0)
    const lead = leadSentence(stats)
    expect(lead.top?.name).toBeTruthy()
    expect(lead.peakHour).toBeGreaterThanOrEqual(0)
  })
})

describe('rankings and groups', () => {
  const stats = demoStats({ start: '2026-09-10', end: '2026-10-09' })
  it('formats ranking values by unit', () => {
    expect(rankingValue({ person_id: 'a', name: 'A', value: 68, unit: 'minutes', detail: null })).toBe('1h 8m')
    expect(rankingValue({ person_id: 'a', name: 'A', value: 42, unit: 'percent', detail: null })).toBe('42%')
    expect(rankingValue({ person_id: 'a', name: 'A', value: 12500, unit: 'characters', detail: null })).toBe('13K')
    expect(rankingValue({ person_id: 'a', name: 'A', value: 7, unit: 'messages', detail: null })).toBe('7')
  })
  it('finds a person across every ranking, best positions first', () => {
    const top = stats.rankings[0].entries[0]
    const positions = rankingPositions(stats.rankings, top.person_id!)
    expect(positions[0].position).toBe(1)
    expect(positions.every((p, i, all) => i === 0 || all[i - 1].position <= p.position)).toBe(true)
    expect(positions.find((p) => p.key === 'most_messages')).toMatchObject({ position: 1, of: 10 })
    expect(rankingPositions(stats.rankings, 'nobody')).toEqual([])
  })
  it('orders groups buckets first in the fixed order, then splits, tags and own groups', () => {
    const keys = orderGroups([...stats.groups].reverse()).map((g) => g.key)
    expect(keys.slice(0, 4)).toEqual(['bucket:people', 'bucket:teams', 'bucket:services', 'bucket:mail'])
    expect(keys.indexOf('split:l1')).toBeLessThan(keys.indexOf('tag:l3'))
    expect(keys.indexOf('tag:l3')).toBeLessThan(keys.indexOf('group:g1'))
  })
  it('demo rankings and groups are consistent with the people', () => {
    expect(stats.rankings.length).toBe(12)
    for (const ranking of stats.rankings) expect(ranking.entries.length).toBeLessThanOrEqual(10)
    const people = stats.groups.find((g) => g.key === 'bucket:people')!
    expect(people.messages).toBe(stats.by_person.reduce((s, p) => s + p.messages, 0))
    expect(people.by_hour).toHaveLength(24)
    expect(people.by_weekday).toHaveLength(7)
    expect(groupSentence(people)).toContain('messages')
  })
})

const day = (date: string, received: number, sent: number, unread = 0): TimelinePoint => ({ date, received, sent, unread })

describe('volume over time', () => {
  const range = { start: '2026-10-01', end: '2026-10-05' }
  it('fills every day of the range with zeros where nothing moved', () => {
    const filled = fillDays([day('2026-10-02', 3, 1), day('2026-10-04', 2, 0)], range)
    expect(filled.map((p) => p.date)).toEqual(['2026-10-01', '2026-10-02', '2026-10-03', '2026-10-04', '2026-10-05'])
    expect(filled.map((p) => p.received)).toEqual([0, 3, 0, 2, 0])
  })
  it('without a comparison carries received and sent per day', () => {
    const series = volumeSeries({ points: [day('2026-10-02', 3, 1)], range }, null)
    expect(series).toHaveLength(5)
    expect(series[1]).toEqual({ date: '2026-10-02', other: null, values: [3, 1] })
    expect(volumePeak(series)?.date).toBe('2026-10-02')
  })
  it('overlays the previous period day by day on the same axis', () => {
    const before = { start: '2026-09-26', end: '2026-09-30' }
    const series = volumeSeries({ points: [day('2026-10-01', 2, 2), day('2026-10-05', 1, 0)], range }, { points: [day('2026-09-26', 4, 1), day('2026-09-29', 0, 3)], range: before })
    expect(series.map((p) => p.values)).toEqual([[4, 5], [0, 0], [0, 0], [0, 3], [1, 0]])
    expect(series[0].other).toBe('2026-09-26')
    expect(series[4].other).toBe('2026-09-30')
  })
  it('overlays two people on the same dates', () => {
    const series = volumeSeries({ points: [day('2026-10-03', 5, 0)], range }, { points: [day('2026-10-03', 1, 1)], range })
    expect(series[2]).toEqual({ date: '2026-10-03', other: '2026-10-03', values: [5, 2] })
    expect(volumePeak(volumeSeries({ points: [], range }, null))).toBeNull()
  })
})

const buckets: ResponseBucket[] = [
  { label: 'under 5m', minutes: 5, count: 6, theirs: 2, mine: 4 },
  { label: '1h', minutes: 60, count: 3, theirs: 2, mine: 1 },
  { label: 'longer', minutes: Number.MAX_SAFE_INTEGER, count: 1, theirs: 1, mine: 0 }
]

describe('reply times', () => {
  it('labels buckets from their bounds', () => {
    expect(bucketLabel(buckets, 0)).toBe('under 5m')
    expect(bucketLabel(buckets, 1)).toBe('5m–1h')
    expect(bucketLabel(buckets, 2)).toBe('over 1h')
  })
  it('splits theirs and mine without a comparison', () => {
    const rows = replySeries(buckets, null)
    expect(rows.map((r) => r.counts)).toEqual([[2, 4], [2, 1], [1, 0]])
    expect(rows[0].shares[1]).toBeCloseTo(0.8)
    expect(withinShare(rows, 60, 1)).toBe(1)
    expect(withinShare(rows, 60, 0)).toBeCloseTo(0.8)
  })
  it('compares two sides as shares so different volumes line up', () => {
    const other = buckets.map((b) => ({ ...b, count: b.count * 10 }))
    const rows = replySeries(buckets, other)
    expect(rows.map((r) => r.counts)).toEqual([[6, 60], [3, 30], [1, 10]])
    expect(rows.map((r) => r.shares[0])).toEqual(rows.map((r) => r.shares[1]))
  })
  it('reads reply time by weekday as theirs and mine, or mine against the other side', () => {
    const a = [{ weekday: 0, theirs: 30, mine: 10, replies: 4 }, { weekday: 1, theirs: null, mine: 5, replies: 1 }]
    const b = [{ weekday: 0, theirs: 1, mine: 40, replies: 2 }, { weekday: 1, theirs: null, mine: null, replies: 0 }]
    expect(weekdayReplyRows(a, null).map((r) => [r.value, r.other])).toEqual([[30, 10], [0, 5]])
    expect(weekdayReplyRows(a, b).map((r) => [r.value, r.other])).toEqual([[10, 40], [5, 0]])
    expect(fastestWeekday(a)?.weekday).toBe(1)
    expect(fastestWeekday([{ weekday: 0, theirs: 1, mine: null, replies: 1 }])).toBeNull()
  })
})

function sample(overrides: Partial<Stats> = {}): Stats {
  const base = demoStats({ start: '2026-10-01', end: '2026-10-07' })
  return { ...base, ...overrides }
}

describe('habits, backlog and mix', () => {
  it('counts active days, the longest run, quiet spells and after hours from the heatmap', () => {
    const heatmap = Array.from({ length: 7 }, () => Array(24).fill(0))
    heatmap[0][10] = 6
    heatmap[0][20] = 2
    heatmap[5][11] = 2
    const stats = sample({ heatmap, timeline: [day('2026-10-01', 1, 0), day('2026-10-02', 1, 1), day('2026-10-03', 2, 0), day('2026-10-07', 1, 0)] })
    const h = habits(stats, { start: '2026-10-01', end: '2026-10-07' })
    expect(h).toMatchObject({ days: 7, active: 4, streak: 3, quiet: 3 })
    expect(h.afterHours).toBeCloseTo(0.4)
    expect(h.weekend).toBeCloseTo(0.2)
    expect(h.perActiveDay).toBeCloseTo(6 / 4)
  })
  it('ages unread messages from the day they arrived', () => {
    const today = new Date(2026, 9, 15)
    const ages = backlog([day('2026-10-15', 2, 0, 2), day('2026-10-14', 1, 0, 1), day('2026-10-10', 3, 0, 3), day('2026-09-20', 1, 0, 1), day('2026-07-01', 4, 0, 4), day('2026-10-13', 5, 0, 0)], today)
    expect(ages).toEqual([
      { key: 'today', value: 2 },
      { key: 'days', value: 1 },
      { key: 'week', value: 3 },
      { key: 'month', value: 1 },
      { key: 'older', value: 4 }
    ])
  })
  it('splits traffic into you, people and services', () => {
    const stats = sample()
    stats.totals = { ...stats.totals, sent: 20, human_messages: 60, service_messages: 20 }
    expect(senderMix(stats)).toEqual([
      { key: 'mine', value: 20, share: 0.2 },
      { key: 'people', value: 60, share: 0.6 },
      { key: 'services', value: 20, share: 0.2 }
    ])
  })
})

describe('paired rows', () => {
  it('merges both sides by key, keeps keys only one side has, and sorts by the first side', () => {
    expect(pairRows([{ key: 'a', value: 2 }, { key: 'b', value: 5 }], null)).toEqual([{ key: 'b', value: 5 }, { key: 'a', value: 2 }])
    expect(pairRows([{ key: 'a', value: 2 }], [{ key: 'a', value: 1 }, { key: 'c', value: 4 }])).toEqual([{ key: 'a', value: 2, other: 1 }, { key: 'c', value: 0, other: 4 }])
  })
  it('shapes kinds and files with translatable labels', () => {
    const rows = kindRows([{ kind: 'mail', messages: 3, sent: 1, received: 2 }, { kind: 'chat', messages: 9, sent: 4, received: 5 }], [{ kind: 'bot', messages: 2, sent: 0, received: 2 }])
    expect(rows.map((r) => [r.key, r.value, r.other])).toEqual([['chat', 9, 0], ['mail', 3, 0], ['bot', 0, 2]])
    expect(kindLabel('notification')).toBe('Notifications')
    expect(kindLabel('unknown')).toBe('unknown')
    expect(fileRows([{ kind: 'pdf', files: 2, bytes: 10 }], null)).toEqual([{ key: 'pdf', value: 2 }])
  })
})

describe('accent ramp', () => {
  it('recedes empty cells and climbs from the low step through the accent to the warm end', () => {
    expect(rampColor(0, 10)).toBe('var(--paper-2)')
    expect(rampColor(2.5, 10)).toBe('color-mix(in oklab, var(--ramp-mid) 50%, var(--ramp-low))')
    expect(rampColor(5, 10)).toBe('color-mix(in oklab, var(--ramp-mid) 100%, var(--ramp-low))')
    expect(rampColor(10, 10)).toBe('color-mix(in oklab, var(--ramp-high) 100%, var(--ramp-mid))')
    expect(rampColor(20, 10)).toBe('color-mix(in oklab, var(--ramp-high) 100%, var(--ramp-mid))')
  })
})

describe('demo stats carry the new panels', () => {
  it('has kinds, files, weekday replies and split reply buckets', () => {
    const stats = demoStats({ start: '2026-09-10', end: '2026-10-09' })
    expect(stats.reply_by_weekday).toHaveLength(7)
    expect(stats.response_times.every((b) => b.theirs + b.mine === b.count)).toBe(true)
    expect(stats.by_kind.length).toBeGreaterThan(0)
    expect(stats.totals.human_messages + stats.totals.service_messages).toBe(stats.totals.received)
    expect(compact(stats.by_file_type[0].files)).toBeTruthy()
    expect(minutes(stats.reply_by_weekday[0].mine)).not.toBe('—')
  })
})
