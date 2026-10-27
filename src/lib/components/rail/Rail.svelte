<script lang="ts">
  import Icon from '@iconify/svelte'
  import type { Snippet } from 'svelte'
  import { page } from '$app/state'
  import { goto } from '$app/navigation'
  import { profileSettingsStore, groupsStore, conversationsStore, casesStore, tagRecordsStore, api, loadTags } from '$lib/tauri'
  import type { Group, Inbox, Tag, SidebarEntry } from '$lib/tauri/types'
  import AccountMenu from './AccountMenu.svelte'
  import { Field, IconPicker, MenuItem, TagEditor, TagMark } from '$lib/components/ui'
  import GroupEditor from '$lib/components/settings/GroupEditor.svelte'
  import InboxEditor from '$lib/components/settings/InboxEditor.svelte'
  import { activeFolder, activeGroup, activeTag, inboxView, loadPipeline, moveToGroup, openFolder, openGroup, openInbox, openTag, reorder, toggleTag, viewCounts, type InboxView } from '$lib/inbox'
  import { buildTree, flattenTree, groupDialog, groupUnread } from '$lib/tauri/groups'
  import { activeCases, caseSignal } from '$lib/tauri/cases'
  import { activeInbox, addMembers, defaultInbox, inboxGroupIds, inboxMembership, inboxesStore, loadInboxes, userInboxes } from '$lib/inboxes'
  import { layoutSettings, putLayoutSetting, viewIconKey, viewLabelKey, sortKey, countKey, sidebarCollapsedKey, SORT_CHOICES, toggleFlag, flag, sidebarLayout, loadSidebarLayout, listenSidebarLayout, saveSidebarLayout, putSidebarEntry, type SidebarZone } from '$lib/layoutSettings'
  import { ensureIcons } from '$lib/icons'
  import { featureShown } from '$lib/settings'
  import { anchored, boxOf, pointBox, type Box } from '$lib/popover'
  import { t } from '$lib/i18n.svelte'

  type Target =
    | { kind: 'section'; key: string; label: string; fallback: string; sortable: boolean }
    | { kind: 'inbox'; inbox: Inbox }
    | { kind: 'group'; group: Group; inbox: Inbox | null }
    | { kind: 'tag'; label: Tag }

  type MenuTarget = Exclude<Target, { kind: 'group' } | { kind: 'inbox' }>

  let { ontoggle }: { ontoggle: () => void } = $props()

  const collapsed = $derived($profileSettingsStore?.navigation_collapsed ?? false)
  const path = $derived(page.url.pathname)
  const onInbox = $derived(path.startsWith('/main/conversation') || path.startsWith('/main/person') || path.startsWith('/main/merged') || path === '/main')
  const onSettings = $derived(path.startsWith('/main/profile'))
  const openCases = $derived(activeCases($casesStore))
  const groupNodes = $derived(flattenTree(buildTree($groupsStore)))
  const labels = $derived($tagRecordsStore.filter((label) => label.kind === 'tag'))
  const favouriteTags = $derived(labels.filter((label) => label.favourite).sort((a, b) => (a.favourite_position ?? 0) - (b.favourite_position ?? 0)))
  const otherTags = $derived(labels.filter((label) => !label.favourite).sort((a, b) => a.name.localeCompare(b.name)))
  const membership = $derived(inboxMembership($inboxesStore, $conversationsStore.data, $groupsStore))
  const linkedGroups = $derived(new Set($userInboxes.flatMap((inbox) => [...inboxGroupIds(inbox, $groupsStore)])))
  const looseGroups = $derived(groupNodes.filter((node) => !linkedGroups.has(node.group.id)))
  const plainView = $derived(onInbox && !$activeGroup && !$activeTag)

  const views: { key: InboxView; label: string; icon: string; keys: string }[] = $derived([
    { key: 'done', label: t('app.rail.done'), icon: 'lucide:circle-check', keys: 'G E' },
    { key: 'snoozed', label: t('app.rail.snoozed'), icon: 'lucide:clock', keys: 'G H' },
    { key: 'notifications', label: t('app.rail.notifications'), icon: 'lucide:bell', keys: 'G N' },
    { key: 'deleted', label: t('app.rail.deleted'), icon: 'lucide:trash-2', keys: 'G D' },
    { key: 'pipeline', label: t('app.rail.pipeline'), icon: 'lucide:git-pull-request-arrow', keys: 'G P' }
  ])

  const pages: { key: string; label: string; icon: string; href: string; title: string }[] = $derived([
    { key: 'activity', label: t('inbox.view.activity'), icon: 'lucide:at-sign', href: '/main/activity', title: t('inbox.view.activity') + ' · ⌘⇧A' },
    { key: 'saved', label: t('inbox.view.saved'), icon: 'lucide:bookmark', href: '/main/saved', title: t('inbox.view.saved') + ' · G S' },
    { key: 'reminders', label: t('inbox.view.reminders'), icon: 'lucide:alarm-clock', href: '/main/reminders', title: t('inbox.view.reminders') + ' · G M' },
    { key: 'actions', label: t('app.rail.actions'), icon: 'lucide:workflow', href: '/main/actions', title: t('app.rail.actions') },
    { key: 'content', label: t('content.title'), icon: 'lucide:library', href: '/main/content', title: t('content.title') },
    { key: 'calendar', label: t('app.rail.calendar'), icon: 'lucide:calendar', href: '/main/calendar', title: t('app.rail.calendar') + ' · ⌘⇧C' },
    { key: 'statistics', label: t('app.rail.statistics'), icon: 'lucide:chart-no-axes-column', href: '/main/stats', title: t('app.rail.statistics') },
    { key: 'cases', label: t('app.rail.cases'), icon: 'lucide:briefcase', href: '/main/cases', title: t('app.rail.cases') },
    { key: 'history', label: t('app.rail.history'), icon: 'lucide:history', href: '/main/history', title: t('app.rail.history') }
  ].filter((item) => featureShown(item.key)))

  const BUILT_IN = ['inbox', 'tags', 'done', 'snoozed', 'notifications', 'deleted', 'pipeline']

  let menu = $state<{ x: number; y: number; target: MenuTarget } | null>(null)
  let picking = $state<{ target: MenuTarget; at: Box } | null>(null)
  let renaming = $state(false)
  let renameDraft = $state('')
  let editingInbox = $state<{ inbox: Inbox | null } | null>(null)
  let editingTag = $state<{ tag: Tag | null } | null>(null)
  let dragging = $state<{ list: 'tags'; index: number } | null>(null)
  let dropAt = $state<string | null>(null)

  $effect(() => {
    loadInboxes().catch(() => {})
    loadSidebarLayout().catch(() => {})
    const off = listenSidebarLayout()
    return () => {
      off.then((stop) => stop()).catch(() => {})
    }
  })

  $effect(() => {
    const load = () => loadPipeline().catch(() => {})
    load()
    const timer = setInterval(load, 60000)
    return () => clearInterval(timer)
  })

  $effect(() => {
    ensureIcons([...Object.values($layoutSettings), ...$tagRecordsStore.map((label) => label.icon), ...$groupsStore.map((group) => group.icon), ...$inboxesStore.map((inbox) => inbox.icon)])
  })

  const unreadById = $derived(new Map($conversationsStore.data.map((c) => [c.id, !c.deleted_at && !c.is_archived && c.unread_count > 0])))

  function unreadIn(ids: Set<string> | undefined): number {
    if (!ids) return 0
    let count = 0
    for (const id of ids) if (unreadById.get(id)) count += 1
    return count
  }

  const tagCounts = $derived.by(() => {
    const counts = new Map<string, number>()
    for (const conversation of $conversationsStore.data) {
      if (conversation.deleted_at || conversation.is_archived || conversation.unread_count === 0) continue
      for (const label of conversation.tags) counts.set(label.tag_id, (counts.get(label.tag_id) ?? 0) + 1)
    }
    return counts
  })

  function sectionIcon(key: string, fallback: string): string {
    return $layoutSettings[viewIconKey(key)] || fallback
  }

  function showCount(key: string): boolean {
    return $layoutSettings[countKey(key)] !== 'off'
  }

  function isOpen(key: string): boolean {
    return !flag($layoutSettings, sidebarCollapsedKey(key))
  }

  function groupFallback(group: Group): string {
    return group.kind === 'tag' ? 'lucide:tag' : group.kind === 'query' ? 'lucide:filter' : 'lucide:folder'
  }

  function targetKey(target: Target): string {
    if (target.kind === 'section') return target.key
    if (target.kind === 'inbox') return target.inbox.id
    if (target.kind === 'group') return target.group.id
    return 'tag-' + target.label.id
  }

  function targetName(target: Target): string {
    if (target.kind === 'section') return named(target.key, target.label)
    if (target.kind === 'inbox') return target.inbox.name
    if (target.kind === 'group') return target.group.name
    return target.label.name
  }

  function named(key: string, label: string): string {
    return $layoutSettings[viewLabelKey(key)]?.trim() || label
  }

  async function rename(target: MenuTarget, name: string) {
    renaming = false
    menu = null
    if (target.kind !== 'section') return
    const next = name.trim()
    await putLayoutSetting(viewLabelKey(target.key), next && next !== target.label ? next : null)
  }

  function iconOf(target: MenuTarget): string | null {
    if (target.kind === 'section') return $layoutSettings[viewIconKey(target.key)] || null
    return target.label.icon || null
  }

  function fallbackOf(target: MenuTarget): string {
    if (target.kind === 'section') return target.fallback
    return 'lucide:tag'
  }

  async function refreshGroups() {
    groupsStore.set(await api.getGroups())
  }

  async function saveIcon(target: MenuTarget, icon: string | null) {
    if (target.kind === 'section') await putLayoutSetting(viewIconKey(target.key), icon)
  }

  async function toggleFavourite(label: Tag) {
    await api.putTag({ tag_id: label.id, favourite: !label.favourite })
    await loadTags()
  }

  async function dropReorder(list: 'tags', target: number) {
    if (!dragging || dragging.list !== list) return
    const from = dragging.index
    dragging = null
    dropAt = null
    await api.putTagFavourites(reorder(favouriteTags, from, target).map((label) => label.id))
    await loadTags()
  }

  function openMenu(e: MouseEvent, target: Target) {
    e.preventDefault()
    e.stopPropagation()
    if (target.kind === 'group') {
      menu = null
      groupDialog.set({ group: target.group, parentId: target.group.parent_id, inbox: target.inbox })
      return
    }
    if (target.kind === 'inbox') {
      menu = null
      editingInbox = { inbox: target.inbox }
      return
    }
    const box = boxOf(e.currentTarget)
    const atPointer = e.type === 'contextmenu' || !box
    menu = { x: atPointer ? e.clientX : box.left, y: atPointer ? e.clientY : box.top + box.height, target }
  }

  function editKeydown(e: KeyboardEvent, group: Group, inbox: Inbox | null) {
    if (e.key !== 'Enter' || !(e.metaKey || e.ctrlKey)) return
    e.preventDefault()
    e.stopPropagation()
    groupDialog.set({ group, parentId: group.parent_id, inbox })
  }

  function go() {
    if (!onInbox) goto('/main/conversation')
  }

  function pickInbox(inbox: Inbox) {
    openInbox(inbox.id)
    go()
  }

  function openView(view: InboxView) {
    inboxView.set(view)
    openGroup(null)
    activeTag.set(null)
    go()
  }

  function pickGroup(group: Group, inbox: Inbox | null) {
    if (inbox && $activeInbox !== inbox.id) openInbox(inbox.id)
    inboxView.set('inbox')
    openGroup($activeGroup === group.id ? null : group)
    go()
  }

  function pickTag(label: Tag) {
    openTag($activeTag === label.id ? null : label.id)
    go()
  }

  function collapseGroup(group: Group) {
    api.putGroup({ group_id: group.id, is_collapsed: !group.is_collapsed }).then(refreshGroups).catch(() => {})
  }

  function groupHidden(id: string, nodes = groupNodes): boolean {
    return nodes.some((node) => node.group.is_collapsed && node.group.id !== id && flattenTree(node.children).some((child) => child.group.id === id))
  }

  function droppedIds(e: DragEvent): string[] {
    const raw = e.dataTransfer?.getData('application/x-chatnip-conversations') ?? ''
    return raw ? raw.split(',').filter(Boolean) : []
  }

  function acceptsDrop(e: DragEvent): boolean {
    return !!e.dataTransfer?.types.includes('application/x-chatnip-conversations')
  }

  function nodesFor(inbox: Inbox) {
    const ids = inboxGroupIds(inbox, $groupsStore)
    const roots = new Set(inbox.members.filter((m) => m.kind === 'group').map((m) => m.member_id))
    const nodes = groupNodes.filter((node) => ids.has(node.group.id))
    const base = new Map(nodes.map((node) => [node.group.id, roots.has(node.group.id) ? node.depth : Infinity]))
    for (const node of nodes) {
      if (base.get(node.group.id) === Infinity) {
        const parent = nodes.find((candidate) => candidate.group.id === node.group.parent_id)
        base.set(node.group.id, (parent ? (base.get(parent.group.id) ?? 0) : 0) + 1)
      }
    }
    return nodes.map((node) => ({ ...node, depth: roots.has(node.group.id) ? 0 : (base.get(node.group.id) ?? node.depth) }))
  }

  const row = 'rail-row group/row relative grid h-8 w-full shrink-0 grid-cols-[var(--icon-cell)_minmax(0,1fr)_auto] items-center rounded-[0.5rem] pr-1.5 text-left text-[14px]'

  function tone(active: boolean): string {
    return active ? 'bg-accent-soft text-ink' : 'text-ink-2 hover:bg-[rgb(var(--frost-rgb)/0.55)] hover:text-ink'
  }

  function glyphTone(active: boolean): string {
    return active ? 'text-accent' : 'text-ink-3'
  }
