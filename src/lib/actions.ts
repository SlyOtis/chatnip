import dagre from '@dagrejs/dagre'
import { formatDate, t, type MessageKey } from '$lib/i18n.svelte'
import { requirementAnchor, settingHref } from '$lib/settings'
import type {
  Action,
  ActionAssistMessage,
  ActionCatalog,
  ActionCatalogEntry,
  ActionDiagnostic,
  ActionEdge,
  ActionGraph,
  ActionGroup,
  ActionNode,
  ActionOperation,
  ActionOperationPreview,
  ActionPort,
  ActionRun,
  ActionSample,
  ActionUsage,
  BuiltinKind,
  ChatMode,
  ContentPrompt,
  NodeRole,
  PortType,
  PostActionInput,
  Prompt,
  PromptOverride,
  TriggerConfig,
  TriggerOn
} from '$lib/tauri/types'

export const NODE_TRIGGER = 'trigger'
export const NODE_SCRIPT = 'script'
export const NODE_CHAT = 'chat'
export const BUILTIN = 'builtin:'
export const ACTION = 'action:'
export const NODE_WIDTH = 240
export const NODE_HEIGHT = 120
export const IN = 'in:'
export const OUT = 'out:'

export const TRIGGER_OUTPUTS: ActionPort[] = [
  { name: 'message', type: 'message' },
  { name: 'conversation', type: 'conversation' },
  { name: 'person', type: 'person' },
  { name: 'text', type: 'text' }
]

const TRIGGER_EXTRA: Partial<Record<TriggerOn, ActionPort[]>> = {
  reaction: [{ name: 'reaction', type: 'text' }],
  event_soon: [{ name: 'event', type: 'json' }],
  schedule: [{ name: 'at', type: 'text' }],
  custom: [
    { name: 'payload', type: 'json' },
    { name: 'value', type: 'json' },
    { name: 'previous', type: 'json' }
  ]
}

export const TRIGGER_ICONS: Record<TriggerOn, string> = {
  arrival: 'lucide:inbox',
  sent: 'lucide:send',
  edited: 'lucide:pencil',
  deleted: 'lucide:trash',
  reaction: 'lucide:smile-plus',
  mention: 'lucide:at-sign',
  filter: 'lucide:filter',
  schedule: 'lucide:calendar-clock',
  event_soon: 'lucide:alarm-clock',
  manual: 'lucide:play',
  called: 'lucide:git-branch',
  custom: 'lucide:webhook'
}

export function triggerOutputs(on: TriggerOn): ActionPort[] {
  const extra = TRIGGER_EXTRA[on]
  return extra ? [...TRIGGER_OUTPUTS, ...extra] : TRIGGER_OUTPUTS
}

export interface Owner {
  inputs: ActionPort[]
}

export function accepts(to: PortType, from: PortType): boolean {
  return to === from || to === 'any' || from === 'any' || to === 'json' || (to === 'text' && (from === 'tag' || from === 'number' || from === 'bool'))
}

export interface ChatNodeConfig {
  mode: ChatMode
  conversation_id: string | null
}

export function chatOf(node: ActionNode): ChatNodeConfig | null {
  if (node.type !== NODE_CHAT) return null
  const config = (node.config ?? {}) as Partial<ChatNodeConfig>
  return { mode: config.mode === 'read' ? 'read' : 'write', conversation_id: config.conversation_id || null }
}

export function triggerOf(node: ActionNode): TriggerConfig | null {
  if (node.type !== NODE_TRIGGER) return null
  const config = (node.config ?? {}) as Partial<TriggerConfig>
  return { on: config.on ?? 'arrival', scope: config.scope, group_ids: config.group_ids ?? [], direction: config.direction ?? null, message_kinds: config.message_kinds ?? [], minutes_before: config.minutes_before ?? null, schedule: config.schedule ?? null, disabled: config.disabled, labels: config.labels }
}

function declared(node: ActionNode, key: 'inputs' | 'outputs'): ActionPort[] {
  const value = node.config?.[key]
  return Array.isArray(value) ? (value as ActionPort[]) : []
}

export function portsOf(node: ActionNode, catalog: ActionCatalog | null, owner: Owner): { inputs: ActionPort[]; outputs: ActionPort[] } | null {
  if (node.type === NODE_TRIGGER) {
    const on = triggerOf(node)?.on ?? 'arrival'
    const labels = isRecord(node.config?.labels) ? (node.config.labels as Record<string, unknown>) : {}
    const base = triggerOutputs(on).map((p) => (typeof labels[p.name] === 'string' && labels[p.name] ? { ...p, label: labels[p.name] as string } : p))
    const extra = on === 'manual' || on === 'called' ? owner.inputs.filter((p) => !base.some((o) => o.name === p.name)) : []
    return { inputs: [], outputs: [...base, ...extra] }
  }
  if (node.type === NODE_SCRIPT) return { inputs: declared(node, 'inputs'), outputs: declared(node, 'outputs') }
  const chat = chatOf(node)
  if (chat) {
    const variant = catalog?.chat_node?.variants[chat.mode]
    return variant ? { inputs: variant.inputs, outputs: variant.outputs } : null
  }
  if (node.type.startsWith(BUILTIN)) {
    const builtin = catalog?.builtins.find((b) => b.key === node.type.slice(BUILTIN.length))
    return builtin ? { inputs: builtin.inputs, outputs: builtin.outputs } : null
  }
  if (node.type.startsWith(ACTION)) {
    const action = catalog?.actions.find((a) => a.id === node.type.slice(ACTION.length))
    return action ? { inputs: action.role === 'trigger' ? [] : action.inputs, outputs: action.outputs } : null
  }
  return null
}

export function roleOf(node: ActionNode, catalog: ActionCatalog | null): NodeRole {
  if (node.type === NODE_TRIGGER) return 'trigger'
  if (node.type === NODE_CHAT) return 'chat'
  if (node.type.startsWith(BUILTIN)) {
    const kind = catalog?.builtins.find((b) => b.key === node.type.slice(BUILTIN.length))?.kind
    return kind === 'trigger' || kind === 'landing' ? kind : 'step'
  }
  if (node.type.startsWith(ACTION)) return catalog?.actions.find((a) => a.id === node.type.slice(ACTION.length))?.role ?? 'step'
  return 'step'
}

export function graphRole(graph: ActionGraph, node: ActionNode, catalog: ActionCatalog | null): NodeRole {
  const role = roleOf(node, catalog)
  if (role !== 'chat') return role
  if (graph.edges.some((e) => e.target === node.id)) return 'landing'
  if (graph.edges.some((e) => e.source === node.id)) return 'trigger'
  return 'chat'
}

export interface UsageBadge {
  key: keyof ActionUsage
  icon: string
  label: string
  hint: string
}

const USAGE: { key: keyof ActionUsage; icon: string; label: MessageKey; hint: MessageKey }[] = [
  { key: 'standalone', icon: 'lucide:radio-tower', label: 'actions.usage.standalone', hint: 'actions.usage.standaloneHint' },
  { key: 'chat', icon: 'lucide:messages-square', label: 'actions.usage.chat', hint: 'actions.usage.chatHint' },
  { key: 'step', icon: 'lucide:workflow', label: 'actions.usage.step', hint: 'actions.usage.stepHint' },
  { key: 'landing', icon: 'lucide:map-pin', label: 'actions.usage.landing', hint: 'actions.usage.landingHint' }
]

export function usageBadges(usage: ActionUsage | null | undefined): UsageBadge[] {
  return usage ? USAGE.filter((u) => usage[u.key]).map((u) => ({ key: u.key, icon: u.icon, label: t(u.label), hint: t(u.hint) })) : []
}

export function nestable(action: Pick<ActionCatalogEntry, 'builtin' | 'builtin_kind' | 'graph' | 'usage'>): boolean {
  const single = action.builtin && action.builtin_kind !== 'action' && action.graph.nodes.length === 1
  return single || action.usage.step || action.usage.landing
}

export function reaches(graph: ActionGraph, from: string, to: string): boolean {
  const seen = new Set<string>()
  const stack = [from]
  while (stack.length) {
    const id = stack.pop()!
    if (id === to) return true
    if (seen.has(id)) continue
    seen.add(id)
    for (const edge of graph.edges) if (edge.source === id) stack.push(edge.target)
  }
  return false
}

export interface Wire {
  source: string
  sourceHandle?: string | null
  target: string
  targetHandle?: string | null
}

export function portName(handle: string | null | undefined, prefix: string): string {
  return handle?.startsWith(prefix) ? handle.slice(prefix.length) : ''
}

export function connectionProblem(graph: ActionGraph, catalog: ActionCatalog | null, owner: Owner, wire: Wire): string | null {
  if (wire.source === wire.target) return t('actions.wire.self')
  const source = graph.nodes.find((n) => n.id === wire.source)
  const target = graph.nodes.find((n) => n.id === wire.target)
  if (!source || !target) return t('actions.wire.gone')
  const out = portsOf(source, catalog, owner)?.outputs.find((p) => p.name === portName(wire.sourceHandle, OUT))
  const into = portsOf(target, catalog, owner)?.inputs.find((p) => p.name === portName(wire.targetHandle, IN))
  if (roleOf(target, catalog) === 'trigger') return t('actions.wire.intoTrigger')
  if (target.type === NODE_CHAT && graph.edges.some((e) => e.source === target.id)) return t('actions.wire.intoStartedChat')
  if (source.type === NODE_CHAT && graph.edges.some((e) => e.target === source.id)) return t('actions.wire.fromEndedChat')
  if (!out && roleOf(source, catalog) === 'landing') return t('actions.wire.fromLanding', { node: label(source, catalog) })
  if (!out) return t('actions.wire.noOutput', { node: label(source, catalog) })
  if (!into) return t('actions.wire.noInput', { node: label(target, catalog) })
  if (!accepts(into.type, out.type)) return t('actions.wire.mismatch', { output: out.name, outputType: out.type, input: into.name, inputType: into.type })
  if (reaches(graph, wire.target, wire.source)) return t('actions.wire.loop')
  return null
}

