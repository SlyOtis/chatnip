import { writable } from 'svelte/store'
import type { MessageKey } from '$lib/i18n.svelte'

export interface Shortcut {
  key: string
  ctrl?: boolean
  meta?: boolean
  shift?: boolean
  alt?: boolean
  description: MessageKey
  action: () => unknown
  category: MessageKey
  fields?: boolean
  after?: string
}

const shortcuts: Shortcut[] = []
export const shortcutList = writable<Shortcut[]>([])

export function registerShortcut(shortcut: Shortcut) {
  const existing = shortcuts.findIndex(
    s => s.key === shortcut.key
      && s.ctrl === shortcut.ctrl
      && s.meta === shortcut.meta
      && s.shift === shortcut.shift
      && s.alt === shortcut.alt
      && s.after === shortcut.after
  )
  if (existing >= 0) {
    shortcuts[existing] = shortcut
  } else {
    shortcuts.push(shortcut)
  }
  shortcutList.set([...shortcuts])
}

export interface Binding {
  key: string
  mod: boolean
  shift: boolean
  alt: boolean
}

export const SHORTCUT_PREFIX = 'shortcuts.'
const MODIFIERS = ['Meta', 'Control', 'Shift', 'Alt']
let overrides: Record<string, string> = {}

export function comboOf(binding: { key: string; mod?: boolean; meta?: boolean; ctrl?: boolean; shift?: boolean; alt?: boolean }): string {
  const parts = [binding.mod || binding.meta || binding.ctrl ? 'mod' : '', binding.shift ? 'shift' : '', binding.alt ? 'alt' : ''].filter(Boolean)
  return [...parts, binding.key.length === 1 ? binding.key.toLowerCase() : binding.key].join('+')
}

export function parseCombo(combo: string): Binding {
  const parts = combo.split('+')
  const key = combo.endsWith('++') ? '+' : (parts.pop() ?? '')
  return { key, mod: parts.includes('mod'), shift: parts.includes('shift'), alt: parts.includes('alt') }
}

export function comboFromEvent(e: KeyboardEvent): string | null {
  if (MODIFIERS.includes(e.key)) return null
  return comboOf({ key: e.key, mod: e.metaKey || e.ctrlKey, shift: e.shiftKey, alt: e.altKey })
}

export function setShortcutOverrides(settings: Record<string, string>) {
  overrides = Object.fromEntries(Object.entries(settings).filter(([key, value]) => key.startsWith(SHORTCUT_PREFIX) && !!value).map(([key, value]) => [key.slice(SHORTCUT_PREFIX.length), value]))
}

export function ownCombo(shortcut: Shortcut): string {
  return shortcut.after ? `${shortcut.after} ${comboOf(shortcut)}` : comboOf(shortcut)
}

export function boundCombo(shortcut: Shortcut, settings: Record<string, string> | null = null): string {
  const own = ownCombo(shortcut)
  return (settings ? settings[SHORTCUT_PREFIX + own] : overrides[own]) || own
}

export function conflictOf(combo: string, except: Shortcut, settings: Record<string, string>): Shortcut | null {
  return shortcuts.find((other) => other !== except && boundCombo(other, settings) === combo) ?? null
}

export const TYPING = 'input, textarea, select, [contenteditable="true"], .cm-editor'

export function isTyping(node: EventTarget | Element | null): boolean {
  return node instanceof Element && !!node.closest(TYPING)
}

const SEQUENCE_MS = 800
let armed: string | null = null
let armedTimer: ReturnType<typeof setTimeout> | undefined

export function armSequence(prefix: string) {
  armed = prefix
  clearTimeout(armedTimer)
  armedTimer = setTimeout(() => (armed = null), SEQUENCE_MS)
}

export function armedSequence(): string | null {
  return armed
}

export function handleKeydown(e: KeyboardEvent) {
  if (e.defaultPrevented) return
  const pressed = comboFromEvent(e)
  if (!pressed) return
  const prefix = armed
  armed = null
  const wanted = prefix ? `${prefix} ${pressed}` : pressed
  const candidates = prefix ? [...shortcuts.filter((s) => boundCombo(s).includes(' ')), ...shortcuts.filter((s) => !boundCombo(s).includes(' '))] : shortcuts.filter((s) => !boundCombo(s).includes(' '))
  for (const s of candidates) {
    const bound = boundCombo(s)
    if (bound === wanted || (prefix && bound === pressed)) {
      const typing = isTyping(document.activeElement) || isTyping(e.target)
      if (typing && (s.fields === false || !parseCombo(bound.split(' ').pop() ?? bound).mod)) continue
      if (s.action() === false) return
      e.preventDefault()
      return
    }
  }
}

export function getShortcuts(): Shortcut[] {
  return [...shortcuts]
}

export function getShortcutsByCategory(list: Shortcut[] = shortcuts): Map<MessageKey, Shortcut[]> {
  const categories = new Map<MessageKey, Shortcut[]>()
  for (const s of list) {
    const list = categories.get(s.category) || []
    list.push(s)
    categories.set(s.category, list)
  }
  return categories
}

export interface ComposerKey {
  keys: string[]
  description: MessageKey
}

export const COMPOSER_CATEGORY: MessageKey = 'app.shortcutCategory.composer'

export function sendKeys(sendWith: string): string {
  return sendWith === 'enter' ? '↵' : '⌘↵'
}

export function composerKeys(sendWith: string): ComposerKey[] {
  const enter = sendWith === 'enter'
  return [
    { keys: enter ? ['↵'] : ['⌘', '↵'], description: 'app.shortcut.send' },
    { keys: enter ? ['⇧', '↵'] : ['↵'], description: 'app.shortcut.newLine' },
    ...(enter ? [{ keys: ['⌘', '↵'], description: 'app.shortcut.sendInBlock' as MessageKey }] : []),
    { keys: ['⌘', '⇧', '↵'], description: 'app.shortcut.sendDone' },
    { keys: ['↵'], description: 'app.shortcut.pickSuggestion' },
    { keys: ['⌘', '↵'], description: 'app.shortcut.sendMail' },
    { keys: ['↑'], description: 'conversation.composer.editLast' }
  ]
}

export function comboKeys(combo: string): string[] {
  return combo.split(' ').filter(Boolean).flatMap((part) => {
    const binding = parseCombo(part)
    return [binding.mod ? '⌘' : '', binding.shift ? '⇧' : '', binding.alt ? '⌥' : '', binding.key.length === 1 ? binding.key.toUpperCase() : binding.key].filter(Boolean)
  })
}

export function formatShortcutKeys(shortcut: Shortcut, settings: Record<string, string> | null = null): string[] {
  return comboKeys(boundCombo(shortcut, settings))
}
