<script lang="ts">
  import Icon from '@iconify/svelte'
  import { Handle, Position } from '@xyflow/svelte'
  import type { ActionPort, NodeRole } from '$lib/tauri/types'
  import { goto } from '$app/navigation'
  import { FRAME_HEAD, IN, OUT, PORT_ROW, requirementBadge, type JsonSchema, type NodeRequirement } from '$lib/actions'
  import SchemaControls from './SchemaControls.svelte'
  import { Button, IconButton, MenuItem } from '$lib/components/ui'
  import { anchored, boxOf } from '$lib/popover'
  import { STATUS_DOT } from './RunOutcome.svelte'
  import { t } from '$lib/i18n.svelte'

  interface NestedData {
    name: string
    key: string | null
    enabled: boolean
    count: number
    problems: number
    needs: number
    collapsed: boolean
    more: boolean
    onedit: () => void
    onclone?: () => void
    onfold?: () => void
  }

  interface CardData {
    nested?: NestedData | null
    frame?: { width: number; height: number } | null
    locked?: boolean
    title: string
    kind: string
    role: NodeRole
    ends?: () => string[] | null
    icon: string
    description: string
    inputs: ActionPort[]
    outputs: ActionPort[]
    problems: string[]
    requirements: NodeRequirement[]
    status: string | null
    enabled: boolean | null
    controls?: { schema: JsonSchema; value: Record<string, unknown>; onchange: (next: Record<string, unknown>) => void } | null
  }

  let { data, selected = false }: { data: CardData; selected?: boolean; [key: string]: unknown } = $props()

  const rows = $derived(Math.max(data.inputs.length, data.outputs.length))
  const badge = $derived(requirementBadge(data.requirements ?? []))
  const accepting = $derived(data.ends?.() ?? null)
  const wiringClass = $derived(accepting ? (accepting.length ? 'is-target' : 'is-dimmed') : '')
  let menuOpen = $state(false)
  let menuButton = $state<HTMLSpanElement | null>(null)

  function act(e: MouseEvent, run: (() => void) | undefined) {
    e.stopPropagation()
    menuOpen = false
    run?.()
  }
</script>

