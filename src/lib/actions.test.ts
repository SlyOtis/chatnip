import { describe, expect, it } from 'vitest'
import type { Action, ActionCatalog, ActionCatalogEntry, ActionDiagnostic, ActionGraph, ActionNode, ActionRun, Prompt } from '$lib/tauri/types'
import { accepts, chatOf, codeFocus, codeItems, foldToggle, graphOrder, runRow, runSteps, withCode, connect, connectionProblem, landings, landingTitle, draftFor, IN, layout, mockEdit, mockOf, mockProblems, mockWith, nodeFromAction, OUT, outputSources, groupName, partition, signatureLine, triggerOutputs, TRIGGER_ICONS, playPorts, portsOf, prefillFrom, problemsFor, promptOverride, promptsUsed, removeNodes, requirementBadge, requirementsFor, resolvePrompt, schemaOf, tree, treeRowState, uniqueId, withPrompt, withReturns, type JsonSchema } from './actions'
import { FRAME_HEAD, FRAME_MAX_WIDTH, FRAME_MIN_WIDTH, FRAME_PAD, frameBounds, graphPlan, inlineAction, MAX_DEPTH, partOf, PORT_ROW, refuses, withCollapsed } from './actions'
import { graphRole, nestable, roleOf, usageBadges, validEnds, withoutInvalidEdges } from './actions'
import { assistHistory, assistTurn, operationChange, operationTarget } from './actions'
import type { ActionOperationPreview } from '$lib/tauri/types'

const catalog = {
  builtins: [{ key: 'done', label: 'Mark done', icon: 'lucide:check', description: '', inputs: [{ name: 'conversation', type: 'conversation' }], outputs: [{ name: 'conversation', type: 'conversation' }], body: '' }],
  actions: [{ id: 'a2', name: 'Shout', icon: null, inputs: [{ name: 'text', type: 'text' }], outputs: [{ name: 'loud', type: 'text' }], group_id: null }]
} as unknown as ActionCatalog

const node = (id: string, type: string, config: Record<string, unknown> = {}): ActionNode => ({ id, type, label: '', config, body: '', x: 0, y: 0 })
const owner = { inputs: [{ name: 'topic', type: 'text' as const }] }

describe('ports and wiring', () => {
  it('types accept like the engine does', () => {
    expect(accepts('text', 'tag')).toBe(true)
    expect(accepts('json', 'person')).toBe(true)
    expect(accepts('conversation', 'message')).toBe(false)
  })

  it('reads ports for every kind of node', () => {
    expect(portsOf(node('t', 'trigger', { on: 'manual' }), catalog, owner)!.outputs.map((p) => p.name)).toEqual(['message', 'conversation', 'person', 'text', 'topic'])
    expect(portsOf(node('t', 'trigger', { on: 'arrival' }), catalog, owner)!.outputs).toHaveLength(4)
    expect(portsOf(node('d', 'builtin:done'), catalog, owner)!.inputs[0].type).toBe('conversation')
    expect(portsOf(node('s', 'action:a2'), catalog, owner)!.outputs[0].name).toBe('loud')
    expect(portsOf(node('x', 'builtin:nope'), catalog, owner)).toBeNull()
  })

  it('refuses wrong types, missing ports and loops with a reason', () => {
    const graph: ActionGraph = { nodes: [node('t', 'trigger'), node('d', 'builtin:done'), node('s', 'action:a2')], edges: [] }
    expect(connectionProblem(graph, catalog, owner, { source: 't', sourceHandle: `${OUT}message`, target: 'd', targetHandle: `${IN}conversation` })).toBe('message gives message, but conversation needs conversation')
    expect(connectionProblem(graph, catalog, owner, { source: 't', sourceHandle: `${OUT}conversation`, target: 'd', targetHandle: `${IN}conversation` })).toBeNull()
    expect(connectionProblem(graph, catalog, owner, { source: 't', sourceHandle: `${OUT}text`, target: 's', targetHandle: `${IN}missing` })).toContain('no such input')
    const wired = connect(graph, { source: 't', sourceHandle: `${OUT}text`, target: 's', targetHandle: `${IN}text` })
    expect(wired.edges).toEqual([{ id: 't.text-s.text', source: 't', source_port: 'text', target: 's', target_port: 'text' }])
    const twice = connect(wired, { source: 't', sourceHandle: `${OUT}text`, target: 's', targetHandle: `${IN}text` })
    expect(twice.edges).toHaveLength(1)
    const back = connect({ ...graph, nodes: [...graph.nodes, node('u', 'action:a2')] }, { source: 's', sourceHandle: `${OUT}loud`, target: 'u', targetHandle: `${IN}text` })
    expect(connectionProblem(back, catalog, owner, { source: 'u', sourceHandle: `${OUT}loud`, target: 's', targetHandle: `${IN}text` })).toBe('That would make a loop')
    expect(removeNodes(wired, ['s']).edges).toEqual([])
  })
})

