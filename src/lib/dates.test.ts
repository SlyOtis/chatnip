import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { formatTime, formatDateSeparator } from '$lib/dates'
import { applyLocale } from '$lib/i18n.svelte'

describe('formatTime', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2025-06-15T12:00:00'))
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('returns time only for today timestamps', () => {
    const result = formatTime('2025-06-15T09:30:00')
    expect(result).toMatch(/09:30/)
  })

  it('returns time only for today at midnight', () => {
    const result = formatTime('2025-06-15T00:00:00')
    expect(result).toMatch(/00:00/)
  })

  it('returns Yesterday with time for yesterday timestamps', () => {
    const result = formatTime('2025-06-14T15:45:00')
    expect(result).toMatch(/Yesterday/)
    expect(result).toMatch(/15:45/)
  })

  it('returns date with time for older timestamps', () => {
    const result = formatTime('2025-06-10T08:00:00')
    expect(result).toMatch(/Jun/)
    expect(result).toMatch(/10/)
    expect(result).toMatch(/08:00/)
  })

  it('returns date with time for previous year timestamps', () => {
    const result = formatTime('2024-12-25T18:00:00')
    expect(result).toMatch(/Dec/)
    expect(result).toMatch(/25/)
    expect(result).toMatch(/18:00/)
  })
})

describe('formatDateSeparator', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2025-06-15T12:00:00'))
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('returns Today for today timestamps', () => {
    expect(formatDateSeparator('2025-06-15T09:00:00')).toBe('Today')
  })

  it('returns Today for today at midnight', () => {
    expect(formatDateSeparator('2025-06-15T00:00:00')).toBe('Today')
  })

  it('returns Yesterday for yesterday timestamps', () => {
    expect(formatDateSeparator('2025-06-14T23:59:59')).toBe('Yesterday')
  })

  it('returns full date for older timestamps in the same year', () => {
    const result = formatDateSeparator('2025-06-10T08:00:00')
    expect(result).toMatch(/Tuesday/)
    expect(result).toMatch(/June/)
    expect(result).toMatch(/10/)
    expect(result).not.toMatch(/2025/)
  })

  it('includes year for timestamps from a different year', () => {
    const result = formatDateSeparator('2024-12-25T18:00:00')
    expect(result).toMatch(/Wednesday/)
    expect(result).toMatch(/December/)
    expect(result).toMatch(/25/)
    expect(result).toMatch(/2024/)
  })

  it('returns Yesterday when near midnight boundary', () => {
    vi.setSystemTime(new Date('2025-06-15T00:01:00'))
    expect(formatDateSeparator('2025-06-14T23:59:00')).toBe('Yesterday')
  })
})

describe('dates in Norwegian', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2025-06-15T12:00:00'))
    applyLocale('nb')
  })

  afterEach(() => {
    applyLocale('en')
    vi.useRealTimers()
  })

  it('names today and yesterday in Bokmål', () => {
    expect(formatDateSeparator('2025-06-15T09:00:00')).toBe('I dag')
    expect(formatDateSeparator('2025-06-14T09:00:00')).toBe('I går')
    expect(formatTime('2025-06-14T15:45:00')).toMatch(/^I går 15[:.]45$/)
  })

  it('formats weekdays and months with the Norwegian calendar', () => {
    const result = formatDateSeparator('2025-06-10T08:00:00')
    expect(result).toMatch(/tirsdag/)
    expect(result).toMatch(/juni/)
  })
})
