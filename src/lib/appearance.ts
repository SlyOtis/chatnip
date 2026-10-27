import { derived, get, writable } from 'svelte/store'
import { getAppSettings, putAppSetting } from '$lib/tauri/api'
import { resolvedTheme, themeChoice } from '$lib/theme-mode'

export type Tint = 'light' | 'dark' | 'auto'
export type ThemeOverride = 'inherit' | 'light' | 'dark'
export type Surface = 'solid' | 'glass'
export type Intensity = 'soft' | 'normal' | 'vivid'
export type Motion = 'slow' | 'normal' | 'fast'
export type BackgroundKind = 'none' | 'colour' | 'gradient' | 'art' | 'image'
export type AvatarForm = 'round' | 'square'
export type AvatarScale = 'small' | 'normal' | 'large'
export type Presence = 'dots' | 'wave' | 'pulse' | 'none'
export type Entry = 'rise' | 'fade' | 'slide' | 'none'

export interface Background {
  kind: BackgroundKind
  value?: string
  from?: string
  to?: string
  src?: string
}

export interface Appearance {
  background: Background
  tint: Tint
  blur: number
  opacity: number
  accent: string | null
  bubbleMine: string | null
  bubbleTheirs: string | null
  theme: ThemeOverride
  surface: Surface
  intensity: Intensity
  motion: Motion
  avatarForm: AvatarForm
  avatarBorder: boolean
  avatarScale: AvatarScale
  presence: Presence
  entry: Entry
}

export type ScopedAppearance = Partial<Appearance>

export const PREFIX = 'appearance.'
export const GLOBAL_KEY = 'appearance.global'
export const CACHE_KEY = 'chatnip-appearance'

export const DEFAULT_APPEARANCE: Appearance = {
  background: { kind: 'art', value: 'meadow-flow' },
  tint: 'auto',
  blur: 24,
  opacity: 36,
  accent: null,
  bubbleMine: null,
  bubbleTheirs: null,
  theme: 'inherit',
  surface: 'solid',
  intensity: 'normal',
  motion: 'normal',
  avatarForm: 'round',
  avatarBorder: false,
  avatarScale: 'normal',
  presence: 'dots',
  entry: 'rise'
}

export interface GradientPreset {
  key: string
  name: string
  stops: string[]
  angle: number
}

export const GRADIENTS: GradientPreset[] = [
  { key: 'warm-dusk', name: 'Warm dusk', stops: ['#f6c38b', '#e2786b', '#6b4a8c'], angle: 160 },
  { key: 'aurora', name: 'Aurora', stops: ['#0f2d3a', '#1f7a6b', '#8fd6a1'], angle: 200 },
  { key: 'deep-sea', name: 'Deep sea', stops: ['#0b1a33', '#1c3f73', '#3f7fb5'], angle: 180 },
  { key: 'sand', name: 'Sand', stops: ['#f5ead9', '#e6cfa8', '#c9a77a'], angle: 150 },
  { key: 'plum', name: 'Plum', stops: ['#2a1630', '#5b2a5e', '#a35a8f'], angle: 165 },
  { key: 'graphite', name: 'Graphite', stops: ['#1c1c1e', '#3a3a3f', '#6b6b73'], angle: 170 },
  { key: 'forest', name: 'Forest', stops: ['#10241a', '#2d5a3d', '#7ea36a'], angle: 190 },
  { key: 'rose', name: 'Rose', stops: ['#fbe3e4', '#f2a7b5', '#c8607e'], angle: 155 }
]

export interface ArtPreset {
  key: string
  name: string
  stops: string[]
  css: string
  glass?: string
  spots?: number
}

