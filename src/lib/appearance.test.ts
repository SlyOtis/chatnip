import { describe, expect, it, vi } from 'vitest'

vi.mock('$lib/tauri/api', () => ({ getAppSettings: vi.fn(async () => ({})), putAppSetting: vi.fn(async () => ({})) }))

import {
  DEFAULT_APPEARANCE,
  FROST,
  appearanceVars,
  backgroundCss,
  contrast,
  hasBackground,
  backgroundClass,
  mergeAppearance,
  minFrostAlpha,
  parseAppearance,
  parseHex,
  presetArt,
  presetGradient,
  resolveTheme,
  resolveTint,
  safeOpacity,
  scoped,
  worstBackground
} from './appearance'

describe('parseAppearance', () => {
  it('returns nothing for bad input', () => {
    expect(parseAppearance(null)).toEqual({})
    expect(parseAppearance('not json')).toEqual({})
    expect(parseAppearance('[1,2]')).toEqual({})
    expect(parseAppearance('"text"')).toEqual({})
  })

  it('keeps only valid fields and clamps numbers', () => {
    const parsed = parseAppearance(JSON.stringify({ tint: 'purple', theme: 'dark', blur: 99, opacity: 10, accent: 'red', bubbleMine: '#abc', background: { kind: 'video' } }))
    expect(parsed).toEqual({ theme: 'dark', blur: 40, opacity: 15, bubbleMine: '#abc' })
  })

  it('reads a full background', () => {
    const parsed = parseAppearance(JSON.stringify({ background: { kind: 'gradient', value: 'aurora' }, tint: 'dark' }))
    expect(parsed.background).toEqual({ kind: 'gradient', value: 'aurora', from: undefined, to: undefined, src: undefined })
    expect(parsed.tint).toBe('dark')
  })
})

describe('mergeAppearance', () => {
  it('lets scoped layers override the global one field by field', () => {
    const global = mergeAppearance(DEFAULT_APPEARANCE, { background: { kind: 'colour', value: '#112233' }, accent: '#2f6fdb' })
    const merged = mergeAppearance(global, { tint: 'dark', bubbleMine: '#ffeeaa' })
    expect(merged.background).toEqual({ kind: 'colour', value: '#112233' })
    expect(merged.accent).toBe('#2f6fdb')
    expect(merged.tint).toBe('dark')
    expect(merged.bubbleMine).toBe('#ffeeaa')
  })

  it('treats an inherit theme as no override and null as reset', () => {
    const global = mergeAppearance(DEFAULT_APPEARANCE, { theme: 'light', accent: '#2f6fdb' })
    const merged = mergeAppearance(global, { theme: 'inherit', accent: null })
    expect(merged.theme).toBe('light')
    expect(merged.accent).toBeNull()
  })

  it('picks the first scoped key that has settings', () => {
    const settings = { 'appearance.person.p': '', 'appearance.conversation.c': JSON.stringify({ tint: 'light' }) }
    expect(scoped(settings, ['appearance.person.p', 'appearance.conversation.c'])).toEqual({ tint: 'light' })
    expect(scoped(settings, ['appearance.person.x'])).toEqual({})
  })
})

describe('contrast clamp', () => {
  it('computes WCAG contrast', () => {
    expect(contrast([0, 0, 0], [255, 255, 255])).toBeCloseTo(21, 0)
    expect(contrast([255, 255, 255], [255, 255, 255])).toBeCloseTo(1, 5)
  })

  it('finds the lowest frost alpha that keeps body text at 4.5:1', () => {
    const alpha = minFrostAlpha(FROST.light.rgb, FROST.light.ink, [0, 0, 0])
    expect(alpha).toBeGreaterThan(0.3)
    expect(alpha).toBeLessThan(1)
    expect(minFrostAlpha(FROST.light.rgb, FROST.light.ink, [255, 255, 255])).toBe(0)
  })

  it('raises the requested opacity over a dark background for light frost', () => {
    const dark = mergeAppearance(DEFAULT_APPEARANCE, { background: { kind: 'colour', value: '#000000' }, opacity: 40 })
    expect(safeOpacity(dark, 'light')).toBeGreaterThan(0.4)
    const pale = mergeAppearance(DEFAULT_APPEARANCE, { background: { kind: 'colour', value: '#ffffff' }, opacity: 40 })
    expect(safeOpacity(pale, 'light')).toBe(0.4)
  })

  it('uses the extreme stop of a gradient as the worst case', () => {
    expect(worstBackground({ kind: 'gradient', value: 'warm-dusk' }, 'light')).toEqual(parseHex('#6b4a8c'))
    expect(worstBackground({ kind: 'gradient', value: 'warm-dusk' }, 'dark')).toEqual(parseHex('#f6c38b'))
    expect(worstBackground({ kind: 'image', src: 'x' }, 'dark')).toEqual([255, 255, 255])
  })
})

