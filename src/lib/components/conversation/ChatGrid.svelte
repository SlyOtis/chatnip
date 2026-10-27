<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import { isTyping } from '$lib/shortcuts'
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import { listen } from '@tauri-apps/api/event'
  import Icon from '@iconify/svelte'
  import { api, conversationsById, groupsStore, putChatViewSettings, tagRecordsStore, objectTagsStore, tagIdsOn } from '$lib/tauri'
  import type { Conversation, GridGroup, GridItem, GridShow, GridSort } from '$lib/tauri/types'
  import { anchored, boxOf } from '$lib/popover'
  import { showTooltip } from '$lib/tooltip'
  import { arrange, DEFAULT_GRID, GRID_GROUPS, GRID_SHOWS, GRID_SORTS, gridObject, opensInApp, selectItems, toggleShow, TOOLBAR, withGrid, type GridContext } from '$lib/grid'
  import { IconButton, MenuItem } from '$lib/components/ui'
  import ItemGrid from '$lib/components/grid/ItemGrid.svelte'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    conversation: Conversation
    onsources?: () => void
  }

  let { conversation, onsources }: Props = $props()

  type Menu = 'show' | 'group' | 'sort' | 'more'

  let items = $state<GridItem[]>([])
  let loading = $state(true)
  let failure = $state<string | null>(null)
  let query = $state('')
  let selected = $state(new Set<string>())
  let anchor = $state<string | null>(null)
  let menu = $state<Menu | null>(null)
  let menuAt = $state<HTMLElement | null>(null)

  const settings = $derived({ ...DEFAULT_GRID, ...(conversation.view_settings?.grid ?? {}) })
  const context = $derived<GridContext>({
    tagsOf: (item) => {
      const own = gridObject(item)
      const ids = [...($conversationsById.get(item.conversation_id)?.tags ?? []).map((link) => link.tag_id), ...(own ? tagIdsOn($objectTagsStore, own.objectType, own.objectId) : [])]
      return [...new Set(ids)].map((id) => $tagRecordsStore.find((tag) => tag.id === id)?.name ?? '').filter(Boolean)
    },
    groupsOf: (id) => $groupsStore.filter((g) => g.conversation_ids.includes(id)).map((g) => g.name)
  })
  const arranged = $derived(arrange(items, settings, query, new Date(), context))
  const order = $derived(arranged.groups ? arranged.groups.flatMap((g) => g.items.map((f) => f.id)) : arranged.flat.map((f) => f.id))
  const picked = $derived(arranged.flat.filter((f) => selected.has(f.id)))
  const group = $derived(GRID_GROUPS.find((g) => g.value === settings.group) ?? GRID_GROUPS[0])
  const sort = $derived(GRID_SORTS.find((s) => s.value === settings.sort) ?? GRID_SORTS[0])
  const showLabel = $derived(settings.show.length === 0 ? t('chats.grid.show.all') : settings.show.length === 1 ? t(GRID_SHOWS.find((s) => s.value === settings.show[0])!.label) : t('chats.grid.show.some', { count: settings.show.length }))
  const ascending = $derived(settings.direction === 'asc')

  async function load() {
    failure = null
    try {
      items = await api.getChatGrid(conversation.id)
      selected = new Set([...selected].filter((id) => items.some((f) => f.id === id)))
    } catch (error) {
      failure = String(error)
    } finally {
      loading = false
    }
  }

  $effect(() => {
    conversation.id
    loading = true
    items = []
    selected = new Set()
    load()
  })

  onMount(() => {
    const offs = [listen<string>('chat-items-updated', (e) => e.payload === conversation.id && load()), listen('messages-updated', () => load())]
    return () => offs.forEach((off) => off.then((stop) => stop()))
  })

  function save(patch: Parameters<typeof withGrid>[1]) {
    putChatViewSettings(conversation.id, withGrid(conversation.view_settings, patch)).catch((e) => (failure = String(e)))
  }

  function toggleMenu(kind: Menu, e: MouseEvent) {
    menuAt = e.currentTarget as HTMLElement
    menu = menu === kind ? null : kind
  }

  function tip(e: MouseEvent, text: string) {
    showTooltip(e.currentTarget as HTMLElement, text)
  }

  async function show(item: GridItem) {
    if (!item.conversation_id) return
    await api.putCurrentConversation(item.conversation_id).catch(() => {})
    goto(conversationHref(item.conversation_id))
  }

  async function open(item: GridItem, quick_look = false) {
    failure = null
    try {
      if (!opensInApp(item)) return await show(item)
      if (item.message_id && item.attachment_id) await api.postAttachmentOpen({ message_id: item.message_id, attachment_id: item.attachment_id, quick_look })
      else if (item.path) await api.postAttachmentOpen({ path: item.path, quick_look })
    } catch (error) {
      failure = String(error)
    }
  }

  function select(item: GridItem, how: 'only' | 'toggle' | 'range') {
    selected = selectItems(selected, item.id, how, order, anchor)
    if (how !== 'range') anchor = item.id
  }

  function keys(e: KeyboardEvent) {
    if (isTyping(e.target)) return
    if (e.key === 'Escape' && selected.size) {
      selected = new Set()
    } else if ((e.metaKey || e.ctrlKey) && e.key === 'a') {
      e.preventDefault()
      selected = new Set(order)
    }
  }

  const control = 'inline-flex h-7 shrink-0 items-center gap-1 rounded-[0.5rem] px-1.5 text-[12.5px] leading-none hover:bg-paper-3 hover:text-ink'