export function connect(graph: ActionGraph, wire: Wire): ActionGraph {
  const source_port = portName(wire.sourceHandle, OUT)
  const target_port = portName(wire.targetHandle, IN)
  const edge: ActionEdge = { id: `${wire.source}.${source_port}-${wire.target}.${target_port}`, source: wire.source, source_port, target: wire.target, target_port }
  const edges = graph.edges.filter((e) => !(e.target === edge.target && e.target_port === edge.target_port))
  return { ...graph, edges: [...edges, edge] }
}

export interface WireStart {
  node: string
  handle: string
  type: 'source' | 'target'
}

export function validEnds(graph: ActionGraph, catalog: ActionCatalog | null, owner: Owner, from: WireStart): Map<string, string[]> {
  const out = new Map<string, string[]>()
  for (const node of graph.nodes) {
    if (node.id === from.node) continue
    const ports = portsOf(node, catalog, owner) ?? { inputs: [], outputs: [] }
    const handles = from.type === 'source' ? ports.inputs.map((p) => `${IN}${p.name}`) : ports.outputs.map((p) => `${OUT}${p.name}`)
    const wire = (handle: string): Wire => (from.type === 'source' ? { source: from.node, sourceHandle: from.handle, target: node.id, targetHandle: handle } : { source: node.id, sourceHandle: handle, target: from.node, targetHandle: from.handle })
    out.set(node.id, handles.filter((handle) => !connectionProblem(graph, catalog, owner, wire(handle))))
  }
  return out
}

export function withoutInvalidEdges(graph: ActionGraph, diagnostics: ActionDiagnostic[]): ActionGraph {
  const bad = new Set(diagnostics.filter((d) => d.severity === 'error' && d.edge).map((d) => d.edge))
  return { ...graph, edges: graph.edges.filter((e) => !bad.has(e.id)) }
}

export function label(node: ActionNode, catalog: ActionCatalog | null): string {
  if (node.label) return node.label
  const chat = chatOf(node)
  if (chat) return catalog?.chat_node?.variants[chat.mode].label ?? t('actions.kind.chat')
  if (node.type.startsWith(BUILTIN)) return catalog?.builtins.find((b) => b.key === node.type.slice(BUILTIN.length))?.label ?? node.type
  if (node.type.startsWith(ACTION)) return catalog?.actions.find((a) => a.id === node.type.slice(ACTION.length))?.name ?? t('actions.node.missing')
  if (node.type === NODE_TRIGGER) return t('actions.node.trigger')
  return node.id
}

export function iconOf(node: ActionNode, catalog: ActionCatalog | null): string {
  const chat = chatOf(node)
  if (chat) return catalog?.chat_node?.variants[chat.mode].icon ?? 'lucide:messages-square'
  if (node.type === NODE_TRIGGER) {
    const on = triggerOf(node)?.on
    return TRIGGER_ICONS[on ?? 'arrival']
  }
  if (node.type.startsWith(BUILTIN)) return catalog?.builtins.find((b) => b.key === node.type.slice(BUILTIN.length))?.icon ?? 'lucide:box'
  if (node.type.startsWith(ACTION)) return catalog?.actions.find((a) => a.id === node.type.slice(ACTION.length))?.icon ?? 'lucide:zap'
  return 'lucide:code'
}

export function kindLabel(node: ActionNode): string {
  if (node.type === NODE_CHAT) return t('actions.kind.chat')
  if (node.type === NODE_TRIGGER) return t('actions.kind.trigger', { on: triggerOf(node)?.on ?? 'arrival' })
  if (node.type.startsWith(BUILTIN)) return t('actions.kind.step')
  if (node.type.startsWith(ACTION)) return t('actions.kind.action')
  return t('actions.kind.script')
}

const RUN_STATUS: Record<string, MessageKey> = { done: 'actions.status.done', failed: 'actions.status.failed', would: 'actions.status.would', skipped: 'actions.status.skipped', waiting: 'actions.status.waiting', running: 'actions.status.running', requirement: 'actions.status.requirement' }

export function statusLabel(status: string): string {
  return RUN_STATUS[status] ? t(RUN_STATUS[status]) : status
}

export function editable(node: ActionNode): boolean {
  return node.type === NODE_TRIGGER || node.type === NODE_SCRIPT
}

export function uniqueId(base: string, taken: string[]): string {
  const slug = base.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '') || 'node'
  if (!taken.includes(slug)) return slug
  let i = 2
  while (taken.includes(`${slug}_${i}`)) i += 1
  return `${slug}_${i}`
}

export type Placeable = Pick<ActionCatalogEntry, 'id' | 'name' | 'builtin' | 'graph'> & { builtin_kind?: BuiltinKind | null }

export function nodeFromAction(action: Placeable, graph: ActionGraph, at: { x: number; y: number }): ActionNode {
  const id = uniqueId(action.name, graph.nodes.map((n) => n.id))
  const single = action.builtin && action.builtin_kind !== 'action' && action.graph.nodes.length === 1 ? action.graph.nodes[0] : null
  const base: ActionNode = single ? (JSON.parse(JSON.stringify(single)) as ActionNode) : { id, type: `${ACTION}${action.id}`, label: action.name, config: { values: {} }, body: '', x: 0, y: 0 }
  return { ...base, id, label: base.label || action.name, x: Math.round(at.x), y: Math.round(at.y) }
}

export function removeNodes(graph: ActionGraph, ids: string[]): ActionGraph {
  return { nodes: graph.nodes.filter((n) => !ids.includes(n.id)), edges: graph.edges.filter((e) => !ids.includes(e.source) && !ids.includes(e.target)) }
}

export function layout(graph: ActionGraph, direction: 'TB' | 'LR', sizes: Map<string, { width: number; height: number }> = new Map()): ActionGraph {
  const g = new dagre.graphlib.Graph()
  g.setGraph({ rankdir: direction, nodesep: 48, ranksep: 72, marginx: 24, marginy: 24 })
  g.setDefaultEdgeLabel(() => ({}))
  const size = (id: string) => sizes.get(id) ?? { width: NODE_WIDTH, height: NODE_HEIGHT }
  for (const node of graph.nodes) g.setNode(node.id, size(node.id))
  for (const edge of graph.edges) if (g.hasNode(edge.source) && g.hasNode(edge.target)) g.setEdge(edge.source, edge.target)
  dagre.layout(g)
  return {
    ...graph,
    nodes: graph.nodes.map((node) => {
      const placed = g.node(node.id)
      return placed ? { ...node, x: Math.round(placed.x - size(node.id).width / 2), y: Math.round(placed.y - size(node.id).height / 2) } : node
    })
  }
}

export function problemsFor(diagnostics: ActionDiagnostic[], id: string, field: 'node' | 'edge'): ActionDiagnostic[] {
  return diagnostics.filter((d) => d[field] === id && d.severity !== REQUIREMENT)
}

export const REQUIREMENT = 'requirement'

export interface NodeRequirement {
  setting: string
  label: string
  href: string
}

export function requirementsFor(diagnostics: (ActionDiagnostic | null | undefined)[], node: string | null = null): NodeRequirement[] {
  const seen = new Set<string>()
  const out: NodeRequirement[] = []
  for (const d of diagnostics) {
    if (!d || d.severity !== REQUIREMENT || (node !== null && d.node !== node)) continue
    const anchor = requirementAnchor(d.setting)
    if (!anchor || seen.has(anchor)) continue
    seen.add(anchor)
    out.push({ setting: anchor, label: d.message, href: settingHref(anchor) })
  }
  return out
}

export interface RequirementBadge {
  count: number
  label: string
  title: string
  href: string
}

export function requirementBadge(requirements: NodeRequirement[]): RequirementBadge | null {
  if (!requirements.length) return null
  return {
    count: requirements.length,
    label: t('actions.requirement.badge', { count: requirements.length }),
    title: t('actions.requirement.title', { names: requirements.map((r) => r.label).join(', ') }),
    href: requirements[0].href
  }
}

export type TreeAction = Pick<Action, 'id' | 'group_id' | 'position' | 'name' | 'description'>

export interface TreeItem<A extends TreeAction = Action> {
  kind: 'group' | 'action'
  id: string
  depth: number
  group: ActionGroup | null
  action: A | null
}

export function tree<A extends TreeAction>(groups: ActionGroup[], actions: A[], collapsed: Set<string> = new Set(), query = ''): TreeItem<A>[] {
  const out: TreeItem<A>[] = []
  const known = new Set(groups.map((g) => g.id))
  const parentOf = (id: string | null) => (id && known.has(id) ? id : null)
  const byPosition = <T extends { position: number }>(a: T, b: T) => a.position - b.position
  const needle = query.trim().toLowerCase()
  const named = new Set(needle ? groups.filter((g) => `${g.name} ${g.description}`.toLowerCase().includes(needle)).map((g) => g.id) : [])
  const within = (id: string | null) => {
    for (let cursor = parentOf(id), hops = 0; cursor && hops < 64; hops += 1) {
      if (named.has(cursor)) return true
      cursor = parentOf(groups.find((g) => g.id === cursor)?.parent_id ?? null)
    }
    return false
  }
  const shown = needle ? actions.filter((a) => `${a.name} ${a.description}`.toLowerCase().includes(needle) || within(a.group_id)) : actions
  const open = new Set<string>()
  if (needle) {
    const lift = (id: string | null) => {
      for (let cursor = parentOf(id), hops = 0; cursor && hops < 64 && !open.has(cursor); hops += 1) {
        open.add(cursor)
        cursor = parentOf(groups.find((g) => g.id === cursor)?.parent_id ?? null)
      }
    }
    for (const action of shown) lift(action.group_id)
    for (const id of named) lift(id)
  }
  const walk = (parent: string | null, depth: number) => {
    for (const group of groups.filter((g) => parentOf(g.parent_id) === parent && (!needle || open.has(g.id))).sort(byPosition)) {
      out.push({ kind: 'group', id: group.id, depth, group, action: null })
      if (needle || !collapsed.has(group.id)) walk(group.id, depth + 1)
    }
    for (const action of shown.filter((a) => parentOf(a.group_id) === parent).sort(byPosition)) {
      out.push({ kind: 'action', id: action.id, depth, group: null, action })
    }
  }
  walk(null, 0)
  return out
}