describe('wiring rules by role', () => {
  const port = (name: string, type: string) => ({ name, type })
  const builtin = (key: string, label: string, kind: string, inputs: ReturnType<typeof port>[], outputs: ReturnType<typeof port>[]) => ({ key, label, icon: '', description: '', inputs, outputs, body: '', kind })
  const roles = {
    builtins: [
      builtin('summarize', 'Summarize', 'step', [port('text', 'text')], [port('summary', 'text')]),
      builtin('quote', 'Quote', 'step', [port('message', 'message')], [port('text', 'text')]),
      builtin('tag_add', 'Add tag', 'landing', [port('conversation', 'conversation'), port('tag', 'tag')], []),
      builtin('reply', 'Reply', 'landing', [port('conversation', 'conversation'), port('text', 'text')], [port('message', 'message')])
    ],
    actions: [{ id: 'starter', name: 'Starter', icon: null, role: 'trigger', inputs: [port('text', 'text')], outputs: [port('found', 'text')], group_id: null }],
    chat_node: { variants: { read: { label: 'Read chat', icon: '', inputs: [port('item', 'json')], outputs: [port('text', 'text')] }, write: { label: 'Write chat', icon: '', inputs: [port('text', 'text')], outputs: [port('text', 'text')] } } }
  } as unknown as ActionCatalog
  const write = (id: string) => node(id, 'chat', { mode: 'write' })
  const graph: ActionGraph = {
    nodes: [node('t', 'trigger'), node('s', 'builtin:summarize'), node('q', 'builtin:quote'), node('l', 'builtin:tag_add'), node('r', 'builtin:reply'), node('n', 'action:starter'), write('ended'), write('started')],
    edges: [
      { id: 's.summary-ended.text', source: 's', source_port: 'summary', target: 'ended', target_port: 'text' },
      { id: 'started.text-q.message', source: 'started', source_port: 'text', target: 'q', target_port: 'message' }
    ]
  }
  const wire = (source: string, out: string, target: string, into: string) => ({ source, sourceHandle: `${OUT}${out}`, target, targetHandle: `${IN}${into}` })
  const problem = (w: ReturnType<typeof wire>) => connectionProblem(graph, roles, owner, w)

  it('reads roles from the catalog and the wiring of chat nodes', () => {
    expect(['t', 's', 'l', 'n', 'ended', 'started'].map((id) => roleOf(graph.nodes.find((n) => n.id === id)!, roles))).toEqual(['trigger', 'step', 'landing', 'trigger', 'chat', 'chat'])
    expect(graphRole(graph, graph.nodes.find((n) => n.id === 'ended')!, roles)).toBe('landing')
    expect(graphRole(graph, graph.nodes.find((n) => n.id === 'started')!, roles)).toBe('trigger')
    expect(graphRole(graph, write('loose'), roles)).toBe('chat')
    expect(portsOf(node('n', 'action:starter'), roles, owner)!.inputs).toEqual([])
  })

  it('refuses a self connection, input into a trigger and flow out of a landing, with a reason', () => {
    expect(problem(wire('s', 'summary', 's', 'text'))).toBe('A node cannot connect to itself')
    expect(problem(wire('s', 'summary', 't', 'text'))).toBe('A trigger cannot receive input; it only starts a flow')
    expect(problem(wire('s', 'summary', 'n', 'text'))).toBe('A trigger cannot receive input; it only starts a flow')
    expect(problem(wire('l', 'conversation', 's', 'text'))).toBe('Add tag is a landing; nothing continues from it')
    expect(problem(wire('t', 'message', 'l', 'conversation'))).toBe('message gives message, but conversation needs conversation')
    expect(problem(wire('ended', 'text', 's', 'text'))).toBe('This chat receives input, so it ends the flow')
    expect(problem(wire('s', 'summary', 'started', 'text'))).toBe('This chat starts the flow, so it cannot receive input')
  })

  it('accepts steps both ways, a landing that declares an output, and a loose chat as either end', () => {
    expect(problem(wire('t', 'text', 's', 'text'))).toBeNull()
    expect(problem(wire('r', 'message', 'q', 'message'))).toBeNull()
    expect(problem(wire('t', 'text', 'ended', 'text'))).toBeNull()
    expect(connectionProblem({ ...graph, nodes: [...graph.nodes, write('loose')] }, roles, owner, wire('loose', 'text', 's', 'text'))).toBeNull()
  })

  it('lists the handles a dragged wire may end on, from either side', () => {
    const fromTrigger = validEnds(graph, roles, owner, { node: 't', handle: `${OUT}text`, type: 'source' })
    expect(fromTrigger.has('t')).toBe(false)
    expect(fromTrigger.get('s')).toEqual([`${IN}text`])
    expect(fromTrigger.get('r')).toEqual([`${IN}text`])
    expect(fromTrigger.get('l')).toEqual([])
    expect(fromTrigger.get('n')).toEqual([])
    expect(fromTrigger.get('started')).toEqual([])
    const intoStep = validEnds(graph, roles, owner, { node: 's', handle: `${IN}text`, type: 'target' })
    expect(intoStep.get('t')).toEqual([`${OUT}text`])
    expect(intoStep.get('l')).toEqual([])
    expect(intoStep.get('ended')).toEqual([])
    expect(intoStep.get('q')).toEqual([`${OUT}text`])
  })

  it('removes the connections the check reports and nothing else', () => {
    const broken: ActionGraph = { ...graph, edges: [...graph.edges, { id: 'loop', source: 's', source_port: 'summary', target: 's', target_port: 'text' }] }
    const diagnostics = [{ severity: 'error', message: 'A node cannot connect to itself', edge: 'loop' }, { severity: 'error', message: 'Bad script', node: 's' }, { severity: 'requirement', message: 'AI', edge: 's.summary-ended.text' }] as unknown as ActionDiagnostic[]
    expect(withoutInvalidEdges(broken, diagnostics).edges.map((e) => e.id)).toEqual(graph.edges.map((e) => e.id))
  })

  it('names what an action is good for', () => {
    expect(usageBadges({ standalone: false, chat: true, step: true, landing: false }).map((b) => b.label)).toEqual(['Attachable to chats', 'Usable as a step'])
    expect(usageBadges(undefined)).toEqual([])
    const entry = (usage: Partial<ActionCatalogEntry['usage']>, extra: Partial<ActionCatalogEntry> = {}) => ({ builtin: false, builtin_kind: null, graph: { nodes: [], edges: [] }, usage: { standalone: false, chat: false, step: false, landing: false, ...usage }, ...extra }) as ActionCatalogEntry
    expect(nestable(entry({ standalone: true }))).toBe(false)
    expect(nestable(entry({ step: true }))).toBe(true)
    expect(nestable(entry({ landing: true }))).toBe(true)
    expect(nestable(entry({}, { builtin: true, builtin_kind: 'trigger', graph: { nodes: [node('t', 'trigger')], edges: [] } }))).toBe(true)
  })
})

describe('editing helpers', () => {
  it('makes unique ids from labels', () => {
    expect(uniqueId('Mark done', ['mark_done'])).toBe('mark_done_2')
    expect(uniqueId('Ærlig svar', [])).toBe('rlig_svar')
  })

  it('places a built in as its own node and anything else as a nested action, and lays the graph out', () => {
    const step = { id: 'b1', name: 'Mark done', builtin: true, builtin_kind: 'landing' as const, graph: { nodes: [node('node', 'builtin:done')], edges: [] } }
    const placed = nodeFromAction(step, { nodes: [], edges: [] }, { x: 10.4, y: 20.6 })
    expect(placed).toMatchObject({ id: 'mark_done', type: 'builtin:done', x: 10, y: 21, label: 'Mark done' })
    const trigger = nodeFromAction({ id: 't1', name: 'Message arrived', builtin: true, builtin_kind: 'trigger', graph: { nodes: [{ ...node('trigger', 'trigger'), config: { on: 'arrival' } }], edges: [] } }, { nodes: [placed], edges: [] }, { x: 0, y: 0 })
    expect(trigger).toMatchObject({ id: 'message_arrived', type: 'trigger', config: { on: 'arrival' } })
    const shipped = nodeFromAction({ id: 'r1', name: 'Receipts', builtin: true, builtin_kind: 'action', graph: { nodes: [node('a', 'trigger'), node('b', 'builtin:receipt_extract')], edges: [] } }, { nodes: [], edges: [] }, { x: 0, y: 0 })
    expect(shipped).toMatchObject({ id: 'receipts', type: 'action:r1', label: 'Receipts' })
    expect(nodeFromAction({ id: 'm1', name: 'Mine', builtin: false, graph: { nodes: [node('x', 'builtin:done')], edges: [] } }, { nodes: [], edges: [] }, { x: 0, y: 0 }).type).toBe('action:m1')
    const laid = layout({ nodes: [node('a', 'trigger'), node('b', 'builtin:done')], edges: [{ id: 'e', source: 'a', source_port: 'conversation', target: 'b', target_port: 'conversation' }] }, 'LR')
    expect(laid.nodes[1].x).toBeGreaterThan(laid.nodes[0].x)
  })

  it('builds the group tree with collapsed groups hidden', () => {
    const groups = [
      { id: 'g1', profile_id: 'p', key: null, builtin: false, parent_id: null, name: 'Mail', description: '', icon: null, position: 0 },
      { id: 'g2', profile_id: 'p', key: null, builtin: false, parent_id: 'g1', name: 'Invoices', description: '', icon: null, position: 0 }
    ]
    const actions = [{ id: 'a', group_id: 'g2', position: 0 }, { id: 'b', group_id: null, position: 1 }, { id: 'c', group_id: 'gone', position: 0 }] as never
    expect(tree(groups, actions).map((i) => `${i.kind}:${i.id}:${i.depth}`)).toEqual(['group:g1:0', 'group:g2:1', 'action:a:2', 'action:c:0', 'action:b:0'])
    expect(tree(groups, actions, new Set(['g1'])).map((i) => i.id)).toEqual(['g1', 'c', 'b'])
  })

  it('opens a chat prefilled trigger from the route', () => {
    const prefill = prefillFrom(new URLSearchParams('conversation=c1'))!
    const draft = draftFor(prefill)
    expect(draft.enabled).toBe(false)
    expect(draft.graph!.nodes[0].config).toEqual({ on: 'arrival', scope: { conversation_ids: ['c1'], person_ids: [] } })
    expect(prefillFrom(new URLSearchParams('id=x'))).toBeNull()
    expect(prefillFrom(new URLSearchParams('new=1'))).toEqual({ conversation: null, person: null, name: null })
  })
})

