import { readFileSync, writeFileSync, readdirSync, statSync, mkdirSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const srcDir = join(root, 'src')
const outFile = join(srcDir, 'lib', 'icons.json')

function walk(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry)
    const info = statSync(full)
    if (info.isDirectory()) walk(full, out)
    else if (/\.(svelte|ts|js)$/.test(entry)) out.push(full)
  }
  return out
}

const used = new Map()
for (const file of walk(srcDir)) {
  const text = readFileSync(file, 'utf8')
  for (const match of text.matchAll(/['"`]([a-z0-9-]+):([a-z0-9-]+)['"`]/g)) {
    const [, prefix, name] = match
    if (!used.has(prefix)) used.set(prefix, new Set())
    used.get(prefix).add(name)
  }
}

const collections = []
let total = 0
for (const [prefix, names] of used) {
  let source
  try {
    source = JSON.parse(readFileSync(join(root, 'node_modules', '@iconify', 'json', 'json', `${prefix}.json`), 'utf8'))
  } catch {
    continue
  }
  const icons = {}
  const aliases = {}
  for (const name of names) {
    if (source.icons[name]) {
      icons[name] = source.icons[name]
    } else if (source.aliases && source.aliases[name]) {
      const alias = source.aliases[name]
      aliases[name] = alias
      if (source.icons[alias.parent]) icons[alias.parent] = source.icons[alias.parent]
    }
  }
  const count = Object.keys(icons).length
  if (count === 0) continue
  total += count
  const collection = { prefix, icons, width: source.width, height: source.height }
  if (Object.keys(aliases).length) collection.aliases = aliases
  collections.push(collection)
}

mkdirSync(dirname(outFile), { recursive: true })
writeFileSync(outFile, JSON.stringify(collections))
console.log(`icons: ${total} icons from ${collections.length} sets into src/lib/icons.json`)
