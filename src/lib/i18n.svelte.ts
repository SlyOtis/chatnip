import { getAppSettings, putAppSetting } from '$lib/tauri/api'
import { en, type MessageKey } from '$lib/i18n/en'
import { nb } from '$lib/i18n/nb'
import type { Message } from '$lib/i18n/types'

export type { MessageKey } from '$lib/i18n/en'
export type { Message, Plural } from '$lib/i18n/types'

export type Locale = 'en' | 'nb'
export type LocaleChoice = 'system' | Locale
export type Params = Record<string, string | number>

export const LOCALE_SETTING = 'app.locale'

const KEY = 'chatnip-locale'
const NORWEGIAN = ['nb', 'nn', 'no']
const INTL: Record<Locale, string> = { en: 'en-GB', nb: 'nb-NO' }
const DICTIONARIES: Record<Locale, Partial<Record<MessageKey, Message>>> = { en, nb }

const state = $state<{ choice: LocaleChoice; locale: Locale }>({ choice: 'system', locale: 'en' })

const dateFormats = new Map<string, Intl.DateTimeFormat>()
const numberFormats = new Map<string, Intl.NumberFormat>()
const pluralRules = new Map<string, Intl.PluralRules>()

export function detectLocale(languages: readonly string[]): Locale {
  for (const language of languages) {
    const base = language.toLowerCase().split(/[-_]/)[0]
    if (NORWEGIAN.includes(base)) return 'nb'
    if (base === 'en') return 'en'
  }
  return 'en'
}

export function systemLocale(): Locale {
  if (typeof navigator === 'undefined') return 'en'
  return detectLocale(navigator.languages?.length ? navigator.languages : [navigator.language ?? 'en'])
}

export function parseLocaleChoice(value: string | null | undefined): LocaleChoice {
  return value === 'en' || value === 'nb' ? value : 'system'
}

export function resolveLocale(choice: LocaleChoice): Locale {
  return choice === 'system' ? systemLocale() : choice
}

export function getLocale(): Locale {
  return state.locale
}

export function getLocaleChoice(): LocaleChoice {
  return state.choice
}

export function intlLocale(locale: Locale = state.locale): string {
  return INTL[locale]
}

export function applyLocale(choice: LocaleChoice) {
  state.choice = choice
  state.locale = resolveLocale(choice)
  if (typeof document !== 'undefined') document.documentElement.lang = state.locale
}

export async function loadLocale(): Promise<void> {
  const all = await getAppSettings().catch(() => null)
  if (!all) return
  const choice = parseLocaleChoice(all[LOCALE_SETTING])
  if (typeof localStorage !== 'undefined') localStorage.setItem(KEY, choice)
  applyLocale(choice)
}

export async function putLocale(choice: LocaleChoice): Promise<void> {
  if (typeof localStorage !== 'undefined') localStorage.setItem(KEY, choice)
  applyLocale(choice)
  await putAppSetting(LOCALE_SETTING, choice)
}

function plural(locale: Locale, count: number): Intl.LDMLPluralRule {
  let rules = pluralRules.get(locale)
  if (!rules) {
    rules = new Intl.PluralRules(INTL[locale])
    pluralRules.set(locale, rules)
  }
  return rules.select(count)
}

function pick(message: Message, locale: Locale, params?: Params): string {
  if (typeof message === 'string') return message
  const count = Number(params?.count ?? 0)
  if (count === 0 && message.zero !== undefined) return message.zero
  return message[plural(locale, count)] ?? message.other
}

function interpolate(text: string, locale: Locale, params?: Params): string {
  if (!params) return text
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const value = params[name]
    if (value === undefined) return whole
    return typeof value === 'number' ? formatNumber(value, undefined, locale) : value
  })
}

export function translate(locale: Locale, key: MessageKey, params?: Params): string {
  const message = DICTIONARIES[locale][key] ?? DICTIONARIES.en[key]
  if (message === undefined) return key
  return interpolate(pick(message, DICTIONARIES[locale][key] === undefined ? 'en' : locale, params), locale, params)
}

export function t(key: MessageKey, params?: Params): string {
  return translate(state.locale, key, params)
}

export function formatDate(value: Date | string | number, options: Intl.DateTimeFormatOptions = {}, locale: Locale = state.locale): string {
  const date = value instanceof Date ? value : new Date(value)
  const id = `${locale}|${JSON.stringify(options)}`
  let format = dateFormats.get(id)
  if (!format) {
    format = new Intl.DateTimeFormat(INTL[locale], options)
    dateFormats.set(id, format)
  }
  return format.format(date)
}

export function formatNumber(value: number, options: Intl.NumberFormatOptions = {}, locale: Locale = state.locale): string {
  const id = `${locale}|${JSON.stringify(options)}`
  let format = numberFormats.get(id)
  if (!format) {
    format = new Intl.NumberFormat(INTL[locale], options)
    numberFormats.set(id, format)
  }
  return format.format(value)
}

applyLocale(parseLocaleChoice(typeof localStorage !== 'undefined' ? localStorage.getItem(KEY) : null))
