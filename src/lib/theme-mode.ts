import { writable } from 'svelte/store'

export type ThemeChoice = 'system' | 'day' | 'night'

const KEY = 'chatnip-theme'
const media = typeof window !== 'undefined' && typeof window.matchMedia === 'function' ? window.matchMedia('(prefers-color-scheme: dark)') : null

export const themeChoice = writable<ThemeChoice>(read())

function read(): ThemeChoice {
  if (typeof localStorage === 'undefined') return 'system'
  const stored = localStorage.getItem(KEY)
  return stored === 'day' || stored === 'night' ? stored : 'system'
}

export function resolvedTheme(choice: ThemeChoice): 'day' | 'night' {
  if (choice !== 'system') return choice
  return media?.matches ? 'night' : 'day'
}

export function applyThemeChoice(choice: ThemeChoice) {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  const theme = resolvedTheme(choice)
  root.dataset.theme = theme
  root.classList.toggle('dark', theme === 'night')
}

export function setThemeChoice(choice: ThemeChoice) {
  localStorage.setItem(KEY, choice)
  themeChoice.set(choice)
  applyThemeChoice(choice)
}

export function watchTheme() {
  applyThemeChoice(read())
  const listener = () => applyThemeChoice(read())
  media?.addEventListener('change', listener)
  return () => media?.removeEventListener('change', listener)
}
