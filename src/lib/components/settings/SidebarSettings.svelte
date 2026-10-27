<script lang="ts">
  import Icon from '@iconify/svelte'
  import type { SidebarEntry, SidebarLayout } from '$lib/tauri/types'
  import { Button, IconButton, IconPicker, Section } from '$lib/components/ui'
  import { inboxesStore, loadInboxes } from '$lib/inboxes'
  import {
    sidebarLayout,
    loadSidebarLayout,
    saveSidebarLayout,
    moveSidebarEntry,
    insertSidebarDivider,
    insertSidebarFolder,
    insertSidebarHeading,
    removeSidebarEntry,
    putSidebarEntry,
    type SidebarSlot,
    type SidebarZone
  } from '$lib/layoutSettings'
  import { featureShown } from '$lib/settings'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  const BUILT_IN: Record<string, { key: MessageKey; icon: string }> = {
    inbox: { key: 'settings.sidebar.item.inbox', icon: 'lucide:inbox' },
    tags: { key: 'settings.sidebar.item.tags', icon: 'lucide:tags' },
    done: { key: 'settings.sidebar.item.done', icon: 'lucide:circle-check' },
    snoozed: { key: 'settings.sidebar.item.snoozed', icon: 'lucide:clock' },
    notifications: { key: 'settings.sidebar.item.notifications', icon: 'lucide:bell' },
    deleted: { key: 'settings.sidebar.item.deleted', icon: 'lucide:trash-2' },
    pipeline: { key: 'settings.sidebar.item.pipeline', icon: 'lucide:git-pull-request-arrow' },
    groups: { key: 'settings.sidebar.item.groups', icon: 'lucide:folder-tree' },
    cases: { key: 'settings.sidebar.item.cases', icon: 'lucide:briefcase' },
    actions: { key: 'settings.sidebar.item.actions', icon: 'lucide:workflow' },
    content: { key: 'content.title', icon: 'lucide:library' },
    calendar: { key: 'settings.sidebar.item.calendar', icon: 'lucide:calendar' },
    statistics: { key: 'settings.sidebar.item.statistics', icon: 'lucide:chart-no-axes-column' }
  }

  let dragging = $state<SidebarSlot | 'boundary' | null>(null)
  let over = $state<string | null>(null)
  let picking = $state<SidebarSlot | null>(null)

  $effect(() => {
    loadInboxes().catch(() => {})
    loadSidebarLayout().catch(() => {})
  })

  function describe(key: string): { label: string; icon: string } {
    if (key.startsWith('inbox:')) {
      const inbox = $inboxesStore.find((candidate) => 'inbox:' + candidate.id === key)
      return { label: inbox?.name ?? t('settings.sidebar.removedInbox'), icon: inbox?.icon || 'lucide:inbox' }
    }
    const info = BUILT_IN[key]
    if (!info) return { label: key, icon: 'lucide:circle' }
    return { label: t(info.key), icon: info.icon }
  }

  function slotKey(slot: SidebarSlot): string {
    return `${slot.zone}:${slot.folder ?? '-'}:${slot.index}`
  }

  function commit(next: SidebarLayout) {
    if (next === $sidebarLayout) return
    saveSidebarLayout(next).catch(() => loadSidebarLayout().catch(() => {}))
  }

  function split(at: number) {
    const all = [...$sidebarLayout.top, ...$sidebarLayout.footer]
    commit({ top: all.slice(0, at), footer: all.slice(at) })
  }

  function flat(slot: SidebarSlot): number {
    const index = slot.folder ?? slot.index
    return slot.zone === 'top' ? index : $sidebarLayout.top.length + index
  }

  function drop(to: SidebarSlot | 'boundary' | 'end') {
    const from = dragging
    dragging = null
    over = null
    if (!from) return
    if (from === 'boundary') {
      if (to === 'end') split($sidebarLayout.top.length + $sidebarLayout.footer.length)
      else if (to !== 'boundary') split(flat(to))
      return
    }
    if (to === 'boundary') commit(moveSidebarEntry($sidebarLayout, from, from.zone === 'top' && from.folder === undefined ? { zone: 'footer', index: 0 } : { zone: 'top', index: $sidebarLayout.top.length }))
    else if (to === 'end') commit(moveSidebarEntry($sidebarLayout, from, { zone: 'footer', index: $sidebarLayout.footer.length }))
    else commit(moveSidebarEntry($sidebarLayout, from, to))
  }

  function length(zone: SidebarZone, folder?: number): number {
    if (folder === undefined) return $sidebarLayout[zone].length
    const entry = $sidebarLayout[zone][folder]
    return entry?.kind === 'folder' ? entry.items.length : 0
  }

  function first(slot: SidebarSlot): boolean {
    return slot.index === 0 && (slot.folder !== undefined || slot.zone === 'top')
  }

  function last(slot: SidebarSlot): boolean {
    return slot.index >= length(slot.zone, slot.folder) - 1 && (slot.folder !== undefined || slot.zone === 'footer')
  }

  function step(slot: SidebarSlot, delta: number) {
    if (slot.folder === undefined && delta < 0 && slot.zone === 'footer' && slot.index === 0) {
      commit(moveSidebarEntry($sidebarLayout, slot, { zone: 'top', index: length('top') }))
      return
    }
    if (slot.folder === undefined && delta > 0 && slot.zone === 'top' && slot.index === length('top') - 1) {
      commit(moveSidebarEntry($sidebarLayout, slot, { zone: 'footer', index: 0 }))
      return
    }
    const target = slot.index + delta
    if (target < 0 || target >= length(slot.zone, slot.folder)) return
    commit(moveSidebarEntry($sidebarLayout, slot, { ...slot, index: delta > 0 ? target + 1 : target }))
  }

  function outOfFolder(slot: SidebarSlot) {
    if (slot.folder === undefined) return
    commit(moveSidebarEntry($sidebarLayout, slot, { zone: slot.zone, index: slot.folder + 1 }))
  }

  function rename(slot: SidebarSlot, name: string) {
    if (!name.trim()) return
    commit(putSidebarEntry($sidebarLayout, slot, { name: name.trim() }))
  }

  function target(e: DragEvent, key: string) {
    if (!dragging) return
    e.preventDefault()
    over = key
  }
