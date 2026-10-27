import type { CalendarEvent, EventSource, ProviderCalendar } from '$lib/tauri/types'
import { providerIcon, providerLabel } from '$lib/providerIcons'
import { t } from '$lib/i18n.svelte'

export const LOCAL_TARGET = 'local'

export type MeetingKind = 'teams' | 'meet'

export interface CalendarConnection {
  provider_id: string
  name: string
  mark: { provider_type: string; external_id: string | null; name: string }
  calendars: ProviderCalendar[]
}

function source(calendar: Pick<ProviderCalendar, 'provider_type' | 'provider_external_id'>) {
  return { provider_type: calendar.provider_type, external_id: calendar.provider_external_id }
}

export function meetingKind(calendar: Pick<ProviderCalendar, 'provider_type' | 'provider_external_id'>): MeetingKind | null {
  const icon = providerIcon(source(calendar), 'email')
  if (icon.includes('microsoft')) return 'teams'
  if (icon.includes('google')) return 'meet'
  return null
}

export function meetingKinds(calendars: ProviderCalendar[], targets: string[]): MeetingKind[] {
  const kinds = new Set<MeetingKind>()
  for (const calendar of calendars) {
    if (!targets.includes(calendar.id)) continue
    const kind = meetingKind(calendar)
    if (kind) kinds.add(kind)
  }
  return [...kinds]
}

export function meetingLabel(kinds: MeetingKind[]): string {
  if (kinds.length === 2) return t('calendar.meeting.either')
  return kinds[0] === 'meet' ? 'Google Meet' : t('calendar.meeting.teams')
}

export function connections(calendars: ProviderCalendar[]): CalendarConnection[] {
  const byProvider = new Map<string, CalendarConnection>()
  for (const calendar of calendars) {
    const known = byProvider.get(calendar.provider_id)
    if (known) {
      known.calendars.push(calendar)
      continue
    }
    byProvider.set(calendar.provider_id, {
      provider_id: calendar.provider_id,
      name: calendar.provider_name || providerLabel(source(calendar)),
      mark: { ...source(calendar), name: calendar.provider_name },
      calendars: [calendar]
    })
  }
  return [...byProvider.values()]
}

export function defaultTargets(calendars: ProviderCalendar[]): string[] {
  const primary = calendars.find((c) => c.can_write && c.enabled && c.is_primary) ?? calendars.find((c) => c.can_write && c.enabled)
  return primary ? [primary.id] : [LOCAL_TARGET]
}

export function sourceMarks(sources: EventSource[]) {
  return sources
    .filter((s) => s.provider_type)
    .map((s) => ({ provider_type: s.provider_type as string, external_id: s.provider_external_id, name: s.calendar_name, conversation_type: 'email' }))
}

export function eventColour(event: CalendarEvent): string | null {
  return event.sources.find((s) => s.colour)?.colour ?? null
}

export function joinUrl(event: CalendarEvent): string | null {
  return event.online_meeting_url ?? event.sources.find((s) => s.online_meeting_url)?.online_meeting_url ?? null
}
