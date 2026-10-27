import { describe, expect, it } from 'vitest'
import { connections, defaultTargets, eventColour, joinUrl, LOCAL_TARGET, meetingKind, meetingKinds, meetingLabel, sourceMarks } from './calendar'
import type { CalendarEvent, EventSource, ProviderCalendar } from '$lib/tauri/types'

function calendar(id: string, overrides: Partial<ProviderCalendar> = {}): ProviderCalendar {
  return { id, profile_id: 'p', provider_id: 'prov-' + id, provider_type: 'microsoft', provider_name: 'Northwind', provider_external_id: null, external_id: 'x-' + id, name: id, colour: null, is_primary: false, can_write: true, enabled: true, ...overrides }
}

function sourceOf(overrides: Partial<EventSource>): EventSource {
  return { event_id: 'e', calendar_id: 'c', calendar_name: 'Work', colour: null, provider_id: 'p', provider_type: 'microsoft', provider_external_id: null, response_status: null, online_meeting_url: null, ...overrides }
}

describe('calendar targets', () => {
  it('knows which meeting each connection can attach', () => {
    expect(meetingKind(calendar('a'))).toBe('teams')
    expect(meetingKind(calendar('b', { provider_type: 'google' }))).toBe('meet')
    expect(meetingKind(calendar('c', { provider_type: 'mock', provider_external_id: 'mock-gmail-me' }))).toBe('meet')
    expect(meetingKind(calendar('d', { provider_type: 'mock', provider_external_id: 'mock-teams-me' }))).toBe('teams')
    expect(meetingKind(calendar('e', { provider_type: 'imap' }))).toBeNull()
  })

  it('labels the meeting toggle by the chosen targets', () => {
    const list = [calendar('a'), calendar('b', { provider_type: 'google' })]
    expect(meetingKinds(list, ['a'])).toEqual(['teams'])
    expect(meetingKinds(list, ['a', 'b', LOCAL_TARGET])).toEqual(['teams', 'meet'])
    expect(meetingLabel(['teams'])).toBe('Teams meeting')
    expect(meetingLabel(['meet'])).toBe('Google Meet')
    expect(meetingLabel(['teams', 'meet'])).toBe('Teams or Meet link')
  })

  it('groups calendars per connection and picks the primary writable one first', () => {
    const list = [calendar('a', { provider_id: 'one', can_write: false }), calendar('b', { provider_id: 'one', is_primary: true }), calendar('c', { provider_id: 'two' })]
    expect(connections(list).map((c) => [c.provider_id, c.calendars.length])).toEqual([['one', 2], ['two', 1]])
    expect(defaultTargets(list)).toEqual(['b'])
    expect(defaultTargets([calendar('x', { can_write: false })])).toEqual([LOCAL_TARGET])
    expect(defaultTargets([])).toEqual([LOCAL_TARGET])
  })

  it('reads colour, meeting link and marks from merged sources', () => {
    const event = { online_meeting_url: null, sources: [sourceOf({ provider_type: null, calendar_name: 'Chatnip' }), sourceOf({ colour: '#5b5fc7', online_meeting_url: 'https://teams.microsoft.com/l/x' })] } as unknown as CalendarEvent
    expect(eventColour(event)).toBe('#5b5fc7')
    expect(joinUrl(event)).toBe('https://teams.microsoft.com/l/x')
    expect(sourceMarks(event.sources)).toEqual([{ provider_type: 'microsoft', external_id: null, name: 'Work', conversation_type: 'email' }])
  })
})
