import { describe, expect, it } from 'vitest'
import { flag, groupExpandedKey, isLayoutKey, sectionCollapsedKey, sectionIconKey, slug, viewIconKey } from './layoutSettings'
import { searchIcons } from './icons'

describe('layout setting keys', () => {
  it('builds stable keys the backend accepts', () => {
    expect(viewIconKey('Inbox')).toBe('sidebar.icon.inbox')
    expect(sectionIconKey('Teams and channels')).toBe('list.section.teams-and-channels.icon')
    expect(sectionCollapsedKey('Direct messages')).toBe('list.section.direct-messages.collapsed')
    expect(groupExpandedKey('a1B2')).toBe('list.group.a1b2.expanded')
    expect(slug('Løpende saker')).toBe('l-pende-saker')
    expect(isLayoutKey('sidebar.icon.inbox')).toBe(true)
    expect(isLayoutKey('sidebar.')).toBe(false)
    expect(isLayoutKey('appearance.global')).toBe(false)
  })

  it('reads flags only when set to true', () => {
    expect(flag({ a: 'true' }, 'a')).toBe(true)
    expect(flag({ a: 'false' }, 'a')).toBe(false)
    expect(flag({}, 'a')).toBe(false)
  })
})

describe('icon search', () => {
  const names = ['lucide:inbox', 'lucide:mail', 'lucide:mail-open', 'lucide:mailbox', 'lucide:star', 'lucide:message-square']
  it('matches every word and prefers names that start with the query', () => {
    expect(searchIcons(names, 'mail')).toEqual(['lucide:mail', 'lucide:mailbox', 'lucide:mail-open'])
    expect(searchIcons(names, 'mail open')).toEqual(['lucide:mail-open'])
    expect(searchIcons(names, '')).toHaveLength(6)
    expect(searchIcons(names, 'zzz')).toEqual([])
  })
})

describe('sidebar section settings', () => {
  it('maps sort choices to inbox orderings', async () => {
    const { sortOrder, sortKey, countKey, sidebarCollapsedKey } = await import('./layoutSettings')
    expect(sortOrder('newest')).toEqual({ sort: 'activity', direction: 'desc' })
    expect(sortOrder('sender')).toEqual({ sort: 'name', direction: 'asc' })
    expect(sortOrder('manual')).toBeNull()
    expect(sortOrder(undefined)).toBeNull()
    expect(sortKey('Inbox')).toBe('sidebar.sort.inbox')
    expect(countKey('g1')).toBe('sidebar.count.g1')
    expect(sidebarCollapsedKey('Tags')).toBe('sidebar.section.tags.collapsed')
  })
})

describe('message settings', () => {
  it('falls back to defaults and turns line counts into a height', async () => {
    const { messageSetting, collapseLimitRem, isLayoutKey } = await import('./layoutSettings')
    expect(messageSetting({}, 'messages.enter_sends')).toBe('enter')
    expect(messageSetting({ 'messages.enter_sends': 'mod' }, 'messages.enter_sends')).toBe('mod')
    expect(collapseLimitRem({})).toBe(22)
    expect(collapseLimitRem({ 'messages.collapse_lines': '0' })).toBe(0)
    expect(isLayoutKey('messages.emoji_size')).toBe(true)
  })
})

describe('section breaks', () => {
  it('breaks on a new day, on long gaps, and never when off', async () => {
    const { sectionBreak } = await import('./layoutSettings')
    expect(sectionBreak({}, null, '2026-10-14T10:00:00')).toBe('none')
    expect(sectionBreak({}, '2026-10-13T23:50:00', '2026-10-14T00:05:00')).toBe('day')
    expect(sectionBreak({}, '2026-10-14T10:00:00', '2026-10-14T10:30:00')).toBe('none')
    expect(sectionBreak({}, '2026-10-14T10:00:00', '2026-10-14T11:15:00')).toBe('gap')
    expect(sectionBreak({ 'messages.section_gap': '15' }, '2026-10-14T10:00:00', '2026-10-14T10:20:00')).toBe('gap')
    expect(sectionBreak({ 'messages.section_gap': 'day' }, '2026-10-14T10:00:00', '2026-10-14T18:00:00')).toBe('none')
    expect(sectionBreak({ 'messages.section_gap': 'off' }, '2026-10-13T10:00:00', '2026-10-14T10:00:00')).toBe('none')
  })
})