export interface TreePart<A extends TreeAction> {
  groups: ActionGroup[]
  actions: A[]
}

export function partition<A extends TreeAction & { builtin: boolean }>(groups: ActionGroup[], actions: A[]): { mine: TreePart<A>; builtin: TreePart<A> } {
  const root = groups.find((g) => g.builtin && !g.parent_id)?.id ?? null
  const inside = groups.filter((g) => g.builtin && g.id !== root).map((g) => (g.parent_id === root ? { ...g, parent_id: null } : g))
  return {
    mine: { groups: groups.filter((g) => !g.builtin), actions: actions.filter((a) => !a.builtin) },
    builtin: { groups: inside, actions: actions.filter((a) => a.builtin).map((a) => (a.group_id === root ? { ...a, group_id: null } : a)) }
  }
}

const BUILTIN_GROUP_NAMES: Record<string, MessageKey> = {
  builtin: 'actions.builtin.group.builtin',
  'builtin.triggers': 'actions.builtin.group.triggers',
  'builtin.triggers.messages': 'actions.builtin.group.messages',
  'builtin.triggers.chats': 'actions.builtin.group.chats',
  'builtin.triggers.time': 'actions.builtin.group.time',
  'builtin.triggers.calendar': 'actions.builtin.group.calendar',
  'builtin.triggers.calls': 'actions.builtin.group.calls',
  'builtin.steps': 'actions.builtin.group.steps',
  'builtin.steps.ai': 'actions.builtin.group.ai',
  'builtin.steps.text': 'actions.builtin.group.text',
  'builtin.steps.match': 'actions.builtin.group.match',
  'builtin.steps.flow': 'actions.builtin.group.flow',
  'builtin.landings': 'actions.builtin.group.landings',
  'builtin.landings.tags': 'actions.builtin.group.tags',
  'builtin.landings.inbox': 'actions.builtin.group.inbox',
  'builtin.landings.chat': 'actions.builtin.group.chat',
  'builtin.landings.notify': 'actions.builtin.group.notify',
  'builtin.landings.calendar': 'actions.builtin.group.calendar',
  'builtin.landings.look': 'actions.builtin.group.look',
  'builtin.landings.people': 'actions.builtin.group.people',
  'builtin.actions': 'actions.builtin.group.actions'
}

export function groupName(group: Pick<ActionGroup, 'key' | 'name' | 'builtin'>): string {
  const key = group.builtin && group.key ? BUILTIN_GROUP_NAMES[group.key] : undefined
  return key ? t(key) : group.name
}

export function signatureLine(inputs: ActionPort[], outputs: ActionPort[]): string {
  const list = (ports: ActionPort[]) => (ports.length ? ports.map((p) => `${p.name}: ${p.type}`).join(', ') : '–')
  return `${list(inputs)} → ${list(outputs)}`
}

export function descendsFrom(groups: ActionGroup[], id: string | null, ancestor: string): boolean {
  let cursor = id
  for (let hops = 0; cursor && hops < 64; hops += 1) {
    if (cursor === ancestor) return true
    cursor = groups.find((g) => g.id === cursor)?.parent_id ?? null
  }
  return false
}

export interface ChatPrefill {
  conversation: string | null
  person: string | null
  name: string | null
}

export function prefillFrom(params: URLSearchParams): ChatPrefill | null {
  const conversation = params.get('conversation')
  const person = params.get('person')
  if (!conversation && !person && params.get('new') !== '1') return null
  return { conversation, person, name: params.get('name') }
}

export function draftFor(prefill: ChatPrefill): PostActionInput {
  const scope = { conversation_ids: prefill.conversation ? [prefill.conversation] : [], person_ids: prefill.person ? [prefill.person] : [] }
  const trigger: ActionNode = { id: 'trigger', type: NODE_TRIGGER, label: prefill.conversation || prefill.person ? t('actions.draft.triggerChat') : t('actions.draft.trigger'), config: { on: 'arrival', scope }, body: '', x: 0, y: 0 }
  return { name: prefill.name ?? (prefill.conversation || prefill.person ? t('actions.draft.nameChat') : t('actions.draft.name')), graph: { nodes: [trigger], edges: [] }, enabled: false }
}

export type PickerKind = 'message' | 'conversation' | 'person' | 'tag'

export interface JsonSchema {
  type?: string | string[]
  title?: string
  description?: string
  properties?: Record<string, JsonSchema>
  items?: JsonSchema
  required?: string[]
  enum?: unknown[]
  const?: unknown
  format?: string
  minimum?: number
  maximum?: number
  exclusiveMinimum?: number
  exclusiveMaximum?: number
  minLength?: number
  maxLength?: number
  minItems?: number
  maxItems?: number
  labels?: string[]
  'x-picker'?: PickerKind
}

const PICKERS: PickerKind[] = ['message', 'conversation', 'person', 'tag']
const TEXT: JsonSchema = { type: 'string' }
const FLAG: JsonSchema = { type: 'boolean' }
const EMAIL: JsonSchema = { type: 'string', format: 'email' }
const listOf = (items: JsonSchema): JsonSchema => ({ type: 'array', items })

const SHAPES: Record<PickerKind, JsonSchema> = {
  message: {
    type: 'object',
    properties: { id: TEXT, conversation_id: TEXT, sender: TEXT, sender_id: TEXT, sender_email: EMAIL, sender_domain: TEXT, subject: TEXT, text: TEXT, at: { type: 'string', format: 'date-time' }, unread: FLAG, has_attachments: FLAG, summary: TEXT, mine: FLAG }
  },
  conversation: {
    type: 'object',
    properties: { id: TEXT, name: TEXT, kind: TEXT, provider: TEXT, provider_id: TEXT, connection: TEXT, namespace: TEXT, person_id: TEXT, members: { type: 'integer', minimum: 1, maximum: 12 }, done: FLAG, unread: { type: 'integer', minimum: 0, maximum: 20 }, tags: listOf(TEXT), tag_ids: listOf(TEXT) }
  },
  person: {
    type: 'object',
    properties: { id: TEXT, name: TEXT, emails: listOf(EMAIL), human: FLAG, conversation_ids: listOf(TEXT), identities: listOf({ type: 'object', properties: { provider: TEXT, email: EMAIL, name: TEXT } }) }
  },
  tag: { type: 'string' }
}

function portSchema(port: ActionPort): JsonSchema {
  const base = { title: port.name, ...(port.description ? { description: port.description } : {}) }
  if ((PICKERS as string[]).includes(port.type)) return { ...base, ...structuredClone(SHAPES[port.type as PickerKind]), 'x-picker': port.type as PickerKind }
  switch (port.type) {
    case 'text':
      return { ...base, type: 'string' }
    case 'number':
      return { ...base, type: 'number' }
    case 'bool':
      return { ...base, type: 'boolean' }
    case 'list':
      return { ...base, type: 'array', items: {} }
    default:
      return base
  }
}

export function schemaOf(ports: ActionPort[]): JsonSchema {
  return { type: 'object', properties: Object.fromEntries(ports.map((p) => [p.name, portSchema(p)])), required: ports.map((p) => p.name) }
}

export function playPorts(graph: ActionGraph, owner: Owner): ActionPort[] {
  const base = graph.nodes.some((n) => n.type === NODE_TRIGGER) ? TRIGGER_OUTPUTS : []
  return [...base, ...owner.inputs.filter((p) => !base.some((b) => b.name === p.name))]
}

export type MockOverrides = Record<string, unknown>

export interface MockProblem {
  path: string
  message: string
}

const CAST = [
  ['Eva', 'Olsen', 'northwind.example'],
  ['Nora', 'Gale', 'northwind.example'],
  ['Lea', 'Larsen', 'northwind.example'],
  ['Anna', 'Berg', 'client.example'],
  ['Jonas', 'Lie', 'client.example'],
  ['Kari', 'Nordmann', 'tidewater.example'],
  ['Mia', 'Sund', 'freelance.example']
] as const
const SUBJECTS = ['Invoice 42 is overdue', 'Lunch on Thursday?', 'Sprint demo moved to Friday', 'Contract draft for review', 'Access to the staging tenant', 'Quarterly numbers']
const BODIES = [
  'Hi, the invoice from last month is still open. Can you confirm it is paid by Friday?',
  'Are you free for lunch on Thursday at 12:00? The usual place works for me.',
  'The sprint demo moves to Friday 14:00. Same room, same agenda.',
  'Attached is the contract draft. Comments before Wednesday would be great.',
  'Could you grant me access to the staging tenant? I need it for the migration test.'
]
const PROVIDERS = ['teams', 'outlook', 'slack', 'gmail', 'imap']
const KINDS = ['chat', 'email', 'channel']
const WORDS = ['budget', 'roadmap', 'release', 'invoice', 'meeting', 'design', 'review', 'support']

function hash(key: string): number {
  let h = 0x811c9dc5
  for (let i = 0; i < key.length; i += 1) {
    h ^= key.charCodeAt(i)
    h = Math.imul(h, 0x01000193)
  }
  return h >>> 0
}

