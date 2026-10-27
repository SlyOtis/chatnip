<script lang="ts">
  import { onMount, tick } from 'svelte'
  import Icon from '@iconify/svelte'
  import { Button, IconButton } from '$lib/components/ui'
  import CodeEditor from './CodeEditor.svelte'
  import { api } from '$lib/tauri'
  import type { ActionCatalog, ActionDiagnostic, ActionGraph, ActionNode } from '$lib/tauri/types'
  import { ACTION, codeFocus, codeItems, foldToggle, NODE_TRIGGER, presenterKind, problemsFor, withCode, type CodeItem, type CodeSection, type Owner } from '$lib/actions'
  import { defaultTemplate } from '$lib/template'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    graph: ActionGraph
    catalog: ActionCatalog | null
    owner: Owner
    diagnostics: ActionDiagnostic[]
    selected: string | null
    locked: boolean
    onchange: (node: ActionNode) => void
    onselect: (nodeId: string) => void
  }

  let { graph, catalog, owner, diagnostics, selected, locked, onchange, onselect }: Props = $props()

  const items = $derived(codeItems(graph, catalog, owner, locked))
  let collapsed = $state<Set<string>>(new Set())
  let nested = $state<Record<string, string>>({})
  let scroller = $state<HTMLDivElement | null>(null)

  const signature = (item: CodeItem) => `(${item.inputs.map((p) => `${p.name}: ${p.type}`).join(', ')}) → (${item.outputs.map((p) => `${p.name}: ${p.type}`).join(', ')})`

  $effect(() => {
    for (const item of items) {
      if (item.badge !== 'action' || nested[item.id] !== undefined) continue
      const entry = catalog?.actions.find((a) => a.id === item.node.type.slice(ACTION.length))
      if (!entry) continue
      nested = { ...nested, [item.id]: '' }
      api
        .postActionRender({ name: entry.name, description: '', icon: entry.icon, max_sends_per_hour: null, inputs: entry.inputs, graph: entry.graph })
        .then((code) => (nested = { ...nested, [item.id]: code }))
        .catch((e) => (nested = { ...nested, [item.id]: String(e) }))
    }
  })

  onMount(async () => {
    const focus = codeFocus(items, selected)
    if (!focus) return
    collapsed = new Set([...collapsed].filter((id) => id !== focus))
    await tick()
    scroller?.querySelector(`[data-code="${CSS.escape(focus)}"]`)?.scrollIntoView({ block: 'start' })
  })

  function valueOf(item: CodeItem, section: CodeSection): string {
    if (section.kind === 'action') return nested[item.id] ?? ''
    if (section.kind === 'template') return section.value.trim() || defaultTemplate(presenterKind(item.node) ?? 'mail')
    return section.value
  }

  function edit(item: CodeItem, section: CodeSection, text: string) {
    const node = graph.nodes.find((n) => n.id === item.id)
    if (node) onchange(withCode(node, section, text, catalog))
  }
</script>

<div class="flex h-full min-h-0 flex-col">
  <div class="flex items-center gap-2 border-b border-line px-4 py-1.5">
    <span class="text-[12px] text-ink-3">{t('actions.code.count', { count: items.length })}</span>
    <span class="ml-auto flex items-center gap-1">
      <Button size="sm" testid="code-expand-all" icon="lucide:chevrons-up-down" disabled={!collapsed.size} onclick={() => (collapsed = new Set())}>{t('actions.code.expandAll')}</Button>
      <Button size="sm" testid="code-collapse-all" icon="lucide:chevrons-down-up" disabled={collapsed.size === items.length} onclick={() => (collapsed = new Set(items.map((i) => i.id)))}>{t('actions.code.collapseAll')}</Button>
    </span>
  </div>
  <div bind:this={scroller} class="code-stack no-scrollbar min-h-0 flex-1 overflow-y-auto">
    {#each items as item (item.id)}
      {@const open = !collapsed.has(item.id)}
      {@const problems = problemsFor(diagnostics, item.id, 'node').filter((d) => d.line)}
      <section data-testid="code-item" data-open={open} data-code={item.id} class="flex flex-col border-b border-line {open ? 'h-full' : ''}" aria-label={item.title}>
        <header class="flex min-w-0 shrink-0 items-center gap-2 px-3 py-1.5">
          <IconButton size="sm" testid="code-toggle" icon={open ? 'lucide:chevron-down' : 'lucide:chevron-right'} label={open ? t('actions.code.collapse', { name: item.title }) : t('actions.code.expand', { name: item.title })} onclick={() => (collapsed = foldToggle(collapsed, item.id))} />
          <span class="flex size-6 shrink-0 items-center justify-center rounded-[0.375rem] bg-paper-3 text-[13px] text-ink-2"><Icon icon={item.icon} /></span>
          <button type="button" class="min-w-0 shrink overflow-hidden text-ellipsis whitespace-nowrap text-left text-[13px] font-semibold text-ink hover:underline" title={t('actions.code.show', { name: item.title })} onclick={() => onselect(item.id)}>{item.title}</button>
          <span class="shrink-0 font-mono text-[10.5px] uppercase tracking-wide text-ink-3">{item.kind}</span>
          <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11px] text-ink-3" title={signature(item)}>{signature(item)}</span>
          {#if problems.length}<span class="flex shrink-0 items-center gap-1 text-[11.5px] text-error-600"><Icon icon="lucide:circle-alert" />{problems.length}</span>{/if}
          {#if item.badge || locked}
            <span class="flex shrink-0 items-center gap-1 rounded-full bg-paper-3 px-2 text-[11px] leading-5 text-ink-3"><Icon icon="lucide:lock" class="text-[11px]" />{item.badge === 'action' ? t('actions.code.nestedBadge') : item.badge === 'builtin' ? t('actions.code.builtinBadge') : t('actions.code.readOnly')}</span>
          {/if}
        </header>
        {#if open}
          {#each item.sections as section (section.key)}
            <div data-testid="code-section" data-kind={section.kind} class="flex min-h-0 flex-1 flex-col border-t border-line">
              {#if item.sections.length > 1 || section.kind !== 'body'}
                <span class="flex shrink-0 items-center gap-1.5 px-4 pt-1.5 text-[11.5px] font-medium uppercase tracking-wide text-ink-3">{section.title}{#if section.readonly && !item.badge && !locked}<Icon icon="lucide:lock" class="text-[11px]" />{/if}</span>
              {/if}
              <div class="min-h-0 flex-1 px-2">
                <CodeEditor label={t('actions.code.editor', { name: item.title, part: section.title })} language={section.language} value={valueOf(item, section)} readonly={section.readonly} functions={catalog?.functions ?? []} inputs={item.node.type === NODE_TRIGGER ? item.outputs : item.inputs} diagnostics={section.kind === 'body' ? problems : []} onchange={(text) => edit(item, section, text)} />
              </div>
            </div>
          {/each}
        {/if}
      </section>
    {:else}
      <p class="px-4 py-6 text-[13px] text-ink-3">{t('actions.code.empty')}</p>
    {/each}
  </div>
</div>

<style>
  .code-stack {
    scroll-snap-type: y proximity;
  }
  .code-stack > section {
    scroll-snap-align: start;
  }
</style>
