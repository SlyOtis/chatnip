<script lang="ts">
  import { portal } from '$lib/popover'
  import Icon from '@iconify/svelte'
  import { IconButton } from '$lib/components/ui'
  import type { Action, ActionCatalog, ActionGroup } from '$lib/tauri/types'
  import { conversationsStore } from '$lib/tauri'
  import { descendsFrom, groupName, landings, landingTitle, partition, tree, treeRowState, usageBadges, type TreeItem } from '$lib/actions'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    groups: ActionGroup[]
    actions: Action[]
    catalog?: ActionCatalog | null
    selected: string | null
    onselect: (id: string) => void
    oncreate: (groupId: string | null) => void
    ongroup: (group: ActionGroup | null) => void
    onmove: (item: TreeItem, groupId: string | null, before: string | null) => void
    ongraph?: (action: Action, clientX: number, clientY: number) => void
  }

  let { groups, actions, catalog = null, selected, onselect, oncreate, ongroup, onmove, ongraph }: Props = $props()

  const STORE = 'chatnip.actions.collapsed'
  const BUILTIN_STORE = 'chatnip.actions.builtin.open'
  const row = 'group/row relative grid h-8 w-full grid-cols-[var(--icon-cell)_minmax(0,1fr)_auto] items-center rounded-[0.5rem] pr-1.5 text-left text-[14px] select-none'
  let collapsed = $state(new Set<string>(JSON.parse(localStorage.getItem(STORE) ?? '[]')))
  let builtinOpen = $state(localStorage.getItem(BUILTIN_STORE) !== 'false')
  let query = $state('')
  let dragging = $state<{ item: TreeItem; x: number; y: number } | null>(null)
  let target = $state<{ id: string; kind: 'group' | 'action' } | null>(null)

  const parts = $derived(partition(groups, actions))
  const items = $derived(tree(parts.mine.groups, parts.mine.actions, collapsed, query))
  const shipped = $derived(tree(parts.builtin.groups, parts.builtin.actions, collapsed, query))

  function toggle(id: string) {
    const next = new Set(collapsed)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    collapsed = next
    localStorage.setItem(STORE, JSON.stringify([...next]))
  }

  function toggleBuiltin() {
    builtinOpen = !builtinOpen
    localStorage.setItem(BUILTIN_STORE, String(builtinOpen))
  }

  function down(event: PointerEvent, item: TreeItem) {
    if (event.button !== 0 || (event.target as HTMLElement).closest('button[data-control], input')) return
    const fixed = !!item.group?.builtin || !!item.action?.builtin
    const start = { x: event.clientX, y: event.clientY }
    const move = (e: PointerEvent) => {
      if (!dragging && Math.hypot(e.clientX - start.x, e.clientY - start.y) < 6) return
      dragging = { item, x: e.clientX, y: e.clientY }
      if (fixed) return
      const hit = document.elementFromPoint(e.clientX, e.clientY)?.closest('[data-tree-row]') as HTMLElement | null
      const id = hit?.dataset.treeRow ?? null
      const kind = hit?.dataset.treeKind as 'group' | 'action' | undefined
      const invalid = !id || id === item.id || hit?.dataset.treeBuiltin === 'true' || (item.kind === 'group' && kind === 'group' && descendsFrom(groups, id, item.id))
      target = invalid || !kind ? null : { id: id!, kind }
    }
    const up = (e: PointerEvent) => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      const drop = target
      const was = dragging
      dragging = null
      target = null
      if (!was) return
      if (item.action && ongraph && document.elementFromPoint(e.clientX, e.clientY)?.closest('.action-graph')) {
        ongraph(item.action, e.clientX, e.clientY)
        return
      }
      if (!drop || fixed) return
      if (drop.kind === 'group') onmove(item, drop.id, null)
      else {
        const action = actions.find((a) => a.id === drop.id)
        onmove(item, action?.group_id ?? null, drop.id)
      }
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
  }
</script>