describe('mock inputs from JSON Schema', () => {
  const ports = [
    { name: 'message', type: 'message' as const, description: 'The mail to look at' },
    { name: 'topic', type: 'text' as const },
    { name: 'limit', type: 'number' as const },
    { name: 'loud', type: 'bool' as const },
    { name: 'names', type: 'list' as const },
    { name: 'options', type: 'json' as const }
  ]
  const sample = { message: { id: 'm-seed', subject: 'Invoice 7', sender_email: 'eva.ortega@northwind.example', at: '2026-10-02T08:00:00Z' }, text: 'Invoice 7', tag: 'Important' }

  it('builds a schema with typed properties and entity shapes', () => {
    const schema = schemaOf(ports)
    expect(schema.required).toEqual(['message', 'topic', 'limit', 'loud', 'names', 'options'])
    expect(schema.properties?.message).toMatchObject({ type: 'object', 'x-picker': 'message', description: 'The mail to look at' })
    expect(schema.properties?.message.properties?.sender_email.format).toBe('email')
    expect(schema.properties?.limit.type).toBe('number')
    expect(schema.properties?.names.type).toBe('array')
    expect(schema.properties?.options.type).toBeUndefined()
  })

  it('generates one value per type and format, the same every time', () => {
    const schema: JsonSchema = {
      type: 'object',
      properties: {
        subject: { type: 'string' },
        body: { type: 'string' },
        email: { type: 'string' },
        name: { type: 'string' },
        url: { type: 'string' },
        when: { type: 'string', format: 'date-time' },
        day: { type: 'string', format: 'date' },
        score: { type: 'integer', minimum: 3, maximum: 5 },
        ratio: { type: 'number', minimum: 0, maximum: 1 },
        mood: { enum: ['calm', 'busy'] },
        ok: { type: 'boolean' },
        items: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' } } } },
        nested: { type: 'object', properties: { inner: { type: 'object', properties: { title: { type: 'string', maxLength: 5 } } } } }
      }
    }
    const value = mockOf(schema) as Record<string, any>
    expect(mockOf(schema)).toEqual(value)
    expect(typeof value.subject).toBe('string')
    expect(value.subject.length).toBeGreaterThan(3)
    expect(value.body.length).toBeGreaterThan(value.subject.length)
    expect(value.email).toMatch(/^[a-z.]+@[a-z.]+\.[a-z]+$/)
    expect(value.name).toMatch(/^\S+ \S+$/)
    expect(value.url).toMatch(/^https:\/\//)
    expect(Date.parse(value.when)).not.toBeNaN()
    expect(value.when).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/)
    expect(value.day).toMatch(/^\d{4}-\d{2}-\d{2}$/)
    expect(Number.isInteger(value.score)).toBe(true)
    expect(value.score).toBeGreaterThanOrEqual(3)
    expect(value.score).toBeLessThanOrEqual(5)
    expect(value.ratio).toBeGreaterThanOrEqual(0)
    expect(value.ratio).toBeLessThanOrEqual(1)
    expect(['calm', 'busy']).toContain(value.mood)
    expect(typeof value.ok).toBe('boolean')
    expect(value.items.length).toBeGreaterThanOrEqual(1)
    expect(value.items.length).toBeLessThanOrEqual(3)
    expect(typeof value.items[0].id).toBe('string')
    expect(value.nested.inner.title.length).toBeLessThanOrEqual(5)
    expect(mockProblems(schema, value)).toEqual([])
  })

  it('fills entity shaped inputs from the seeded sample and the rest from the schema', () => {
    const schema = schemaOf([...ports, { name: 'text', type: 'text' as const }, { name: 'label', type: 'tag' as const }])
    const value = mockWith(schema, sample, {}).value as Record<string, any>
    expect(value.message.id).toBe('m-seed')
    expect(value.message.subject).toBe('Invoice 7')
    expect(typeof value.message.unread).toBe('boolean')
    expect(value.text).toBe('Invoice 7')
    expect(value.label).toBe('Important')
    expect(value.topic).toBe((mockWith(schema, null, {}).value as Record<string, any>).topic)
    expect(mockProblems(schema, value)).toEqual([])
  })

  it('keeps edited fields across schema changes, drops removed ones and fills new ones', () => {
    const before = schemaOf(ports)
    const generated = mockWith(before, sample, {}).value as Record<string, any>
    const edit = mockEdit(before, sample, JSON.stringify({ ...generated, topic: 'Budget', limit: 77, message: { ...generated.message, subject: 'Edited' } }))
    expect(edit.problems).toEqual([])
    expect(edit.overrides).toEqual({ '/topic': 'Budget', '/limit': 77, '/message/subject': 'Edited' })
    const after = schemaOf([{ name: 'message', type: 'message' as const }, { name: 'topic', type: 'text' as const }, { name: 'limit', type: 'text' as const }, { name: 'extra', type: 'number' as const }])
    const merged = mockWith(after, sample, edit.overrides!)
    expect(merged.overrides).toEqual({ '/topic': 'Budget', '/message/subject': 'Edited' })
    const value = merged.value as Record<string, any>
    expect(value.topic).toBe('Budget')
    expect(value.message.subject).toBe('Edited')
    expect(value.message.id).toBe('m-seed')
    expect(typeof value.limit).toBe('string')
    expect(typeof value.extra).toBe('number')
    expect('loud' in value).toBe(false)
  })

  it('reports parse errors and schema problems in the dialog editor', () => {
    const schema = schemaOf(ports)
    const broken = mockEdit(schema, null, '{ "topic": ')
    expect(broken.overrides).toBeNull()
    expect(broken.problems[0].message).toMatch(/does not parse/)
    expect(mockEdit(schema, null, '[1]').overrides).toBeNull()
    const wrong = mockEdit(schema, null, JSON.stringify({ topic: 3, limit: 'many', loud: true, names: [], message: { sender_email: 'nope' } }))
    expect(wrong.problems.map((p) => p.path).sort()).toEqual(['limit', 'message.sender_email', 'options', 'topic'])
    expect(wrong.problems.find((p) => p.path === 'options')?.message).toBe('is missing')
    expect(wrong.problems.find((p) => p.path === 'message.sender_email')?.message).toBe('is not a valid email')
  })

  it('plays an action with the trigger payload plus its own inputs', () => {
    const graph: ActionGraph = { nodes: [{ id: 't', type: 'trigger', label: '', config: { on: 'arrival' }, body: '', x: 0, y: 0 }], edges: [] }
    expect(playPorts(graph, { inputs: [{ name: 'topic', type: 'text' }, { name: 'text', type: 'text' }] }).map((p) => p.name)).toEqual(['message', 'conversation', 'person', 'text', 'topic'])
    expect(playPorts({ nodes: [], edges: [] }, { inputs: [{ name: 'topic', type: 'text' }] }).map((p) => p.name)).toEqual(['topic'])
  })
})

