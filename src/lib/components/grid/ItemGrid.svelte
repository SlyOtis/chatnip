<script lang="ts">
  import Icon from '@iconify/svelte'
  import { anchored, boxOf } from '$lib/popover'
  import { eventDate, gridObject, iconFor, opensInApp, tileOf, type GridBucket } from '$lib/grid'
  import { renderInline } from '$lib/content'
  import { usersStore } from '$lib/tauri/users'
  import type { ChatItem, GridItem, GridRows } from '$lib/tauri/types'
  import { Avatar, MenuItem, TagMark, TagPicker } from '$lib/components/ui'
  import { peopleStore } from '$lib/tauri/people'
  import { userAvatar } from '$lib/avatar'
  import { objectTagsStore, tagIdsOn, tagRecordsStore } from '$lib/tauri/tags'
  import type { Box } from '$lib/popover'
  import type { ObjectType, Tag } from '$lib/tauri/types'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import LinkPreview from '$lib/components/conversation/messages/LinkPreview.svelte'
  import RecordCard from '$lib/components/conversation/RecordCard.svelte'
  import { formatDate, formatNumber, t } from '$lib/i18n.svelte'

  type How = 'only' | 'toggle' | 'range'

  interface Props {
    items: GridItem[]
    groups: GridBucket[] | null
    rows: GridRows
    selected: Set<string>
    selectable?: boolean
    showTags?: boolean
    loading?: boolean
    showAdd?: boolean
    addLabel?: string
    addIcon?: string
    onopen: (item: GridItem) => void
    onselect: (item: GridItem, how: How) => void
    onquicklook: (item: GridItem) => void
    onshow?: (item: GridItem) => void
    onadd?: () => void
  }

  let { items, groups, rows, selected, selectable = true, showTags = true, loading = false, showAdd = false, addLabel, addIcon = 'lucide:plus', onopen, onselect, onquicklook, onshow, onadd }: Props = $props()

  let menuFor = $state<string | null>(null)
  let menuButton = $state<HTMLElement | null>(null)
  let tagging = $state<{ objectType: ObjectType; ids: string[]; at: Box | null } | null>(null)

  function marksOf(item: GridItem): Tag[] {
    const target = gridObject(item)
    if (!target) return []
    return tagIdsOn($objectTagsStore, target.objectType, target.objectId).map((id) => $tagRecordsStore.find((tag) => tag.id === id)).filter((tag): tag is Tag => !!tag)
  }

  function tagTargets(item: GridItem): { objectType: ObjectType; ids: string[] } | null {
    const target = gridObject(item)
    if (!target) return null
    const pool = selected.has(item.id) && selected.size > 1 ? items.filter((f) => selected.has(f.id)) : [item]
    const ids = pool.map(gridObject).filter((other) => other?.objectType === target.objectType).map((other) => other!.objectId)
    return { objectType: target.objectType, ids }
  }

  const menuItem = $derived(menuFor ? (items.find((f) => f.id === menuFor) ?? null) : null)
  const wide = new Set(['message', 'mail', 'link', 'record', 'event'])

  function how(e: MouseEvent | KeyboardEvent): How {
    if (e.shiftKey) return 'range'
    if (e.metaKey || e.ctrlKey) return 'toggle'
    return 'only'
  }

  function key(e: KeyboardEvent, item: GridItem) {
    if (e.key === ' ') {
      e.preventDefault()
      if (opensInApp(item)) onquicklook(item)
      else onopen(item)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      onopen(item)
    }
  }

  function title(bucket: GridBucket): string {
    return 'text' in bucket.title ? bucket.title.text : t(bucket.title.key)
  }

  function openMenu(e: MouseEvent, item: GridItem) {
    e.stopPropagation()
    menuButton = e.currentTarget as HTMLElement
    menuFor = menuFor === item.id ? null : item.id
  }

  function pick(action: () => void) {
    menuFor = null
    action()
  }

  function author(item: GridItem) {
    const user = item.sender_id ? $usersStore.data.find((u) => u.id === item.sender_id) : undefined
    const name = item.sender || user?.display_name || ''
    return { name, face: userAvatar(user ?? { id: item.sender_id ?? item.id, display_name: name, email: null, avatar_url: null }, $peopleStore, name) }
  }

  function asChatItem(item: GridItem): ChatItem {
    return { id: item.id, kind: 'record', message: null, record: item.record, text: null, title: item.name, conversation_id: item.conversation_id, conversation_name: item.conversation_name, conversation_type: '', action_id: null, at: item.at, reactions: 0 }
  }

  function provider(item: GridItem) {
    return item.provider_type ? [{ provider_type: item.provider_type, name: item.conversation_name, conversation_type: item.kind === 'mail' ? 'email' : 'direct_message' }] : []
  }
