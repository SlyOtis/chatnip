import { describe, expect, it } from 'vitest'
import { EditorState } from '@codemirror/state'
import { CompletionContext } from '@codemirror/autocomplete'
import { lineOffset, rhaiCompletions } from './rhai'

describe('rhai editor helpers', () => {
  it('maps a diagnostic line and column to an offset', () => {
    expect(lineOffset('let a = 1;\nlet b = ;', 2, 9)).toBe(19)
    expect(lineOffset('abc', null, null)).toBe(0)
  })

  it('completes library functions and input names', () => {
    const complete = rhaiCompletions(
      () => [{ name: 'reply', params: [], required: 2, returns: 'message', description: 'Reply', effect: true, signature: 'reply(conversation: conversation, text: text) -> message', requires: ['accounts.connections'] }],
      () => [{ name: 'message', type: 'message' }]
    )
    const state = EditorState.create({ doc: 're' })
    const result = complete(new CompletionContext(state, 2, false))!
    const labels = result.options.map((o) => o.label)
    expect(labels).toContain('reply')
    expect(labels).toContain('message')
    expect(result.options.find((o) => o.label === 'reply')!.info).toContain('dry runs')
  })
})
