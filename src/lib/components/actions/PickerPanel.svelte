<script lang="ts">
  import { portal } from '$lib/popover'
  import Icon from '@iconify/svelte'
  import type { ActionCatalogEntry, ActionGroup } from '$lib/tauri/types'
  import { groupName, nestable, tree } from '$lib/actions'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    actions: ActionCatalogEntry[]
    groups: ActionGroup[]
    exclude?: string | null
    ondrop: (item: ActionCatalogEntry, clientX: number, clientY: number) => void
    onadd: (item: ActionCatalogEntry) => void
  }

  let { actions, groups, exclude = null, ondrop, onadd }: Props = $props()

  const STORE = 'chatnip.actions.picker.collapsed'
  let query = $state('')
  let collapsed = $state(new Set<string>(JSON.parse(localStorage.getItem(STORE) ?? '[]')))
  let ghost = $state<{ item: ActionCatalogEntry; x: number; y: number } | null>(null)
  let start: { x: number; y: number } | null = null

  const items = $derived(tree(groups, actions.filter((a) => a.id !== exclude && nestable(a)), collapsed, query))

  function toggle(id: string) {
    const next = new Set(collapsed)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    collapsed = next
    localStorage.setItem(STORE, JSON.stringify([...next]))
  }

  function down(event: PointerEvent, item: ActionCatalogEntry) {
    if (event.button !== 0) return
    start = { x: event.clientX, y: event.clientY }
    const move = (e: PointerEvent) => {
      if (!start) return
      if (!ghost && Math.hypot(e.clientX - start.x, e.clientY - start.y) < 6) return
      ghost = { item, x: e.clientX, y: e.clientY }
    }
    const up = (e: PointerEvent) => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      const dragged = ghost
      ghost = null
      start = null
      if (!dragged) return
      if (document.elementFromPoint(e.clientX, e.clientY)?.closest('.action-graph')) ondrop(item, e.clientX, e.clientY)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
  }
</script>

<div class="flex min-h-0 flex-1 flex-col gap-2" style:--icon-cell="1.75rem">
  <input
    class="h-8 w-full border-b border-line bg-transparent text-[13px] text-ink outline-none placeholder:text-ink-3 focus:border-ink-3"
    data-testid="picker-search"
    placeholder={t('actions.picker.find')}
    aria-label={t('actions.picker.findLabel')}
    bind:value={query}
  />
  <ul class="no-scrollbar flex min-h-0 flex-1 flex-col gap-px overflow-y-auto pb-4" role="tree" aria-label={t('actions.page.picker')}>
    {#each items as item (item.kind + item.id)}
      {#if item.kind === 'group' && item.group}
        {@const group = item.group}
        {@const open = query.trim() !== '' || !collapsed.has(group.id)}
        <li role="treeitem" aria-expanded={open} aria-selected="false" style:padding-left="{item.depth * 0.75}rem">
          <button type="button" data-testid="picker-group" data-id={group.id} class="grid h-7 w-full grid-cols-[var(--icon-cell)_minmax(0,1fr)_auto] items-center rounded-[0.5rem] pr-1 text-left text-[12.5px] font-medium text-ink-2 hover:bg-paper-3" title={group.description} onclick={() => toggle(group.id)}>
            <span class="flex items-center justify-center text-[14px] text-ink-3"><Icon icon={group.icon ?? 'lucide:folder'} /></span>
            <span class="overflow-hidden text-ellipsis whitespace-nowrap">{groupName(group)}</span>
            <Icon icon={open ? 'lucide:chevron-down' : 'lucide:chevron-right'} class="text-[13px] text-ink-3" />
          </button>
        </li>
      {:else if item.action}
        {@const action = item.action}
        <li role="treeitem" aria-selected="false" style:padding-left="{item.depth * 0.75}rem">
          <button
            type="button"
            data-testid="picker-item"
            data-id={action.id}
            data-key={action.key}
            class="group/pick grid w-full cursor-grab grid-cols-[var(--icon-cell)_minmax(0,1fr)] items-start gap-x-1 rounded-[0.5rem] py-1 pr-1.5 text-left select-none hover:bg-paper-3 focus-visible:bg-paper-3 active:cursor-grabbing"
            title={t('actions.picker.drag')}
            onpointerdown={(e) => down(e, action)}
            onclick={() => !ghost && onadd(action)}
          >
            <span class="mt-0.5 flex size-6 items-center justify-center rounded-[0.375rem] bg-paper-3 text-[13px] text-ink-2"><Icon icon={action.icon ?? 'lucide:zap'} /></span>
            <span class="flex min-w-0 flex-col">
              <span class="flex min-w-0 items-center gap-1.5 text-[13px] text-ink">
                <span class="overflow-hidden text-ellipsis whitespace-nowrap">{action.name}</span>
                {#if action.builtin}<Icon icon="lucide:lock" class="shrink-0 text-[11px] text-ink-3" aria-label={t('actions.picker.builtin')} />{/if}
              </span>
              {#if action.description}<span class="line-clamp-2 text-[11.5px] leading-4 text-ink-3">{action.description}</span>{/if}
            </span>
          </button>
        </li>
      {/if}
    {/each}
    {#if !items.length}
      <li class="px-2 py-6 text-[13px] text-ink-3">{t('actions.list.noMatch')}</li>
    {/if}
  </ul>
</div>

{#if ghost}
  <div use:portal class="pointer-events-none fixed z-[1100] flex items-center gap-2 rounded-[0.5rem] bg-paper px-2.5 py-1.5 text-[13px] text-ink shadow-[0_8px_24px_rgb(0_0_0/0.2)] hairline" style:left="{ghost.x + 12}px" style:top="{ghost.y + 8}px">
    <Icon icon={ghost.item.icon ?? 'lucide:zap'} class="text-[14px] text-ink-2" />
    {ghost.item.name}
  </div>
{/if}
