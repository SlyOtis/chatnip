<script lang="ts">
  import { anchored, boxOf } from '$lib/popover'
  import Icon from '@iconify/svelte'
  import { sortPreferenceStore, unreadOnlyStore, api, openNewChat } from '$lib/tauri'
  import type { SortPreference } from '$lib/tauri'
  import { GROUP_SORTS, groupDialog } from '$lib/tauri/groups'
  import { modalsShow } from '$lib/modals'
  import { IconButton, MenuItem } from '$lib/components/ui'
  import { activeFolder, activeGroupRecord, groupSort, openGroup, inboxView, sidebarFolder, VIEW_TITLES } from '$lib/inbox'
  import { activeInbox, inboxesStore } from '$lib/inboxes'
  import { sidebarLayout } from '$lib/layoutSettings'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  const sorts: { value: SortPreference; label: MessageKey }[] = [
    { value: 'recent', label: 'inbox.sort.recent' },
    { value: 'unread', label: 'inbox.sort.unread' },
    { value: 'alphabetical', label: 'inbox.sort.alphabetical' }
  ]
  const tabsVisible = $derived($inboxView === 'inbox' && !$activeGroupRecord)
  const currentInbox = $derived($inboxesStore.find((inbox) => inbox.id === $activeInbox) ?? null)
  const currentFolder = $derived($activeFolder ? sidebarFolder($sidebarLayout, $activeFolder) : null)
  let menu = $state(false)
  let filterButton = $state<HTMLDivElement | null>(null)
  let syncing = $state(false)
  async function syncNow() {
    syncing = true
    try {
      await api.postSync()
    } finally {
      syncing = false
    }
  }

</script>

<header data-testid="inbox-header" class="flex h-14 shrink-0 items-center gap-4 pl-6 pr-3 pt-1">
  <div class="relative flex min-w-0 flex-1 items-center gap-3">
    {#if tabsVisible}
      <span class="flex min-w-0 items-center gap-2 text-[15px] font-medium leading-7 tracking-tight text-ink">
        <Icon icon={currentInbox?.icon || 'lucide:inbox'} class="shrink-0 text-[15px] text-ink-2" />
        <span class="overflow-hidden text-ellipsis whitespace-nowrap">{currentFolder?.name ?? currentInbox?.name ?? t(VIEW_TITLES.inbox)}</span>
      </span>
    {:else if $activeGroupRecord}
      <button class="flex min-w-0 items-center gap-2 text-[15px] font-medium leading-7 tracking-tight text-ink" onclick={() => openGroup(null)} title={t('inbox.backTo', { view: t(VIEW_TITLES[$inboxView]) })}>
        <span class="overflow-hidden text-ellipsis whitespace-nowrap">{$activeGroupRecord.name}</span>
      </button>
    {:else}
      <span class="text-[15px] font-medium leading-7 tracking-tight text-ink">{t(VIEW_TITLES[$inboxView])}</span>
      {#if $inboxView === 'deleted'}
        <span class="font-mono text-[12px] text-ink-3">{t('inbox.restoreHint')}</span>
      {/if}
    {/if}
  </div>
  <div class="flex shrink-0 items-center gap-0.5 border-l border-line pl-3">
    <IconButton testid="inbox-new-chat" icon="lucide:messages-square" label={t('chats.new.title')} onclick={() => openNewChat()} />
    <IconButton testid="inbox-search" icon="lucide:search" label={t('common.search')} keys="⌘K" onclick={() => modalsShow({ query: '' })} />
    <div class="relative" bind:this={filterButton}>
      <IconButton testid="inbox-filter" icon="lucide:ellipsis" label={t('common.more')} active={menu || $unreadOnlyStore} onclick={() => (menu = !menu)} />
      {#if menu}
        <div class="menu w-60" use:anchored={{ at: () => boxOf(filterButton), onclose: () => (menu = false) }} data-testid="inbox-filter-menu" role="menu">
          <MenuItem testid="inbox-filter-unread" icon="lucide:mail-open" checked={$unreadOnlyStore} onclick={() => { unreadOnlyStore.update((v) => !v); menu = false }}>{t('inbox.unreadOnly')}</MenuItem>
          <div class="menu-sep"></div>
          {#if $activeGroupRecord && $activeGroupRecord.kind !== 'manual'}
            {#each GROUP_SORTS as sort (sort.value)}
              {@const on = $groupSort?.sort === sort.value}
              <MenuItem testid="inbox-group-sort-{sort.value}" checked={on} detail={on ? ($groupSort?.direction === 'asc' ? '↑' : '↓') : undefined} onclick={() => { groupSort.set({ sort: sort.value, direction: on && $groupSort?.direction === 'desc' ? 'asc' : 'desc' }); menu = false }}>{t(sort.label)}</MenuItem>
            {/each}
            <div class="menu-sep"></div>
          {/if}
          {#each sorts as sort (sort.value)}
            <MenuItem testid="inbox-sort-{sort.value}" checked={$sortPreferenceStore === sort.value} onclick={() => { sortPreferenceStore.set(sort.value); menu = false }}>{t(sort.label)}</MenuItem>
          {/each}
          <div class="menu-sep"></div>
          <MenuItem testid="inbox-sync" icon="lucide:refresh-cw" onclick={() => { syncNow(); menu = false }}>{syncing ? t('inbox.syncing') : t('inbox.syncNow')}</MenuItem>
          <MenuItem testid="inbox-new-group" icon="lucide:folder-tree" onclick={() => { groupDialog.set({ group: null, parentId: null, inbox: currentInbox && !currentInbox.is_default ? currentInbox : null }); menu = false }}>{t('inbox.newGroup')}</MenuItem>
          <MenuItem testid="inbox-browse-channels" icon="lucide:compass" onclick={() => { modalsShow({ browseChannels: true }); menu = false }}>{t('inbox.browseChannels')}</MenuItem>
        </div>
      {/if}
    </div>
  </div>
</header>

