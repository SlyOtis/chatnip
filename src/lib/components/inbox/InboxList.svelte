<script lang="ts">
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { tick } from 'svelte'
  import { conversationStore, conversationsStore, tagsStore, splitsStore, sidebarSectionsStore, unreadOnlyStore, casesStore, openCasePicker, openChatDetails, putChatMode, api } from '$lib/tauri'
  import { memberships } from '$lib/tauri/cases'
  import { Button, ColourSwatch, Field, IconButton, IconPicker, Kbd, MenuItem, TagMark } from '$lib/components/ui'
  import { surfaceStyle } from '$lib/appearance'
  import { contentSrc, readFileAsBase64 } from '$lib/media'
  import { showTooltip } from '$lib/tooltip'
  import TaggedList from './TaggedList.svelte'
  import { layoutSettings, putLayoutSetting, sectionBackgroundKey, sectionBlendKey, sectionFitKey, sectionIconKey, sectionNameKey } from '$lib/layoutSettings'
  import { ensureIcons } from '$lib/icons'
  import { FEATURES } from '$lib/settings'
  import { anchored, boxOf, pointBox, sheetAnchor, type Box } from '$lib/popover'
  import { pulse } from '$lib/window'
  import SplitTabs from './SplitTabs.svelte'
  import InboxRow from './InboxRow.svelte'
  import {
    inboxGroups,
    inboxRows,
    inboxView,
    inboxSheet,
    collapsedGroups,
    groupCollapsed,
    toggleGroup,
    selectedIndex,
    selectedIds,
    selectConversation,
    toggleSelected,
    clearSelected,
    targetIds,
    itemId,
    itemPath,
    itemConversations,
    itemName,
    dateSegments,
    markDone,
    snooze,
    snoozeUntil,
    remove,
    toggleUnread,
    toggleTag,
    toggleSection,
    inboxRows as rowsStore,
    loadMergeGroups,
    mergeItems,
    unmergeItem,
    groupLabel,
    activeTag,
    VIEW_TITLES
  } from '$lib/inbox'
  import { groupExpandedKey, segmentCollapsedKey, toggleFlag, flag } from '$lib/layoutSettings'
  import type { InboxItem, MergedItem } from '$lib/inbox'
  import { formatDate, t, type MessageKey } from '$lib/i18n.svelte'

  let sheetMember = $state<InboxItem | null>(null)
  let hoveredMember = $state<string | null>(null)
  const focused = $derived(sheetMember ?? $inboxRows[Math.min($selectedIndex, $inboxRows.length - 1)] ?? null)
  $effect(() => {
    if (!$inboxSheet) sheetMember = null
  })
  const currentId = $derived($conversationStore.data?.id ?? null)
  let list = $state<HTMLDivElement | null>(null)
  $effect(() => {
    if (!$inboxSheet) {
      sheetAnchor.set(null)
      tagSearch = ''
    }
  })

  let dropIndex = $state<number | null>(null)
  let taggedCount = $state(0)
  let toast = $state<{ text: string; undo: (() => Promise<void>) | null } | null>(null)
  let toastTimer: ReturnType<typeof setTimeout> | undefined

  $effect(() => {
    loadMergeGroups().catch(() => {})
  })

  function notify(text: string, undo: (() => Promise<void>) | null) {
    toast = { text, undo }
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (toast = null), 6000)
  }

  async function merge(items: InboxItem[]) {
    if (items.length < 2) return
    try {
      const undo = await mergeItems(items)
      notify(t('inbox.toast.grouped'), undo)
    } catch (e) {
      notify(String(e), null)
    }
  }

  function picked(): InboxItem[] {
    return $rowsStore.filter((row) => $selectedIds.has(itemId(row)))
  }

  let sectionMenu = $state<{ x: number; y: number; key: string } | null>(null)
  let pickingSection = $state<{ key: string; at: Box } | null>(null)
  let sectionDraft = $state('')

  $effect(() => {
    ensureIcons(Object.values($layoutSettings))
  })

  let opened: string[] = []
  let revealing = $state<string | null>(null)

  $effect(() => {
    if (!currentId) return
    selectConversation(currentId)
    revealing = opened.includes(currentId) ? null : currentId
    opened = []
  })

  $effect(() => {
    const id = revealing
    const rows = $inboxRows
    if (!id || !list) return
    const index = rows.findIndex((item) => itemConversations(item).some((c) => c.id === id))
    if (index < 0) {
      if (rows.length) revealing = null
      return
    }
    revealing = null
    selectedIndex.set(index)
    tick().then(() => pulse(list?.querySelector(`[data-index="${index}"]`)))
  })

  $effect(() => {
    const row = list?.querySelector<HTMLElement>(`[data-index="${$selectedIndex}"]`)
    row?.scrollIntoView({ block: 'nearest' })
  })

  const snoozeChoices: { key: string; label: MessageKey; choice: 'later' | 'tomorrow' | 'weekend' | 'week' }[] = [
    { key: '1', label: 'inbox.snoozeChoice.later', choice: 'later' },
    { key: '2', label: 'common.tomorrow', choice: 'tomorrow' },
    { key: '3', label: 'inbox.snoozeChoice.weekend', choice: 'weekend' },
    { key: '4', label: 'inbox.snoozeChoice.week', choice: 'week' }
  ]

  async function applySnooze(choice: (typeof snoozeChoices)[number]['choice']) {
    inboxSheet.set(null)
    await snooze(targetIds(focused), snoozeUntil(choice))
  }

  async function applyTag(tag_id: string) {
    inboxSheet.set(null)
    await toggleTag(targetIds(focused), tag_id, $conversationsStore.data)
  }

  async function applySection(section: (typeof $sidebarSectionsStore)[number]) {
    inboxSheet.set(null)
    await toggleSection(targetIds(focused), section)
  }

  let tagSearch = $state('')

  const tagUsage = $derived.by(() => {
    const out = new Map<string, { count: number; latest: string }>()
    for (const conversation of $conversationsStore.data) {
      for (const link of conversation.tags) {
        const prev = out.get(link.tag_id)
        out.set(link.tag_id, { count: (prev?.count ?? 0) + 1, latest: prev && prev.latest > link.created_at ? prev.latest : link.created_at })
      }
    }
    return out
  })
  const tagChoices = $derived.by(() => {
    const needle = tagSearch.trim().toLowerCase()
    return [...$splitsStore, ...$tagsStore]
      .filter((label) => !needle || label.name.toLowerCase().includes(needle))
      .sort((a, b) => {
        const ua = tagUsage.get(a.id)
        const ub = tagUsage.get(b.id)
        const la = ua?.latest ?? ''
        const lb = ub?.latest ?? ''
        if (la !== lb) return lb < la ? -1 : 1
        return (ub?.count ?? 0) - (ua?.count ?? 0)
      })
  })

  function act(fn: () => unknown) {
    inboxSheet.set(null)
    fn()
  }

  function sheetKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      inboxSheet.set(null)
      return
    }
    if ($inboxSheet === 'snooze') {
      const hit = snoozeChoices.find((c) => c.key === e.key)
      if (hit) {
        e.preventDefault()
        applySnooze(hit.choice)
      }
    }
    if ($inboxSheet === 'tag' && !(e.target instanceof HTMLInputElement)) {
      const index = Number(e.key) - 1
      if (tagChoices[index]) {
        e.preventDefault()
        applyTag(tagChoices[index].id)
      }
    }
  }

  function focusedTags() {
    return focused ? itemConversations(focused).flatMap((c) => c.tags) : []
  }

  async function deleteSection(id: string) {
    await api.deleteSidebarSection({ id })
    sidebarSectionsStore.set(await api.getSidebarSections())
  }

  function openSectionMenu(e: MouseEvent, key: string) {
    e.stopPropagation()
    const group = $inboxGroups.find((candidate) => candidate.key === key)
    sectionDraft = group ? groupLabel(group, $layoutSettings) : ''
    const box = boxOf(e.currentTarget)
    sectionMenu = box ? { x: box.left + box.width, y: box.top + box.height, key } : { x: e.clientX, y: e.clientY, key }
  }

  async function sectionImage(key: string, input: HTMLInputElement) {
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    const saved = await api.postContent({ kind: 'image', name: file.name.replace(/\.[^.]+$/, ''), data: await readFileAsBase64(file), mime_type: file.type || 'image/png' })
    await putLayoutSetting(sectionBackgroundKey(key), contentSrc(saved))
  }

  function blendOf(key: string): number {
    const stored = Number($layoutSettings[sectionBlendKey(key)])
    return Number.isFinite(stored) && $layoutSettings[sectionBlendKey(key)] ? stored : 78
  }

  function openMember(member: InboxItem, sheet: 'snooze' | 'tag') {
    sheetMember = member
    sheetAnchor.set(boxOf(list?.querySelector(`[data-testid="inbox-merged-member"][data-id="${CSS.escape(itemId(member))}"]`) ?? null))
    inboxSheet.set(sheet)
  }

  function closeSectionMenu() {
    const menu = sectionMenu
    sectionMenu = null
    if (menu) renameSection(menu.key, sectionDraft).catch(() => {})
  }

  async function renameSection(key: string, name: string) {
    const group = $inboxGroups.find((candidate) => candidate.key === key)
    if (!group) return
    const trimmed = name.trim()
    if (trimmed === groupLabel(group, $layoutSettings)) return
    if (group.section) {
      if (!trimmed || trimmed === group.section.name) return
      await api.putSidebarSection({ id: group.section.id, name: trimmed })
      sidebarSectionsStore.set(await api.getSidebarSections())
      return
    }
    const fallback = group.label ? t(group.label) : ''
    await putLayoutSetting(sectionNameKey(key), trimmed && trimmed !== fallback ? trimmed : null)
  }