describe('tree rows', () => {
  const action = (patch: Partial<Action>) => ({ id: 'a', name: 'A', enabled: true, last_status: null, ...patch }) as Action
  const row = (a: Action, depth = 0) => ({ kind: 'action' as const, id: a.id, depth, group: null, action: a })

  it('strikes a switched off action and marks only a failed one', () => {
    expect(treeRowState(row(action({ enabled: false })), null)).toEqual({ struck: true, selected: false, failed: false, indent: 0 })
    expect(treeRowState(row(action({ last_status: 'failed' }), 2), null)).toEqual({ struck: false, selected: false, failed: true, indent: 1.5 })
    expect(treeRowState(row(action({ last_status: 'done' })), null).failed).toBe(false)
  })

  it('selects only the chosen action', () => {
    expect(treeRowState(row(action({})), 'a').selected).toBe(true)
    expect(treeRowState({ kind: 'group', id: 'g', depth: 1, group: null, action: null }, 'g')).toEqual({ struck: false, selected: false, failed: false, indent: 0.75 })
  })

  it('reads the outputs a trigger gives by what it waits for', () => {
    expect(triggerOutputs('arrival').map((p) => p.name)).toEqual(['message', 'conversation', 'person', 'text'])
    expect(triggerOutputs('reaction').map((p) => p.name)).toContain('reaction')
    expect(triggerOutputs('event_soon').find((p) => p.name === 'event')?.type).toBe('json')
    expect(TRIGGER_ICONS.mention).toBe('lucide:at-sign')
  })
})

describe('built in box', () => {
  const group = (id: string, parent: string | null, key: string | null, position = 0, name = id) => ({ id, profile_id: 'p', key, builtin: !!key?.startsWith('builtin'), parent_id: parent, name, description: '', icon: null, position })
  const entry = (id: string, group_id: string | null, builtin: boolean, position = 0, name = id) => ({ id, group_id, builtin, position, name, description: `${name} description` })
  const groups = [
    group('mine', null, null),
    group('starter', null, 'starter', 1, 'Starter actions'),
    group('root', null, 'builtin', 10000, 'Built in'),
    group('triggers', 'root', 'builtin.triggers', 0, 'Triggers'),
    group('messages', 'triggers', 'builtin.triggers.messages', 0, 'Messages'),
    group('time', 'triggers', 'builtin.triggers.time', 1, 'Time'),
    group('steps', 'root', 'builtin.steps', 1, 'Steps'),
    group('ai', 'steps', 'builtin.steps.ai', 0, 'AI'),
    group('landings', 'root', 'builtin.landings', 2, 'Landings'),
    group('shipped', 'root', 'builtin.actions', 3, 'Actions')
  ]
  const actions = [
    entry('a1', 'mine', false, 0, 'Mine'),
    entry('a2', 'starter', false, 0, 'Sort into splits'),
    entry('t1', 'messages', true, 0, 'Message arrived'),
    entry('t2', 'messages', true, 1, 'Mention'),
    entry('t3', 'time', true, 0, 'On a schedule'),
    entry('s1', 'ai', true, 0, 'Ask AI'),
    entry('r1', 'shipped', true, 0, 'Receipts'),
    entry('h1', 'shipped', true, 1, 'Human or service')
  ]

  it('keeps your actions above and the built ins in their own box grouped by kind', () => {
    const parts = partition(groups, actions)
    expect(parts.mine.groups.map((g) => g.id)).toEqual(['mine', 'starter'])
    expect(parts.mine.actions.map((a) => a.id)).toEqual(['a1', 'a2'])
    expect(tree(parts.mine.groups, parts.mine.actions).map((i) => i.id)).toEqual(['mine', 'a1', 'starter', 'a2'])
    const shipped = tree(parts.builtin.groups, parts.builtin.actions)
    expect(shipped.filter((i) => i.depth === 0).map((i) => i.id)).toEqual(['triggers', 'steps', 'landings', 'shipped'])
    expect(shipped.map((i) => `${i.id}:${i.depth}`)).toEqual(['triggers:0', 'messages:1', 't1:2', 't2:2', 'time:1', 't3:2', 'steps:0', 'ai:1', 's1:2', 'landings:0', 'shipped:0', 'r1:1', 'h1:1'])
    expect(shipped.some((i) => i.id === 'root')).toBe(false)
    expect(tree(parts.builtin.groups, parts.builtin.actions, new Set(['triggers'])).map((i) => i.id)).toEqual(['triggers', 'steps', 'ai', 's1', 'landings', 'shipped', 'r1', 'h1'])
  })

  it('finds through the nesting and keeps the groups that hold a match', () => {
    const parts = partition(groups, actions)
    expect(tree(parts.builtin.groups, parts.builtin.actions, new Set(['triggers', 'messages']), 'mention').map((i) => i.id)).toEqual(['triggers', 'messages', 't2'])
    expect(tree(parts.builtin.groups, parts.builtin.actions, new Set(), 'time').map((i) => i.id)).toEqual(['triggers', 'time', 't3'])
    expect(tree(parts.mine.groups, parts.mine.actions, new Set(), 'mention')).toEqual([])
    expect(tree(groups, actions, new Set(), 'receipts').map((i) => i.id)).toEqual(['root', 'shipped', 'r1'])
  })

  it('names the built in groups in your language and shows a signature', () => {
    expect(groupName(groups[3])).toBe('Triggers')
    expect(groupName({ key: null, name: 'Mail', builtin: false })).toBe('Mail')
    expect(signatureLine([{ name: 'message', type: 'message' }], [{ name: 'human', type: 'bool' }])).toBe('message: message → human: bool')
    expect(signatureLine([], [{ name: 'text', type: 'text' }])).toBe('– → text: text')
  })
})

