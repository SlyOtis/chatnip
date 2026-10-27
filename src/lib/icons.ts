import { addCollection, iconLoaded } from '@iconify/svelte'
import type { IconifyJSON } from '@iconify/svelte'
import collections from './icons.json'

export function installIcons() {
  for (const collection of collections as unknown as IconifyJSON[]) {
    addCollection(collection)
  }
}

let bank: Promise<string[]> | null = null

export function loadIconBank(): Promise<string[]> {
  bank ??= import('@iconify/json/json/lucide.json').then((module) => {
    const collection = (module.default ?? module) as unknown as IconifyJSON
    addCollection(collection)
    return Object.keys(collection.icons)
      .map((name) => `lucide:${name}`)
      .sort()
  })
  return bank
}

export function ensureIcons(names: (string | null | undefined)[]): void {
  if (names.some((name) => name && name.startsWith('lucide:') && !iconLoaded(name))) loadIconBank().catch(() => {})
}

export function searchIcons(names: string[], query: string, limit = 240): string[] {
  const words = query.toLowerCase().trim().split(/[\s-]+/).filter(Boolean)
  if (!words.length) return names.slice(0, limit)
  const scored: [number, string][] = []
  for (const name of names) {
    const bare = name.slice(name.indexOf(':') + 1)
    if (!words.every((word) => bare.includes(word))) continue
    scored.push([bare.startsWith(words[0]) ? 0 : 1, name])
  }
  return scored
    .sort((a, b) => a[0] - b[0] || a[1].length - b[1].length || a[1].localeCompare(b[1]))
    .slice(0, limit)
    .map(([, name]) => name)
}