</script>

{#snippet tree(parent: MergedItem, depth: number)}
  {#each parent.members as member, m (itemId(member))}
    {@const ids = itemConversations(member).map((c) => c.id)}
    {@const open = member.kind === 'merged' && flag($layoutSettings, groupExpandedKey(member.id))}
    <div
      data-testid="inbox-merged-member"
      data-id={itemId(member)}
      data-depth={depth}
      class="relative before:pointer-events-none before:absolute before:left-[calc(var(--depth)*1.5rem_+_0.375rem)] before:top-0 before:z-[1] before:w-px before:bg-line after:pointer-events-none after:absolute after:left-[calc(var(--depth)*1.5rem_+_0.375rem)] after:top-[calc(var(--row)/2)] after:z-[1] after:h-px after:w-1.5 after:bg-line {m === parent.members.length - 1 ? 'before:h-[calc(var(--row)/2)]' : 'before:bottom-0'}"
      style="--depth: {depth}"
    >
      <div class="group/member relative pl-[calc(var(--depth)*1.5rem)] [&>[data-testid=inbox-row]]:rounded-l-[0.625rem]">
        <InboxRow
          item={member}
          focused={hoveredMember === itemId(member)}
          active={!!currentId && ids.includes(currentId)}
          picked={false}
          onopen={() => { opened = ids; goto(itemPath(member)) }}
          onfocus={() => (hoveredMember = itemId(member))}
          onpick={() => {}}
          ondone={() => markDone(ids, !itemConversations(member).every((c) => c.is_archived))}
          onsnooze={() => openMember(member, 'snooze')}
          onmore={() => openMember(member, 'tag')}
          ondelete={() => remove(ids, $conversationsStore.data)}
          expanded={open}
          onexpand={member.kind === 'merged' ? () => toggleFlag(groupExpandedKey(member.id)) : undefined}
        />
        <button type="button" class="absolute left-[calc((var(--depth)_-_1)*1.5rem_+_0.25rem)] top-1/2 z-[2] hidden size-5 -translate-y-1/2 items-center justify-center rounded text-ink-3 hover:text-ink group-hover/member:flex" title={t('inbox.ungroupTitle')} data-testid="inbox-unmerge" aria-label={t('inbox.ungroup', { name: itemName(member) })} onclick={() => unmergeItem(member, parent).then(() => notify(t('inbox.toast.ungrouped'), null))}>
          <Icon icon="lucide:x" class="text-[12px]" />
        </button>
      </div>
      {#if member.kind === 'merged' && open}
        {@render tree(member, depth + 1)}
      {/if}
    </div>
  {/each}
{/snippet}

<section class="relative flex h-full min-w-0 flex-col bg-paper" aria-label={t(VIEW_TITLES[$inboxView])}>
  <SplitTabs />

  {#if $selectedIds.size > 0}
    <div data-testid="inbox-selection" class="flex h-10 shrink-0 items-center gap-1 px-5 text-[13px] text-ink">
      <span class="mr-2 font-mono text-[12px] text-ink-3">{t('common.selected', { count: $selectedIds.size })}</span>
      {#if $inboxView === 'deleted'}
        <Button size="sm" icon="lucide:archive-restore" onclick={() => remove(targetIds(focused), $conversationsStore.data)}>{t('inbox.restore')}</Button>
      {:else}
        <Button testid="inbox-selection-done" size="sm" icon="lucide:check" keys="E" onclick={() => markDone(targetIds(focused), $inboxView !== 'done')}>{$inboxView === 'done' ? t('inbox.moveToInbox') : t('inbox.view.done')}</Button>
        <Button size="sm" icon="lucide:clock" keys="H" onclick={() => inboxSheet.set('snooze')}>{t('inbox.snooze')}</Button>
        <Button size="sm" icon="lucide:tag" keys="L" onclick={() => inboxSheet.set('tag')}>{t('inbox.tag')}</Button>
        {#if $selectedIds.size > 1}
          <Button testid="inbox-selection-merge" size="sm" icon="lucide:merge" onclick={() => merge(picked())}>{t('inbox.groupSelected')}</Button>
        {/if}
        <Button testid="inbox-selection-delete" size="sm" variant="danger" icon="lucide:trash-2" keys="#" onclick={() => remove(targetIds(focused), $conversationsStore.data)}>{t('common.delete')}</Button>
      {/if}
      <Button testid="inbox-selection-clear" size="sm" class="ml-auto" onclick={clearSelected}>{t('common.clear')}</Button>
    </div>
  {/if}

  <div class="relative min-h-0 flex-1 overflow-y-auto pb-6" style="--list-paper: var(--paper)" bind:this={list} data-testid="inbox-list" role="listbox" aria-label={t('inbox.conversations')}>
    {#if $activeTag}
      <TaggedList tagId={$activeTag} bind:count={taggedCount} />
    {/if}
    {#if $conversationsStore.is_loading && $inboxRows.length === 0}
      <div class="flex flex-col gap-3 px-6 pt-4">
        {#each Array(8) as _, i (i)}
          <div class="h-4 w-[70%] animate-pulse rounded bg-paper-2"></div>
        {/each}
      </div>
    {:else if $inboxRows.length === 0 && !taggedCount}
      <div class="flex h-full w-full flex-col items-center justify-start gap-2 px-8 pt-[18%] text-center">
        <p class="text-[15px] font-medium tracking-tight text-ink">{$inboxView === 'inbox' ? t('inbox.empty.caughtUp') : $inboxView === 'deleted' ? t('inbox.empty.nothingDeleted') : t('inbox.empty.nothingHere')}</p>
        <p class="text-[13.5px] text-ink-3">
          {#if $conversationsStore.data.length === 0}
            {t('inbox.empty.connect')}
          {:else if $unreadOnlyStore}
            {t('inbox.empty.unreadOnly')}
          {:else if $inboxView === 'deleted'}
            {t('inbox.empty.deletedBefore')} <Kbd keys="#" /> {t('inbox.empty.deletedAfter')}
          {:else}
            {t('inbox.empty.searchBefore')} <Kbd keys="⌘K" /> {t('inbox.empty.searchAfter')}
          {/if}
        </p>
      </div>
    {:else}
      {#each $inboxGroups as group, g (group.key)}
        {@const collapsed = groupCollapsed($collapsedGroups, group.key)}
        {@const icon = $layoutSettings[sectionIconKey(group.key)]}
        {@const background = $layoutSettings[sectionBackgroundKey(group.key)] || null}
        {@const fit = $layoutSettings[sectionFitKey(group.key)] === 'scroll' ? 'scroll' : 'fixed'}
        {@const blend = blendOf(group.key)}
        {@const pictured = !!background && !background.startsWith('#')}
        <div class="group/section {background ? 'bg-paper text-ink' : ''} {pictured ? '[&_[data-testid=inbox-row]:hover]:backdrop-blur-md' : ''}" style={surfaceStyle(background, null, fit, blend)} data-testid="inbox-group" data-key={group.key} data-background={background} data-collapsed={collapsed}>
          <div data-testid="inbox-group-header" class="sticky z-[2] flex items-center gap-2 bg-paper pl-3 pr-4 {g === 0 ? '-top-1 h-8 pt-1' : '-top-5 h-12 pt-5'}" style={surfaceStyle(background, g === 0 ? '0.25rem' : '1.25rem', fit, blend)} role="presentation" oncontextmenu={(e) => { e.preventDefault(); sectionMenu = { x: e.clientX, y: e.clientY, key: group.key } }}>
            <button data-testid="inbox-group-toggle" class="flex min-w-0 flex-1 items-center gap-2 text-left text-[13px] text-ink-3 hover:text-ink-2" onclick={() => toggleGroup(group.key)} aria-expanded={!collapsed}>
              {#if icon}
                <Icon {icon} class="shrink-0 text-[13px]" />
              {/if}
              <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{groupLabel(group, $layoutSettings)}</span>
              {#if collapsed}
                <span class="font-mono text-[11px]">{group.rows.length}</span>
              {/if}
            </button>
            {#if group.section}
              <button class="hidden size-5 items-center justify-center rounded text-ink-3 hover:text-ink group-hover/section:flex" onclick={() => deleteSection(group.section!.id)} title={t('inbox.removeSection')} aria-label={t('inbox.removeSection')}>
                <Icon icon="lucide:x" class="text-[12px]" />
              </button>
            {/if}
            <button type="button" data-testid="inbox-group-more" class="flex size-5 shrink-0 items-center justify-center rounded text-ink-3 hover:text-ink focus-visible:opacity-100 group-hover/section:opacity-100 {sectionMenu?.key === group.key ? 'opacity-100' : 'opacity-0'}" onclick={(e) => openSectionMenu(e, group.key)} title={t('inbox.groupMore', { name: groupLabel(group, $layoutSettings) })} aria-label={t('inbox.groupMore', { name: groupLabel(group, $layoutSettings) })} aria-haspopup="menu" aria-expanded={sectionMenu?.key === group.key}>
              <Icon icon="lucide:ellipsis" class="text-[13px]" />
            </button>
            <button class="flex size-5 shrink-0 items-center justify-center rounded text-ink-3 hover:text-ink" onclick={() => toggleGroup(group.key)} aria-label={collapsed ? t('inbox.expandGroup', { name: groupLabel(group, $layoutSettings) }) : t('inbox.collapseGroup', { name: groupLabel(group, $layoutSettings) })}>
              <Icon icon={collapsed ? 'lucide:chevron-right' : 'lucide:chevron-down'} class="text-[13px]" />
            </button>
          </div>
          {#if !collapsed}
            {#each dateSegments(group.rows) as segment (group.key + ':' + segment.offset)}
              {@const folded = !!segment.label && flag($layoutSettings, segmentCollapsedKey(group.key, segment.label))}
              {#if segment.label}
                <button type="button" data-testid="inbox-segment-toggle" class="mt-2 flex w-full items-center gap-1 px-3 pb-0.5 text-left text-[11px] text-ink-3/70 hover:text-ink-3" aria-expanded={!folded} onclick={() => toggleFlag(segmentCollapsedKey(group.key, segment.label!))}>
                  <Icon icon={folded ? 'lucide:chevron-right' : 'lucide:chevron-down'} class="text-[10px]" />
                  <span>{segment.label}</span>
                  {#if folded}<span class="font-mono">{segment.rows.length}</span>{/if}
                </button>
              {/if}
              {#if !folded}
              {#each segment.rows as item, i (itemId(item))}
                {@const index = group.offset + segment.offset + i}
                {@const ids = itemConversations(item).map((c) => c.id)}
                {@const parentOpen = item.kind === 'merged' && flag($layoutSettings, groupExpandedKey(item.id))}
                <div
                  data-index={index}
                  draggable="true"
                  role="presentation"
                  class={dropIndex === index ? 'bg-paper-3' : ''}
                  ondragover={(e) => {
                    if (!e.dataTransfer?.types.includes('application/x-chatnip-item')) return
                    e.preventDefault()
                    dropIndex = index
                  }}
                  ondragleave={() => (dropIndex = null)}
                  ondrop={(e) => {
                    const key = e.dataTransfer?.getData('application/x-chatnip-item')
                    dropIndex = null
                    const source = $rowsStore.find((row) => itemId(row) === key)
                    if (!source || itemId(source) === itemId(item)) return
                    e.preventDefault()
                    merge([item, source])
                  }}
                  ondragend={() => (dropIndex = null)}
                  ondragstart={(e) => {
                    e.dataTransfer?.setData('application/x-chatnip-item', itemId(item))
                    e.dataTransfer?.setData('application/x-chatnip-conversations', ids.join(','))
                    e.dataTransfer?.setData('text/plain', itemConversations(item).map((c) => c.name).join(', '))
                  }}
                >
                  <div class="relative {parentOpen ? 'z-[3] shadow-[0_2px_6px_-2px_rgb(0_0_0/0.18)]' : ''}">
                  <InboxRow
                    {item}
                    focused={index === $selectedIndex}
                    active={!!currentId && ids.includes(currentId)}
                    picked={$selectedIds.has(itemId(item))}
                    onopen={() => {
                      opened = ids
                      selectedIndex.set(index)
                      goto(itemPath(item))
                    }}
                    onfocus={() => selectedIndex.set(index)}
                    onpick={() => toggleSelected(itemId(item))}
                    ondone={() => markDone(ids, !itemConversations(item).every((c) => c.is_archived))}
                    onsnooze={() => {
                      selectedIndex.set(index)
                      sheetAnchor.set(null)
                      inboxSheet.set('snooze')
                    }}
                    onmore={() => {
                      selectedIndex.set(index)
                      sheetAnchor.set(null)
                      inboxSheet.set('tag')
                    }}
                    ondelete={() => remove(ids, $conversationsStore.data)}
                    expanded={item.kind === 'merged' && flag($layoutSettings, groupExpandedKey(item.id))}
                    onexpand={item.kind === 'merged' ? () => toggleFlag(groupExpandedKey(item.id)) : undefined}
                  />
                  </div>
                  {#if parentOpen}
                    <div data-testid="inbox-merged-members" class="bg-paper-2/50 shadow-[inset_0_4px_6px_-4px_rgb(0_0_0/0.22),inset_0_-4px_6px_-4px_rgb(0_0_0/0.12)]">
                      {@render tree(item, 1)}
                    </div>
                  {/if}
                </div>
              {/each}
              {/if}
            {/each}
          {/if}
        </div>
      {/each}
    {/if}
  </div>

  {#if $inboxSheet && focused}
    {@const single = itemConversations(focused).length === 1 ? itemConversations(focused)[0] : null}
    <div class="menu w-[min(20rem,90vw)]" use:anchored={{ at: () => $sheetAnchor ?? boxOf(list?.querySelector(`[data-index="${$selectedIndex}"]`) ?? null), onclose: () => inboxSheet.set(null) }} role="dialog" data-testid="inbox-sheet" data-kind={$inboxSheet} aria-label={$inboxSheet === 'snooze' ? t('inbox.snoozeUntil') : t('inbox.actions')} tabindex="-1" onkeydown={sheetKeydown}>
      {#if $inboxSheet === 'snooze'}
        <p class="px-2.5 pb-1 pt-1.5 text-[12.5px] text-ink-3">{t('inbox.snoozeUntil')}</p>
        {#each snoozeChoices as choice (choice.key)}
          <MenuItem testid="inbox-snooze-{choice.choice}" keys={choice.key} detail={formatDate(snoozeUntil(choice.choice), { weekday: 'short', hour: '2-digit', minute: '2-digit' })} onclick={() => applySnooze(choice.choice)}>{t(choice.label)}</MenuItem>
        {/each}
      {:else}
        <MenuItem testid="inbox-sheet-read" icon="lucide:mail-open" keys="U" onclick={() => act(() => toggleUnread(targetIds(focused), $conversationsStore.data))}>{t('inbox.toggleRead')}</MenuItem>
        {#if itemConversations(focused).length}
          {@const reading = itemConversations(focused).every((c) => c.mode === 'read')}
          <MenuItem testid="inbox-sheet-mode" icon={reading ? 'lucide:pen-line' : 'lucide:book-open'} checked={reading} onclick={() => act(() => Promise.all(itemConversations(focused).map((c) => putChatMode(c.id, reading ? 'write' : 'read'))))}>{t('chats.mode.read')}</MenuItem>
        {/if}
        {#if FEATURES.cases}
          <MenuItem icon="lucide:briefcase" keys="⇧C" detail={memberships($casesStore, targetIds(focused)).size ? String(memberships($casesStore, targetIds(focused)).size) : undefined} onclick={() => act(() => openCasePicker({ conversationIds: targetIds(focused), messageIds: [] }))}>{t('inbox.addToCase')}</MenuItem>
        {/if}
        {#if single}
          <MenuItem testid="inbox-sheet-edit" icon="lucide:pencil" onclick={() => act(() => openChatDetails(single.id))}>{t('common.edit')}</MenuItem>
        {/if}
        <MenuItem testid="inbox-sheet-delete" icon="lucide:trash-2" keys="#" danger onclick={() => act(() => remove(targetIds(focused), $conversationsStore.data))}>{itemConversations(focused).every((c) => !!c.deleted_at) ? t('inbox.restore') : t('common.delete')}</MenuItem>
        <div class="menu-sep"></div>
        <p class="menu-label">{t('inbox.tags')}</p>
        <label class="mx-1 mb-1 flex h-8 items-center gap-2 rounded-[0.5rem] bg-paper-2 px-2.5 focus-within:bg-paper-3">
          <Icon icon="lucide:search" class="shrink-0 text-[13px] text-ink-3" />
          <Field data-testid="inbox-sheet-tag-search" size="sm" class="h-auto! border-b-0! px-0!" placeholder={t('inbox.findTag')} aria-label={t('inbox.findTag')} autocomplete="off" bind:value={tagSearch} onkeydown={(e) => { if (e.key === 'Enter' && tagChoices[0]) { e.preventDefault(); applyTag(tagChoices[0].id) } }} />
        </label>
        <div class="flex max-h-[16.5rem] flex-col gap-px overflow-y-auto" role="group" aria-label={t('inbox.tags')}>
          {#each tagChoices as label, index (label.id)}
            <MenuItem testid="inbox-sheet-tag-{label.id}" keys={index < 9 ? String(index + 1) : undefined} checked={focusedTags().some((l) => l.tag_id === label.id)} onclick={() => applyTag(label.id)}>
              <span class="flex items-center gap-2"><span class="flex w-4 shrink-0 justify-center"><TagMark tag={label} /></span>{label.name}</span>
            </MenuItem>
          {/each}
        </div>
        {#if $sidebarSectionsStore.length > 0}
          <p class="menu-label">{t('inbox.sections')}</p>
          {#each $sidebarSectionsStore as section (section.id)}
            <MenuItem testid="inbox-sheet-section-{section.id}" icon="lucide:folder" checked={itemConversations(focused).some((c) => section.conversations.some((s) => s.id === c.id))} onclick={() => applySection(section)}>{section.name}</MenuItem>
          {/each}
        {/if}
      {/if}
    </div>
  {/if}

</section>

{#if toast}
  {@const current = toast}
  <div class="glass rise fixed bottom-6 left-1/2 z-[1000] flex -translate-x-1/2 items-center gap-3 rounded-[0.75rem] px-4 py-2 text-[13px] text-ink shadow-[0_12px_32px_rgb(0_0_0/0.28)]" data-testid="inbox-toast" role="status">
    <span>{current.text}</span>
    {#if current.undo}
      <Button testid="inbox-toast-undo" size="sm" onclick={() => { current.undo?.().catch(() => {}); toast = null }}>{t('inbox.toast.ungroup')}</Button>
    {/if}
  </div>
{/if}

{#if sectionMenu}
  {@const key = sectionMenu.key}
  {@const background = $layoutSettings[sectionBackgroundKey(key)] || null}
  {@const image = background && !background.startsWith('#') ? background : null}
  {@const at = pointBox(sectionMenu.x, sectionMenu.y)}
  <div class="menu w-[min(17rem,90vw)]" data-testid="inbox-group-menu" use:anchored={{ at, onclose: closeSectionMenu }} role="menu">
    <form class="mb-1 border-b border-line px-1.5 pb-1.5" onsubmit={(e) => { e.preventDefault(); closeSectionMenu() }}>
      <Field data-testid="inbox-group-menu-name" size="sm" inline bind:value={sectionDraft} icon={$layoutSettings[sectionIconKey(key)] || 'lucide:folder'} onicon={() => { pickingSection = { key, at }; closeSectionMenu() }} aria-label={t('inbox.renameSection')} placeholder={t('common.name')} />
    </form>
    <div class="flex h-8 items-center gap-1 pl-2.5 pr-1 text-[14px] text-ink" role="presentation">
      <span class="min-w-0 flex-1">{t('inbox.sectionBackground')}</span>
      {#if image}
        {@const scroll = $layoutSettings[sectionFitKey(key)] === 'scroll'}
        <IconButton size="sm" testid="inbox-group-menu-fit" icon={scroll ? 'lucide:move-vertical' : 'lucide:pin'} active={scroll} label={scroll ? t('inbox.sectionFitScroll') : t('inbox.sectionFitFixed')} onclick={() => putLayoutSetting(sectionFitKey(key), scroll ? null : 'scroll').catch(() => {})} />
      {:else}
        <ColourSwatch size="sm" clearable label={t('inbox.sectionColour')} value={image ? null : background} onpick={(colour) => putLayoutSetting(sectionBackgroundKey(key), colour).catch(() => {})} />
      {/if}
      {#if !background || image}
        <label class="relative flex size-7 shrink-0 cursor-pointer items-center justify-center overflow-hidden rounded-[0.5rem] text-ink-2 hover:bg-paper-3 hover:text-ink" data-testid="inbox-group-menu-image" onmouseenter={(e) => showTooltip(e.currentTarget, t('inbox.sectionImage'))}>
        {#if image}<img src={image} alt="" class="size-5 rounded-[0.25rem] object-cover" />{:else}<Icon icon="lucide:image-plus" class="text-[14px]" />{/if}
        <input type="file" accept="image/*" class="absolute inset-0 cursor-pointer opacity-0" aria-label={t('inbox.sectionImage')} onchange={(e) => sectionImage(key, e.currentTarget)} />
      </label>
      {/if}
      {#if background}
        <IconButton size="sm" icon="lucide:x" label={t('inbox.sectionBackgroundClear')} onclick={() => putLayoutSetting(sectionBackgroundKey(key), null).catch(() => {})} />
      {/if}
    </div>
    {#if image}
      <label class="flex h-8 items-center gap-2 pl-2.5 pr-2 text-[14px] text-ink" onmouseenter={(e) => showTooltip(e.currentTarget, t('inbox.sectionBlendHint'))}>
        <Icon icon="lucide:blend" class="shrink-0 text-[14px] text-ink-3" />
        <span class="shrink-0">{t('inbox.sectionBlend')}</span>
        <input type="range" min="0" max="95" step="1" data-testid="inbox-group-menu-blend" class="min-w-0 flex-1 accent-[var(--accent)]" value={blendOf(key)} onchange={(e) => putLayoutSetting(sectionBlendKey(key), e.currentTarget.value).catch(() => {})} />
        <span class="w-[3ch] shrink-0 text-right font-mono text-[11px] text-ink-3">{blendOf(key)}</span>
      </label>
    {/if}
    <MenuItem icon={groupCollapsed($collapsedGroups, key) ? 'lucide:chevron-down' : 'lucide:chevron-right'} onclick={() => { toggleGroup(key); closeSectionMenu() }}>{groupCollapsed($collapsedGroups, key) ? t('inbox.expand') : t('inbox.collapse')}</MenuItem>
  </div>
{/if}

{#if pickingSection}
  {@const key = pickingSection.key}
  <IconPicker at={pickingSection.at} value={$layoutSettings[sectionIconKey(key)] ?? null} onpick={(icon) => putLayoutSetting(sectionIconKey(key), icon).catch(() => {})} onclose={() => (pickingSection = null)} />
{/if}