describe('prompts inside nodes', () => {
  const base: Prompt = { key: 'sort', name: 'Sort', description: '', connection_id: null, model: null, system: 'S', template: 'T', output_schema: '{}', variables: [] }

  it('finds the prompts a script uses', () => {
    expect(promptsUsed('let a = prompt("sort", #{});\nlet b = prompt( "sort");\nai_json("x", #{});\nai("y")')).toEqual(['sort', 'script_ai_json', 'script_ai'])
    expect(promptsUsed('let x = 1')).toEqual([])
  })

  it('keeps only what differs from the default', () => {
    expect(promptOverride(base, { ...base })).toBeNull()
    expect(promptOverride(base, { ...base, system: 'Mine', model: ' small ', output_schema: null })).toEqual({ system: 'Mine', model: 'small', output_schema: '' })
  })

  it('takes the prompt from Content in place of its own text, never both', () => {
    expect(promptOverride(base, { ...base, template: 'Own text', content_id: 'c1' })).toEqual({ content_id: 'c1' })
    expect(promptOverride(base, { ...base, template: 'Own text', content_id: null })).toEqual({ template: 'Own text' })
    const content = [{ id: 'c1', name: 'Polite no', body: 'Say no to {{ .person.name }}' }]
    expect(resolvePrompt(base, { content_id: 'c1' }, content).template).toBe('Say no to {{ .person.name }}')
    expect(resolvePrompt(base, { content_id: 'gone' }, content).template).toBe('T')
  })

  it('layers an override over the default and stores it on the node', () => {
    expect(resolvePrompt(base, { model: 'x', output_schema: '' })).toMatchObject({ system: 'S', template: 'T', model: 'x', output_schema: null })
    const tuned = withPrompt(node('n', 'script'), 'sort', { model: 'x' })
    expect(tuned.config?.prompts).toEqual({ sort: { model: 'x' } })
    expect(withPrompt(tuned, 'sort', null).config).toEqual({})
  })
})

describe('node requirements', () => {
  const need = (node: string, setting: string, message: string): ActionDiagnostic => ({ severity: 'requirement', message, node, edge: null, line: null, column: null, setting })
  const diagnostics: ActionDiagnostic[] = [
    need('ask', 'ai.connection', 'An AI connection'),
    need('ask', 'ai.default-model', 'A default AI model'),
    need('send', 'accounts.connections', 'A connected account'),
    need('ask', 'ai.connection', 'An AI connection'),
    need('ask', 'nowhere.at-all', 'Unknown'),
    { severity: 'error', message: 'Broken', node: 'ask', edge: null, line: 2, column: 1 }
  ]

  it('keeps requirements apart from problems', () => {
    expect(problemsFor(diagnostics, 'ask', 'node').map((d) => d.message)).toEqual(['Broken'])
  })

  it('lists each known setting a node needs once, with its settings link', () => {
    expect(requirementsFor(diagnostics, 'ask')).toEqual([
      { setting: 'ai.connection', label: 'An AI connection', href: '/main/profile/ai#ai.connection' },
      { setting: 'ai.default-model', label: 'A default AI model', href: '/main/profile/ai#ai.default-model' }
    ])
    expect(requirementsFor(diagnostics).map((r) => r.setting)).toEqual(['ai.connection', 'ai.default-model', 'accounts.connections'])
    expect(requirementsFor(diagnostics, 'done')).toEqual([])
  })

  it('shows a badge only when something is missing and links to the first setting', () => {
    expect(requirementBadge([])).toBeNull()
    const badge = requirementBadge(requirementsFor(diagnostics, 'ask'))
    expect(badge).toMatchObject({ count: 2, label: '2 settings', href: '/main/profile/ai#ai.connection' })
    expect(badge?.title).toContain('An AI connection, A default AI model')
    expect(requirementBadge(requirementsFor(diagnostics, 'send'))).toMatchObject({ count: 1, label: '1 setting', href: '/main/profile/accounts#accounts.connections' })
  })
})

describe('chat nodes and where an action lands', () => {
  const chatCatalog = {
    ...catalog,
    builtins: [...catalog.builtins, { key: 'tag_add', label: 'Add tag', icon: 'lucide:tag', description: '', inputs: [], outputs: [], body: '' }, { key: 'present_invite', label: 'Show as invite', icon: 'lucide:calendar-check', description: '', inputs: [], outputs: [], body: '' }],
    chat_node: {
      key: 'chat',
      label: 'Chat',
      icon: 'lucide:messages-square',
      description: '',
      config: {},
      variants: {
        read: { label: 'Read chat', icon: 'lucide:book-open', description: '', start: '', end: '', inputs: [{ name: 'items', type: 'list' }, { name: 'item', type: 'json' }], outputs: [{ name: 'item', type: 'json' }] },
        write: { label: 'Write chat', icon: 'lucide:pen-line', description: '', start: '', end: '', inputs: [{ name: 'text', type: 'text' }], outputs: [{ name: 'message', type: 'message' }] }
      }
    }
  } as unknown as ActionCatalog
  const action = (id: string, graph: ActionGraph, patch: Partial<Action> = {}) => ({ id, graph, usage: { standalone: false, chat: false, step: false, landing: false }, outputs: [], ...patch }) as Action
  const edge = (source: string, target: string, port = 'value') => ({ id: `${source}-${target}`, source, source_port: port, target, target_port: port })

  it('reads chat node ports from the schema by variant', () => {
    expect(chatOf(node('c', 'chat', { mode: 'read' }))).toEqual({ mode: 'read', conversation_id: null })
    expect(chatOf(node('c', 'chat', { conversation_id: 'x' }))).toEqual({ mode: 'write', conversation_id: 'x' })
    expect(chatOf(node('c', 'builtin:done'))).toBeNull()
    expect(portsOf(node('c', 'chat', { mode: 'read' }), chatCatalog, owner)!.inputs.map((p) => p.name)).toEqual(['items', 'item'])
    expect(portsOf(node('c', 'chat', { mode: 'write' }), chatCatalog, owner)!.inputs.map((p) => p.name)).toEqual(['text'])
  })

  it('lands in a read chat, a write chat, tags and the calendar, with the node icons', () => {
    const graph: ActionGraph = {
      nodes: [node('start', 'chat', { mode: 'write', conversation_id: 'team' }), node('tag', 'builtin:tag_add'), { ...node('s', 'script'), body: 'create_event(event, message); #{}' }, node('into', 'chat', { mode: 'read', conversation_id: 'log' }), node('reply', 'builtin:reply')],
      edges: [edge('start', 'tag'), edge('start', 'into')]
    }
    const found = landings(action('a', graph), [], chatCatalog)
    expect(found.map((l) => [l.kind, l.icon])).toEqual([
      ['tag', 'lucide:tag'],
      ['calendar', 'lucide:calendar-plus'],
      ['read_chat', 'lucide:book-open'],
      ['write_chat', 'lucide:pen-line']
    ])
    expect(landings(action('b', { nodes: [node('start', 'chat', { mode: 'write' })], edges: [] }), [], chatCatalog)).toEqual([])
  })

  it('lands in the chat it is attached to when it gives records, and through nested actions', () => {
    const receipts = action('r', { nodes: [node('organise', 'builtin:receipt_organise')], edges: [] }, { usage: { standalone: false, chat: true, step: true, landing: false }, outputs: [{ name: 'records', type: 'list' }] })
    expect(landings(receipts, [], chatCatalog).map((l) => l.kind)).toEqual(['read_chat'])
    const inner = action('inner', { nodes: [node('i', 'builtin:present_invite')], edges: [] })
    const outer = action('outer', { nodes: [node('n', 'action:inner'), node('loop', 'action:outer')], edges: [] })
    expect(landings(outer, [inner, outer], chatCatalog).map((l) => [l.kind, l.icon])).toEqual([['present', 'lucide:calendar-check']])
    const title = landingTitle(landings(receipts, [], chatCatalog), ['Receipts'])
    expect(title).toContain('Receipts')
    expect(landingTitle([], [])).not.toBe('')
  })
})