export const ART: ArtPreset[] = [
  {
    key: 'meadow-flow',
    name: 'Green to orange',
    stops: ['#1f9d55', '#7cc35f', '#f2a541', '#e8743b'],
    css: 'radial-gradient(60% 70% at 18% 22%, color-mix(in oklab, var(--accent) 90%, transparent) 0, transparent 70%), radial-gradient(55% 65% at 84% 80%, color-mix(in oklab, var(--signal) 85%, transparent) 0, transparent 70%), radial-gradient(45% 55% at 78% 16%, color-mix(in oklab, var(--accent) 45%, var(--signal)) 0, transparent 70%)',
    glass: 'bg-glass-aurora',
    spots: 6
  },
  {
    key: 'mesh-flow',
    name: 'Soft mesh',
    stops: ['#1f9d55', '#a8dfbb', '#f2a541', '#faf8f4'],
    css: 'radial-gradient(at 22% 30%, color-mix(in oklab, var(--accent) 55%, transparent) 0, transparent 55%), radial-gradient(at 78% 24%, color-mix(in oklab, var(--signal) 45%, transparent) 0, transparent 50%), radial-gradient(at 60% 84%, color-mix(in oklab, var(--accent) 35%, var(--signal)) 0, transparent 55%)',
    glass: 'bg-glass-mesh',
    spots: 4
  },
  {
    key: 'clear',
    name: 'Clear',
    stops: [],
    css: 'linear-gradient(135deg, rgb(255 255 255 / 0.22), rgb(255 255 255 / 0.04) 60%, rgb(0 0 0 / 0.06))',
    glass: 'bg-glass-none'
  },
  {
    key: 'mesh-dawn',
    name: 'Mesh dawn',
    stops: ['#f7d9c4', '#f0a38a', '#b98fd6', '#fdf2e6'],
    css: 'radial-gradient(at 18% 22%, #f0a38a 0, transparent 52%), radial-gradient(at 82% 18%, #b98fd6 0, transparent 50%), radial-gradient(at 68% 86%, #f7d9c4 0, transparent 55%), #fdf2e6'
  },
  {
    key: 'mesh-ocean',
    name: 'Mesh ocean',
    stops: ['#0d2a4a', '#1f6f8b', '#5ec2b7', '#08182c'],
    css: 'radial-gradient(at 20% 80%, #1f6f8b 0, transparent 55%), radial-gradient(at 80% 20%, #5ec2b7 0, transparent 45%), radial-gradient(at 50% 50%, #0d2a4a 0, transparent 70%), #08182c'
  },
  {
    key: 'orbs',
    name: 'Orbs',
    stops: ['#1a1530', '#7b5cff', '#ff7ab6', '#36d1c4'],
    css: 'radial-gradient(circle at 25% 30%, #7b5cff 0, transparent 28%), radial-gradient(circle at 75% 65%, #ff7ab6 0, transparent 26%), radial-gradient(circle at 60% 20%, #36d1c4 0, transparent 20%), #1a1530'
  },
  {
    key: 'dune',
    name: 'Dune',
    stops: ['#f3e3c7', '#d9b382', '#a8794e'],
    css: 'repeating-radial-gradient(ellipse at 50% 120%, #d9b382 0, #d9b382 8px, #f3e3c7 8px, #f3e3c7 26px), linear-gradient(#f3e3c7, #a8794e)'
  },
  {
    key: 'night-grid',
    name: 'Night grid',
    stops: ['#101218', '#1d2230', '#3a4566'],
    css: 'linear-gradient(rgb(58 69 102 / 0.35) 1px, transparent 1px) 0 0 / 32px 32px, linear-gradient(90deg, rgb(58 69 102 / 0.35) 1px, transparent 1px) 0 0 / 32px 32px, radial-gradient(at 50% 0%, #1d2230 0, #101218 70%)'
  },
  {
    key: 'conic-bloom',
    name: 'Conic bloom',
    stops: ['#ffd36e', '#ff8a65', '#c66bff', '#5b8cff'],
    css: 'conic-gradient(from 210deg at 60% 45%, #ffd36e, #ff8a65, #c66bff, #5b8cff, #ffd36e)'
  }
]

export const ACCENTS: { key: string; name: string; value: string }[] = [
  { key: 'meadow', name: 'Meadow', value: '#1f9d55' },
  { key: 'lime', name: 'Lime', value: '#6aa83a' },
  { key: 'amber', name: 'Amber', value: '#d8892a' },
  { key: 'ember', name: 'Ember', value: '#e0612f' },
  { key: 'teal', name: 'Teal', value: '#13857a' },
  { key: 'ocean', name: 'Ocean', value: '#1f7fae' },
  { key: 'rose', name: 'Rose', value: '#d0456b' },
  { key: 'graphite', name: 'Graphite', value: '#4a4642' }
]