</script>

<svelte:window onkeydown={keys} />

{#snippet showItems()}
  <MenuItem testid="grid-show-all" icon="lucide:layout-grid" checked={settings.show.length === 0} onclick={() => save({ show: toggleShow(settings.show, 'all') })}>{t('chats.grid.show.all')}</MenuItem>
  {#each GRID_SHOWS as option (option.value)}
    <MenuItem testid={`grid-show-${option.value}`} icon={option.icon} checked={settings.show.includes(option.value)} onclick={() => save({ show: toggleShow(settings.show, option.value as GridShow) })}>{t(option.label)}</MenuItem>
  {/each}
{/snippet}

{#snippet groupItems()}
  {#each GRID_GROUPS as option (option.value)}
    <MenuItem testid={`grid-group-${option.value}`} icon={option.icon} checked={settings.group === option.value} onclick={() => { save({ group: option.value as GridGroup }); menu = null }}>{t(option.label)}</MenuItem>
  {/each}
{/snippet}

{#snippet sortItems()}
  {#each GRID_SORTS as option (option.value)}
    <MenuItem testid={`grid-sort-${option.value}`} checked={settings.sort === option.value} onclick={() => save({ sort: option.value as GridSort })}>{t(option.label)}</MenuItem>
  {/each}
  <div class="menu-sep"></div>
  <MenuItem icon="lucide:arrow-up-narrow-wide" checked={ascending} onclick={() => save({ direction: 'asc' })}>{t('chats.grid.ascending')}</MenuItem>
  <MenuItem icon="lucide:arrow-down-wide-narrow" checked={!ascending} onclick={() => save({ direction: 'desc' })}>{t('chats.grid.descending')}</MenuItem>
{/snippet}

<div class="absolute inset-0 flex min-h-0 flex-col" data-testid="chat-grid" data-group={settings.group} data-sort={settings.sort} data-rows={settings.rows} data-tags={settings.tags} data-show={settings.show.join(',')}>
  <div class="@container shrink-0 border-b border-line">
    <div class="flex min-w-0 items-center gap-2 px-6 py-1">
      <label class="flex min-w-0 flex-1 items-center gap-2 text-[13px]">
        <Icon icon="lucide:search" class="shrink-0 text-[14px] text-ink-3" />
        <input data-testid="grid-search" class="h-7 min-w-0 flex-1 bg-transparent text-ink outline-none placeholder:text-ink-3" placeholder={t('chats.grid.search')} aria-label={t('chats.grid.search')} bind:value={query} />
      </label>
      <div class="flex shrink-0 items-center gap-0.5 text-ink-3" role="toolbar" aria-label={t('chats.grid.controls')}>
        <span class="{TOOLBAR.control} items-center gap-0.5">
          <button type="button" data-testid="grid-show" class="{control} {settings.show.length ? 'text-accent' : ''}" aria-haspopup="menu" aria-expanded={menu === 'show'} aria-label={t('chats.grid.showLabel')} onmouseenter={(e) => tip(e, `${t('chats.grid.showLabel')} · ${showLabel}`)} onclick={(e) => toggleMenu('show', e)}>
            <Icon icon="lucide:list-filter" class="text-[14px]" /><span class={TOOLBAR.value}>{showLabel}</span>
          </button>
          <button type="button" data-testid="grid-group" class={control} aria-haspopup="menu" aria-expanded={menu === 'group'} aria-label={t('chats.grid.groupLabel')} onmouseenter={(e) => tip(e, `${t('chats.grid.groupLabel')} · ${t(group.label)}`)} onclick={(e) => toggleMenu('group', e)}>
            <Icon icon={group.icon} class="text-[14px]" /><span class={TOOLBAR.value}>{t(group.label)}</span>
          </button>
          <span class="inline-flex h-7 shrink-0 items-center rounded-[0.5rem] hover:bg-paper-3">
            <button type="button" data-testid="grid-sort" class="inline-flex h-7 items-center gap-1 rounded-l-[0.5rem] pl-1.5 pr-0.5 text-[12.5px] leading-none hover:text-ink" aria-haspopup="menu" aria-expanded={menu === 'sort'} aria-label={t('chats.grid.sortLabel')} onmouseenter={(e) => tip(e, `${t('chats.grid.sortLabel')} · ${t(sort.label)}`)} onclick={(e) => toggleMenu('sort', e)}>
              <Icon icon="lucide:arrow-up-down" class="text-[14px]" /><span class={TOOLBAR.value}>{t(sort.label)}</span>
            </button>
            <button type="button" data-testid="grid-direction" data-value={settings.direction} class="inline-flex h-7 items-center rounded-r-[0.5rem] pl-0.5 pr-1.5 hover:text-ink" aria-label={ascending ? t('chats.grid.ascending') : t('chats.grid.descending')} onmouseenter={(e) => tip(e, ascending ? t('chats.grid.ascending') : t('chats.grid.descending'))} onclick={() => save({ direction: ascending ? 'desc' : 'asc' })}>
              <Icon icon={ascending ? 'lucide:arrow-up' : 'lucide:arrow-down'} class="text-[13px]" />
            </button>
          </span>
          <span class="mx-1 h-4 w-px bg-line"></span>
          {#if settings.group !== 'none'}
            <button type="button" data-testid="grid-rows-scroll" class="{control} {settings.rows === 'scroll' ? 'text-accent' : ''}" aria-pressed={settings.rows === 'scroll'} aria-label={t('chats.grid.rowsScroll')} onmouseenter={(e) => tip(e, t('chats.grid.rowsScroll'))} onclick={() => save({ rows: 'scroll' })}><Icon icon="lucide:gallery-horizontal-end" class="text-[14px]" /></button>
            <button type="button" data-testid="grid-rows-wrap" class="{control} {settings.rows === 'wrap' ? 'text-accent' : ''}" aria-pressed={settings.rows === 'wrap'} aria-label={t('chats.grid.rowsWrap')} onmouseenter={(e) => tip(e, t('chats.grid.rowsWrap'))} onclick={() => save({ rows: 'wrap' })}><Icon icon="lucide:layout-grid" class="text-[14px]" /></button>
          {/if}
          <button type="button" data-testid="grid-tags" class="{control} {settings.tags ? 'text-accent' : ''}" aria-pressed={settings.tags} aria-label={settings.tags ? t('chats.grid.hideTags') : t('chats.grid.showTags')} onmouseenter={(e) => tip(e, settings.tags ? t('chats.grid.hideTags') : t('chats.grid.showTags'))} onclick={() => save({ tags: !settings.tags })}><Icon icon="lucide:tag" class="text-[14px]" /></button>
        </span>
        <button type="button" data-testid="grid-more" class="{TOOLBAR.more} {control}" aria-haspopup="menu" aria-expanded={menu === 'more'} aria-label={t('common.more')} onmouseenter={(e) => tip(e, t('chats.grid.controls'))} onclick={(e) => toggleMenu('more', e)}><Icon icon="lucide:ellipsis" class="text-[14px]" /></button>
      </div>
    </div>
  </div>

  {#if menu}
    <div class="menu max-h-[70vh] w-60 overflow-y-auto" use:anchored={{ at: () => boxOf(menuAt), align: 'end', onclose: () => (menu = null) }} role="menu" data-testid="grid-menu" data-menu={menu}>
      {#if menu === 'show'}
        {@render showItems()}
      {:else if menu === 'group'}
        {@render groupItems()}
      {:else if menu === 'sort'}
        {@render sortItems()}
      {:else}
        <p class="menu-label">{t('chats.grid.showLabel')}</p>
        {@render showItems()}
        <div class="menu-sep"></div>
        <p class="menu-label">{t('chats.grid.groupLabel')}</p>
        {@render groupItems()}
        <div class="menu-sep"></div>
        <p class="menu-label">{t('chats.grid.sortLabel')}</p>
        {@render sortItems()}
        <div class="menu-sep"></div>
        {#if settings.group !== 'none'}
          <MenuItem icon="lucide:gallery-horizontal-end" checked={settings.rows === 'scroll'} onclick={() => save({ rows: 'scroll' })}>{t('chats.grid.rowsScroll')}</MenuItem>
          <MenuItem icon="lucide:layout-grid" checked={settings.rows === 'wrap'} onclick={() => save({ rows: 'wrap' })}>{t('chats.grid.rowsWrap')}</MenuItem>
        {/if}
        <MenuItem icon="lucide:tag" checked={settings.tags} onclick={() => save({ tags: !settings.tags })}>{t('chats.grid.showTags')}</MenuItem>
      {/if}
    </div>
  {/if}

  {#if picked.length}
    <div class="flex shrink-0 items-center gap-2 border-b border-line bg-accent-soft px-6 py-1 text-[12.5px] text-ink">
      <span>{t('chats.grid.selected', { count: picked.length })}</span>
      <span class="ml-auto flex items-center gap-0.5">
        <IconButton testid="grid-open" size="sm" icon="lucide:external-link" label={t('chats.grid.open')} keys="↵" onclick={() => picked.forEach((f) => open(f))} />
        <IconButton testid="grid-quick-look" size="sm" icon="lucide:eye" label={t('chats.grid.quickLook')} keys="Space" onclick={() => picked[0] && open(picked[0], true)} />
        <IconButton testid="grid-clear" size="sm" icon="lucide:x" label={t('chats.grid.clearSelection')} keys="Esc" onclick={() => (selected = new Set())} />
      </span>
    </div>
  {/if}

  <div class="relative min-h-0 flex-1 overflow-y-auto overflow-x-hidden pb-6">
    {#if failure}
      <p class="px-6 py-3 text-[13px] text-error-600">{failure}</p>
    {/if}
    {#if !loading && items.length === 0}
      <div class="flex flex-col items-center gap-1 py-16 text-center">
        <Icon icon="lucide:layout-grid" class="text-[22px] text-ink-3" />
        <p class="text-[15px] font-semibold text-ink">{t('chats.grid.emptyTitle')}</p>
        <p class="max-w-[28rem] text-[13px] text-ink-3">{t('chats.grid.emptyHint')}</p>
      </div>
    {:else if !loading && arranged.flat.length === 0}
      <p class="py-12 text-center text-[13px] text-ink-3">{query ? t('chats.grid.noMatch', { query }) : t('chats.grid.nothingShown')}</p>
    {:else}
      <ItemGrid
        items={arranged.flat}
        groups={arranged.groups}
        rows={arranged.rows}
        {selected}
        {loading}
        showTags={settings.tags}
        showAdd={!!onsources}
        addLabel={t('chats.grid.addSource')}
        addIcon="lucide:folder-input"
        onopen={(f) => open(f)}
        onquicklook={(f) => open(f, true)}
        onselect={select}
        onshow={show}
        onadd={onsources}
      />
    {/if}
  </div>
  <p data-testid="grid-count" data-count={arranged.flat.length} class="shrink-0 border-t border-line px-6 py-1 font-mono text-[11px] text-ink-3">{t('chats.grid.count', { count: arranged.flat.length })}</p>
</div>
