import { describe, expect, it } from 'vitest'
import type { ActionPort, LandingNode, PipeStep } from '$lib/tauri/types'
import { changeRoute, defaultRoutes, extraInputs, fittingInputs, newRoute, stepState, validateRoutes } from './pipe'

const outputs: ActionPort[] = [
  { name: 'records', type: 'list' },
  { name: 'summary', type: 'text' },
  { name: 'conversation', type: 'conversation' }
]

const landings: LandingNode[] = [
  { node: 'chat', label: 'Read chat', icon: 'lucide:book-open', inputs: [{ name: 'items', type: 'list' }, { name: 'item', type: 'json' }] },
  { node: 'builtin:tag_add', label: 'Add tag', icon: 'lucide:tag', inputs: [{ name: 'conversation', type: 'conversation' }, { name: 'tag', type: 'tag' }] },
  { node: 'builtin:notify', label: 'Notify me', icon: 'lucide:bell', inputs: [{ name: 'title', type: 'text' }, { name: 'body', type: 'text' }] }
]

const step = (patch: Partial<PipeStep> = {}): PipeStep => ({ action_id: 'a', name: 'Shape', icon: null, enabled: true, inputs: [], outputs, routes: defaultRoutes(outputs), stored: false, problems: [], ...patch })

describe('the pipe panel', () => {
  it('starts from default landings into the chat', () => {
    expect(defaultRoutes(outputs)).toEqual([{ output: 'records', node: 'chat', input: 'items', values: {} }])
    const state = stepState(step(), landings, undefined)
    expect(state.problems).toEqual([])
    expect(state.dirty).toBe(false)
    expect(state.lands.map((l) => l.node)).toEqual(['chat'])
  })

  it('flags each landing whose input does not take the output', () => {
    const routes = [
      { output: 'summary', node: 'builtin:tag_add', input: 'conversation', values: {} },
      { output: 'gone', node: 'chat', input: 'item', values: {} },
      { output: 'summary', node: 'builtin:wait', input: 'value', values: {} },
      { output: 'records', node: 'builtin:tag_add', input: 'conversation', values: {} },
      { output: 'summary', node: 'chat', input: 'item', values: {} }
    ]
    const problems = validateRoutes(outputs, landings, routes)
    expect(problems.map((p) => p.index)).toEqual([0, 1, 2])
    expect(problems[0].message).toContain('summary')
    const state = stepState(step(), landings, routes)
    expect(state.dirty).toBe(true)
    expect(state.problems).toHaveLength(3)
    expect(state.lands.map((l) => l.node)).toEqual(['builtin:tag_add', 'chat'])
  })

  it('keeps the input in step with the output and the landing it picks', () => {
    expect(fittingInputs(landings[1], outputs[1]).map((p) => p.name)).toEqual([])
    expect(fittingInputs(landings[2], outputs[1]).map((p) => p.name)).toEqual(['title', 'body'])
    expect(fittingInputs(landings[1], outputs[0]).map((p) => p.name)).toEqual(['conversation', 'tag'])
    const first = newRoute(outputs, landings)
    expect(first).toEqual({ output: 'records', node: 'chat', input: 'items', values: {} })
    const tagged = changeRoute(first, { output: 'conversation', node: 'builtin:tag_add' }, outputs, landings)
    expect(tagged.input).toBe('conversation')
    const notify = changeRoute({ ...tagged, values: { tag: 'Important' } }, { output: 'summary', node: 'builtin:notify' }, outputs, landings)
    expect(notify).toEqual({ output: 'summary', node: 'builtin:notify', input: 'title', values: {} })
    expect(extraInputs(landings[1], tagged).map((p) => p.name)).toEqual(['tag'])
    expect(extraInputs(landings[2], notify).map((p) => p.name)).toEqual(['body'])
  })
})
