<script lang="ts">
  import '@xyflow/svelte/dist/style.css'
  import { SvelteFlow, Background, BackgroundVariant, MiniMap, useSvelteFlow, type Node, type Edge, type Connection } from '@xyflow/svelte'
  import { untrack } from 'svelte'
  import ActionNodeCard from './ActionNodeCard.svelte'
  import { t } from '$lib/i18n.svelte'
  import GraphControls from './GraphControls.svelte'
  import type { ActionCatalog, ActionDiagnostic, ActionGraph, ActionRun } from '$lib/tauri/types'
  import { configSchemaOf, configValueOf, connect, connectionProblem, descriptionOf, graphPlan, graphRole, iconOf, IN, kindLabel, label, layout, nodeFromAction, OUT, portsOf, problemsFor, refuses, removeNodes, requirementsFor, triggerOf, validEnds, withCollapsed, withNodeConfig, type GraphPlan, type Owner, type Placeable, type Wire } from '$lib/actions'

  interface Props {
    graph: ActionGraph
    catalog: ActionCatalog | null
    owner: Owner
    diagnostics: ActionDiagnostic[]
    run: ActionRun | null
    selected: string | null
    dark: boolean
    onchange: (graph: ActionGraph) => void
    onreject: (reason: string) => void
    onopen?: (actionId: string) => void
    onclone?: (nodeId: string) => void
    preview?: boolean
  }

  let { graph, catalog, owner, diagnostics, run, selected = $bindable(), dark, onchange, onreject, onopen, onclone, preview = false }: Props = $props()

  const flow = useSvelteFlow()
  let direction = $state<'TB' | 'LR'>('LR')
  let locked = $state(untrack(() => preview))
  let hits = $state<string[]>([])
  let ends = $state<Map<string, string[]> | null>(null)
  let wiring: string | null = null
  let nodes = $state.raw<Node[]>([])
  let edges = $state.raw<Edge[]>([])
  let plan: GraphPlan = { nodes: [], edges: [] }
  const nodeTypes = { action: ActionNodeCard }

  const statuses = $derived.by(() => {
    const out = new Map<string, string>()
    for (const entry of run?.log ?? []) if (entry.kind !== 'effect' && entry.kind !== 'log') out.set(entry.node, entry.status)
    return out
  })

  $effect(() => {
    const current = graph
    const problems = diagnostics
    const marks = statuses
    const found = hits
    const known = catalog
    untrack(() => {
      plan = graphPlan(current, known, owner)
      nodes = plan.nodes.map((placed) => {
        const node = placed.node
        const top = placed.owner === null
        const notes = placed.owner ? placed.owner.diagnostics : problems
        const ports = portsOf(node, known, placed.owner ? { inputs: placed.owner.inputs } : owner) ?? { inputs: [], outputs: [] }
        const trigger = triggerOf(node)
        const nested = placed.nested
        const frame = placed.kind === 'frame'
        const controls = top ? configSchemaOf(node) : null
        return {
          id: placed.id,
          type: 'action',
          position: { x: placed.x, y: placed.y },
          ...(placed.parent ? { parentId: placed.parent, extent: 'parent' as const, draggable: false, connectable: false, deletable: false } : {}),
          ...(frame ? { width: placed.width, height: placed.height } : {}),
          selected: placed.id === selected,
          class: found.includes(placed.id) ? 'graph-hit' : '',
          data: {
            title: label(node, known),
            kind: kindLabel(node),
            role: graphRole(placed.owner ? placed.owner.graph : current, node, known),
            ends: () => (ends && placed.id !== wiring ? (ends.get(placed.id) ?? []) : null),
            icon: iconOf(node, known),
            description: descriptionOf(node, known),
            inputs: ports.inputs,
            outputs: ports.outputs,
            problems: problemsFor(notes, node.id, 'node').map((d) => (d.line ? t('actions.page.line', { line: d.line }) + d.message : d.message)),
            requirements: requirementsFor(notes, node.id),
            status: marks.get(placed.id) ?? null,
            enabled: trigger ? !trigger.disabled : null,
            locked: !top,
            controls: controls ? { schema: controls.schema, value: configValueOf(node, controls), onchange: (next: Record<string, unknown>) => onchange(withNodeConfig(positions(), node.id, controls.path, next)) } : null,
            frame: frame ? { width: placed.width, height: placed.height } : null,
            nested: nested
              ? {
                  name: nested.name,
                  key: nested.key,
                  enabled: nested.enabled,
                  count: nested.graph.nodes.length,
                  problems: nested.diagnostics.filter((d) => d.severity === 'error').length,
                  needs: requirementsFor(nested.diagnostics).length,
                  collapsed: placed.collapsed,
                  more: placed.kind === 'more',
                  onedit: () => onopen?.(nested.id),
                  onclone: top && onclone ? () => onclone(node.id) : undefined,
                  onfold: placed.kind === 'more' || nested.graph.nodes.length <= 1 ? undefined : () => onchange(withCollapsed(positions(), placed.id, !placed.collapsed))
                }
              : null
          }
        }
      })
      edges = plan.edges.map((placed) => {
        const edge = placed.edge
        const bad = !placed.locked && problemsFor(problems, edge.id, 'edge').length > 0
        const ran = marks.get(placed.source) === 'done' && marks.has(placed.target) && marks.get(placed.target) !== 'skipped'
        return {
          id: placed.id,
          source: placed.source,
          sourceHandle: `${OUT}${edge.source_port}`,
          target: placed.target,
          targetHandle: `${IN}${edge.target_port}`,
          animated: ran,
          ...(placed.locked ? { selectable: false, deletable: false, focusable: false } : {}),
          class: bad ? 'graph-edge-bad' : placed.locked ? 'graph-edge graph-edge-locked' : 'graph-edge'
        }
      })
    })
  })

  function lockedReason(wire: Wire): string | null {
    const id = [wire.source, wire.target].find((end) => refuses(plan, end, 'connect'))
    const holder = id ? plan.nodes.find((n) => n.id === id)?.owner : null
    return id ? t('actions.frame.locked', { name: holder?.name ?? id }) : null
  }

  function positions(): ActionGraph {
    const placed = new Map(flow.getNodes().map((n) => [n.id, n.position]))
    return { ...graph, nodes: graph.nodes.map((n) => ({ ...n, x: Math.round(placed.get(n.id)?.x ?? n.x), y: Math.round(placed.get(n.id)?.y ?? n.y) })) }
  }

  function valid(connection: Edge | Connection): boolean {
    return !lockedReason(connection) && !connectionProblem(graph, catalog, owner, connection)
  }

  function onconnect(connection: Connection) {
    const problem = lockedReason(connection) ?? connectionProblem(graph, catalog, owner, connection)
    if (problem) {
      onreject(problem)
      return
    }
    onchange(connect(positions(), connection))
  }

  function onconnectstart(_: unknown, params: { nodeId: string | null; handleId: string | null; handleType: 'source' | 'target' | null }) {
    const start = params.nodeId && params.handleId && params.handleType && graph.nodes.some((n) => n.id === params.nodeId) ? { node: params.nodeId, handle: params.handleId, type: params.handleType } : null
    wiring = start?.node ?? null
    ends = start ? validEnds(graph, catalog, owner, start) : null
  }

  function onconnectend(_: unknown, state: { isValid: boolean | null; fromNode: { id: string } | null; toNode: { id: string } | null; fromHandle: { id?: string | null; type?: string } | null; toHandle: { id?: string | null } | null }) {
    ends = null
    wiring = null
    if (state.isValid !== false || !state.fromNode || !state.toNode) return
    const pair = [{ node: state.fromNode.id, handle: state.fromHandle?.id }, { node: state.toNode.id, handle: state.toHandle?.id }]
    const [from, to] = state.fromHandle?.type === 'target' ? [pair[1], pair[0]] : pair
    const wire = { source: from.node, sourceHandle: from.handle, target: to.node, targetHandle: to.handle }
    const problem = lockedReason(wire) ?? connectionProblem(graph, catalog, owner, wire)
    if (problem) onreject(problem)
  }

  export function dropAt(item: Placeable, clientX: number, clientY: number) {
    if (locked) return
    const at = flow.screenToFlowPosition({ x: clientX, y: clientY })
    const node = nodeFromAction(item, graph, { x: at.x - 120, y: at.y - 40 })
    selected = node.id
    onchange({ ...positions(), nodes: [...positions().nodes, node] })
  }

  export function center(): { x: number; y: number } {
    const box = document.querySelector('.action-graph')?.getBoundingClientRect()
    return box ? { x: box.left + box.width / 2, y: box.top + box.height / 2 } : { x: 0, y: 0 }
  }

  export function tidy(next: 'TB' | 'LR' = direction) {
    direction = next
    onchange(layout(graph, next, new Map(plan.nodes.filter((n) => !n.parent).map((n) => [n.id, { width: n.width, height: n.height }]))))
    requestAnimationFrame(() => flow.fitView({ padding: 0.3, duration: 300 }))
  }

  function search(query: string): string[] {
    const needle = query.trim().toLowerCase()
    hits = needle ? graph.nodes.filter((n) => `${label(n, catalog)} ${n.id} ${n.type}`.toLowerCase().includes(needle)).map((n) => n.id) : []
    return hits
  }