</script>

<svelte:window onclick={() => (menu = null)} onkeydown={(e) => e.key === 'Escape' && (menu = null)} />

{#snippet glyph(icon: string, active: boolean)}
  <span class="flex items-center justify-center text-[14px] {glyphTone(active)}"><Icon {icon} /></span>
{/snippet}

{#snippet tail(value: number, active: boolean, target: Target | null, chevron: { open: boolean; toggle: () => void } | null)}
  <span class="flex items-center gap-0.5 pl-1.5">
    {#if value > 0 && (!target || showCount(targetKey(target)))}
      <span class="font-mono text-[11px] {active ? 'text-ink-2' : 'text-ink-3'} {target ? 'group-hover/row:hidden' : ''}">{value}</span>
    {/if}
    {#if target && !collapsed}
      <button type="button" class="hidden size-5 items-center justify-center rounded-[0.3rem] text-ink-3 hover:bg-paper-3 hover:text-ink group-hover/row:flex" data-testid="rail-more" onclick={(e) => openMenu(e, target)} aria-label={t('app.rail.moreFor', { name: targetName(target) })}>
        <Icon icon="lucide:ellipsis" class="text-[13px]" />
      </button>
    {/if}
    {#if chevron && !collapsed}
      <button type="button" class="flex size-5 items-center justify-center rounded-[0.3rem] text-ink-3 hover:text-ink" data-testid="rail-chevron" onclick={(e) => { e.stopPropagation(); chevron.toggle() }} aria-label={chevron.open ? t('app.rail.collapse') : t('app.rail.expand')} aria-expanded={chevron.open}>
        <Icon icon={chevron.open ? 'lucide:chevron-down' : 'lucide:chevron-right'} class="text-[12px]" />
      </button>
    {/if}
  </span>
{/snippet}

{#snippet groupTree(nodes: ReturnType<typeof nodesFor>, inbox: Inbox | null)}
  {#each nodes as node (node.group.id)}
    {@const group = node.group}
    {@const active = onInbox && $activeGroup === group.id}
    {@const unread = groupUnread($groupsStore, group.id, $conversationsStore.data)}
    {@const target = { kind: 'group' as const, group, inbox }}
    {#if !groupHidden(group.id)}
      <div
        class="{row} {tone(active)} {dropAt === group.id ? 'bg-paper-3' : ''}"
        role="treeitem"
        data-testid="rail-group"
        data-id={group.id}
        tabindex="-1"
        aria-selected={active}
        oncontextmenu={(e) => openMenu(e, target)}
        onkeydown={(e) => editKeydown(e, group, inbox)}
        ondragover={(e) => {
          if (group.kind !== 'manual' || !acceptsDrop(e)) return
          e.preventDefault()
          dropAt = group.id
        }}
        ondragleave={() => (dropAt = null)}
        ondrop={(e) => {
          e.preventDefault()
          dropAt = null
          const ids = droppedIds(e)
          if (ids.length && group.kind === 'manual') moveToGroup(ids, group)
        }}
      >
        <span></span>
        <button type="button" class="flex min-w-0 items-center gap-2 text-left" style:padding-left="{node.depth * 1.25}rem" onclick={() => pickGroup(group, inbox)} title={group.name} aria-current={active ? 'page' : undefined}>
          <Icon icon={group.icon || groupFallback(group)} class="shrink-0 text-[13px] {glyphTone(active)}" />
          <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{group.name}</span>
        </button>
        {@render tail(unread, active, target, node.children.length ? { open: !group.is_collapsed, toggle: () => collapseGroup(group) } : null)}
      </div>
    {/if}
  {/each}
{/snippet}

{#snippet flat(key: string, label: string, fallback: string, value: number, active: boolean, onopen: () => void, title: string, children: Snippet | null)}
  {@const target = { kind: 'section' as const, key, label, fallback, sortable: BUILT_IN.includes(key) }}
  {@const open = isOpen(key)}
  <div class="{row} {tone(active)}" data-testid="rail-entry" data-key={key} role="treeitem" aria-selected={active} aria-expanded={children ? open : undefined} tabindex="-1" oncontextmenu={(e) => openMenu(e, target)}>
    <button type="button" class="flex h-full items-center justify-center" onclick={onopen} aria-label={named(key, label)}>{@render glyph(sectionIcon(key, fallback), active)}</button>
    <button type="button" class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-left" onclick={onopen} {title}>{named(key, label)}</button>
    {@render tail(value, active, target, children ? { open, toggle: () => toggleFlag(sidebarCollapsedKey(key)) } : null)}
  </div>
  {#if children && open && !collapsed}
    {@render children()}
  {/if}
{/snippet}

{#snippet tagRow(label: Tag, index: number | null)}
  {@const active = onInbox && $activeTag === label.id}
  {@const target = { kind: 'tag' as const, label }}
  <div
    class="{row} {tone(active)} {dropAt === 'tag-' + label.id ? 'bg-paper-3' : ''}"
    role="treeitem"
    data-testid="rail-tag"
    data-id={label.id}
    tabindex="-1"
    aria-selected={active}
    draggable={index !== null}
    oncontextmenu={(e) => openMenu(e, target)}
    ondragstart={(e) => {
      if (index === null) return
      dragging = { list: 'tags', index }
      e.dataTransfer?.setData('text/plain', label.id)
    }}
    ondragover={(e) => {
      e.preventDefault()
      dropAt = 'tag-' + label.id
    }}
    ondragleave={() => (dropAt = null)}
    ondrop={(e) => {
      e.preventDefault()
      dropAt = null
      const ids = droppedIds(e)
      if (ids.length) toggleTag(ids, label.id, $conversationsStore.data)
      else if (index !== null) dropReorder('tags', index)
    }}
    ondragend={() => {
      dragging = null
      dropAt = null
    }}
  >
    <span></span>
    <button type="button" class="flex min-w-0 items-center gap-2 text-left" onclick={() => pickTag(label)} title={label.name} aria-current={active ? 'page' : undefined}>
      <span class="flex w-4 shrink-0 justify-center"><TagMark tag={label} /></span>
      <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{label.name}</span>
    </button>
    {@render tail(tagCounts.get(label.id) ?? 0, active, target, null)}
  </div>
{/snippet}

{#snippet tagChildren()}
  {#each favouriteTags as label, index (label.id)}
    {@render tagRow(label, index)}
  {/each}
  {#if favouriteTags.length && otherTags.length}
    <span class="my-1 rail-divider"></span>
  {/if}
  {#each otherTags as label (label.id)}
    {@render tagRow(label, null)}
  {/each}
  {#if !labels.length}
    <p class="py-1 pl-[var(--icon-cell)] text-[12.5px] text-ink-3">{t('app.rail.tagsEmpty')}</p>
  {/if}
{/snippet}

{#snippet looseGroupChildren()}
  {@render groupTree(looseGroups, null)}
{/snippet}

{#snippet rule()}
  <span class="my-2 rail-divider"></span>
{/snippet}

{#snippet userInbox(inbox: Inbox)}
  {@const active = plainView && !$activeFolder && $inboxView === 'inbox' && $activeInbox === inbox.id}
  {@const target = { kind: 'inbox' as const, inbox }}
  {@const nodes = nodesFor(inbox)}
  {@const open = isOpen(inbox.id)}
  <div
    class="{row} {tone(active)} {dropAt === 'inbox-' + inbox.id ? 'bg-paper-3' : ''}"
    role="treeitem"
    data-testid="rail-entry"
    data-key={'inbox:' + inbox.id}
    tabindex="-1"
    aria-selected={active}
    aria-expanded={nodes.length ? open : undefined}
    oncontextmenu={(e) => openMenu(e, target)}
    ondragover={(e) => {
      if (!acceptsDrop(e)) return
      e.preventDefault()
      dropAt = 'inbox-' + inbox.id
    }}
    ondragleave={() => (dropAt = null)}
    ondrop={(e) => {
      e.preventDefault()
      dropAt = null
      const ids = droppedIds(e)
      if (ids.length) addMembers(inbox, ids.map((id) => ({ kind: 'conversation', member_id: id }))).catch(() => {})
    }}
  >
    <button type="button" class="flex h-full items-center justify-center" onclick={() => pickInbox(inbox)} aria-label={inbox.name}>{@render glyph(inbox.icon || 'lucide:inbox', active)}</button>
    <button type="button" class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-left" onclick={() => pickInbox(inbox)} title={inbox.name}>{inbox.name}</button>
    {@render tail(unreadIn(membership.get(inbox.id)), active, target, nodes.length ? { open, toggle: () => toggleFlag(sidebarCollapsedKey(inbox.id)) } : null)}
  </div>
  {#if nodes.length && open && !collapsed}
    {@render groupTree(nodes, inbox)}
  {/if}
{/snippet}

{#snippet pageRow(item: { key: string; label: string; icon: string; href: string; title: string }, count: number)}
  {@const active = item.key === 'cases' ? path.startsWith('/main/case') : path.startsWith(item.href)}
  <button class="{row} {tone(active)}" data-testid="rail-entry" data-key={item.key} onclick={() => goto(item.href)} oncontextmenu={(e) => openMenu(e, { kind: 'section', key: item.key, label: item.label, fallback: item.icon, sortable: false })} title={item.title} aria-current={active ? 'page' : undefined}>
    {@render glyph(sectionIcon(item.key, item.icon), active)}
    <span class="overflow-hidden text-ellipsis whitespace-nowrap">{named(item.key, item.label)}</span>
    <span class="pl-2 font-mono text-[11px] text-ink-3">{count > 0 ? count : ''}</span>
  </button>
{/snippet}

{#snippet itemRow(key: string)}
  {#if key.startsWith('inbox:')}
    {@const inbox = $userInboxes.find((candidate) => 'inbox:' + candidate.id === key)}
    {#if inbox}{@render userInbox(inbox)}{/if}
  {:else if key === 'inbox'}
    {#if $defaultInbox}
      {@const fallback = $defaultInbox}
      {@render flat('inbox', fallback.name, 'lucide:inbox', unreadIn(membership.get(fallback.id)), plainView && !$activeFolder && $inboxView === 'inbox' && $activeInbox === fallback.id, () => pickInbox(fallback), t('app.rail.inboxTitle'), null)}
    {/if}
  {:else if key === 'tags'}
    {@render flat('tags', t('app.rail.tags'), 'lucide:tags', 0, false, () => toggleFlag(sidebarCollapsedKey('tags')), t('app.rail.tags'), tagChildren)}
  {:else if key === 'groups'}
    {#if looseGroups.length}
      {@render flat('groups', t('app.rail.groups'), 'lucide:folder-tree', 0, false, () => toggleFlag(sidebarCollapsedKey('groups')), t('app.rail.groups'), looseGroupChildren)}
    {/if}
  {:else if views.some((view) => view.key === key)}
    {@const view = views.find((candidate) => candidate.key === key)!}
    {@render flat(view.key, view.label, view.icon, view.key === 'deleted' ? 0 : ($viewCounts.get(view.key) ?? 0), plainView && $inboxView === view.key, () => openView(view.key), `${view.label} · ${view.keys}`, null)}
  {:else if pages.some((item) => item.key === key)}
    {@const item = pages.find((candidate) => candidate.key === key)!}
    {@render pageRow(item, key === 'cases' ? openCases.filter((c) => caseSignal(c)).length : 0)}
  {/if}
{/snippet}

{#snippet entries(list: SidebarEntry[], zone: SidebarZone, folder: number | undefined)}
  {#each list as entry, index (entry.kind === 'item' ? entry.key : entry.id)}
    {#if entry.kind === 'item'}
      {@render itemRow(entry.key)}
    {:else if entry.kind === 'divider'}
      {@render rule()}
    {:else if entry.kind === 'heading'}
      {#if collapsed}
        {@render rule()}
      {:else}
        <p data-testid="rail-heading" data-id={entry.id} class="overflow-hidden text-ellipsis whitespace-nowrap px-2 pb-1 pt-3 text-[12px] font-medium text-ink-3" role="presentation">{entry.name}</p>
      {/if}
    {:else if collapsed}
      {@render entries(entry.items, zone, index)}
    {:else}
      {@const slot = { zone, index, folder }}
      {@const folderOpen = plainView && $activeFolder === entry.id}
      <div class="{row} {tone(folderOpen)}" data-testid="rail-folder" data-id={entry.id} role="treeitem" aria-selected={folderOpen} aria-expanded={!entry.collapsed} tabindex="-1">
        <button type="button" class="flex h-full items-center justify-center" onclick={() => saveSidebarLayout(putSidebarEntry($sidebarLayout, slot, { collapsed: !entry.collapsed })).catch(() => {})} aria-label={entry.name}>{@render glyph(entry.icon || 'lucide:folder', false)}</button>
        <button type="button" data-testid="rail-folder-open" class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-left" onclick={() => { openFolder(entry.id); go() }} title={entry.name}>{entry.name}</button>
        {@render tail(0, false, null, { open: !entry.collapsed, toggle: () => saveSidebarLayout(putSidebarEntry($sidebarLayout, slot, { collapsed: !entry.collapsed })).catch(() => {}) })}
      </div>
      {#if !entry.collapsed}
        <div class="flex flex-col gap-px pl-3">{@render entries(entry.items, zone, index)}</div>
      {/if}
    {/if}
  {/each}
{/snippet}

<nav data-rail data-collapsed={collapsed || undefined} class="absolute inset-y-0 left-0 flex flex-col pb-2 pt-9" style:width="var(--sidebar-open)" style:padding-inline="var(--rail-pad)" aria-label={t('app.rail.primary')}>
  <div class="no-scrollbar flex min-h-0 flex-1 flex-col gap-px overflow-x-hidden overflow-y-auto" role="tree">
    {@render entries($sidebarLayout.top, 'top', undefined)}
  </div>

  <div class="mt-2 flex shrink-0 flex-col gap-px border-t border-line pt-2">
    {@render entries($sidebarLayout.footer, 'footer', undefined)}
    <button class="{row} {tone(onSettings)}" data-testid="rail-settings" onclick={() => goto('/main/profile')} title="{t('common.settings')} · ⌘," aria-current={onSettings ? 'page' : undefined}>
      {@render glyph('lucide:settings', onSettings)}
      <span>{t('common.settings')}</span>
      <span></span>
    </button>
    <button class="{row} {tone(false)}" data-testid="rail-collapse" onclick={ontoggle} aria-expanded={!collapsed} title="{collapsed ? t('app.rail.expandMenu') : t('app.rail.collapseMenu')} · ⌘⇧|">
      {@render glyph(collapsed ? 'lucide:chevrons-right' : 'lucide:chevrons-left', false)}
      <span class="text-[13px]">{t('app.rail.collapseMenu')}</span>
      <span></span>
    </button>
    <span class="my-1 rail-divider"></span>
    <AccountMenu class="{row} h-9 {tone(false)}" />
  </div>
</nav>

{#if menu}
  {@const target = menu.target}
  {@const key = targetKey(target)}
  {@const sortable = target.kind === 'section' && target.sortable}
  {@const at = pointBox(menu.x, menu.y)}
  <div class="menu w-[min(16rem,90vw)]" data-testid="rail-menu" use:anchored={{ at, side: 'below', align: 'start', onclose: () => { menu = null; renaming = false } }} role="dialog" aria-label={targetName(target)}>
    <div class="flex items-center gap-2.5 border-b border-line px-3 py-2">
      {#if target.kind === 'tag'}
        <TagMark tag={target.label} size="md" />
      {:else}
        <Icon icon={iconOf(target) ?? fallbackOf(target)} class="text-[15px] text-ink-2" />
      {/if}
      {#if renaming && target.kind === 'section'}
        <form class="min-w-0 flex-1" onsubmit={(e) => { e.preventDefault(); rename(target, renameDraft).catch(() => {}) }}>
          <Field size="sm" data-testid="rail-menu-rename-field" bind:value={renameDraft} aria-label={t('common.name')} placeholder={target.label} autofocus onkeydown={(e) => { if (e.key !== 'Escape') return; e.preventDefault(); e.stopPropagation(); renaming = false }} />
        </form>
      {:else}
        <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[14px] font-medium text-ink">{targetName(target)}</span>
      {/if}
    </div>
    <div class="p-1.5" role="menu">
    {#if target.kind === 'tag'}
      <MenuItem icon="lucide:pencil" testid="rail-menu-edit-tag" onclick={() => { editingTag = { tag: target.label }; menu = null }}>{t('app.tagEditor.edit')}</MenuItem>
    {:else}
      {#if target.kind === 'section'}
        <MenuItem icon="lucide:pencil" testid="rail-menu-rename" onclick={() => { renameDraft = targetName(target); renaming = true }}>{t('app.rail.rename')}</MenuItem>
      {/if}
      <MenuItem icon="lucide:shapes" testid="rail-menu-icon" onclick={() => { picking = { target, at }; menu = null }}>{t('app.rail.changeIcon')}</MenuItem>
    {/if}
    {#if target.kind === 'section' && target.key === 'tags'}
      <MenuItem icon="lucide:plus" testid="rail-menu-new-tag" onclick={() => { editingTag = { tag: null }; menu = null }}>{t('app.tagEditor.new')}</MenuItem>
    {/if}
    {#if target.kind === 'section' && target.key === 'inbox' && $defaultInbox}
      <MenuItem icon="lucide:link" testid="rail-menu-edit-inbox" onclick={() => { editingInbox = { inbox: $defaultInbox }; menu = null }}>{t('app.rail.editInbox')}</MenuItem>
    {/if}
    {#if target.kind === 'tag'}
      <MenuItem icon="lucide:pin" testid="rail-menu-favourite" checked={target.label.favourite} onclick={() => { toggleFavourite(target.label).catch(() => {}); menu = null }}>{t('app.rail.favourite')}</MenuItem>
    {/if}
    {#if sortable}
      <div class="menu-sep"></div>
      <p class="menu-label">{t('app.rail.sortOrder')}</p>
      {#each SORT_CHOICES as choice (choice.value)}
        <MenuItem testid="rail-menu-sort-{choice.value}" checked={($layoutSettings[sortKey(key)] ?? 'manual') === choice.value} onclick={() => { putLayoutSetting(sortKey(key), choice.value === 'manual' ? null : choice.value).catch(() => {}); menu = null }}>{t(choice.label)}</MenuItem>
      {/each}
      <MenuItem icon="lucide:hash" testid="rail-menu-count" checked={showCount(key)} onclick={() => { putLayoutSetting(countKey(key), showCount(key) ? 'off' : null).catch(() => {}); menu = null }}>{t('app.rail.showCount')}</MenuItem>
    {/if}
    </div>
  </div>
{/if}

{#if picking}
  {@const target = picking.target}
  <IconPicker at={picking.at} value={iconOf(target)} fallback={fallbackOf(target)} onpick={(icon) => saveIcon(target, icon).catch(() => {})} onclose={() => (picking = null)} />
{/if}

{#if $groupDialog}
  {@const context = $groupDialog}
  {#key context}
    <GroupEditor
      subject={{ scope: 'conversation', group: context.group }}
      parentId={context.parentId}
      onsaved={(group) => {
        const { group: edited, inbox, parentId } = context
        groupDialog.set(null)
        if (!edited && inbox && !parentId) addMembers(inbox, [{ kind: 'group', member_id: group.id }]).catch(() => {})
      }}
      oncancel={() => groupDialog.set(null)}
      onnewgroup={(parentId) => groupDialog.set({ group: null, parentId, inbox: context.inbox })}
      ondeleted={(group) => {
        groupDialog.set(null)
        if ($activeGroup === group.id) openGroup(null)
      }}
    />
  {/key}
{/if}

{#if editingInbox}
  <InboxEditor
    inbox={editingInbox.inbox}
    onclose={() => (editingInbox = null)}
    onnewgroup={(inbox) => {
      editingInbox = null
      groupDialog.set({ group: null, parentId: null, inbox })
    }}
    ondeleted={(inbox) => {
      if ($activeInbox === inbox.id) openInbox($defaultInbox?.id ?? null)
    }}
  />
{/if}

{#if editingTag}
  <TagEditor tag={editingTag.tag} onclose={() => (editingTag = null)} />
{/if}