{#snippet ports()}
  <div class="grid {data.inputs.length && data.outputs.length ? 'grid-cols-2' : 'grid-cols-1'} border-t border-line py-1">
    {#if data.inputs.length}
    <div class="flex flex-col">
      {#each data.inputs as port (port.name)}
        <span class="relative flex items-center gap-1 pl-3 pr-1 text-[11.5px] text-ink-2" style="height: {PORT_ROW}px" title="{port.name}: {port.type}">
          <Handle type="target" position={Position.Left} id="{IN}{port.name}" class="action-handle {accepting?.includes(`${IN}${port.name}`) ? 'is-valid' : ''}" />
          <span class="overflow-hidden text-ellipsis whitespace-nowrap">{port.name}</span>
          <span class="font-mono text-[10px] text-ink-3">{port.type}</span>
          {#if port.label}<span data-testid="port-label" class="overflow-hidden text-ellipsis whitespace-nowrap text-ink-3">{port.label}</span>{/if}
        </span>
      {/each}
    </div>
    {/if}
    {#if data.outputs.length}
    <div class="flex flex-col items-end">
      {#each data.outputs as port (port.name)}
        <span class="relative flex items-center justify-end gap-1 pl-1 pr-3 text-[11.5px] text-ink-2" style="height: {PORT_ROW}px" title="{port.name}: {port.type}">
          {#if port.label}<span data-testid="port-label" class="overflow-hidden text-ellipsis whitespace-nowrap text-ink-3">{port.label}</span>{/if}
          <span class="font-mono text-[10px] text-ink-3">{port.type}</span>
          <span class="overflow-hidden text-ellipsis whitespace-nowrap">{port.name}</span>
          <Handle type="source" position={Position.Right} id="{OUT}{port.name}" class="action-handle {accepting?.includes(`${OUT}${port.name}`) ? 'is-valid' : ''}" />
        </span>
      {/each}
    </div>
    {/if}
  </div>
{/snippet}

{#snippet health(nested: NestedData)}
  {#if data.problems.length}
    <span class="flex min-w-0 items-center gap-1 text-error-600" title={data.problems.join('\n')}><Icon icon="lucide:circle-alert" class="shrink-0" /><span class="overflow-hidden text-ellipsis whitespace-nowrap">{data.problems[0]}</span></span>
  {:else if nested.problems}
    <span class="flex items-center gap-1 text-error-600"><Icon icon="lucide:circle-alert" />{t('actions.page.problems', { count: nested.problems })}</span>
  {:else}
    <span class="flex items-center gap-1"><Icon icon="lucide:circle-check" />{t('actions.page.ready')}</span>
  {/if}
  {#if nested.needs}
    <span class="flex items-center gap-1 text-warn-600"><Icon icon="lucide:triangle-alert" />{t('actions.page.needs', { count: nested.needs })}</span>
  {/if}
{/snippet}

{#if data.frame && data.nested}
  {@const nested = data.nested}
  <div data-testid="action-frame" data-role={data.role} class="action-frame relative flex flex-col rounded-[0.75rem] text-left {wiringClass} {selected ? 'is-selected' : ''} {data.problems.length || nested.problems ? 'has-problem' : badge ? 'has-requirement' : ''} {data.locked ? 'is-locked' : ''}" style="width: {data.frame.width}px; height: {data.frame.height}px">
    {#if badge}
      <button
        type="button"
        data-testid="node-requirement"
        data-href={badge.href}
        class="requirement-badge nodrag nopan absolute -top-2.5 right-3 flex h-5 items-center gap-1 rounded-[0.25rem] px-1.5 font-mono text-[10.5px] font-medium"
        title={badge.title}
        aria-label={badge.title}
        onpointerdown={(e) => e.stopPropagation()}
        onclick={(e) => { e.stopPropagation(); goto(badge.href) }}
      ><Icon icon="lucide:triangle-alert" class="text-[11px]" />{badge.label}</button>
    {/if}
    <header class="flex min-w-0 items-center gap-2 px-3" style="height: {FRAME_HEAD}px">
      <span class="flex size-7 shrink-0 items-center justify-center rounded-[0.375rem] bg-paper-3 text-[14px] text-ink-2"><Icon icon={data.icon} /></span>
      <span class="flex min-w-0 flex-1 flex-col gap-0.5">
        <span class="flex min-w-0 items-baseline gap-2">
          <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[13.5px] font-semibold {nested.enabled ? 'text-ink' : 'text-ink-3 line-through decoration-ink-3'}">{nested.name}</span>
          {#if nested.key}<span class="shrink-0 font-mono text-[10.5px] text-ink-3">{nested.key}</span>{/if}
        </span>
        <span class="flex min-w-0 items-center gap-2.5 font-mono text-[10.5px] text-ink-3">
          <span class="shrink-0">{nested.enabled ? t('actions.frame.on') : t('actions.frame.off')}</span>
          <span class="shrink-0 tabular-nums">{t('actions.frame.nodes', { count: nested.count })}</span>
          {@render health(nested)}
        </span>
      </span>
      {#if data.status}
        <span class="size-2 shrink-0 rounded-full {STATUS_DOT[data.status] ?? 'bg-ink-3'}" title={t('actions.node.lastRun', { status: data.status })}></span>
      {/if}
      <span class="nodrag nopan flex shrink-0 items-center gap-0.5" role="presentation" onpointerdown={(e) => e.stopPropagation()}>
        <Button size="sm" testid="node-edit" icon="lucide:square-pen" title={t('actions.frame.editNamed', { name: nested.name })} onclick={(e) => act(e, nested.onedit)}>{t('actions.frame.edit')}</Button>
        {#if nested.onclone}
          <Button size="sm" testid="node-clone" icon="lucide:copy-plus" title={t('actions.frame.cloneNamed', { name: nested.name })} onclick={(e) => act(e, nested.onclone)}>{t('actions.frame.clone')}</Button>
        {/if}
        {#if nested.onfold}
          <IconButton size="sm" testid="node-fold" icon="lucide:chevrons-down-up" label={t('actions.frame.collapse')} onclick={(e) => act(e, nested.onfold)} />
        {/if}
      </span>
    </header>
    {#if rows}{@render ports()}{/if}
    <div class="action-frame-body min-h-0 flex-1 border-t border-line"></div>
  </div>
{:else}
<div data-testid="action-node" data-role={data.role} data-locked={data.locked ? 'true' : 'false'} class="action-card relative flex w-max min-w-[15rem] flex-col rounded-[0.5rem] text-left {wiringClass} {selected ? 'is-selected' : ''} {data.problems.length ? 'has-problem' : badge ? 'has-requirement' : ''} {data.enabled === false || data.nested?.enabled === false ? 'is-off' : ''} {data.locked ? 'is-locked' : ''}">
  {#if badge}
    <button
      type="button"
      data-testid="node-requirement"
      data-href={badge.href}
      class="requirement-badge nodrag nopan absolute -top-2.5 right-3 flex h-5 items-center gap-1 rounded-[0.25rem] px-1.5 font-mono text-[10.5px] font-medium"
      title={badge.title}
      aria-label={badge.title}
      onpointerdown={(e) => e.stopPropagation()}
      onclick={(e) => { e.stopPropagation(); goto(badge.href) }}
    ><Icon icon="lucide:triangle-alert" class="text-[11px]" />{badge.label}</button>
  {/if}
  <div class="flex min-w-0 items-start gap-2 px-3 pt-2.5 pb-2">
    <span class="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-[0.375rem] bg-paper-3 text-[14px] text-ink-2"><Icon icon={data.icon} /></span>
    <span class="flex min-w-0 flex-1 flex-col">
      <span class="whitespace-nowrap text-[13px] font-semibold text-ink">{data.title}</span>
      <span data-testid="node-label" data-role={data.role} class="mt-0.5 w-max rounded-[0.25rem] bg-paper-3 px-1 font-mono text-[9.5px] uppercase leading-4 tracking-wide text-ink-2">{data.role === 'trigger' ? t('actions.role.trigger') : t('actions.role.function')}</span>
      {#if data.nested}<span class="whitespace-nowrap font-mono text-[10.5px] uppercase tracking-wide text-ink-3" title={data.nested.more ? t('actions.frame.moreHint') : undefined}>{data.nested.more ? t('actions.frame.more') : t('actions.frame.nodes', { count: data.nested.count })}</span>{/if}
    </span>
    {#if data.nested}
      {@const nested = data.nested}
      <span class="nodrag nopan mt-0.5 flex shrink-0" role="presentation" bind:this={menuButton} onpointerdown={(e) => e.stopPropagation()}>
        <button type="button" data-testid="node-menu" class="flex size-6 items-center justify-center rounded-[0.375rem] text-[12px] text-ink-3 hover:bg-paper-3 hover:text-ink {menuOpen ? 'bg-paper-3 text-ink' : ''}" title={t('actions.frame.menu', { name: nested.name })} aria-label={t('actions.frame.menu', { name: nested.name })} aria-expanded={menuOpen} onclick={(e) => { e.stopPropagation(); menuOpen = !menuOpen }}><Icon icon="lucide:ellipsis-vertical" /></button>
      </span>
      {#if menuOpen}
        <div class="menu w-56" use:anchored={{ at: () => boxOf(menuButton), align: 'end', onclose: () => (menuOpen = false) }} role="menu">
          <MenuItem testid="node-menu-edit" icon="lucide:square-pen" onclick={() => { menuOpen = false; nested.onedit() }}>{t('actions.frame.edit')}</MenuItem>
          {#if nested.onclone}
            <MenuItem testid="node-menu-clone" icon="lucide:copy-plus" onclick={() => { menuOpen = false; nested.onclone?.() }}>{t('actions.frame.clone')}</MenuItem>
          {/if}
          {#if nested.onfold}
            <MenuItem testid="node-menu-fold" icon="lucide:chevrons-up-down" onclick={() => { menuOpen = false; nested.onfold?.() }}>{t('actions.frame.expand')}</MenuItem>
          {/if}
        </div>
      {/if}
    {/if}
    {#if data.status}
      <span class="mt-1.5 size-2 shrink-0 rounded-full {STATUS_DOT[data.status] ?? 'bg-ink-3'}" title={t('actions.node.lastRun', { status: data.status })}></span>
    {/if}
  </div>
  {#if data.description}
    <p class="action-card-description w-0 min-w-full px-3 pb-2 text-[12px] leading-4 text-ink-3">{data.description}</p>
  {/if}
  {#if data.controls}
    <div class="nodrag nopan border-t border-line px-3 py-1" role="presentation" onpointerdown={(e) => e.stopPropagation()}>
      <SchemaControls size="sm" testid="node-controls" schema={data.controls.schema} value={data.controls.value} onchange={data.controls.onchange} />
    </div>
  {/if}
  {#if rows}{@render ports()}{/if}
  {#if data.problems.length}
    <p class="border-t border-line px-3 py-1.5 text-[11.5px] leading-4 text-error-600" title={data.problems.join('\n')}>{data.problems[0]}{data.problems.length > 1 ? ` (+${data.problems.length - 1})` : ''}</p>
  {/if}
</div>
{/if}

<style>
  .action-card {
    background: var(--paper);
    box-shadow: inset 0 0 0 1px var(--line-strong);
  }
  .action-card:hover {
    box-shadow:
      inset 0 0 0 1px color-mix(in srgb, var(--ink) 22%, transparent),
      0 4px 12px rgb(0 0 0 / 0.12);
  }
  .action-card.is-selected {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      0 0 0 2px var(--accent-soft);
  }
  .action-card.is-off {
    opacity: 0.6;
  }
  .action-card.is-locked {
    cursor: default;
  }
  .action-card.is-locked:not(.is-selected):hover {
    box-shadow: inset 0 0 0 1px var(--line-strong);
  }
  .action-frame {
    background: color-mix(in srgb, var(--paper-2) 70%, transparent);
    box-shadow: inset 0 0 0 1px var(--line-strong);
  }
  .action-frame > header {
    background: var(--paper);
    border-radius: 0.75rem 0.75rem 0 0;
  }
  .action-frame.is-selected {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      0 0 0 2px var(--accent-soft);
  }
  .action-frame.has-problem {
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-error-500, #d33) 70%, transparent);
  }
  .action-frame.has-requirement {
    box-shadow: inset 0 0 0 1.5px var(--color-warn-500);
  }
  .action-frame :global(.action-handle) {
    width: 0.625rem;
    height: 0.625rem;
    background: var(--paper);
    border: 1px solid var(--ink-3);
  }
  .action-card.has-problem {
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-error-500, #d33) 70%, transparent);
  }
  .action-card.has-requirement {
    box-shadow: inset 0 0 0 1.5px var(--color-warn-500);
  }
  .action-card.has-requirement.is-selected {
    box-shadow:
      inset 0 0 0 1.5px var(--color-warn-500),
      0 0 0 2px var(--accent-soft);
  }
  .requirement-badge {
    background: var(--color-warn-500);
    color: var(--paper);
  }
  .requirement-badge:hover {
    background: var(--color-warn-600);
  }
  .action-card :global(.action-handle) {
    width: 0.625rem;
    height: 0.625rem;
    background: var(--paper);
    border: 1px solid var(--ink-3);
    opacity: 0.35;
  }
  .action-card:hover :global(.action-handle),
  .action-card.is-selected :global(.action-handle) {
    opacity: 1;
  }
  .action-card.is-dimmed,
  .action-frame.is-dimmed {
    opacity: 0.35;
  }
  .action-card.is-target,
  .action-frame.is-target {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      0 0 0 3px var(--accent-soft);
  }
  .action-card.is-target :global(.action-handle),
  .action-card.is-dimmed :global(.action-handle) {
    opacity: 1;
  }
  .action-card :global(.action-handle.is-valid),
  .action-frame :global(.action-handle.is-valid) {
    background: var(--accent);
    border-color: var(--accent);
    transform: scale(1.3);
  }
</style>