</script>

{#snippet body(item: GridItem, on: boolean)}
  {@const tile = tileOf(item)}
  {#if tile === 'message'}
    {@const who = author(item)}
    <span class="flex w-full min-w-0 items-center gap-1.5 text-[12px]">
      <Avatar source={who.face} size="xs" />
      <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-left font-medium text-ink">{who.name}</span>
      <ProviderIcon providers={provider(item) as never} size="xs" max={1} />
      <time class="shrink-0 font-mono text-[10.5px] text-ink-3" datetime={item.at}>{formatDate(item.at, { dateStyle: 'short' })}</time>
    </span>
    <span class="tile-clamp w-full text-left text-[12.5px] leading-[1.35] text-ink-2">{@html renderInline(item.excerpt || item.name)}</span>
    {#if item.reactions}<span class="flex items-center gap-1 self-start font-mono text-[10.5px] text-ink-3"><Icon icon="lucide:smile" class="text-[11px]" />{formatNumber(item.reactions)}</span>{/if}
  {:else if tile === 'mail'}
    <span class="flex w-full min-w-0 items-center gap-1.5 text-[12px] text-ink-3">
      <Icon icon="lucide:mail" class="shrink-0 text-[13px]" />
      <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-left">{item.sender}</span>
      <time class="shrink-0 font-mono text-[10.5px]" datetime={item.at}>{formatDate(item.at, { dateStyle: 'short' })}</time>
    </span>
    <span class="w-full overflow-hidden text-ellipsis whitespace-nowrap text-left text-[13px] font-semibold text-ink">{item.name}</span>
    <span class="tile-clamp w-full text-left text-[12px] leading-[1.35] text-ink-3">{item.excerpt}</span>
  {:else if tile === 'link' && item.url}
    <span class="tile-link w-full overflow-hidden text-left"><LinkPreview url={item.url} /></span>
  {:else if tile === 'record'}
    <span class="w-full overflow-hidden text-left"><RecordCard item={asChatItem(item)} compact /></span>
  {:else if tile === 'event'}
    {@const at = eventDate(item)}
    <span class="flex w-full min-w-0 items-center gap-2.5">
      <span class="flex aspect-square w-[3.25em] shrink-0 flex-col items-center justify-center rounded-[0.5rem] bg-accent-soft leading-none">
        <span class="font-mono text-[10px] uppercase text-accent">{formatDate(at, { month: 'short' })}</span>
        <span class="text-[18px] font-semibold text-ink">{formatDate(at, { day: 'numeric' })}</span>
      </span>
      <span class="flex min-w-0 flex-col text-left">
        <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[13px] font-semibold text-ink">{item.name}</span>
        <span class="font-mono text-[11px] text-ink-3">{formatDate(at, { weekday: 'short', hour: '2-digit', minute: '2-digit' })}</span>
        <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[11.5px] text-ink-3">{item.sender || item.conversation_name}</span>
      </span>
    </span>
  {:else}
    {@const thumb = tile === 'media' ? item.url : null}
    <span class="tile-icon relative flex items-center justify-center overflow-hidden rounded-[0.5rem] {on ? 'text-accent' : 'text-ink-2'} {tile === 'media' ? 'bg-paper-3' : ''}">
      {#if thumb && item.kind === 'video'}
        <video src={thumb} class="size-full object-cover" muted playsinline preload="metadata"></video>
      {:else if thumb}
        <img src={thumb} alt="" class="size-full object-cover" draggable="false" />
      {:else}
        <Icon icon={iconFor(item)} class="tile-glyph" />
      {/if}
    </span>
    <span class="tile-name w-full text-center text-[12px] font-medium leading-[1.3] text-ink">{item.name}</span>
  {/if}
{/snippet}

{#snippet tile(item: GridItem)}
  {@const on = selected.has(item.id)}
  <div data-testid="grid-tile" data-id={item.id} data-kind={item.kind} data-selected={on} class="grid-tile group/tile relative flex min-w-0 flex-col items-center gap-1.5 rounded-[0.625rem] px-2 pb-2 pt-3 {wide.has(item.kind) ? 'wide' : ''} {on ? 'bg-accent-soft' : 'hover:bg-paper-3'}">
    <button type="button" data-testid="grid-tile-open" class="flex w-full min-w-0 flex-col items-center gap-1.5 outline-none" aria-pressed={on} aria-label={item.name} title={item.name} onclick={(e) => onselect(item, how(e))} ondblclick={() => onopen(item)} onkeydown={(e) => key(e, item)}>
      {@render body(item, on)}
    </button>
    {#if marksOf(item).length}
      <span class="flex max-w-full items-center justify-center gap-1 overflow-hidden" data-tile-tags>
        {#each marksOf(item).slice(0, 4) as mark (mark.id)}<TagMark tag={mark} size="xs" />{/each}
      </span>
    {/if}
    {#if showTags && item.tags.length}
      <span class="flex max-w-full items-center justify-center gap-1 overflow-hidden">
        {#each item.tags.slice(0, 3) as tag (tag)}
          <span class="min-w-0 max-w-[6em] shrink overflow-hidden text-ellipsis whitespace-nowrap rounded-full bg-paper-3 px-1.5 font-mono text-[10px] leading-4 text-ink-2">{tag}</span>
        {/each}
        {#if item.tags.length > 3}
          <span class="shrink-0 rounded-full bg-accent-soft px-1.5 font-mono text-[10px] leading-4 text-ink">+{formatNumber(item.tags.length - 3)}</span>
        {/if}
      </span>
    {/if}
    {#if selectable}
      <button type="button" data-testid="grid-tile-select" class="absolute left-1.5 top-1.5 flex size-5 items-center justify-center rounded-full text-[13px] {on ? 'text-accent opacity-100' : 'text-ink-3 opacity-0 group-hover/tile:opacity-100 focus-visible:opacity-100'}" aria-label={on ? t('chats.grid.deselect') : t('chats.grid.select')} onclick={(e) => { e.stopPropagation(); onselect(item, 'toggle') }}>
        <Icon icon={on ? 'lucide:circle-check' : 'lucide:circle'} />
      </button>
    {/if}
    <button type="button" class="absolute right-1.5 top-1.5 flex size-5 items-center justify-center rounded-[0.375rem] text-[13px] text-ink-3 hover:text-ink {menuFor === item.id ? 'opacity-100' : 'opacity-0 group-hover/tile:opacity-100 focus-visible:opacity-100'}" aria-label={t('common.more')} onclick={(e) => openMenu(e, item)}>
      <Icon icon="lucide:ellipsis" />
    </button>
  </div>
{/snippet}

{#snippet add()}
  <button type="button" class="grid-tile flex min-w-0 flex-col items-center gap-1.5 rounded-[0.625rem] px-2 pb-2 pt-3 text-ink-3 hover:bg-paper-3 hover:text-ink" onclick={() => onadd?.()}>
    <span class="tile-icon flex items-center justify-center rounded-[0.5rem] border border-dashed border-line"><Icon icon={addIcon} class="tile-glyph" /></span>
    <span class="tile-name w-full text-center text-[12px] font-medium leading-[1.3]">{addLabel ?? t('common.add')}</span>
  </button>
{/snippet}

<div class="item-grid select-none">
  {#if showAdd && groups}
    <div class="grid-track wrap px-4 pt-3">{@render add()}</div>
  {/if}
  {#if loading && items.length === 0}
    <div class="grid-track wrap px-4 py-3" aria-busy="true">
      {#each Array.from({ length: 8 }, (_, i) => i) as i (i)}
        <div class="grid-tile flex flex-col items-center gap-1.5 px-2 pb-2 pt-3">
          <span class="tile-icon animate-pulse rounded-[0.5rem] bg-paper-3"></span>
          <span class="h-[0.75em] w-3/4 animate-pulse rounded-full bg-paper-3"></span>
        </div>
      {/each}
    </div>
  {:else if groups}
    {#each groups as bucket (bucket.key)}
      <section class="pb-1 pt-3">
        <h3 class="flex items-center gap-2 px-6 text-[13px] font-medium text-ink-2">
          <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{title(bucket)}</span>
          <span class="font-mono text-[11px] text-ink-3">{formatNumber(bucket.items.length)}</span>
        </h3>
        <div class="grid-track {rows} px-4 pt-1.5">
          {#each bucket.items as item (item.id)}
            {@render tile(item)}
          {/each}
        </div>
      </section>
    {/each}
  {:else}
    <div class="grid-track wrap px-4 py-3">
      {#if showAdd}{@render add()}{/if}
      {#each items as item (item.id)}
        {@render tile(item)}
      {/each}
    </div>
  {/if}
</div>

{#if menuItem}
  {@const on = selected.has(menuItem.id)}
  <div class="menu z-50 w-56" use:anchored={{ at: () => boxOf(menuButton), align: 'end', onclose: () => (menuFor = null) }} role="menu">
    <MenuItem icon="lucide:external-link" keys="↵" onclick={() => pick(() => onopen(menuItem))}>{opensInApp(menuItem) ? t('chats.grid.open') : t('chats.grid.showInChat')}</MenuItem>
    {#if opensInApp(menuItem)}
      <MenuItem icon="lucide:eye" keys="Space" onclick={() => pick(() => onquicklook(menuItem))}>{t('chats.grid.quickLook')}</MenuItem>
      {#if onshow}<MenuItem icon="lucide:message-square" onclick={() => pick(() => onshow?.(menuItem))}>{t('chats.grid.showInChat')}</MenuItem>{/if}
    {/if}
    {#if gridObject(menuItem)}
      {@const target = tagTargets(menuItem)}
      <MenuItem icon="lucide:tag" detail={marksOf(menuItem).length ? String(marksOf(menuItem).length) : undefined} onclick={() => { const at = boxOf(menuButton); pick(() => target && (tagging = { ...target, at })) }}>{t('conversation.menu.tags')}</MenuItem>
    {/if}
    {#if selectable}
      <MenuItem icon={on ? 'lucide:square' : 'lucide:square-check'} onclick={() => pick(() => onselect(menuItem, 'toggle'))}>{on ? t('chats.grid.deselect') : t('chats.grid.select')}</MenuItem>
    {/if}
  </div>
{/if}

{#if tagging}
  <TagPicker objectType={tagging.objectType} ids={tagging.ids} at={tagging.at} onclose={() => (tagging = null)} />
{/if}

<style>
  .item-grid {
    --grid-cell: 8.5em;
    font-size: 1rem;
  }
  .grid-track {
    display: grid;
    gap: 0.25rem;
  }
  .grid-track.wrap {
    grid-template-columns: repeat(auto-fill, minmax(var(--grid-cell), 1fr));
    grid-auto-flow: row dense;
  }
  .grid-track.scroll {
    grid-auto-flow: column;
    grid-auto-columns: var(--grid-cell);
    overflow-x: auto;
    overscroll-behavior-x: contain;
    scroll-snap-type: x proximity;
    padding-bottom: 0.5rem;
  }
  .grid-track > :global(.grid-tile.wide) {
    grid-column: span 2;
    align-items: stretch;
  }
  .grid-track.scroll > :global(.grid-tile) {
    scroll-snap-align: start;
  }
  .tile-icon {
    width: 56%;
    aspect-ratio: 1;
  }
  .tile-icon :global(.tile-glyph) {
    width: 72%;
    height: 72%;
  }
  .tile-name {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }
  .tile-clamp {
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }
  .tile-link {
    max-height: 9em;
  }
</style>
