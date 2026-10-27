import { mkdirSync, writeFileSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { build } from 'esbuild'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const outDir = join(root, 'src', 'lib', 'generated')
const outFile = join(outDir, 'svelte-runtime.js')

const result = await build({
  stdin: { contents: "export * from 'svelte/internal/client'\nexport { mount, unmount } from 'svelte'\n", resolveDir: root, loader: 'js' },
  bundle: true,
  format: 'esm',
  platform: 'browser',
  conditions: ['browser'],
  minify: true,
  write: false,
  legalComments: 'none',
  banner: { js: '// @ts-nocheck' }
})

mkdirSync(outDir, { recursive: true })
writeFileSync(outFile, result.outputFiles[0].text)
console.log(`svelte runtime: ${Math.round(result.outputFiles[0].text.length / 1024)} kB into src/lib/generated/svelte-runtime.js`)
