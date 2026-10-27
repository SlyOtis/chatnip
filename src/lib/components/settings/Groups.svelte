<script lang="ts">
  import Icon from '@iconify/svelte'
  import { api, conversationsStore, groupsStore, tagRecordsStore, loadGroups } from '$lib/tauri'
  import type { Group } from '$lib/tauri/types'
  import { buildTree, canNest, describeQuery, flattenTree, groupUnread, type GroupNode } from '$lib/tauri/groups'
  import { Button, IconButton, Section } from '$lib/components/ui'
  import GroupEditor from './GroupEditor.svelte'
  import { t } from '$lib/i18n.svelte'

  let editing = $state<{ group: Group | null; parentId: string | null } | null>(null)
  let failure = $state<string | null>(null)
  let dragging = $state<string | null>(null)
  let dropTarget = $state<{ id: string | null; where: 'before' | 'inside' } | null>(null)

  const nodes = $derived(flattenTree(buildTree($groupsStore)))
  const visible = $derived(nodes.filter((node) => !nodes.some((other) => other.group.is_collapsed && other.group.id !== node.group.id && flattenTree(other.children).some((child) => child.group.id === node.group.id))))

  async function run(action: () => Promise<unknown>) {
    try {
      await action()
      await loadGroups()
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  function siblings(group: Group): Group[] {
    return $groupsStore.filter((g) => g.parent_id === group.parent_id).sort((a, b) => a.position - b.position)
  }

  function move(group: Group, delta: number) {
    const list = siblings(group)
    const index = list.findIndex((g) => g.id === group.id)
    const target = index + delta
    if (target < 0 || target >= list.length) return
    const order = [...list]
    const [item] = order.splice(index, 1)
    order.splice(target, 0, item)
    run(() => Promise.all(order.map((g, position) => api.putGroup({ group_id: g.id, position }))))
  }

  function indent(group: Group) {
    const list = siblings(group)
    const index = list.findIndex((g) => g.id === group.id)
    const newParent = list[index - 1]
    if (!newParent || !canNest($groupsStore, group.id, newParent.id)) return
    run(() => api.putGroup({ group_id: group.id, parent_id: newParent.id, position: $groupsStore.filter((g) => g.parent_id === newParent.id).length }))
  }

  function outdent(group: Group) {
    if (!group.parent_id) return
    const parent = $groupsStore.find((g) => g.id === group.parent_id)
    run(() => api.putGroup({ group_id: group.id, parent_id: parent?.parent_id ?? null, position: (parent?.position ?? 0) + 1 }))
  }

  function collapse(group: Group) {
    run(() => api.putGroup({ group_id: group.id, is_collapsed: !group.is_collapsed }))
  }

  function remove(group: Group) {
    const children = $groupsStore.filter((g) => g.parent_id === group.id).length
    if (!confirm(children ? t('settings.groups.removeNested', { name: group.name, count: children }) : t('settings.groups.remove', { name: group.name }))) return
    run(() => api.deleteGroup(group.id))
  }

  function drop(target: GroupNode | null, where: 'before' | 'inside') {
    const id = dragging
    dragging = null
    dropTarget = null
    if (!id) return
    const group = $groupsStore.find((g) => g.id === id)
    if (!group) return
    if (!target) {
      run(() => api.putGroup({ group_id: id, parent_id: null, position: $groupsStore.filter((g) => !g.parent_id).length }))
      return
    }
    if (target.group.id === id) return
    if (where === 'inside') {
      if (!canNest($groupsStore, id, target.group.id)) return
      run(() => api.putGroup({ group_id: id, parent_id: target.group.id, position: $groupsStore.filter((g) => g.parent_id === target.group.id).length }))
      return
    }
    const parentId = target.group.parent_id
    if (!canNest($groupsStore, id, parentId)) return
    const order = $groupsStore.filter((g) => g.parent_id === parentId && g.id !== id).sort((a, b) => a.position - b.position)
    const at = order.findIndex((g) => g.id === target.group.id)
    order.splice(at < 0 ? order.length : at, 0, { ...group, parent_id: parentId })
    run(() => Promise.all(order.map((g, position) => api.putGroup({ group_id: g.id, parent_id: parentId, position }))))
  }

  function kindIcon(group: Group): string {
    return group.icon ?? (group.kind === 'tag' ? 'lucide:tag' : group.kind === 'query' ? 'lucide:filter' : 'lucide:folder')
  }

  function detail(group: Group): string {
    if (group.kind === 'tag') return $tagRecordsStore.find((l) => l.id === group.tag_id)?.name ?? t('settings.groups.noTag')
    if (group.kind === 'query') return describeQuery(group.query, $tagRecordsStore)
    return t('settings.groups.conversations', { count: group.conversation_ids.length })
  }
</script>

<Section id="organise.groups" title={t('settings.inbox.groups')} description={t('settings.groups.description')} total={visible.length}>
  {#snippet actions()}
    <Button variant="primary" icon="lucide:plus" onclick={() => (editing = { group: null, parentId: null })}>{t('settings.group.new')}</Button>
  {/snippet}

  {#snippet children(shown)}
  {#if failure}
    <p class="mb-2 text-[13px] text-error-500">{failure}</p>
  {/if}
  <ul role="tree">
    {#each visible.slice(0, shown) as node (node.group.id)}
      {@const group = node.group}
      {@const unread = groupUnread($groupsStore, group.id, $conversationsStore.data)}
        <li
          role="treeitem"
          aria-level={node.depth + 1}
          aria-expanded={node.children.length ? !group.is_collapsed : undefined}
          aria-selected={editing?.group?.id === group.id}
          class="group/row grid h-10 grid-cols-[1rem_1rem_1rem_minmax(0,1fr)_auto] items-center gap-2 border-b border-line last:border-b-0 {dropTarget?.id === group.id ? (dropTarget.where === 'inside' ? 'bg-accent-soft' : 'shadow-[inset_0_2px_0_var(--accent)]') : ''}"
          style:padding-left="{node.depth}rem"
          draggable="true"
          ondragstart={(e) => {
            dragging = group.id
            e.dataTransfer?.setData('text/plain', group.id)
          }}
          ondragover={(e) => {
            e.preventDefault()
            const rect = e.currentTarget.getBoundingClientRect()
            dropTarget = { id: group.id, where: e.clientY - rect.top > rect.height * 0.5 ? 'inside' : 'before' }
          }}
          ondragleave={() => (dropTarget = null)}
          ondrop={(e) => {
            e.preventDefault()
            drop(node, dropTarget?.where ?? 'before')
          }}
          ondragend={() => {
            dragging = null
            dropTarget = null
          }}
        >
          <Icon icon="lucide:grip-vertical" class="cursor-grab text-[14px] text-ink-3 opacity-0 group-hover/row:opacity-100" />
          <button class="flex size-4 items-center justify-center text-ink-3 hover:text-ink {node.children.length ? '' : 'invisible'}" onclick={() => collapse(group)} aria-label={group.is_collapsed ? t('settings.groups.expand') : t('settings.groups.collapse')}>
            <Icon icon={group.is_collapsed ? 'lucide:chevron-right' : 'lucide:chevron-down'} class="text-[14px]" />
          </button>
          <Icon icon={kindIcon(group)} class="text-[14px]" style={group.color ? `color: ${group.color}` : 'color: var(--ink-3)'} />
          <div class="flex min-w-0 items-baseline gap-2">
            <span class="truncate text-[14px] font-medium text-ink">{group.name}</span>
            <span class="truncate text-[13px] text-ink-3">{detail(group)}</span>
            {#if unread > 0}
              <span class="shrink-0 font-mono text-[11px] text-ink-3">{unread}</span>
            {/if}
          </div>
          <div class="flex items-center gap-0.5 opacity-0 group-hover/row:opacity-100 focus-within:opacity-100">
            <IconButton size="sm" icon="lucide:plus" label={t('settings.groups.newInside')} onclick={() => (editing = { group: null, parentId: group.id })} />
            <IconButton size="sm" icon="lucide:chevron-left" label={t('settings.groups.moveOut')} disabled={!group.parent_id} onclick={() => outdent(group)} />
            <IconButton size="sm" icon="lucide:chevron-right" label={t('settings.groups.moveInto')} onclick={() => indent(group)} />
            <IconButton size="sm" icon="lucide:chevron-up" label={t('settings.moveUp')} onclick={() => move(group, -1)} />
            <IconButton size="sm" icon="lucide:chevron-down" label={t('settings.moveDown')} onclick={() => move(group, 1)} />
            <IconButton size="sm" icon="lucide:pencil" label={t('common.edit')} onclick={() => (editing = { group, parentId: group.parent_id })} />
            <IconButton size="sm" icon="lucide:x" label={t('common.remove')} tone="danger" onclick={() => remove(group)} />
          </div>
        </li>
    {:else}
      <li class="py-4 text-[13px] text-ink-3">{t('settings.groups.empty')}</li>
    {/each}
    {#if nodes.length > 0}
      <li
        class="flex h-8 items-center text-[12px] text-ink-3 {dropTarget?.id === null && dragging ? 'text-accent' : ''}"
        ondragover={(e) => {
          e.preventDefault()
          dropTarget = { id: null, where: 'before' }
        }}
        ondragleave={() => (dropTarget = null)}
        ondrop={(e) => {
          e.preventDefault()
          drop(null, 'before')
        }}
      >
        {dragging ? t('settings.groups.dropTop') : t('settings.groups.dragHint')}
      </li>
    {/if}
  </ul>
  {/snippet}
</Section>

{#if editing}
  {@const context = editing}
  {#key context}
    <GroupEditor subject={{ scope: 'conversation', group: context.group }} parentId={context.parentId} onsaved={() => (editing = null)} oncancel={() => (editing = null)} onnewgroup={(parentId) => (editing = { group: null, parentId })} ondeleted={() => (editing = null)} />
  {/key}
{/if}