describe('outputs the final nodes give', () => {
  const graph: ActionGraph = {
    nodes: [node('t', 'trigger'), { ...node('sum', 'builtin:summarize'), label: 'Gist' }, node('d', 'builtin:done')],
    edges: [{ id: 'e1', source: 't', source_port: 'text', target: 'sum', target_port: 'text' }]
  }

  it('shows each output read only with the node it comes from', () => {
    const outputs = [
      { name: 'summary', type: 'text' as const, node: 'sum', source_port: 'summary' },
      { name: 'conversation', type: 'conversation' as const, node: 'd', source_port: 'conversation' },
      { name: 'stale', type: 'text' as const, node: 'gone', source_port: 'stale' },
      { name: 'loose', type: 'text' as const }
    ]
    expect(outputSources(outputs, graph, catalog)).toEqual([{ node: 'sum', label: 'Gist' }, { node: 'd', label: 'Mark done' }, null, null])
    expect(outputSources([], graph, catalog)).toEqual([])
  })

  it('renames and describes a port on the final node and forgets what matches the default', () => {
    const before = [{ name: 'summary', type: 'text' as const, description: '', node: 'sum', source_port: 'summary' }]
    const renamed = withReturns(graph.nodes[1], before, [{ ...before[0], name: 'gist', description: 'The short of it' }])
    expect(renamed.config?.returns).toEqual({ summary: { name: 'gist', description: 'The short of it' } })
    const back = withReturns(renamed, [{ ...before[0], name: 'gist', description: 'The short of it' }], [{ ...before[0], name: 'summary', description: '' }])
    expect(back.config).not.toHaveProperty('returns')
  })
})

describe('nested action frames', () => {
  const script = (id: string, inputs: string[], outputs: string[], x = 0, y = 0): ActionNode => ({ id, type: 'script', label: '', config: { inputs: inputs.map((name) => ({ name, type: 'text' })), outputs: outputs.map((name) => ({ name, type: 'text' })) }, body: '', x, y })
  const use = (id: string, action: string, x = 0, y = 0, config: Record<string, unknown> = {}): ActionNode => ({ id, type: `action:${action}`, label: '', config, body: '', x, y })
  const entry = (id: string, graph: ActionGraph, extra: Record<string, unknown> = {}) => ({ id, name: id.toUpperCase(), icon: null, key: null, builtin: false, enabled: true, inputs: [], outputs: [], group_id: null, graph, diagnostics: [], ...extra })
  const multi = entry('multi', {
    nodes: [{ id: 'start', type: 'trigger', label: '', config: { on: 'called' }, body: '', x: 0, y: 0 }, script('a', ['text'], ['upper'], 300, 0), script('b', ['text'], ['loud'], 600, 40)],
    edges: [
      { id: 'start.text-a.text', source: 'start', source_port: 'text', target: 'a', target_port: 'text' },
      { id: 'a.upper-b.text', source: 'a', source_port: 'upper', target: 'b', target_port: 'text' }
    ]
  }, { inputs: [{ name: 'text', type: 'text' }], outputs: [{ name: 'loud', type: 'text', node: 'b', source_port: 'loud' }] })
  const one = entry('one', { nodes: [script('only', [], ['x'])], edges: [] })
  const outer = entry('outer', { nodes: [script('first', [], ['x']), use('inner', 'multi', 400, 0)], edges: [] })
  const loop = entry('loop', { nodes: [script('step', [], ['x']), use('again', 'loop', 400, 0)], edges: [] })
  const chain = Array.from({ length: MAX_DEPTH + 3 }, (_, i) => entry(`c${i}`, { nodes: [script('s', [], ['x']), use('next', `c${i + 1}`, 400, 0)], edges: [] }))
  const known = { builtins: [], actions: [multi, one, outer, loop, ...chain] } as unknown as ActionCatalog
  const parent: ActionGraph = {
    nodes: [{ id: 't', type: 'trigger', label: '', config: { on: 'manual' }, body: '', x: 0, y: 0 }, use('n', 'multi', 300, 0), use('s', 'one', 300, 400), script('r', ['loud'], [], 900, 0)],
    edges: [
      { id: 't.text-n.text', source: 't', source_port: 'text', target: 'n', target_port: 'text' },
      { id: 'n.loud-r.loud', source: 'n', source_port: 'loud', target: 'r', target_port: 'loud' }
    ]
  }
  const me = { inputs: [] }

  it('sizes a frame from the stored positions of its nodes and the card sizes', () => {
    const bounds = frameBounds([{ x: 100, y: 200, width: 240, height: 120 }, { x: 500, y: 200, width: 240, height: 120 }], 2)
    const top = FRAME_HEAD + 2 * PORT_ROW + 8
    expect(bounds.scale).toBe(1)
    expect(bounds.positions).toEqual([{ x: FRAME_PAD, y: top + FRAME_PAD }, { x: FRAME_PAD + 400, y: top + FRAME_PAD }])
    expect(bounds.width).toBe(FRAME_PAD + 400 + 240 + FRAME_PAD)
    expect(bounds.height).toBe(top + FRAME_PAD + 120 + FRAME_PAD)
    expect(frameBounds([], 0)).toEqual({ width: FRAME_MIN_WIDTH, height: FRAME_HEAD + FRAME_PAD, scale: 1, positions: [] })
  })

  it('scales positions only when the nodes would not fit', () => {
    const wide = frameBounds([{ x: 0, y: 0, width: 240, height: 120 }, { x: 5000, y: 0, width: 240, height: 120 }], 0)
    expect(wide.scale).toBeLessThan(1)
    expect(wide.width).toBeLessThanOrEqual(FRAME_MAX_WIDTH)
    expect(wide.positions[1].x).toBeGreaterThan(wide.positions[0].x)
  })

  it('frames an action of several nodes and keeps a single node action as one card', () => {
    const plan = graphPlan(parent, known, me)
    expect(plan.nodes.map((n) => [n.id, n.kind, n.parent])).toEqual([['t', 'card', null], ['n', 'frame', null], ['n/start', 'card', 'n'], ['n/a', 'card', 'n'], ['n/b', 'card', 'n'], ['s', 'card', null], ['r', 'card', null]])
    const frame = plan.nodes.find((n) => n.id === 'n')!
    const children = plan.nodes.filter((n) => n.parent === 'n')
    for (const child of children) {
      expect(child.owner?.id).toBe('multi')
      expect(child.x + child.width).toBeLessThanOrEqual(frame.width)
      expect(child.y + child.height).toBeLessThanOrEqual(frame.height)
    }
    expect(plan.nodes.find((n) => n.id === 'n/a')!.x - plan.nodes.find((n) => n.id === 'n/start')!.x).toBe(300)
    expect(plan.edges.filter((e) => e.locked).map((e) => [e.id, e.source, e.target])).toEqual([['n/start.text-a.text', 'n/start', 'n/a'], ['n/a.upper-b.text', 'n/a', 'n/b']])
    expect(plan.nodes.find((n) => n.id === 's')!.nested?.id).toBe('one')
  })

  it('refuses every change inside a frame but lets it be selected', () => {
    const plan = graphPlan(parent, known, me)
    for (const gesture of ['drag', 'connect', 'delete', 'edit', 'rewire'] as const) {
      expect(refuses(plan, 'n/a', gesture)).toBe(true)
      expect(refuses(plan, 'n/a.upper-b.text', gesture)).toBe(true)
      expect(refuses(plan, 'n', gesture)).toBe(false)
      expect(refuses(plan, 't.text-n.text', gesture)).toBe(false)
    }
    expect(refuses(plan, 'n/a', 'select')).toBe(false)
    expect(refuses(plan, 'n/a', 'inspect')).toBe(false)
    expect(partOf(parent, known, 'n/a')).toMatchObject({ node: { id: 'a' }, owner: { id: 'multi' }, root: 'n' })
    expect(partOf(parent, known, 'n')).toBeNull()
  })

  it('collapses a frame to one box and keeps that on the node', () => {
    const folded = withCollapsed(parent, 'n', true)
    expect(folded.nodes.find((n) => n.id === 'n')!.config).toMatchObject({ collapsed: true })
    const plan = graphPlan(folded, known, me)
    expect(plan.nodes.find((n) => n.id === 'n')).toMatchObject({ kind: 'card', collapsed: true })
    expect(plan.nodes.some((n) => n.parent === 'n')).toBe(false)
    expect(plan.edges.some((e) => e.locked)).toBe(false)
    expect(withCollapsed(folded, 'n', false).nodes.find((n) => n.id === 'n')!.config).not.toHaveProperty('collapsed')
    const host: ActionGraph = { nodes: [use('o', 'outer')], edges: [] }
    expect(graphPlan(host, known, me).nodes.find((n) => n.id === 'o/inner')!.kind).toBe('frame')
    const inner = withCollapsed(host, 'o/inner', true)
    expect(inner.nodes[0].config).toMatchObject({ folds: { inner: true } })
    expect(graphPlan(inner, known, me).nodes.find((n) => n.id === 'o/inner')).toMatchObject({ kind: 'card', collapsed: true })
    expect(graphPlan(inner, known, me).nodes.some((n) => n.parent === 'o/inner')).toBe(false)
  })

  it('stops nesting at the engine depth and at a loop with a more box', () => {
    const plan = graphPlan({ nodes: [use('c', 'c0')], edges: [] }, known, me)
    const frames = plan.nodes.filter((n) => n.kind === 'frame')
    const more = plan.nodes.filter((n) => n.kind === 'more')
    expect(frames).toHaveLength(MAX_DEPTH)
    expect(more).toHaveLength(1)
    expect(more[0].depth).toBe(MAX_DEPTH)
    expect(more[0].id.split('/')).toHaveLength(MAX_DEPTH + 1)
    const looped = graphPlan({ nodes: [use('l', 'loop')], edges: [] }, known, me)
    expect(looped.nodes.map((n) => [n.id, n.kind])).toEqual([['l', 'frame'], ['l/step', 'card'], ['l/again', 'more']])
  })

  it('clones a framed action into the graph with its wiring', () => {
    const next = inlineAction(parent, 'n', known, me)
    expect(next.nodes.map((n) => n.id).sort()).toEqual(['a', 'b', 'r', 's', 't'])
    expect(next.nodes.find((n) => n.id === 'a')).toMatchObject({ x: 300, y: 0 })
    expect(next.nodes.find((n) => n.id === 'b')).toMatchObject({ x: 600, y: 40 })
    expect(next.edges.map((e) => `${e.source}.${e.source_port}>${e.target}.${e.target_port}`).sort()).toEqual(['a.upper>b.text', 'b.loud>r.loud', 't.text>a.text'])
    const fixed = inlineAction({ nodes: [use('n', 'multi', 0, 0, { values: { text: 'hei' } })], edges: [] }, 'n', known, me)
    expect(fixed.nodes.find((n) => n.id === 'a')!.config).toMatchObject({ values: { text: 'hei' } })
  })
})