</script>

{#snippet rowFor(entry: SidebarEntry, slot: SidebarSlot)}
  {#if entry.kind !== 'item' || featureShown(entry.key)}
  {@const key = slotKey(slot)}
  <div
    class="grid h-9 grid-cols-[1.25rem_1.5rem_minmax(0,1fr)_auto] items-center gap-2 rounded-[0.5rem] pl-1.5 pr-1 {over === key ? 'bg-accent-soft' : 'hover:bg-paper-2'}"
    role="listitem"
    data-testid="sidebar-entry"
    data-zone={slot.zone}
    data-key={entry.kind === 'item' ? entry.key : entry.id}
    draggable="true"
    ondragstart={(e) => {
      dragging = slot
      e.dataTransfer?.setData('text/plain', key)
    }}
    ondragover={(e) => target(e, key)}
    ondragleave={() => (over = null)}
    ondrop={(e) => {
      e.preventDefault()
      drop(slot)
    }}
    ondragend={() => {
      dragging = null
      over = null
    }}
  >
    <Icon icon="lucide:grip-vertical" class="cursor-grab text-[13px] text-ink-3" />
    {#if entry.kind === 'item'}
      {@const info = describe(entry.key)}
      <Icon icon={info.icon} class="text-[14px] text-ink-2" />
      <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[14px] text-ink">{info.label}</span>
    {:else if entry.kind === 'divider'}
      <span></span>
      <span class="flex items-center gap-2 text-[12.5px] text-ink-3"><span class="h-px flex-1 bg-line-strong"></span>{t('settings.sidebar.divider')}<span class="h-px flex-1 bg-line-strong"></span></span>
    {:else if entry.kind === 'heading'}
      <Icon icon="lucide:heading" class="text-[14px] text-ink-3" />
      <input
        class="h-7 min-w-0 border-b border-transparent bg-transparent text-[12.5px] font-medium uppercase tracking-wide text-ink-3 outline-none focus:border-line-strong"
        data-testid="sidebar-heading-name"
        value={entry.name}
        aria-label={t('settings.sidebar.headingName')}
        onchange={(e) => rename(slot, e.currentTarget.value)}
      />
    {:else}
      <button type="button" class="flex items-center justify-center text-[14px] text-ink-2 hover:text-ink" onclick={() => (picking = slot)} aria-label={t('settings.sidebar.changeFolderIcon')}>
        <Icon icon={entry.icon || 'lucide:folder'} />
      </button>
      <input
        class="h-7 min-w-0 border-b border-transparent bg-transparent text-[14px] font-medium text-ink outline-none focus:border-line-strong"
        value={entry.name}
        aria-label={t('settings.sidebar.folderName')}
        onchange={(e) => rename(slot, e.currentTarget.value)}
      />
    {/if}
    <span class="flex items-center gap-0.5">
      <IconButton size="sm" icon="lucide:arrow-up" label={t('settings.moveUp')} disabled={first(slot)} onclick={() => step(slot, -1)} />
      <IconButton size="sm" icon="lucide:arrow-down" label={t('settings.moveDown')} disabled={last(slot)} onclick={() => step(slot, 1)} />
      {#if slot.folder !== undefined}
        <IconButton size="sm" icon="lucide:folder-output" label={t('settings.sidebar.moveOutOfFolder')} onclick={() => outOfFolder(slot)} />
      {/if}
      {#if entry.kind !== 'item'}
        <IconButton size="sm" icon="lucide:x" tone="danger" label={entry.kind === 'folder' ? t('settings.sidebar.removeFolder') : entry.kind === 'heading' ? t('settings.sidebar.removeHeading') : t('settings.sidebar.removeDivider')} onclick={() => commit(removeSidebarEntry($sidebarLayout, slot))} />
      {/if}
    </span>
  </div>
  {/if}
{/snippet}

{#snippet zone(name: SidebarZone)}
  {#each $sidebarLayout[name] as entry, index (entry.kind === 'item' ? entry.key : entry.id)}
    {@render rowFor(entry, { zone: name, index })}
    {#if entry.kind === 'folder'}
      <div
        class="ml-6 flex min-h-8 flex-col gap-px border-l border-line pl-2 {over === `${name}:in:${index}` ? 'bg-accent-soft' : ''}"
        role="list"
        ondragover={(e) => {
          if (dragging !== 'boundary') target(e, `${name}:in:${index}`)
        }}
        ondragleave={() => (over = null)}
        ondrop={(e) => {
          e.preventDefault()
          drop({ zone: name, index: entry.items.length, folder: index })
        }}
      >
        {#each entry.items as inner, innerIndex (inner.kind === 'item' ? inner.key : inner.id)}
          {@render rowFor(inner, { zone: name, index: innerIndex, folder: index })}
        {/each}
        {#if !entry.items.length}
          <p class="py-2 text-[12.5px] text-ink-3">{t('settings.sidebar.dropHere')}</p>
        {/if}
      </div>
    {/if}
  {/each}
{/snippet}

<Section id="organise.sidebar" title={t('settings.tab.sidebar')} description={t('settings.sidebar.description')}>
  {#snippet actions()}
    <Button size="sm" icon="lucide:minus" onclick={() => commit(insertSidebarDivider($sidebarLayout, { zone: 'top', index: $sidebarLayout.top.length }))}>{t('settings.sidebar.divider')}</Button>
    <Button size="sm" testid="sidebar-add-heading" icon="lucide:heading" onclick={() => commit(insertSidebarHeading($sidebarLayout, 'top', t('settings.sidebar.newHeading')))}>{t('settings.sidebar.heading')}</Button>
    <Button size="sm" icon="lucide:folder-plus" onclick={() => commit(insertSidebarFolder($sidebarLayout, 'top', t('settings.sidebar.newFolder')))}>{t('settings.sidebar.folder')}</Button>
  {/snippet}
  <div class="flex flex-col gap-px border-t border-line pt-2" role="list" data-testid="sidebar-layout">
    {@render zone('top')}
    <div
      class="my-1 grid h-9 cursor-grab grid-cols-[1.25rem_minmax(0,1fr)] items-center gap-2 rounded-[0.5rem] pl-1.5 pr-2 text-[12.5px] {over === 'boundary' ? 'bg-accent-soft text-accent' : 'text-ink-3 hover:bg-paper-2'}"
      role="separator"
      aria-label={t('settings.sidebar.boundary')}
      title={t('settings.sidebar.boundary')}
      data-testid="sidebar-boundary"
      draggable="true"
      ondragstart={(e) => {
        dragging = 'boundary'
        e.dataTransfer?.setData('text/plain', 'boundary')
      }}
      ondragover={(e) => target(e, 'boundary')}
      ondragleave={() => (over = null)}
      ondrop={(e) => {
        e.preventDefault()
        drop('boundary')
      }}
      ondragend={() => {
        dragging = null
        over = null
      }}
    >
      <Icon icon="lucide:grip-vertical" class="text-[13px]" />
      <span class="flex min-w-0 items-center gap-2">
        <span class="h-px flex-1 bg-line-strong"></span>
        <Icon icon="lucide:pin" class="shrink-0 text-[13px]" />
        <span class="shrink-0 font-medium text-ink-2">{t('settings.sidebar.footer')}</span>
        <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{t('settings.sidebar.footerHint')}</span>
        <span class="h-px flex-1 bg-line-strong"></span>
      </span>
    </div>
    {@render zone('footer')}
    <div
      class="h-8 rounded-[0.5rem] {over === 'end' ? 'bg-accent-soft' : ''}"
      role="presentation"
      ondragover={(e) => target(e, 'end')}
      ondragleave={() => (over = null)}
      ondrop={(e) => {
        e.preventDefault()
        drop('end')
      }}
    ></div>
  </div>
</Section>

{#if picking}
  {@const slot = picking}
  {@const folder = $sidebarLayout[slot.zone][slot.index]}
  <IconPicker value={folder?.kind === 'folder' ? folder.icon : null} fallback="lucide:folder" onpick={(icon) => commit(putSidebarEntry($sidebarLayout, slot, { icon }))} onclose={() => (picking = null)} />
{/if}