function pick<T>(list: readonly T[], key: string): T {
  return list[hash(key) % list.length]
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

function same(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

function typesOf(schema: JsonSchema): string[] {
  if (Array.isArray(schema.type)) return schema.type
  if (schema.type) return [schema.type]
  if (schema.properties) return ['object']
  if (schema.items) return ['array']
  return []
}

export function typeOf(schema: JsonSchema): string | null {
  const types = typesOf(schema)
  return types.find((type) => type !== 'null') ?? types[0] ?? null
}

function typeFits(schema: JsonSchema, value: unknown): boolean {
  const types = typesOf(schema)
  if (!types.length) return true
  return types.some((type) => {
    switch (type) {
      case 'string':
        return typeof value === 'string'
      case 'number':
        return typeof value === 'number' && Number.isFinite(value)
      case 'integer':
        return Number.isInteger(value)
      case 'boolean':
        return typeof value === 'boolean'
      case 'array':
        return Array.isArray(value)
      case 'object':
        return isRecord(value)
      case 'null':
        return value === null
      default:
        return true
    }
  })
}

function fits(schema: JsonSchema, value: unknown): boolean {
  if (!typeFits(schema, value)) return false
  if (Array.isArray(value) && schema.items) return value.every((item) => fits(schema.items!, item))
  if (isRecord(value) && schema.properties) return Object.entries(schema.properties).every(([key, sub]) => value[key] === undefined || fits(sub, value[key]))
  return true
}

function nameOf(path: string[]): string {
  return ([...path].reverse().find((p) => !/^\d+$/.test(p)) ?? '').toLowerCase()
}

function isoAt(seed: string, dateOnly: boolean): string {
  const h = hash(seed)
  const at = new Date(Date.UTC(2026, 9, 1 + (h % 28), 8 + ((h >>> 5) % 9), ((h >>> 9) % 4) * 15))
  return dateOnly ? at.toISOString().slice(0, 10) : at.toISOString().replace('.000Z', 'Z')
}

function stringMock(schema: JsonSchema, path: string[]): string {
  const seed = path.join('/')
  const name = nameOf(path)
  const [first, last, domain] = pick(CAST, path.slice(0, -1).join('/') || seed)
  const mail = `${first.toLowerCase()}.${last.toLowerCase()}@${domain}`
  const format = schema.format ?? ''
  let out: string
  if (format === 'date-time' || format === 'date') out = isoAt(seed, format === 'date')
  else if (format === 'email' || name.includes('email')) out = mail
  else if (format === 'uri' || format === 'url' || /(url|link|href|website)/.test(name)) out = `https://${domain}/${pick(WORDS, seed)}`
  else if (format === 'uuid') out = hash(seed).toString(16).padStart(8, '0') + '-0000-4000-8000-' + hash(seed + '#').toString(16).padStart(12, '0')
  else if (/(subject|title|topic)/.test(name)) out = pick(SUBJECTS, seed)
  else if (/(body|text|content|summary|message|reply|note|description|prompt)/.test(name)) out = pick(BODIES, seed)
  else if (name === 'domain' || name.endsWith('_domain')) out = domain
  else if (name === 'provider') out = pick(PROVIDERS, seed)
  else if (name === 'kind' || name === 'type') out = pick(KINDS, seed)
  else if (/(^|_)(id|ids)$/.test(name)) out = `mock-${name.replace(/_?ids?$/, '') || 'item'}-${(hash(seed) % 9000) + 1000}`
  else if (/(^|_)(at|date|time|when)$|timestamp/.test(name)) out = isoAt(seed, false)
  else if (/(name|sender|author|organizer|from|to|connection)/.test(name)) out = `${first} ${last}`
  else if (name.includes('phone')) out = `+47 9${(hash(seed) % 10000000).toString().padStart(7, '0')}`
  else if (name.includes('tag') || name.includes('label')) out = pick(['Important', 'Invoice', 'Follow up', 'Customer'], seed)
  else out = `${pick(WORDS, seed)} ${(hash(seed) % 90) + 10}`
  if (schema.minLength && out.length < schema.minLength) out = out.padEnd(schema.minLength, '.')
  if (schema.maxLength !== undefined && out.length > schema.maxLength) out = out.slice(0, schema.maxLength)
  return out
}

function numberMock(schema: JsonSchema, path: string[], integer: boolean): number {
  const step = integer ? 1 : 0.01
  const low = schema.minimum ?? (schema.exclusiveMinimum !== undefined ? schema.exclusiveMinimum + step : 0)
  const high = schema.maximum ?? (schema.exclusiveMaximum !== undefined ? schema.exclusiveMaximum - step : low + (/(count|limit|max|min|days|hours|minutes)/.test(nameOf(path)) ? 9 : 100))
  const h = hash(path.join('/'))
  if (high <= low) return low
  if (integer) return Math.ceil(low) + (h % (Math.floor(high) - Math.ceil(low) + 1))
  return Math.round((low + ((h % 1000) / 999) * (high - low)) * 100) / 100
}

export function mockOf(schema: JsonSchema, sample: unknown = undefined, path: string[] = []): unknown {
  if (schema.const !== undefined) return schema.const
  const type = typeOf(schema)
  if (sample !== undefined && sample !== null && !(type === 'object' && schema.properties) && fits(schema, sample) && (!schema.enum || schema.enum.some((e) => same(e, sample)))) return sample
  const seed = path.join('/')
  if (schema.enum?.length) return pick(schema.enum, seed)
  switch (type) {
    case 'object': {
      const source = isRecord(sample) ? sample : {}
      return Object.fromEntries(Object.entries(schema.properties ?? {}).map(([key, sub]) => [key, mockOf(sub, source[key], [...path, key])]))
    }
    case 'array': {
      const low = Math.max(1, schema.minItems ?? 1)
      const high = Math.max(low, Math.min(3, schema.maxItems ?? 3))
      const count = low + (hash(seed) % (high - low + 1))
      return Array.from({ length: count }, (_, i) => mockOf(schema.items ?? {}, undefined, [...path, String(i)]))
    }
    case 'integer':
      return numberMock(schema, path, true)
    case 'number':
      return numberMock(schema, path, false)
    case 'boolean':
      return hash(seed) % 2 === 0
    case 'null':
      return null
    default:
      return stringMock(schema, path)
  }
}

export function mockSource(schema: JsonSchema, sample: ActionSample | null): Record<string, unknown> | undefined {
  if (!sample) return undefined
  return Object.fromEntries(Object.entries(schema.properties ?? {}).map(([key, sub]) => [key, sub['x-picker'] ? sample[sub['x-picker']] : key === 'text' ? sample.text : undefined]))
}

export function pointer(path: string[]): string {
  return path.map((p) => '/' + p.replace(/~/g, '~0').replace(/\//g, '~1')).join('')
}

export function unpointer(key: string): string[] {
  return key ? key.slice(1).split('/').map((p) => p.replace(/~1/g, '/').replace(/~0/g, '~')) : []
}

export function leaves(schema: JsonSchema, path: string[] = [], out: Map<string, JsonSchema> = new Map()): Map<string, JsonSchema> {
  if (typeOf(schema) === 'object' && schema.properties) for (const [key, sub] of Object.entries(schema.properties)) leaves(sub, [...path, key], out)
  else if (path.length) out.set(pointer(path), schema)
  return out
}

export function valueAt(value: unknown, path: string[]): unknown {
  let cursor = value
  for (const key of path) {
    if (!isRecord(cursor)) return undefined
    cursor = cursor[key]
  }
  return cursor
}

export function withValue(value: unknown, path: string[], leaf: unknown): unknown {
  if (!path.length) return leaf
  const [key, ...rest] = path
  const base = isRecord(value) ? value : {}
  return { ...base, [key]: withValue(base[key], rest, leaf) }
}

export function overridesOf(schema: JsonSchema, edited: unknown, generated: unknown): MockOverrides {
  const out: MockOverrides = {}
  for (const key of leaves(schema).keys()) {
    const path = unpointer(key)
    const mine = valueAt(edited, path)
    if (mine !== undefined && !same(mine, valueAt(generated, path))) out[key] = mine
  }
  return out
}

export function mockWith(schema: JsonSchema, sample: ActionSample | null, overrides: MockOverrides): { value: Record<string, unknown>; overrides: MockOverrides } {
  let value = mockOf(schema, mockSource(schema, sample)) as Record<string, unknown>
  const known = leaves(schema)
  const kept: MockOverrides = {}
  for (const [key, leaf] of Object.entries(overrides)) {
    const sub = known.get(key)
    if (!sub || !fits(sub, leaf)) continue
    kept[key] = leaf
    value = withValue(value, unpointer(key), leaf) as Record<string, unknown>
  }
  return { value, overrides: kept }
}

export function mockProblems(schema: JsonSchema, value: unknown, path: string[] = []): MockProblem[] {
  if (value === undefined) return []
  const where = path.join('.') || t('actions.mock.root')
  const out: MockProblem[] = []
  const add = (key: MessageKey, params?: Record<string, string | number>) => out.push({ path: where, message: t(key, params) })
  if (!typeFits(schema, value)) {
    add('actions.mock.problem.type', { type: typesOf(schema).join(' | ') })
    return out
  }
  if (schema.const !== undefined && !same(schema.const, value)) add('actions.mock.problem.enum', { values: JSON.stringify(schema.const) })
  if (schema.enum && !schema.enum.some((e) => same(e, value))) add('actions.mock.problem.enum', { values: schema.enum.map((e) => JSON.stringify(e)).join(', ') })
  if (typeof value === 'string') {
    if (schema.minLength !== undefined && value.length < schema.minLength) add('actions.mock.problem.minLength', { min: schema.minLength })
    if (schema.maxLength !== undefined && value.length > schema.maxLength) add('actions.mock.problem.maxLength', { max: schema.maxLength })
    const bad =
      (schema.format === 'email' && !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(value)) ||
      (schema.format === 'date-time' && (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(value) || Number.isNaN(Date.parse(value)))) ||
      (schema.format === 'date' && !/^\d{4}-\d{2}-\d{2}$/.test(value)) ||
      ((schema.format === 'uri' || schema.format === 'url') && !/^[a-z][a-z0-9+.-]*:\/\/\S+$/i.test(value))
    if (bad) add('actions.mock.problem.format', { format: schema.format! })
  }
  if (typeof value === 'number') {
    if (schema.minimum !== undefined && value < schema.minimum) add('actions.mock.problem.minimum', { min: schema.minimum })
    if (schema.exclusiveMinimum !== undefined && value <= schema.exclusiveMinimum) add('actions.mock.problem.minimum', { min: schema.exclusiveMinimum })
    if (schema.maximum !== undefined && value > schema.maximum) add('actions.mock.problem.maximum', { max: schema.maximum })
    if (schema.exclusiveMaximum !== undefined && value >= schema.exclusiveMaximum) add('actions.mock.problem.maximum', { max: schema.exclusiveMaximum })
  }
  if (Array.isArray(value)) {
    if (schema.minItems !== undefined && value.length < schema.minItems) add('actions.mock.problem.minItems', { min: schema.minItems })
    if (schema.maxItems !== undefined && value.length > schema.maxItems) add('actions.mock.problem.maxItems', { max: schema.maxItems })
    if (schema.items) value.forEach((item, i) => out.push(...mockProblems(schema.items!, item, [...path, String(i)])))
  }
  if (isRecord(value) && schema.properties) {
    for (const key of schema.required ?? []) if (value[key] === undefined) out.push({ path: [...path, key].join('.'), message: t('actions.mock.problem.required') })
    for (const [key, sub] of Object.entries(schema.properties)) out.push(...mockProblems(sub, value[key], [...path, key]))
  }
  return out
}

export interface MockEdit {
  overrides: MockOverrides | null
  problems: MockProblem[]
}

export function mockEdit(schema: JsonSchema, sample: ActionSample | null, text: string): MockEdit {
  let parsed: unknown
  try {
    parsed = JSON.parse(text)
  } catch (e) {
    return { overrides: null, problems: [{ path: t('actions.mock.root'), message: t('actions.mock.problem.json', { reason: e instanceof Error ? e.message : String(e) }) }] }
  }
  if (!isRecord(parsed)) return { overrides: null, problems: [{ path: t('actions.mock.root'), message: t('actions.play.notObject') }] }
  return { overrides: overridesOf(schema, parsed, mockOf(schema, mockSource(schema, sample))), problems: mockProblems(schema, parsed) }
}

export const MOCK_PREFIX = 'actions.mock.'

export function mockKey(actionId: string, nodeId: string | null): string {
  return `${MOCK_PREFIX}${actionId}${nodeId ? `.${nodeId}` : ''}`
}

export function mockStored(raw: string | undefined): MockOverrides {
  if (!raw) return {}
  try {
    const parsed = JSON.parse(raw)
    return isRecord(parsed) ? parsed : {}
  } catch {
    return {}
  }
}

export const PRESENTER = `${BUILTIN}present_`

export function presenterKind(node: ActionNode): string | null {
  return node.type.startsWith(PRESENTER) ? node.type.slice(PRESENTER.length) : null
}

export interface TreeRowState {
  struck: boolean
  selected: boolean
  failed: boolean
  indent: number
}

export function treeRowState(item: TreeItem, selected: string | null): TreeRowState {
  const action = item.action
  return {
    struck: !!action && !action.enabled,
    selected: !!action && action.id === selected,
    failed: action?.last_status === 'failed',
    indent: item.depth * 0.75
  }
}

export const BUILTIN_KEY = 'trigger:'

export function promptsUsed(body: string): string[] {
  const keys = [...body.matchAll(/\bprompt\(\s*"([^"]+)"/g)].map((m) => m[1])
  if (/\bai_json\(/.test(body)) keys.push('script_ai_json')
  if (/\bai\(/.test(body)) keys.push('script_ai')
  if (/\breceipt_confirm\(/.test(body)) keys.push('receipt_confirm')
  return [...new Set(keys)]
}

export function resolvePrompt(base: Prompt, override: PromptOverride | undefined, contentPrompts: ContentPrompt[] = []): Prompt {
  if (!override) return base
  const content = override.content_id ? contentPrompts.find((entry) => entry.id === override.content_id) : undefined
  return {
    ...base,
    system: override.system ?? base.system,
    template: content ? content.body : override.template || base.template,
    output_schema: override.output_schema === undefined ? base.output_schema : override.output_schema || null,
    connection_id: override.connection_id || null,
    model: override.model || null
  }
}

export function promptOverride(base: Prompt, edited: Pick<Prompt, 'system' | 'template' | 'output_schema' | 'connection_id' | 'model'> & { content_id?: string | null }): PromptOverride | null {
  const out: PromptOverride = {}
  if (edited.system !== base.system) out.system = edited.system
  if (edited.content_id) out.content_id = edited.content_id
  else if (edited.template.trim() && edited.template !== base.template) out.template = edited.template
  if ((edited.output_schema ?? '') !== (base.output_schema ?? '')) out.output_schema = edited.output_schema ?? ''
  if (edited.connection_id) out.connection_id = edited.connection_id
  if (edited.model?.trim()) out.model = edited.model.trim()
  return Object.keys(out).length ? out : null
}

export function withPrompt(node: ActionNode, key: string, override: PromptOverride | null): ActionNode {
  const prompts = { ...((node.config?.prompts ?? {}) as Record<string, PromptOverride>) }
  if (override) prompts[key] = override
  else delete prompts[key]
  const config: Record<string, unknown> = { ...(node.config ?? {}) }
  if (Object.keys(prompts).length) config.prompts = prompts
  else delete config.prompts
  return { ...node, config }
}

export const RETURNS = 'returns'

export interface PortReturn {
  name?: string
  description?: string
}

export interface OutputSource {
  node: string
  label: string
}

export function outputSources(outputs: ActionPort[], graph: ActionGraph, catalog: ActionCatalog | null): (OutputSource | null)[] {
  return outputs.map((port) => {
    const node = port.node ? graph.nodes.find((n) => n.id === port.node) : undefined
    return node ? { node: node.id, label: label(node, catalog) } : null
  })
}

export function withReturns(node: ActionNode, before: ActionPort[], after: ActionPort[]): ActionNode {
  const returns = { ...((node.config?.[RETURNS] ?? {}) as Record<string, PortReturn>) }
  after.forEach((port, i) => {
    const was = before[i]
    const source = was?.source_port
    if (!source) return
    const entry = { ...(returns[source] ?? {}) }
    if (port.name !== was.name) entry.name = port.name
    if ((port.description ?? '') !== (was.description ?? '')) entry.description = port.description ?? ''
    if (entry.name === source || entry.name === '') delete entry.name
    if (entry.description === '') delete entry.description
    if (Object.keys(entry).length) returns[source] = entry
    else delete returns[source]
  })
  const config: Record<string, unknown> = { ...(node.config ?? {}) }
  if (Object.keys(returns).length) config[RETURNS] = returns
  else delete config[RETURNS]
  return { ...node, config }
}

export type LandingKind = 'read_chat' | 'write_chat' | 'notify' | 'calendar' | 'tag' | 'split' | 'group' | 'conversation' | 'present' | 'summary' | 'person'

export interface Landing {
  kind: LandingKind
  icon: string
  label: MessageKey
}

const LANDINGS: Record<LandingKind, { icon: string; label: MessageKey }> = {
  read_chat: { icon: 'lucide:book-open', label: 'actions.landing.readChat' },
  write_chat: { icon: 'lucide:pen-line', label: 'actions.landing.writeChat' },
  notify: { icon: 'lucide:bell', label: 'actions.landing.notify' },
  calendar: { icon: 'lucide:calendar-plus', label: 'actions.landing.calendar' },
  tag: { icon: 'lucide:tag', label: 'actions.landing.tag' },
  split: { icon: 'lucide:inbox', label: 'actions.landing.split' },
  group: { icon: 'lucide:folder-plus', label: 'actions.landing.group' },
  conversation: { icon: 'lucide:check', label: 'actions.landing.conversation' },
  present: { icon: 'lucide:message-circle', label: 'actions.landing.present' },
  summary: { icon: 'lucide:align-left', label: 'actions.landing.summary' },
  person: { icon: 'lucide:user-search', label: 'actions.landing.person' }
}

const BUILTIN_LANDINGS: Record<string, LandingKind> = {
  tag_add: 'tag',
  tag_remove: 'tag',
  move: 'split',
  group_add: 'group',
  reply: 'write_chat',
  send: 'write_chat',
  draft: 'write_chat',
  forward: 'write_chat',
  schedule_send: 'write_chat',
  notify: 'notify',
  snooze: 'conversation',
  done: 'conversation',
  mute: 'conversation',
  pin: 'conversation',
  mark_read: 'conversation',
  store_summary: 'summary',
  create_event: 'calendar',
  propose_event: 'calendar',
  mark_person: 'person',
  highlight: 'present',
  collapse: 'present',
  hide: 'present',
  present_conversation: 'present',
  style_conversation: 'present'
}

const SCRIPT_LANDINGS: [RegExp, LandingKind][] = [
  [/\b(reply|send|draft|forward|schedule_send)\s*\(/, 'write_chat'],
  [/\bnotify\s*\(/, 'notify'],
  [/\b(create_event|propose_event)\s*\(/, 'calendar'],
  [/\btag_(add|remove)\s*\(/, 'tag'],
  [/\bmove\s*\(/, 'split'],
  [/\bgroup_add\s*\(/, 'group'],
  [/\b(done|snooze|mute|pin|mark_read)\s*\(/, 'conversation'],
  [/\b(present|highlight|collapse|hide|present_conversation|style_conversation)\s*\(/, 'present'],
  [/\bstore_summary\s*\(/, 'summary'],
  [/\bmark_person\s*\(/, 'person']
]

type LandingAction = Pick<Action, 'id' | 'graph' | 'usage' | 'outputs'>

export function landings(action: LandingAction, actions: LandingAction[] = [], catalog: ActionCatalog | null = null, seen: Set<string> = new Set()): Landing[] {
  const out: Landing[] = []
  const add = (kind: LandingKind, icon?: string) => {
    const shown = kind === 'read_chat' || kind === 'write_chat' ? (catalog?.chat_node?.variants[kind === 'read_chat' ? 'read' : 'write'].icon ?? LANDINGS[kind].icon) : (icon ?? LANDINGS[kind].icon)
    if (!out.some((l) => l.kind === kind && l.icon === shown)) out.push({ kind, icon: shown, label: LANDINGS[kind].label })
  }
  seen.add(action.id)
  if (action.usage.chat && action.outputs.some((p) => p.type === 'list' || p.type === 'json')) add('read_chat')
  for (const node of action.graph.nodes) {
    const chat = chatOf(node)
    if (chat) {
      if (action.graph.edges.some((e) => e.target === node.id)) add(chat.mode === 'read' ? 'read_chat' : 'write_chat')
      continue
    }
    if (node.type.startsWith(PRESENTER)) {
      add('present', catalog?.builtins.find((b) => `${BUILTIN}${b.key}` === node.type)?.icon)
      continue
    }
    if (node.type.startsWith(BUILTIN)) {
      const key = node.type.slice(BUILTIN.length)
      const kind = BUILTIN_LANDINGS[key]
      if (kind) add(kind, catalog?.builtins.find((b) => b.key === key)?.icon)
      continue
    }
    if (node.type.startsWith(ACTION)) {
      const inner = actions.find((a) => a.id === node.type.slice(ACTION.length))
      if (inner && !seen.has(inner.id)) for (const landing of landings({ ...inner, usage: { ...inner.usage, chat: false } }, actions, catalog, seen)) add(landing.kind, landing.icon)
      continue
    }
    if (node.type === NODE_SCRIPT) for (const [pattern, kind] of SCRIPT_LANDINGS) if (pattern.test(node.body)) add(kind)
  }
  return out
}

export function landingTitle(found: Landing[], chats: string[]): string {
  const parts: string[] = [found.length ? t('actions.landing.into', { targets: found.map((l) => t(l.label)).join(', ') }) : t('actions.landing.nowhere')]
  if (chats.length) parts.push(t('actions.landing.attached', { chats: chats.join(', ') }))
  return parts.join(' · ')
}

export const MAX_DEPTH = 8
export const FRAME_PAD = 24
export const FRAME_HEAD = 60
export const FRAME_MIN_WIDTH = 440
export const FRAME_MAX_WIDTH = 1600
export const FRAME_MAX_HEIGHT = 1200
export const PORT_ROW = 22
export const FLOW_PATH = '/'

export function descriptionOf(node: ActionNode, catalog: ActionCatalog | null): string {
  if (node.type.startsWith(BUILTIN)) return catalog?.builtins.find((b) => `${BUILTIN}${b.key}` === node.type)?.description ?? ''
  const first = node.body.split('\n').find((line) => line.trim())
  return first ? first.trim().slice(0, 90) : ''
}

export function nestedOf(node: ActionNode, catalog: ActionCatalog | null): ActionCatalogEntry | null {
  return node.type.startsWith(ACTION) ? (catalog?.actions.find((a) => a.id === node.type.slice(ACTION.length)) ?? null) : null
}

export function portBand(rows: number): number {
  return rows ? rows * PORT_ROW + 8 : 0
}

export const CONTROL_ROW = 30

export function cardHeight(rows: number, described: boolean, controls = 0): number {
  return 52 + (described ? 40 : 0) + (controls ? controls * CONTROL_ROW + 9 : 0) + (rows ? portBand(rows) + 1 : 0)
}

export interface Box {
  x: number
  y: number
  width: number
  height: number
}

export interface FrameBounds {
  width: number
  height: number
  scale: number
  positions: { x: number; y: number }[]
}

export function frameBounds(boxes: Box[], rows: number): FrameBounds {
  const top = FRAME_HEAD + portBand(rows)
  if (!boxes.length) return { width: FRAME_MIN_WIDTH, height: top + FRAME_PAD, scale: 1, positions: [] }
  const minX = Math.min(...boxes.map((b) => b.x))
  const minY = Math.min(...boxes.map((b) => b.y))
  const spanX = Math.max(...boxes.map((b) => b.x)) - minX
  const spanY = Math.max(...boxes.map((b) => b.y)) - minY
  const widest = Math.max(...boxes.map((b) => b.width))
  const tallest = Math.max(...boxes.map((b) => b.height))
  const room = (max: number, span: number, size: number) => (span > 0 && span + size > max ? Math.max((max - size) / span, 0.25) : 1)
  const scale = Math.min(1, room(FRAME_MAX_WIDTH - 2 * FRAME_PAD, spanX, widest), room(FRAME_MAX_HEIGHT - top - 2 * FRAME_PAD, spanY, tallest))
  const positions = boxes.map((b) => ({ x: Math.round(FRAME_PAD + (b.x - minX) * scale), y: Math.round(top + FRAME_PAD + (b.y - minY) * scale) }))
  const right = Math.max(...boxes.map((b, i) => positions[i].x + b.width))
  const bottom = Math.max(...boxes.map((b, i) => positions[i].y + b.height))
  return { width: Math.max(FRAME_MIN_WIDTH, right + FRAME_PAD), height: bottom + FRAME_PAD, scale, positions }
}

export type PlacedKind = 'card' | 'frame' | 'more'

export interface Placed {
  id: string
  node: ActionNode
  parent: string | null
  root: string
  owner: ActionCatalogEntry | null
  nested: ActionCatalogEntry | null
  kind: PlacedKind
  depth: number
  x: number
  y: number
  width: number
  height: number
  collapsed: boolean
}

export interface PlacedEdge {
  id: string
  edge: ActionEdge
  source: string
  target: string
  locked: boolean
}

export interface GraphPlan {
  nodes: Placed[]
  edges: PlacedEdge[]
}

function folded(root: ActionNode, id: string, node: ActionNode): boolean {
  if (id === root.id) return root.config?.collapsed === true
  const folds = (root.config?.folds ?? {}) as Record<string, boolean>
  const own = folds[id.slice(root.id.length + FLOW_PATH.length)]
  return own === undefined ? node.config?.collapsed === true : own
}

export function graphPlan(graph: ActionGraph, catalog: ActionCatalog | null, owner: Owner): GraphPlan {
  const edges: PlacedEdge[] = graph.edges.map((edge) => ({ id: edge.id, edge, source: edge.source, target: edge.target, locked: false }))
  const place = (node: ActionNode, id: string, parent: string | null, root: ActionNode, within: ActionCatalogEntry | null, depth: number, stack: string[]): Placed[] => {
    const nested = nestedOf(node, catalog)
    const ports = portsOf(node, catalog, within ? { inputs: within.inputs } : owner) ?? { inputs: [], outputs: [] }
    const rows = Math.max(ports.inputs.length, ports.outputs.length)
    const base: Placed = { id, node, parent, root: root.id, owner: within, nested, kind: 'card', depth, x: node.x, y: node.y, width: NODE_WIDTH, height: cardHeight(rows, !!descriptionOf(node, catalog), controlCount(node)), collapsed: false }
    if (!nested) return [base]
    if (depth >= MAX_DEPTH || stack.includes(nested.id)) return [{ ...base, kind: 'more', collapsed: true }]
    if (nested.graph.nodes.length <= 1) return [base]
    if (folded(root, id, node)) return [{ ...base, collapsed: true }]
    const children = nested.graph.nodes.map((child) => place(child, `${id}${FLOW_PATH}${child.id}`, id, root, nested, depth + 1, [...stack, nested.id]))
    const heads = children.map((c) => c[0])
    const bounds = frameBounds(heads.map((h) => ({ x: h.node.x, y: h.node.y, width: h.width, height: h.height })), rows)
    heads.forEach((head, i) => {
      head.x = bounds.positions[i].x
      head.y = bounds.positions[i].y
    })
    const known = new Set(heads.map((h) => h.node.id))
    for (const edge of nested.graph.edges) {
      if (known.has(edge.source) && known.has(edge.target)) edges.push({ id: `${id}${FLOW_PATH}${edge.id}`, edge, source: `${id}${FLOW_PATH}${edge.source}`, target: `${id}${FLOW_PATH}${edge.target}`, locked: true })
    }
    return [{ ...base, kind: 'frame', width: bounds.width, height: bounds.height }, ...children.flat()]
  }
  return { nodes: graph.nodes.flatMap((node) => place(node, node.id, null, node, null, 0, [])), edges }
}

export type GraphGesture = 'select' | 'inspect' | 'drag' | 'connect' | 'delete' | 'edit' | 'rewire'

export function refuses(plan: GraphPlan, id: string, gesture: GraphGesture): boolean {
  if (gesture === 'select' || gesture === 'inspect') return false
  const node = plan.nodes.find((n) => n.id === id)
  if (node) return node.owner !== null
  return plan.edges.find((e) => e.id === id)?.locked ?? false
}

export function withCollapsed(graph: ActionGraph, id: string, collapsed: boolean): ActionGraph {
  const [head, ...rest] = id.split(FLOW_PATH)
  return {
    ...graph,
    nodes: graph.nodes.map((n) => {
      if (n.id !== head) return n
      const config: Record<string, unknown> = { ...(n.config ?? {}) }
      if (rest.length) config.folds = { ...((config.folds ?? {}) as Record<string, boolean>), [rest.join(FLOW_PATH)]: collapsed }
      else if (collapsed) config.collapsed = true
      else delete config.collapsed
      return { ...n, config }
    })
  }
}

export interface NestedPart {
  node: ActionNode
  owner: ActionCatalogEntry
  root: string
}

export function partOf(graph: ActionGraph, catalog: ActionCatalog | null, id: string | null): NestedPart | null {
  const [head, ...rest] = (id ?? '').split(FLOW_PATH)
  if (!rest.length) return null
  let node = graph.nodes.find((n) => n.id === head)
  let owner: ActionCatalogEntry | null = null
  for (const part of rest) {
    owner = node ? nestedOf(node, catalog) : null
    if (!owner) return null
    node = owner.graph.nodes.find((n) => n.id === part)
  }
  return node && owner ? { node, owner, root: head } : null
}

export function inlineAction(graph: ActionGraph, id: string, catalog: ActionCatalog | null, owner: Owner): ActionGraph {
  const host = graph.nodes.find((n) => n.id === id)
  const entry = host ? nestedOf(host, catalog) : null
  if (!host || !entry) return graph
  const inner = JSON.parse(JSON.stringify(entry.graph)) as ActionGraph
  const signature = { inputs: entry.inputs }
  const starts = new Set(inner.nodes.filter((n) => n.type === NODE_TRIGGER).map((n) => n.id))
  const kept = inner.nodes.filter((n) => !starts.has(n.id))
  const taken = graph.nodes.filter((n) => n.id !== id).map((n) => n.id)
  const ids = new Map<string, string>()
  for (const n of kept) {
    const next = uniqueId(n.id, taken)
    taken.push(next)
    ids.set(n.id, next)
  }
  const minX = Math.min(...kept.map((n) => n.x))
  const minY = Math.min(...kept.map((n) => n.y))
  const nodes = new Map(kept.map((n) => [n.id, { ...n, id: ids.get(n.id)!, x: Math.round(host.x + n.x - minX), y: Math.round(host.y + n.y - minY) }]))
  const incoming = graph.edges.filter((e) => e.target === id)
  const values = (host.config?.values ?? {}) as Record<string, unknown>
  const trigger = graph.nodes.find((n) => n.type === NODE_TRIGGER)
  const fromTrigger = new Set((trigger ? (portsOf(trigger, catalog, owner)?.outputs ?? []) : []).map((p) => p.name))
  const edges = new Map(graph.edges.filter((e) => e.source !== id && e.target !== id).map((e) => [e.id, e]))
  const add = (source: string, source_port: string, target: string, target_port: string) => {
    for (const [key, e] of edges) if (e.target === target && e.target_port === target_port) edges.delete(key)
    const edge: ActionEdge = { id: `${source}.${source_port}-${target}.${target_port}`, source, source_port, target, target_port }
    edges.set(edge.id, edge)
  }
  for (const e of inner.edges) {
    const target = nodes.get(e.target)
    if (!target) continue
    if (!starts.has(e.source)) {
      if (ids.has(e.source)) add(ids.get(e.source)!, e.source_port, target.id, e.target_port)
      continue
    }
    const fed = incoming.find((i) => i.target_port === e.source_port)
    if (fed) add(fed.source, fed.source_port, target.id, e.target_port)
    else if (values[e.source_port] !== undefined && values[e.source_port] !== null) target.config = { ...(target.config ?? {}), values: { ...((target.config?.values ?? {}) as Record<string, unknown>), [e.target_port]: values[e.source_port] } }
    else if (trigger && fromTrigger.has(e.source_port)) add(trigger.id, e.source_port, target.id, e.target_port)
  }
  for (const e of graph.edges.filter((edge) => edge.source === id)) {
    const port = entry.outputs.find((p) => p.name === e.source_port)
    const named = port?.node && ids.has(port.node) ? { node: port.node, port: port.source_port || port.name } : null
    const candidates = kept.filter((n) => portsOf(n, catalog, signature)?.outputs.some((p) => p.name === e.source_port))
    const last = candidates.find((c) => !candidates.some((o) => o !== c && reaches(inner, c.id, o.id)))
    const from = named ?? (last ? { node: last.id, port: e.source_port } : null)
    if (from) add(ids.get(from.node)!, from.port, e.target, e.target_port)
  }
  return { nodes: [...graph.nodes.filter((n) => n.id !== id), ...nodes.values()], edges: [...edges.values()] }
}

export type CodeKind = 'body' | 'prompt' | 'template' | 'action'

export interface CodeSection {
  key: string
  kind: CodeKind
  title: string
  language: 'rhai' | 'html' | 'svelte'
  readonly: boolean
  value: string
  prompt: string | null
}

export interface CodeItem {
  id: string
  node: ActionNode
  title: string
  icon: string
  kind: string
  inputs: ActionPort[]
  outputs: ActionPort[]
  badge: 'builtin' | 'action' | null
  sections: CodeSection[]
}

export function graphOrder(graph: ActionGraph): ActionNode[] {
  const rank = (n: ActionNode) => (n.type === NODE_TRIGGER ? 0 : 1)
  const place = (a: ActionNode, b: ActionNode) => rank(a) - rank(b) || a.y - b.y || a.x - b.x || a.id.localeCompare(b.id)
  const known = new Set(graph.nodes.map((n) => n.id))
  const edges = graph.edges.filter((e) => known.has(e.source) && known.has(e.target) && e.source !== e.target)
  const waiting = new Map(graph.nodes.map((n) => [n.id, 0]))
  for (const e of edges) waiting.set(e.target, (waiting.get(e.target) ?? 0) + 1)
  const ready = graph.nodes.filter((n) => waiting.get(n.id) === 0).sort(place)
  const out: ActionNode[] = []
  while (ready.length) {
    const next = ready.shift()!
    out.push(next)
    for (const e of edges) {
      if (e.source !== next.id) continue
      const left = (waiting.get(e.target) ?? 1) - 1
      waiting.set(e.target, left)
      if (left === 0) {
        ready.push(graph.nodes.find((n) => n.id === e.target)!)
        ready.sort(place)
      }
    }
  }
  const placed = new Set(out.map((n) => n.id))
  return [...out, ...graph.nodes.filter((n) => !placed.has(n.id)).sort(place)]
}

export function codeItems(graph: ActionGraph, catalog: ActionCatalog | null, owner: Owner, locked = false): CodeItem[] {
  const items: CodeItem[] = []
  for (const node of graphOrder(graph)) {
    if (chatOf(node)) continue
    const ports = portsOf(node, catalog, owner) ?? { inputs: [], outputs: [] }
    const builtin = node.type.startsWith(BUILTIN) ? (catalog?.builtins.find((b) => `${BUILTIN}${b.key}` === node.type) ?? null) : null
    const nested = node.type.startsWith(ACTION)
    const own = node.type === NODE_SCRIPT || node.type === NODE_TRIGGER
    const body = own ? node.body : (builtin?.body ?? node.body ?? '')
    const sections: CodeSection[] = []
    if (own) sections.push({ key: 'body', kind: 'body', title: node.type === NODE_TRIGGER ? t('actions.inspector.condition') : t('actions.inspector.code'), language: 'rhai', readonly: locked, value: node.body, prompt: null })
    else if (nested) sections.push({ key: 'action', kind: 'action', title: t('actions.code.nested'), language: 'rhai', readonly: true, value: '', prompt: null })
    else if (body.trim()) sections.push({ key: 'body', kind: 'body', title: t('actions.inspector.codeReadOnly'), language: 'rhai', readonly: true, value: body, prompt: null })
    if (presenterKind(node)) sections.push({ key: 'template', kind: 'template', title: t('actions.inspector.template'), language: 'svelte', readonly: locked, value: typeof node.config?.template === 'string' ? node.config.template : '', prompt: null })
    const overrides = (node.config?.prompts ?? {}) as Record<string, PromptOverride>
    for (const key of promptsUsed(body)) {
      const base = catalog?.prompts.find((p) => p.key === key)
      if (!base) continue
      sections.push({ key: `prompt:${key}`, kind: 'prompt', title: base.name, language: 'html', readonly: locked || !!overrides[key]?.content_id, value: resolvePrompt(base, overrides[key], catalog?.content_prompts ?? []).template, prompt: key })
    }
    if (!sections.length) continue
    items.push({ id: node.id, node, title: label(node, catalog), icon: iconOf(node, catalog), kind: kindLabel(node), inputs: ports.inputs, outputs: ports.outputs, badge: nested ? 'action' : node.type.startsWith(BUILTIN) ? 'builtin' : null, sections })
  }
  return items
}

export function withCode(node: ActionNode, section: CodeSection, text: string, catalog: ActionCatalog | null): ActionNode {
  if (section.readonly) return node
  if (section.kind === 'body') return { ...node, body: text }
  if (section.kind === 'template') return { ...node, config: { ...(node.config ?? {}), template: text } }
  if (section.kind !== 'prompt' || !section.prompt) return node
  const base = catalog?.prompts.find((p) => p.key === section.prompt)
  if (!base) return node
  const overrides = (node.config?.prompts ?? {}) as Record<string, PromptOverride>
  const resolved = resolvePrompt(base, overrides[section.prompt], catalog?.content_prompts ?? [])
  return withPrompt(node, section.prompt, promptOverride(base, { system: resolved.system, template: text, output_schema: resolved.output_schema, connection_id: resolved.connection_id, model: resolved.model }))
}

export function foldToggle(collapsed: Set<string>, id: string): Set<string> {
  const next = new Set(collapsed)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  return next
}

export function codeFocus(items: CodeItem[], selected: string | null): string | null {
  const root = (selected ?? '').split(FLOW_PATH)[0]
  return items.some((item) => item.id === root) ? root : null
}

export const LOG_STEP = 'step'

export interface RunRow {
  id: string
  at: string
  status: string
  label: string
  dry: boolean
  source: string
  duration: string | null
  summary: string
  failed: boolean
  live: boolean
  needs: NodeRequirement[]
}

export interface RunStep {
  node: string
  status: string
  details: string[]
  inputs: unknown
  outputs: unknown
  ms: number | null
  error: string | null
}

function oneLine(value: unknown): string {
  return (typeof value === 'string' ? value : (JSON.stringify(value) ?? '')).replace(/\s+/g, ' ').trim()
}

export function durationLabel(ms: number): string {
  if (ms < 1000) return t('actions.runs.ms', { count: Math.round(ms) })
  if (ms < 60000) return t('actions.runs.seconds', { count: Math.round(ms / 100) / 10 })
  return t('actions.runs.minutes', { count: Math.round(ms / 6000) / 10 })
}

export function runSource(run: ActionRun): string {
  if (run.dry_run) return t('actions.runs.source.mock')
  if (run.depth > 0) return t('actions.runs.source.called')
  const trigger = (isRecord(run.trigger) ? run.trigger : {}) as Record<string, unknown>
  if (trigger.chat) return t('actions.runs.source.chat')
  const payload = isRecord(trigger.payload) ? trigger.payload : {}
  const message = isRecord(payload.message) ? payload.message : null
  if (message) return t('actions.runs.source.message', { name: oneLine(message.sender || message.sender_email || message.subject || message.id) })
  if (typeof trigger.trigger === 'string') return t('actions.runs.source.trigger', { name: trigger.trigger })
  return t('actions.runs.source.manual')
}

export function runRow(run: ActionRun): RunRow {
  const report = runReport(run)
  return {
    id: run.id,
    at: run.started_at,
    status: report.status,
    label: report.label,
    dry: report.dry,
    source: runSource(run),
    duration: report.duration,
    summary: report.message,
    failed: report.failed,
    live: run.status === 'running',
    needs: report.needs
  }
}

function parsed(text: string): Record<string, unknown> | null {
  try {
    const value = JSON.parse(text)
    return isRecord(value) ? value : null
  } catch {
    return null
  }
}

export function runSteps(run: ActionRun): RunStep[] {
  const steps = new Map<string, RunStep>()
  for (const entry of run.log) {
    const step = steps.get(entry.node) ?? { node: entry.node, status: entry.status, details: [], inputs: undefined, outputs: undefined, ms: null, error: null }
    steps.set(entry.node, step)
    if (entry.kind === LOG_STEP) {
      const io = parsed(entry.detail)
      step.inputs = io?.inputs
      step.outputs = io?.outputs
      step.ms = typeof io?.ms === 'number' ? io.ms : null
      continue
    }
    if (entry.kind !== 'effect' && entry.kind !== 'log') step.status = entry.status
    if (entry.status === 'failed') step.error = entry.detail
    else step.details.push(entry.detail)
  }
  return [...steps.values()]
}

export const ASSISTANT_PROMPT = 'assistant.prompt'
export const ASSISTANT_PROMPT_KEY = 'author_action'

export type AssistStatus = 'applied' | 'discarded' | 'undone'

export interface AssistTurn {
  role: 'user' | 'assistant'
  text: string
  scope: string | null
  operations: ActionOperation[]
  previews: ActionOperationPreview[]
  errors: string[]
  ids: Record<string, string>
  status: AssistStatus | null
  failed: boolean
}

export function assistTurn(role: AssistTurn['role'], text: string, scope: string | null, more: Partial<AssistTurn> = {}): AssistTurn {
  return { role, text, scope, operations: [], previews: [], errors: [], ids: {}, status: null, failed: false, ...more }
}

export function assistHistory(turns: AssistTurn[]): ActionAssistMessage[] {
  return turns
    .filter((turn) => !turn.failed)
    .map((turn): ActionAssistMessage =>
      turn.role === 'user'
        ? { role: 'user', content: turn.text }
        : { role: 'assistant', content: JSON.stringify(turn.errors.length ? { reply: turn.text, errors: turn.errors } : { reply: turn.text, operations: turn.operations }), status: turn.status }
    )
}

export interface OperationChange {
  added: string[]
  removed: string[]
  changed: string[]
  wires: number
  renamed: { from: string; to: string } | null
  moved: boolean
}

function settled(node: ActionNode): string {
  return JSON.stringify({ ...node, x: 0, y: 0 })
}

export function operationChange(preview: ActionOperationPreview): OperationChange {
  const before = preview.before?.graph ?? { nodes: [], edges: [] }
  const after = preview.after?.graph ?? { nodes: [], edges: [] }
  const was = new Map(before.nodes.map((n) => [n.id, n]))
  const now = new Set(after.nodes.map((n) => n.id))
  const wiredBefore = new Set(before.edges.map((e) => e.id))
  const wiredAfter = new Set(after.edges.map((e) => e.id))
  return {
    added: after.nodes.filter((n) => !was.has(n.id)).map((n) => n.id),
    removed: before.nodes.filter((n) => !now.has(n.id)).map((n) => n.id),
    changed: after.nodes.filter((n) => was.has(n.id) && settled(was.get(n.id)!) !== settled(n)).map((n) => n.id),
    wires: [...wiredAfter].filter((id) => !wiredBefore.has(id)).length + [...wiredBefore].filter((id) => !wiredAfter.has(id)).length,
    renamed: preview.before && preview.after && preview.before.name !== preview.after.name ? { from: preview.before.name, to: preview.after.name } : null,
    moved: !!preview.before && !!preview.after && (preview.group_before ?? null) !== (preview.group_after ?? null)
  }
}

export function operationTarget(preview: ActionOperationPreview, ids: Record<string, string>): string | null {
  if (!preview.after) return null
  return preview.before ? preview.action_id : (ids[preview.action_id] ?? null)
}

export interface NodeControls {
  path: string[]
  schema: JsonSchema
}

export function configSchemaOf(node: ActionNode): NodeControls | null {
  const trigger = triggerOf(node)
  if (!trigger) return null
  if (trigger.on === 'schedule') {
    const days = [1, 2, 3, 4, 5, 6, 7]
    return {
      path: ['schedule'],
      schema: {
        type: 'object',
        properties: {
          every_minutes: { type: ['integer', 'null'], minimum: 1, title: t('actions.inspector.everyMinutes') },
          at: { type: ['string', 'null'], format: 'time', title: t('actions.inspector.orAt') },
          days: { type: 'array', title: t('actions.inspector.days'), items: { type: 'integer', enum: days, labels: days.map((day) => formatDate(new Date(2024, 0, day), { weekday: 'short' })) } }
        }
      }
    }
  }
  if (trigger.on === 'custom') {
    return {
      path: ['watch'],
      schema: {
        type: 'object',
        properties: {
          mode: { type: 'string', enum: ['webhook', 'poll'], labels: [t('actions.inspector.watchWebhook'), t('actions.inspector.watchPoll')], title: t('actions.inspector.watchMode') },
          url: { type: ['string', 'null'], format: 'uri', title: t('actions.inspector.watchUrl') },
          every_minutes: { type: ['integer', 'null'], minimum: 1, title: t('actions.inspector.everyMinutes') },
          pick: { type: ['string', 'null'], title: t('actions.inspector.watchPick') },
          token: { type: ['string', 'null'], title: t('actions.inspector.watchToken') }
        }
      }
    }
  }
  if (trigger.on === 'event_soon') {
    return { path: [], schema: { type: 'object', properties: { minutes_before: { type: ['integer', 'null'], minimum: 1, title: t('actions.filter.minutesBefore') } } } }
  }
  return null
}

export function controlCount(node: ActionNode): number {
  return Object.keys(configSchemaOf(node)?.schema.properties ?? {}).length
}

export function configValueOf(node: ActionNode, controls: NodeControls): Record<string, unknown> {
  const value = valueAt(node.config ?? {}, controls.path)
  return isRecord(value) ? value : {}
}

export function withNodeConfig(graph: ActionGraph, id: string, path: string[], value: Record<string, unknown>): ActionGraph {
  return { ...graph, nodes: graph.nodes.map((n) => (n.id === id ? { ...n, config: withValue(n.config ?? {}, path, value) as Record<string, unknown> } : n)) }
}

export interface RunReport {
  status: string
  label: string
  dry: boolean
  counts: { status: string; count: number }[]
  effects: string[]
  message: string
  duration: string | null
  needs: NodeRequirement[]
  failed: boolean
}

export function runReport(run: ActionRun): RunReport {
  const ms = run.finished_at ? Date.parse(run.finished_at) - Date.parse(run.started_at) : NaN
  const outputs = Object.entries(run.outputs ?? {})
  const needs = requirementsFor([run.requirement])
  const steps = runSteps(run)
  const tally = new Map<string, number>()
  for (const step of steps) tally.set(step.status, (tally.get(step.status) ?? 0) + 1)
  const effects = run.log.filter((entry) => entry.kind === 'effect').map((entry) => oneLine(entry.detail))
  const told = run.result && typeof run.result === 'object' && typeof run.result.message === 'string' && run.result.message.trim() ? oneLine(run.result.message) : null
  const message = needs.length
    ? needs.map((n) => n.label).join(', ')
    : run.error
      ? oneLine(run.error)
      : told
        ? told
        : outputs.length
        ? outputs.map(([name, value]) => `${name}: ${oneLine(value)}`).join(' · ')
        : effects.length
          ? effects.length > 1
            ? `${effects[0]} (+${effects.length - 1})`
            : effects[0]
          : steps.length
            ? t('actions.runs.report.steps', { count: steps.length })
            : t('actions.runs.noOutputs')
  const status = needs.length ? REQUIREMENT : run.status
  return {
    status,
    label: statusLabel(status),
    dry: run.dry_run,
    counts: ['done', 'would', 'skipped', 'failed', 'waiting', 'requirement'].filter((key) => tally.has(key)).map((key) => ({ status: key, count: tally.get(key) ?? 0 })),
    effects,
    message: message.length > 200 ? `${message.slice(0, 199)}…` : message,
    duration: Number.isFinite(ms) && ms >= 0 ? durationLabel(ms) : null,
    needs,
    failed: run.status === 'failed' && !needs.length
  }
}