describe('code view', () => {
  const at = (id: string, type: string, x: number, y: number, body = '', config: Record<string, unknown> = {}): ActionNode => ({ id, type, label: id, config, body, x, y })
  const catalog = {
    builtins: [{ key: 'tag_add', label: 'Add tag', icon: 'lucide:tag', description: '', inputs: [], outputs: [], body: '#{ conversation: tag_add(conversation, tag) }' }],
    actions: [],
    prompts: [{ key: 'script_ai', name: 'Script AI', description: '', connection_id: null, model: null, system: 'sys', template: 'Hello {{.text}}', output_schema: null, variables: [] }],
    content_prompts: [],
    functions: []
  } as unknown as ActionCatalog
  const graph: ActionGraph = {
    nodes: [at('late', 'script', 0, 50, 'ai("x")'), at('tag', 'builtin:tag_add', 0, 10), at('t', 'trigger', 0, 400, 'true', { on: 'arrival' }), at('chat', 'chat', 0, 0), at('first', 'script', 0, 900, 'x')],
    edges: [
      { id: 'e1', source: 't', source_port: 'text', target: 'first', target_port: 'text' },
      { id: 'e2', source: 'first', source_port: 'value', target: 'late', target_port: 'text' },
      { id: 'e3', source: 'late', source_port: 'value', target: 'tag', target_port: 'tag' }
    ]
  }

  it('orders code bearing nodes from the trigger along the wires and skips chats', () => {
    const items = codeItems(graph, catalog, { inputs: [] })
    expect(items.map((i) => i.id)).toEqual(['t', 'first', 'late', 'tag'])
    expect(items[0].sections[0]).toMatchObject({ kind: 'body', readonly: false, value: 'true' })
    expect(items[2].sections.map((s) => s.kind)).toEqual(['body', 'prompt'])
    expect(items[2].sections[1]).toMatchObject({ value: 'Hello {{.text}}', readonly: false, prompt: 'script_ai' })
    expect(items[3]).toMatchObject({ badge: 'builtin' })
    expect(items[3].sections[0]).toMatchObject({ readonly: true, value: '#{ conversation: tag_add(conversation, tag) }' })
    expect(codeItems(graph, catalog, { inputs: [] }, true).every((i) => i.sections.every((s) => s.readonly))).toBe(true)
  })

  it('saves edits through the node body and the prompt override', () => {
    const items = codeItems(graph, catalog, { inputs: [] })
    const late = items[2]
    expect(withCode(late.node, late.sections[0], 'ai("y")', catalog).body).toBe('ai("y")')
    const prompted = withCode(late.node, late.sections[1], 'Bye {{.text}}', catalog)
    expect((prompted.config?.prompts as Record<string, { template: string }>).script_ai.template).toBe('Bye {{.text}}')
    expect(withCode(items[3].node, items[3].sections[0], 'nope', catalog)).toBe(items[3].node)
  })

  it('folds one view at a time and focuses the node selected in the graph', () => {
    const items = codeItems(graph, catalog, { inputs: [] })
    const folded = foldToggle(new Set(), 'late')
    expect([...folded]).toEqual(['late'])
    expect([...foldToggle(folded, 'late')]).toEqual([])
    expect(codeFocus(items, 'late')).toBe('late')
    expect(codeFocus(items, 'late/inner')).toBe('late')
    expect(codeFocus(items, 'chat')).toBeNull()
    expect(codeFocus(items, null)).toBeNull()
    expect(graphOrder({ nodes: graph.nodes, edges: [...graph.edges, { id: 'loop', source: 'tag', source_port: 'x', target: 'first', target_port: 'y' }] }).map((n) => n.id)).toEqual(['t', 'chat', 'tag', 'late', 'first'])
  })
})