export const SWATCHES = ['#f7f4ee', '#e9e4dc', '#dfe7ef', '#e5efe4', '#f3e1e6', '#26231f', '#14161c', '#1d2a3a']

export const FROST = {
  light: { rgb: [255, 255, 255] as Rgb, ink: [29, 26, 23] as Rgb, border: 'rgb(29 26 23 / 0.1)', highlight: 'rgb(255 255 255 / 0.6)' },
  dark: { rgb: [32, 32, 34] as Rgb, ink: [244, 241, 236] as Rgb, border: 'rgb(244 241 236 / 0.1)', highlight: 'rgb(255 255 255 / 0.09)' }
}

export type Rgb = [number, number, number]

export function presetGradient(key: string | undefined): GradientPreset | undefined {
  return GRADIENTS.find((g) => g.key === key)
}

export function presetArt(key: string | undefined): ArtPreset | undefined {
  return ART.find((a) => a.key === key)
}

export function parseHex(hex: string | undefined | null): Rgb | null {
  if (!hex) return null
  const match = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex.trim())
  if (!match) return null
  const raw = match[1].length === 3 ? match[1].split('').map((c) => c + c).join('') : match[1]
  return [parseInt(raw.slice(0, 2), 16), parseInt(raw.slice(2, 4), 16), parseInt(raw.slice(4, 6), 16)]
}

function channel(value: number): number {
  const v = value / 255
  return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4)
}

