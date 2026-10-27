import { afterEach, describe, expect, it } from 'vitest'
import { applyLocale, detectLocale, formatDate, formatNumber, getLocale, parseLocaleChoice, t, translate, type MessageKey } from '$lib/i18n.svelte'
import { en } from '$lib/i18n/en'
import { nb } from '$lib/i18n/nb'

const placeholders = (message: unknown): string[] => [...JSON.stringify(message).matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort()

afterEach(() => applyLocale('en'))

describe('dictionaries', () => {
  it('gives Norwegian exactly the English keys', () => {
    expect(Object.keys(nb).sort()).toEqual(Object.keys(en).sort())
  })

  it('keeps the same placeholders in both languages', () => {
    for (const key of Object.keys(en) as MessageKey[]) {
      expect([key, ...new Set(placeholders(nb[key]))]).toEqual([key, ...new Set(placeholders(en[key]))])
    }
  })

  it('has no empty messages', () => {
    for (const dictionary of [en, nb]) {
      for (const [key, message] of Object.entries(dictionary)) expect([key, JSON.stringify(message).length > 2]).toEqual([key, true])
    }
  })
})

describe('t', () => {
  it('reads the active locale', () => {
    expect(t('common.save')).toBe('Save')
    applyLocale('nb')
    expect(getLocale()).toBe('nb')
    expect(t('common.save')).toBe('Lagre')
  })

  it('falls back to English for a key missing in Norwegian', () => {
    const key = 'common.save' as MessageKey
    const saved = nb[key]
    delete (nb as Partial<typeof nb>)[key]
    try {
      expect(translate('nb', key)).toBe('Save')
    } finally {
      nb[key] = saved
    }
  })

  it('returns the key when no dictionary has it', () => {
    expect(translate('nb', 'missing.key' as MessageKey)).toBe('missing.key')
  })

  it('interpolates named values and leaves unknown ones', () => {
    expect(translate('en', 'dates.yesterdayAt', { time: '14:30' })).toBe('Yesterday 14:30')
    expect(translate('nb', 'dates.yesterdayAt', { time: '14:30' })).toBe('I går 14:30')
    expect(translate('en', 'dates.yesterdayAt')).toBe('Yesterday {time}')
  })

  it('selects plural forms with Intl.PluralRules', () => {
    expect(translate('en', 'common.messages', { count: 1 })).toBe('1 message')
    expect(translate('en', 'common.messages', { count: 2 })).toBe('2 messages')
    expect(translate('nb', 'common.messages', { count: 1 })).toBe('1 melding')
    expect(translate('nb', 'common.messages', { count: 0 })).toBe('0 meldinger')
    expect(translate('nb', 'common.unread', { count: 5 })).toBe('5 uleste')
  })

  it('formats counts with the locale number format', () => {
    expect(translate('en', 'common.messages', { count: 12000 })).toBe('12,000 messages')
    expect(translate('nb', 'common.messages', { count: 12000 }).replace(/\s/g, ' ')).toBe('12 000 meldinger')
  })
})

describe('locale detection', () => {
  it('maps every Norwegian tag to Bokmål', () => {
    expect(detectLocale(['nb-NO'])).toBe('nb')
    expect(detectLocale(['nn'])).toBe('nb')
    expect(detectLocale(['no_NO'])).toBe('nb')
  })

  it('uses the first supported language and defaults to English', () => {
    expect(detectLocale(['en-US', 'nb-NO'])).toBe('en')
    expect(detectLocale(['de-DE', 'nb'])).toBe('nb')
    expect(detectLocale(['sv-SE', 'de'])).toBe('en')
    expect(detectLocale([])).toBe('en')
  })

  it('reads stored choices and treats anything else as system', () => {
    expect(parseLocaleChoice('nb')).toBe('nb')
    expect(parseLocaleChoice('en')).toBe('en')
    expect(parseLocaleChoice('fr')).toBe('system')
    expect(parseLocaleChoice(undefined)).toBe('system')
  })
})

describe('Intl formatting', () => {
  it('formats dates and numbers in the active locale', () => {
    const date = new Date(2025, 5, 10, 8, 5)
    expect(formatDate(date, { month: 'long' })).toBe('June')
    expect(formatNumber(1234.5)).toBe('1,234.5')
    applyLocale('nb')
    expect(formatDate(date, { month: 'long' })).toBe('juni')
    expect(formatNumber(1234.5).replace(/\s/g, ' ')).toBe('1 234,5')
  })
})