{#snippet rows(list: TreeItem[])}
  {#each list as item (item.kind + item.id)}
    {@const state = treeRowState(item, selected)}
    {@const isTarget = target?.id === item.id}
    {#if item.kind === 'group' && item.group}
      {@const group = item.group}
      {@const open = query.trim() !== '' || !collapsed.has(group.id)}
      {@const name = groupName(group)}
      <li
        data-tree-row={group.id}
        data-tree-kind="group"
        data-tree-builtin={group.builtin}
        role="treeitem"
        aria-expanded={open}
        aria-selected="false"
        class="{row} text-ink-2 {isTarget ? 'bg-accent-soft' : 'hover:bg-paper-3'}"
        style:padding-left="{state.indent}rem"
        tabindex="0"
        onpointerdown={(e) => down(e, item)}
        oncontextmenu={(e) => {
          if (group.builtin) return
          e.preventDefault()
          ongroup(group)
        }}
        onkeydown={(e) => {
          if (e.key !== 'Enter' || !(e.metaKey || e.ctrlKey) || group.builtin) return
          e.preventDefault()
          e.stopPropagation()
          ongroup(group)
        }}
      >
        <span class="flex items-center justify-center text-[15px] text-ink-3"><Icon icon={group.icon ?? 'lucide:folder'} /></span>
        <span class="overflow-hidden text-ellipsis whitespace-nowrap font-medium" title={group.description}>{name}</span>
        <span class="flex items-center">
          {#if !group.builtin}
            <button type="button" data-control data-testid="action-group-more" class="hidden size-6 items-center justify-center rounded-[0.375rem] text-ink-3 hover:text-ink group-hover/row:flex group-focus-within/row:flex" aria-label={t('app.rail.moreFor', { name })} onclick={() => ongroup(group)}><Icon icon="lucide:ellipsis" class="text-[14px]" /></button>
          {/if}
          <button type="button" data-control data-testid="action-group-toggle" class="flex size-6 items-center justify-center text-ink-3 hover:text-ink" aria-label={open ? t('actions.list.collapse', { name }) : t('actions.list.expand', { name })} onclick={() => toggle(group.id)}>
            <Icon icon={open ? 'lucide:chevron-down' : 'lucide:chevron-right'} class="text-[14px]" />
          </button>
        </span>
      </li>
    {:else if item.action}
      {@const action = item.action}
      {@const found = landings(action, actions, catalog)}
      {@const chatNames = action.chats.map((id) => $conversationsStore.data.find((c) => c.id === id)?.name ?? id)}
      <li
        data-tree-row={action.id}
        data-tree-kind="action"
        data-tree-builtin={action.builtin}
        role="treeitem"
        aria-selected={state.selected}
        class="{row} {action.builtin ? 'cursor-grab active:cursor-grabbing' : ''} {isTarget ? 'shadow-[inset_0_2px_0_var(--accent)]' : ''} {state.selected ? 'bg-accent-soft text-ink' : 'text-ink-2 hover:bg-paper-3'}"
        style:padding-left="{state.indent}rem"
        tabindex="0"
        onpointerdown={(e) => down(e, item)}
        onclick={() => onselect(action.id)}
        onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onselect(action.id) } }}
      >
        <span class="flex items-center justify-center text-[15px] {state.selected ? 'text-accent' : 'text-ink-3'}"><Icon icon={action.icon ?? 'lucide:zap'} /></span>
        <span class="overflow-hidden text-ellipsis whitespace-nowrap {state.struck ? 'text-ink-3' : ''} {state.struck && state.failed ? '[text-decoration-line:line-through_underline] decoration-error-500' : state.failed ? 'underline decoration-error-500' : state.struck ? 'line-through decoration-ink-3' : ''} {state.failed ? 'underline-offset-4' : ''}" title={state.failed ? t('actions.list.lastFailed') : state.struck ? t('actions.list.switchedOff', { name: action.name }) : action.builtin && action.builtin_kind !== 'action' ? t('actions.builtin.dragHint', { description: action.description }) : action.description}>{action.name}</span>
        <span class="flex items-center gap-1">
          {#each usageBadges(action.usage) as badge (badge.key)}<span data-testid="action-row-usage" data-key={badge.key} class="flex text-[12px] text-ink-3" title="{badge.label}: {badge.hint}" aria-label={badge.label}><Icon icon={badge.icon} /></span>{/each}
          {#if found.length}
            <span class="flex items-center gap-0.5 text-[12px] {action.chats.length ? 'text-accent' : 'text-ink-3'}" title={landingTitle(found, chatNames)}>
              {#each found.slice(0, 2) as landing (landing.kind + landing.icon)}<Icon icon={landing.icon} />{/each}
              {#if found.length > 2}<span class="font-mono text-[10.5px]">+{found.length - 2}</span>{/if}
            </span>
          {/if}
        </span>
      </li>
    {/if}
  {/each}
{/snippet}

<nav class="flex h-full min-h-0 flex-col gap-2 py-3" style:--icon-cell="2rem" aria-label={t('actions.page.title')}>
  <div class="flex items-center gap-1 px-2">
    <span class="flex w-[var(--icon-cell)] shrink-0 items-center justify-center text-ink-3"><Icon icon="lucide:search" class="text-[15px]" /></span>
    <input data-testid="actions-search" class="h-8 min-w-0 flex-1 bg-transparent text-[14px] text-ink outline-none placeholder:text-ink-3" placeholder={t('actions.list.find')} aria-label={t('actions.list.find')} bind:value={query} />
    <IconButton size="sm" testid="actions-new-group" icon="lucide:folder-plus" label={t('actions.list.newGroup')} onclick={() => ongroup(null)} />
    <IconButton size="sm" testid="actions-new" icon="lucide:plus" label={t('actions.draft.name')} keys="N" onclick={() => oncreate(null)} />
  </div>

  <div class="no-scrollbar flex min-h-0 flex-1 flex-col overflow-y-auto">
  <ul class="flex flex-col gap-px px-2" role="tree" data-testid="actions-tree-mine" aria-label={t('actions.list.mine')}>
    {@render rows(items)}
    {#if !items.length}
      <li class="px-3 py-6 text-[13px] text-ink-3">{query ? t('actions.list.noMatch') : t('actions.list.empty')}</li>
    {/if}
  </ul>

  {#if parts.builtin.actions.length}
    <section class="mt-2 flex flex-col border-t border-line px-2 pt-2" aria-label={t('actions.builtin.title')}>
      <button type="button" data-testid="actions-builtin-toggle" class="{row} shrink-0 text-ink-3 hover:bg-paper-3 hover:text-ink-2" aria-expanded={builtinOpen} title={t('actions.builtin.hint')} onclick={toggleBuiltin}>
        <span class="flex items-center justify-center text-[15px]"><Icon icon="lucide:package" /></span>
        <span class="text-[12px] font-medium uppercase tracking-wide">{t('actions.builtin.title')}</span>
        <span class="flex items-center gap-1.5">
          <span class="font-mono text-[11px]">{parts.builtin.actions.length}</span>
          <Icon icon={builtinOpen ? 'lucide:chevron-down' : 'lucide:chevron-right'} class="text-[14px]" />
        </span>
      </button>
      {#if builtinOpen}
        <ul class="flex flex-col gap-px" role="tree" data-testid="actions-tree-builtin" aria-label={t('actions.builtin.title')}>
          {@render rows(shipped)}
          {#if !shipped.length}
            <li class="px-3 py-3 text-[13px] text-ink-3">{t('actions.list.noMatch')}</li>
          {/if}
        </ul>
      {/if}
    </section>
  {/if}
  </div>
</nav>

{#if dragging}
  <div use:portal class="pointer-events-none fixed z-[1100] flex items-center gap-2 rounded-[0.5rem] bg-paper px-2.5 py-1.5 text-[13px] text-ink shadow-[0_8px_24px_rgb(0_0_0/0.2)] hairline" style:left="{dragging.x + 12}px" style:top="{dragging.y + 8}px">
    <Icon icon={dragging.item.group?.icon ?? dragging.item.action?.icon ?? (dragging.item.kind === 'group' ? 'lucide:folder' : 'lucide:zap')} class="text-[14px] text-ink-2" />
    {dragging.item.group ? groupName(dragging.item.group) : dragging.item.action?.name}
  </div>
{/if}