</script>

<div class="action-graph relative h-full w-full {dark ? 'dark' : ''}">
  <SvelteFlow
    bind:nodes
    bind:edges
    {nodeTypes}
    colorMode={dark ? 'dark' : 'light'}
    minZoom={0.2}
    maxZoom={2.5}
    fitView
    fitViewOptions={{ padding: 0.3 }}
    zoomOnScroll={!preview}
    preventScrolling={!preview}
    nodesDraggable={!locked}
    nodesConnectable={!locked}
    elementsSelectable
    deleteKey={locked ? null : ['Backspace', 'Delete']}
    isValidConnection={valid}
    {onconnect}
    {onconnectstart}
    {onconnectend}
    onnodeclick={({ node }) => (selected = node.id)}
    onpaneclick={() => (selected = null)}
    onnodedragstop={() => onchange(positions())}
    ondelete={({ nodes: gone, edges: cut }) => {
      let next = removeNodes(positions(), gone.filter((n) => !refuses(plan, n.id, 'delete')).map((n) => n.id))
      next = { ...next, edges: next.edges.filter((e) => !cut.some((c) => c.id === e.id && !refuses(plan, c.id, 'delete'))) }
      if (selected && gone.some((n) => n.id === selected)) selected = null
      onchange(next)
    }}
    proOptions={{ hideAttribution: true }}
  >
    <Background variant={BackgroundVariant.Lines} gap={50} patternColor={dark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.04)'} />
    {#if !preview}
      <MiniMap position="top-left" pannable zoomable nodeColor={() => 'var(--ink-3)'} />
      <GraphControls {direction} {locked} onlayout={(d) => tidy(d)} onlock={(value) => (locked = value)} onsearch={search} />
    {/if}
  </SvelteFlow>
</div>

<style>
  .action-graph :global(.svelte-flow) {
    background: transparent;
    --xy-background-color: transparent;
    --xy-node-background-color: transparent;
    --xy-node-border: none;
    --xy-node-boxshadow-selected: none;
    --xy-edge-stroke: var(--ink-3);
    --xy-edge-stroke-selected: var(--accent);
    --xy-connectionline-stroke: var(--accent);
    --xy-minimap-background-color: color-mix(in srgb, var(--paper) 80%, transparent);
    --xy-minimap-mask-background-color: color-mix(in srgb, var(--ink) 18%, transparent);
    --xy-controls-button-background-color: var(--paper);
  }
  .action-graph :global(.svelte-flow__node) {
    padding: 0;
    border-radius: 0.5rem;
  }
  .action-graph :global(.svelte-flow__node.graph-hit) {
    outline: 3px solid var(--accent);
    outline-offset: 2px;
    border-radius: 0.625rem;
  }
  .action-graph :global(.graph-edge .svelte-flow__edge-path) {
    stroke-width: 1.5;
  }
  .action-graph :global(.graph-edge-bad .svelte-flow__edge-path) {
    stroke: var(--color-error-500, #d33);
    stroke-dasharray: 5 4;
  }
  .action-graph :global(.graph-edge-locked .svelte-flow__edge-path) {
    stroke: color-mix(in srgb, var(--ink-3) 70%, transparent);
  }
  .action-graph :global(.graph-edge-locked) {
    pointer-events: none;
  }
  .action-graph :global(.svelte-flow__minimap) {
    border-radius: 0.5rem;
    transform: scale(0.75);
    transform-origin: top left;
    box-shadow: inset 0 0 0 1px var(--line-strong);
  }
</style>