export function luminance([r, g, b]: Rgb): number {
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

export function contrast(a: Rgb, b: Rgb): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

export function blend(top: Rgb, bottom: Rgb, alpha: number): Rgb {
  return [0, 1, 2].map((i) => Math.round(top[i] * alpha + bottom[i] * (1 - alpha))) as Rgb
}

export function minFrostAlpha(frost: Rgb, ink: Rgb, worst: Rgb, target = 4.5): number {
  for (let step = 0; step <= 100; step++) {
    const alpha = step / 100
    if (contrast(ink, blend(frost, worst, alpha)) >= target) return alpha
  }
  return 1
}

export function backgroundStops(background: Background): Rgb[] {
  switch (background.kind) {
    case 'colour':
      return [parseHex(background.value)].filter((c): c is Rgb => !!c)
    case 'gradient': {
      const preset = presetGradient(background.value)
      const stops = preset ? preset.stops : [background.from, background.to]
      return stops.map(parseHex).filter((c): c is Rgb => !!c)
    }
    case 'art':
      return (presetArt(background.value)?.stops ?? []).map(parseHex).filter((c): c is Rgb => !!c)
    default:
      return []
  }
}

export const SPOT_ALPHA = 0.35

export function worstBackground(background: Background, tint: 'light' | 'dark'): Rgb {
  const stops = backgroundStops(background)
  if (backgroundClass(background)) {
    const material = FROST[tint].rgb
    const sorted = [...stops].sort((a, b) => luminance(a) - luminance(b))
    const worst = tint === 'light' ? sorted[0] : sorted[sorted.length - 1]
    return worst ? blend(worst, material, SPOT_ALPHA) : material
  }
  if (stops.length === 0) return tint === 'light' ? [0, 0, 0] : [255, 255, 255]
  const sorted = [...stops].sort((a, b) => luminance(a) - luminance(b))
  return tint === 'light' ? sorted[0] : sorted[sorted.length - 1]
}

export function safeOpacity(appearance: Appearance, tint: 'light' | 'dark'): number {
  const frost = FROST[tint]
  const floor = minFrostAlpha(frost.rgb, frost.ink, worstBackground(appearance.background, tint))
  const requested = Math.min(95, Math.max(15, appearance.opacity)) / 100
  return Math.round(Math.max(requested, floor) * 100) / 100
}

export function resolveTint(tint: Tint, theme: 'day' | 'night'): 'light' | 'dark' {
  if (tint === 'auto') return theme === 'night' ? 'dark' : 'light'
  return tint
}

export function resolveTheme(override: ThemeOverride, global: 'day' | 'night'): 'day' | 'night' {
  if (override === 'light') return 'day'
  if (override === 'dark') return 'night'
  return global
}

export function backgroundCss(background: Background): string | null {
  switch (background.kind) {
    case 'colour':
      return parseHex(background.value) ? background.value! : null
    case 'gradient': {
      const preset = presetGradient(background.value)
      if (preset) return `linear-gradient(${preset.angle}deg, ${preset.stops.join(', ')})`
      if (parseHex(background.from) && parseHex(background.to)) return `linear-gradient(160deg, ${background.from}, ${background.to})`
      return null
    }
    case 'art':
      return presetArt(background.value)?.css ?? null
    case 'image':
      return background.src ? `center / cover no-repeat url("${background.src.replace(/"/g, '%22')}")` : null
    default:
      return null
  }
}

export function hasBackground(appearance: Appearance): boolean {
  return backgroundCss(appearance.background) !== null
}

export function backgroundClass(background: Background | null | undefined): string | null {
  return (background?.kind === 'art' && presetArt(background.value)?.glass) || null
}

export interface ShellPresentation {
  glass: boolean
  tint: 'light' | 'dark'
  style: string
  backdrop: string | null
  backdropClass: string | null
  spots: number
  frost: string
}

export function shellPresentation(appearance: Appearance, theme: 'day' | 'night'): ShellPresentation {
  const tint = resolveTint(appearance.tint, theme)
  const glassClass = backgroundClass(appearance.background)
  const backdrop = backgroundCss(appearance.background)
  const glass = backdrop !== null
  const modifiers = [appearance.intensity !== 'normal' ? `bg-glass-${appearance.intensity}` : '', appearance.motion !== 'normal' ? `bg-glass-${appearance.motion}` : ''].filter(Boolean)
  return {
    glass,
    tint,
    style: styleString(appearanceVars(appearance, theme)),
    backdrop: glassClass ? null : backdrop,
    backdropClass: glassClass ? [glassClass, ...modifiers].join(' ') : null,
    spots: appearance.background.kind === 'art' ? (presetArt(appearance.background.value)?.spots ?? 0) : 0,
    frost: glass ? `frost frost-edge frost-${tint}` : 'bg-paper'
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function parseBackground(value: unknown): Background | undefined {
  if (!isRecord(value)) return undefined
  const kinds: BackgroundKind[] = ['none', 'colour', 'gradient', 'art', 'image']
  if (!kinds.includes(value.kind as BackgroundKind)) return undefined
  const text = (v: unknown) => (typeof v === 'string' ? v : undefined)
  return { kind: value.kind as BackgroundKind, value: text(value.value), from: text(value.from), to: text(value.to), src: text(value.src) }
}

export function parseAppearance(raw: string | null | undefined): ScopedAppearance {
  if (!raw) return {}
  let data: unknown
  try {
    data = JSON.parse(raw)
  } catch {
    return {}
  }
  if (!isRecord(data)) return {}
  const out: ScopedAppearance = {}
  const background = parseBackground(data.background)
  if (background) out.background = background
  if (data.tint === 'light' || data.tint === 'dark' || data.tint === 'auto') out.tint = data.tint
  if (data.theme === 'inherit' || data.theme === 'light' || data.theme === 'dark') out.theme = data.theme
  if (data.surface === 'solid' || data.surface === 'glass') out.surface = data.surface
  if (data.intensity === 'soft' || data.intensity === 'normal' || data.intensity === 'vivid') out.intensity = data.intensity
  if (data.motion === 'slow' || data.motion === 'normal' || data.motion === 'fast') out.motion = data.motion
  if (data.avatarForm === 'round' || data.avatarForm === 'square') out.avatarForm = data.avatarForm
  if (typeof data.avatarBorder === 'boolean') out.avatarBorder = data.avatarBorder
  if (data.avatarScale === 'small' || data.avatarScale === 'normal' || data.avatarScale === 'large') out.avatarScale = data.avatarScale
  if (data.presence === 'dots' || data.presence === 'wave' || data.presence === 'pulse' || data.presence === 'none') out.presence = data.presence
  if (data.entry === 'rise' || data.entry === 'fade' || data.entry === 'slide' || data.entry === 'none') out.entry = data.entry
  if (typeof data.blur === 'number' && Number.isFinite(data.blur)) out.blur = Math.min(40, Math.max(0, data.blur))
  if (typeof data.opacity === 'number' && Number.isFinite(data.opacity)) out.opacity = Math.min(95, Math.max(15, data.opacity))
  for (const key of ['accent', 'bubbleMine', 'bubbleTheirs'] as const) {
    if (data[key] === null) out[key] = null
    else if (typeof data[key] === 'string' && parseHex(data[key] as string)) out[key] = data[key] as string
  }
  return out
}

export function mergeAppearance(base: Appearance, ...layers: ScopedAppearance[]): Appearance {
  const out = { ...base }
  for (const layer of layers) {
    for (const [key, value] of Object.entries(layer) as [keyof Appearance, Appearance[keyof Appearance]][]) {
      if (value === undefined) continue
      if (key === 'theme' && value === 'inherit') continue
      ;(out as Record<string, unknown>)[key] = value
    }
  }
  return out
}

function inkFor(hex: string): string {
  const rgb = parseHex(hex)
  if (!rgb) return '#ffffff'
  return contrast(rgb, [255, 255, 255]) >= contrast(rgb, [20, 18, 16]) ? '#ffffff' : '#15132e'
}

export function surfaceStyle(background: string | null | undefined, band: string | null = null, fit: 'fixed' | 'scroll' = 'fixed', blend = 78): string {
  if (!background) return ''
  const gap = band ? `linear-gradient(var(--list-paper) ${band}, transparent ${band}), ` : ''
  const scroll = band ? 'scroll, ' : ''
  if (!parseHex(background)) {
    const wash = `color-mix(in oklab, var(--paper) ${Math.min(100, Math.max(0, blend))}%, transparent)`
    if (fit === 'scroll' && band) return `background: ${gap}linear-gradient(${wash}, ${wash}); -webkit-backdrop-filter: blur(1rem); backdrop-filter: blur(1rem)`
    if (fit === 'scroll') return `background: linear-gradient(${wash}, ${wash}), ${backgroundCss({ kind: 'image', src: background })}`
    return `background: ${gap}linear-gradient(${wash}, ${wash}), ${backgroundCss({ kind: 'image', src: background })}; background-attachment: ${scroll}fixed, fixed`
  }
  const ink = inkFor(background)
  const faded = (amount: number) => `color-mix(in oklab, ${ink} ${amount}%, transparent)`
  const tinted = (amount: number) => `color-mix(in oklab, ${background}, ${ink} ${amount}%)`
  const tones: Record<string, string> = { paper: background, 'paper-2': tinted(7), 'paper-3': tinted(13), ink, 'ink-2': faded(78), 'ink-3': faded(58), line: faded(14), 'line-strong': faded(24) }
  const vars = styleString(Object.fromEntries(Object.entries(tones).flatMap(([name, value]) => [[`--${name}`, value], [`--color-${name}`, value]])))
  return band ? `${vars}; background: ${gap}linear-gradient(${background}, ${background})` : vars
}

export function appearanceVars(appearance: Appearance, theme: 'day' | 'night'): Record<string, string> {
  const tint = resolveTint(appearance.tint, theme)
  const vars: Record<string, string> = {
    '--frost-rgb': FROST[tint].rgb.join(' '),
    '--frost-alpha': String(safeOpacity(appearance, tint)),
    '--frost-blur': `${Math.min(40, Math.max(0, appearance.blur))}px`,
    '--frost-border': FROST[tint].border,
    '--frost-highlight': FROST[tint].highlight
  }
  if (appearance.accent && parseHex(appearance.accent)) {
    const accent = tint === 'dark' ? `color-mix(in oklab, ${appearance.accent} 78%, white)` : appearance.accent
    vars['--accent'] = accent
    vars['--color-accent'] = accent
    vars['--accent-ink'] = inkFor(appearance.accent)
    vars['--color-accent-ink'] = vars['--accent-ink']
    vars['--accent-soft'] = `color-mix(in oklab, ${appearance.accent} ${tint === 'dark' ? 22 : 14}%, transparent)`
    vars['--color-accent-soft'] = vars['--accent-soft']
  }
  if (vars['--accent-soft']) vars['--bubble-mine'] = vars['--accent-soft']
  if (appearance.bubbleMine && parseHex(appearance.bubbleMine)) {
    vars['--bubble-mine'] = appearance.bubbleMine
    vars['--bubble-mine-ink'] = inkFor(appearance.bubbleMine)
  }
  if (appearance.bubbleTheirs && parseHex(appearance.bubbleTheirs)) {
    vars['--bubble-theirs'] = appearance.bubbleTheirs
    vars['--bubble-theirs-ink'] = inkFor(appearance.bubbleTheirs)
  }
  return vars
}

export function styleString(vars: Record<string, string>): string {
  return Object.entries(vars)
    .map(([key, value]) => `${key}: ${value}`)
    .join('; ')
}

export function conversationKey(id: string): string {
  return `${PREFIX}conversation.${id}`
}

export function personKey(id: string): string {
  return `${PREFIX}person.${id}`
}

export function memberKey(personId: string, conversationId: string): string {
  return `${PREFIX}person.${personId}.conversation.${conversationId}`
}

function readCache(): Record<string, string> {
  const cached = typeof localStorage === 'undefined' ? null : localStorage.getItem(CACHE_KEY)
  return cached ? { [GLOBAL_KEY]: cached } : {}
}

function writeCache(settings: Record<string, string>): void {
  if (typeof localStorage === 'undefined') return
  if (settings[GLOBAL_KEY]) localStorage.setItem(CACHE_KEY, settings[GLOBAL_KEY])
  else localStorage.removeItem(CACHE_KEY)
}

export const appearanceSettings = writable<Record<string, string>>(readCache())

export const globalAppearance = derived(appearanceSettings, ($settings) => mergeAppearance(DEFAULT_APPEARANCE, parseAppearance($settings[GLOBAL_KEY])))

export const shell = derived([globalAppearance, themeChoice], ([appearance, choice]) => shellPresentation(appearance, resolvedTheme(choice)))

export function resolved(settings: Record<string, string>, global: Appearance, conversationId: string | null, personId: string | null): Appearance {
  const keys = [conversationId ? conversationKey(conversationId) : null, personId ? personKey(personId) : null, personId && conversationId ? memberKey(personId, conversationId) : null]
  return mergeAppearance(global, ...keys.map((key) => (key ? parseAppearance(settings[key]) : {})))
}

export function scoped(settings: Record<string, string>, keys: string[]): ScopedAppearance {
  for (const key of keys) {
    const layer = parseAppearance(settings[key])
    if (Object.keys(layer).length > 0) return layer
  }
  return {}
}

export async function loadAppearance(): Promise<void> {
  const all = await getAppSettings()
  const next = Object.fromEntries(Object.entries(all).filter(([key]) => key.startsWith(PREFIX)))
  writeCache(next)
  appearanceSettings.set(next)
}

export async function saveAppearance(key: string, value: ScopedAppearance | null): Promise<void> {
  const text = value && Object.keys(value).length > 0 ? JSON.stringify(value) : ''
  const next = { ...get(appearanceSettings) }
  if (text) next[key] = text
  else delete next[key]
  writeCache(next)
  appearanceSettings.set(next)
  await putAppSetting(key, text)
}

export async function patchAppearance(key: string, patch: ScopedAppearance): Promise<void> {
  const current = parseAppearance(get(appearanceSettings)[key])
  await saveAppearance(key, { ...current, ...patch })
}

export interface ScopePresentation {
  style: string
  tint: 'light' | 'dark'
  backdrop: string | null
  tinted: boolean
  glass: boolean
  solid: boolean
}

export function scopePresentation(global: Appearance, layer: ScopedAppearance, theme: 'day' | 'night'): ScopePresentation {
  const override = layer.theme && layer.theme !== 'inherit' ? layer.theme : null
  const scopeTheme = resolveTheme(override ?? 'inherit', theme)
  const merged = mergeAppearance(global, layer)
  const tint = resolveTint(layer.tint ?? (override ? 'auto' : merged.tint), scopeTheme)
  const glass = merged.surface === 'glass'
  const backdrop = glass && layer.background ? backgroundCss(layer.background) : null
  const tinted = !!(layer.tint || override || backdrop)
  return {
    style: styleString(appearanceVars({ ...merged, tint }, scopeTheme)),
    tint,
    backdrop,
    tinted,
    glass,
    solid: tinted && (!glass || (!backdrop && !hasBackground(global)))
  }
}
