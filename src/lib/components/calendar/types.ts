import type { CalendarEvent, EventDraft } from '$lib/tauri/types'

export type { CalendarEvent, EventDraft }

export type CalendarView = 'year' | 'quarter' | 'month' | 'week' | 'day' | 'agenda'
