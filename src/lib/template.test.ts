import { describe, expect, it } from 'vitest'
import { compileTemplate, DEFAULT_TEMPLATES, importsOf, renderPlan, templateDocument, type TemplateKind } from './template'

describe('compiling templates', () => {
  it('compiles every default template for the client without foreign imports', async () => {
    for (const kind of Object.keys(DEFAULT_TEMPLATES) as TemplateKind[]) {
      const compiled = await compileTemplate(DEFAULT_TEMPLATES[kind])
      expect(compiled.ok, kind).toBe(true)
      if (compiled.ok) expect(importsOf(compiled.js).every((s) => s.startsWith('svelte/internal/'))).toBe(true)
    }
  }, 30000)

  it('reports syntax errors with their position', async () => {
    const compiled = await compileTemplate('<script>\n  let { message } = $props()\n</script>\n\n<p>{message.subject</p>')
    expect(compiled.ok).toBe(false)
    if (!compiled.ok) {
      expect(compiled.diagnostics[0].line).toBe(5)
      expect(compiled.diagnostics[0].message).toBeTruthy()
    }
  })

  it('refuses templates that import anything', async () => {
    const compiled = await compileTemplate("<script>\n  import { invoke } from '@tauri-apps/api/core'\n</script>\n<p>hi</p>")
    expect(compiled.ok).toBe(false)
    if (!compiled.ok) expect(compiled.diagnostics[0].message).toMatch(/cannot import/)
  })

  it('builds a document that maps the runtime and allows no network', async () => {
    const compiled = await compileTemplate(DEFAULT_TEMPLATES.chat)
    if (!compiled.ok) throw new Error('chat template failed')
    const doc = templateDocument(compiled.js, 'export const mount = () => {}', { '--ink': '#111' })
    expect(doc).toContain('"svelte/internal/client":"data:text/javascript;base64,')
    expect(doc).toContain("default-src 'none'")
    expect(doc).not.toMatch(/connect-src/)
    expect(doc).toContain(':root{--ink:#111}')
  })
})

describe('choosing the renderer', () => {
  it('uses the built in rendering without a template reference', () => {
    expect(renderPlan({ kind: 'invite' }, new Set())).toEqual({ use: 'builtin' })
    expect(renderPlan({ kind: 'invite', template: null }, new Set())).toEqual({ use: 'builtin' })
  })

  it('uses the template when the presentation points to one', () => {
    expect(renderPlan({ kind: 'invite', template: { action_id: 'a', node_id: 'show' } }, new Set())).toEqual({ use: 'template', action_id: 'a', node_id: 'show' })
  })

  it('falls back to the built in rendering once the template failed', () => {
    expect(renderPlan({ kind: 'invite', template: { action_id: 'a', node_id: 'show' } }, new Set(['a:show']))).toEqual({ use: 'builtin' })
  })

  it('uses the layout from settings when no action template applies', () => {
    const layouts = { 'messages.template.mail': '<p>{message.subject}</p>' }
    expect(renderPlan({ kind: 'mail' }, new Set(), layouts, 'mail')).toEqual({ use: 'layout', kind: 'mail', source: '<p>{message.subject}</p>' })
    expect(renderPlan({ kind: 'mail', template: { action_id: 'a', node_id: 'show' } }, new Set(), layouts, 'mail')).toEqual({ use: 'template', action_id: 'a', node_id: 'show' })
    expect(renderPlan({ kind: 'chat' }, new Set(), layouts, 'chat')).toEqual({ use: 'builtin' })
    expect(renderPlan({ kind: 'mail' }, new Set(['layout:mail']), layouts, 'mail')).toEqual({ use: 'builtin' })
  })
})