describe('presets and css', () => {
  it('looks up presets', () => {
    expect(presetGradient('aurora')?.name).toBe('Aurora')
    expect(presetArt('orbs')?.stops.length).toBeGreaterThan(1)
    expect(presetGradient('missing')).toBeUndefined()
  })

  it('renders backgrounds and refuses invalid ones', () => {
    expect(backgroundCss({ kind: 'colour', value: '#123456' })).toBe('#123456')
    expect(backgroundCss({ kind: 'colour', value: 'blue' })).toBeNull()
    expect(backgroundCss({ kind: 'gradient', value: 'sand' })).toContain('linear-gradient(150deg')
    expect(backgroundCss({ kind: 'gradient', from: '#000', to: '#fff' })).toContain('#000, #fff')
    expect(backgroundCss({ kind: 'image', src: 'asset://a.png' })).toContain('url("asset://a.png")')
    expect(backgroundCss({ kind: 'none' })).toBeNull()
  })

  it('resolves tint and theme', () => {
    expect(resolveTint('auto', 'night')).toBe('dark')
    expect(resolveTint('light', 'night')).toBe('light')
    expect(resolveTheme('inherit', 'day')).toBe('day')
    expect(resolveTheme('dark', 'day')).toBe('night')
  })

  it('emits accent and bubble variables', () => {
    const vars = appearanceVars(mergeAppearance(DEFAULT_APPEARANCE, { accent: '#2f6fdb', bubbleMine: '#ffeeaa' }), 'day')
    expect(vars['--accent']).toBe('#2f6fdb')
    expect(vars['--bubble-mine']).toBe('#ffeeaa')
    expect(vars['--bubble-mine-ink']).toBe('#15132e')
    expect(vars['--frost-rgb']).toBe('255 255 255')
  })
})

describe('scopePresentation', () => {
  it('tints only when the scope changes tint, theme or background', async () => {
    const { scopePresentation } = await import('./appearance')
    expect(scopePresentation(DEFAULT_APPEARANCE, { accent: '#2f6fdb' }, 'day').tinted).toBe(false)
    const dark = scopePresentation(DEFAULT_APPEARANCE, { theme: 'dark' }, 'day')
    expect(dark.tinted).toBe(true)
    expect(dark.tint).toBe('dark')
    const art = scopePresentation(DEFAULT_APPEARANCE, { background: { kind: 'art', value: 'orbs' }, surface: 'glass' }, 'night')
    expect(art.backdrop).toContain('radial-gradient')
    expect(art.glass).toBe(true)
    expect(art.tint).toBe('dark')
    const solid = scopePresentation(DEFAULT_APPEARANCE, { background: { kind: 'art', value: 'orbs' } }, 'night')
    expect(solid.backdrop).toBeNull()
    expect(solid.glass).toBe(false)
  })
})

describe('default backdrop', () => {
  it('starts on the flowing green to orange glass backdrop', () => {
    expect(DEFAULT_APPEARANCE.background).toEqual({ kind: 'art', value: 'meadow-flow' })
    expect(hasBackground(DEFAULT_APPEARANCE)).toBe(true)
    expect(backgroundClass(DEFAULT_APPEARANCE.background)).toBe('bg-glass-aurora')
    expect(backgroundClass({ kind: 'art', value: 'mesh-flow' })).toBe('bg-glass-mesh')
    expect(backgroundClass({ kind: 'art', value: 'clear' })).toBe('bg-glass-none')
    expect(backgroundClass({ kind: 'art', value: 'conic-bloom' })).toBeNull()
    expect(backgroundClass({ kind: 'none' })).toBeNull()
  })

})

describe('shell background', () => {
  const store = new Map<string, string>()
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => void store.set(key, value),
    removeItem: (key: string) => void store.delete(key)
  })

  it('reads the cached background before anything mounts', async () => {
    vi.resetModules()
    localStorage.setItem('chatnip-appearance', JSON.stringify({ background: { kind: 'art', value: 'mesh-flow' }, intensity: 'soft', motion: 'slow', blur: 30 }))
    const { get } = await import('svelte/store')
    const { CACHE_KEY, globalAppearance, shell } = await import('./appearance')
    expect(CACHE_KEY).toBe('chatnip-appearance')
    expect(get(globalAppearance).background).toEqual({ kind: 'art', value: 'mesh-flow' })
    const presentation = get(shell)
    expect(presentation.glass).toBe(true)
    expect(presentation.backdropClass).toBe('bg-glass-mesh bg-glass-soft bg-glass-slow')
    expect(presentation.backdrop).toBeNull()
    expect(presentation.style).toContain('--frost-blur: 30px')
    localStorage.removeItem('chatnip-appearance')
  })

  it('caches what is loaded and saved so the next start paints it at once', async () => {
    vi.resetModules()
    localStorage.removeItem('chatnip-appearance')
    const { saveAppearance, GLOBAL_KEY } = await import('./appearance')
    await saveAppearance(GLOBAL_KEY, { background: { kind: 'art', value: 'clear' } })
    expect(localStorage.getItem('chatnip-appearance')).toBe(JSON.stringify({ background: { kind: 'art', value: 'clear' } }))
    await saveAppearance(GLOBAL_KEY, null)
    expect(localStorage.getItem('chatnip-appearance')).toBeNull()
  })







})