describe('run log', () => {
  const run = (patch: Partial<ActionRun>): ActionRun => ({ id: 'r1', action_id: 'a1', action_name: 'A', result: { kind: 'none', message: '', commands: [], content: null, outputs: {} }, status: 'done', dry_run: false, trigger: { trigger: 't', payload: { message: { sender: 'Anna Berg' } } }, log: [], outputs: {}, state: null, error: null, resume_at: null, depth: 0, started_at: '2026-10-15T10:00:00.000Z', finished_at: '2026-10-15T10:00:01.500Z', ...patch })

  it('summarises a run in one row', () => {
    expect(runRow(run({ outputs: { summary: 'Paid\ntoday', count: 2 } }))).toMatchObject({ source: 'Message from Anna Berg', duration: '1.5 s', summary: 'summary: Paid today · count: 2', failed: false, dry: false, live: false })
    expect(runRow(run({ status: 'failed', error: 'tag: no such tag' }))).toMatchObject({ summary: 'tag: no such tag', failed: true })
    expect(runRow(run({ status: 'failed', error: 'ask: Needs setting ai.claude: Claude Code signed in', requirement: { severity: 'requirement', message: 'Claude Code signed in', node: null, edge: null, line: null, column: null, setting: 'ai.claude' } }))).toMatchObject({ summary: 'Claude Code signed in', failed: false, status: 'requirement', label: 'needs a setting', needs: [{ setting: 'ai.claude', href: '/main/profile/ai#ai.claude' }] })
    expect(runRow(run({ dry_run: true }))).toMatchObject({ dry: true, source: 'Mock input', summary: 'No outputs' })
    expect(runRow(run({ status: 'running', finished_at: null }))).toMatchObject({ live: true, duration: null })
    expect(runRow(run({ trigger: { inputs: {}, payload: null } })).source).toBe('Manual')
    expect(runRow(run({ trigger: { chat: 'c1', mode: 'read', payload: null } })).source).toBe('Chat')
    expect(runRow(run({ depth: 1 })).source).toBe('Called by another action')
  })

  it('expands a run into its steps with input, output, duration and errors', () => {
    const steps = runSteps(run({
      log: [
        { node: 't', kind: 'trigger', status: 'done', detail: 't fired' },
        { node: 'tag', kind: 'effect', status: 'would', detail: 'tag_add(Important)' },
        { node: 'tag', kind: 'builtin:tag_add', status: 'done', detail: 'tag gave conversation' },
        { node: 'tag', kind: 'step', status: 'done', detail: JSON.stringify({ inputs: { tag: 'Important' }, outputs: { conversation: { id: 'c1' } }, ms: 12 }) },
        { node: 'send', kind: 'script', status: 'failed', detail: 'no chat' },
        { node: 'send', kind: 'step', status: 'failed', detail: JSON.stringify({ inputs: {}, outputs: null, ms: 3 }) }
      ]
    }))
    expect(steps.map((s) => s.node)).toEqual(['t', 'tag', 'send'])
    expect(steps[1]).toMatchObject({ status: 'done', details: ['tag_add(Important)', 'tag gave conversation'], inputs: { tag: 'Important' }, outputs: { conversation: { id: 'c1' } }, ms: 12, error: null })
    expect(steps[2]).toMatchObject({ status: 'failed', error: 'no chat', ms: 3 })
    const open = foldToggle(new Set(), 'r1')
    expect(open.has('r1')).toBe(true)
    expect(foldToggle(open, 'r1').has('r1')).toBe(false)
  })
})

describe('assistant chat', () => {
  const draftWith = (name: string, graph: ActionGraph) => ({ name, description: '', icon: null, max_sends_per_hour: null, inputs: [], graph })
  const preview = (more: Partial<ActionOperationPreview>): ActionOperationPreview => ({ index: 0, op: 'update', action_id: 'a1', name: 'A', group_before: null, group_after: null, before: null, after: null, diagnostics: [], ...more })

  it('sends every message on its own with the proposals and what became of them', () => {
    const turns = [
      assistTurn('user', 'Tag invoices', null),
      assistTurn('assistant', 'Creates Invoices', null, { operations: [{ op: 'rename', action_id: 'a1', name: 'Invoices' }], status: 'discarded' }),
      assistTurn('user', 'Call it Bills', 'a1'),
      assistTurn('assistant', 'That did not work', 'a1', { failed: true }),
      assistTurn('assistant', 'Renames it', 'a1', { errors: ['operation 1 (rename): an action needs a name'] })
    ]
    const history = assistHistory(turns)
    expect(history.map((m) => m.role)).toEqual(['user', 'assistant', 'user', 'assistant'])
    expect(history[0]).toEqual({ role: 'user', content: 'Tag invoices' })
    expect(JSON.parse(history[1].content)).toEqual({ reply: 'Creates Invoices', operations: [{ op: 'rename', action_id: 'a1', name: 'Invoices' }] })
    expect(history[1].status).toBe('discarded')
    expect(JSON.parse(history[3].content).errors).toEqual(['operation 1 (rename): an action needs a name'])
  })

  it('counts what an operation adds, removes, changes and rewires, ignoring moved cards', () => {
    const before = { nodes: [node('t', 'trigger'), node('s', 'builtin:summarize'), node('d', 'builtin:done')], edges: [{ id: 't.text-s.text', source: 't', source_port: 'text', target: 's', target_port: 'text' }] }
    const after = { nodes: [{ ...node('t', 'trigger'), x: 400 }, node('s', 'builtin:summarize', { values: { text: 'x' } }), node('n', 'builtin:notify')], edges: [{ id: 't.text-n.title', source: 't', source_port: 'text', target: 'n', target_port: 'title' }] }
    const change = operationChange(preview({ before: draftWith('A', before), after: draftWith('B', after), group_before: null, group_after: 'g1' }))
    expect(change.added).toEqual(['n'])
    expect(change.removed).toEqual(['d'])
    expect(change.changed).toEqual(['s'])
    expect(change.wires).toBe(2)
    expect(change.renamed).toEqual({ from: 'A', to: 'B' })
    expect(change.moved).toBe(true)
    const created = operationChange(preview({ op: 'create', action_id: 'new-1', after: draftWith('New', after) }))
    expect(created.added).toEqual(['t', 's', 'n'])
    expect(created.renamed).toBeNull()
    expect(created.moved).toBe(false)
    expect(operationChange(preview({ op: 'delete', before: draftWith('A', before) })).removed).toEqual(['t', 's', 'd'])
  })

  it('opens the real action behind a proposal once it is applied', () => {
    const made = preview({ op: 'create', action_id: 'new-1', after: draftWith('New', { nodes: [], edges: [] }) })
    expect(operationTarget(made, {})).toBeNull()
    expect(operationTarget(made, { 'new-1': 'a9' })).toBe('a9')
    expect(operationTarget(preview({ before: draftWith('A', { nodes: [], edges: [] }), after: draftWith('A', { nodes: [], edges: [] }) }), {})).toBe('a1')
    expect(operationTarget(preview({ op: 'delete', before: draftWith('A', { nodes: [], edges: [] }) }), {})).toBeNull()
  })
})
