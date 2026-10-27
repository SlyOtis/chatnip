<script lang="ts">
  import Icon from '@iconify/svelte'
  import { onMount } from 'svelte'
  import { api } from '$lib/tauri'
  import type { ContentGroup, ContentItem, GifCursor, GifResult, PutContentInput } from '$lib/tauri/types'
  import { GIF_CATEGORIES, appendResults, contentSrc, favouriteInput, favouriteItems, gifSrc, isVideo, isVisual, mostUsed, parseTags, recentlyUsed, resultKey, savedUrls } from '$lib/media'
  import { reorder } from '$lib/inbox'
  import { Field, Kbd, MenuItem, Sheet } from '$lib/components/ui'
  import { anchored, type Align, type Box, type Side } from '$lib/popover'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    onpick: (item: ContentItem) => void
    onclose: () => void
    at?: Box | (() => Box | null) | null
    side?: Side
    align?: Align
  }

  interface Menu {
    item: ContentItem
    x: number
    y: number
    edit: 'name' | 'tags' | null
    text: string
  }

  let { onpick, onclose, at = null, side = 'below', align = 'end' }: Props = $props()

  let tab = $state('mine')
  let tagFilter = $state<string | null>(null)
  let yours = $state<ContentItem[]>([])
  let query = $state('')
  let results = $state<GifResult[]>([])
  let next = $state<GifCursor | null>(null)
  let notice = $state<string | null>(null)
  let loading = $state(false)
  let loadingMore = $state(false)
  let failure = $state<string | null>(null)
  let library = $state<ContentItem[]>([])
  let groups = $state<ContentGroup[]>([])
  let menu = $state<Menu | null>(null)
  let dragFrom = $state<number | null>(null)
  let dragOver = $state<number | null>(null)
  let starring = $state<string[]>([])
  let picker = $state<HTMLDivElement>()
  let timer: ReturnType<typeof setTimeout> | undefined
  let pressTimer: ReturnType<typeof setTimeout> | undefined
  let pressed = false
  let request = 0

  const searching = $derived(query.trim().length > 0)
  const feed = $derived(searching || tab === 'trending')
  const noticeText = $derived(notice && /key/i.test(notice) ? t('conversation.gif.noKey') : notice)
  const saved = $derived(savedUrls(library))
  const favourites = $derived(favouriteItems(library))
  const recent = $derived([...library].filter((item) => item.last_used_at).sort((a, b) => (b.last_used_at ?? '').localeCompare(a.last_used_at ?? '')))
  const mine = $derived.by(() => {
    const seen = new Set<string>()
    return [...mostUsed(library, 60), ...recentlyUsed(library, 60), ...library].filter((item) => !seen.has(item.id) && (seen.add(item.id), true)).filter((item) => !tagFilter || item.tags.includes(tagFilter))
  })
  const topTags = $derived.by(() => {
    const counts = new Map<string, number>()
    for (const item of library) for (const tag of item.tags) counts.set(tag, (counts.get(tag) ?? 0) + 1)
    return [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 12).map(([name]) => name)
  })
  const webResults = $derived(results.filter((result) => result.source !== 'library'))
  const shelf = $derived(tab === 'mine' ? mine : tab === 'favourites' ? favourites : tab === 'recent' ? recent : tab.startsWith('g:') ? favouriteItems(library, groups.find((group) => group.id === tab.slice(2))) : [])
  const tabs = $derived([{ value: 'mine', label: t('conversation.gif.mine') }, { value: 'favourites', label: t('conversation.gif.favourites') }, ...groups.map((group) => ({ value: `g:${group.id}`, label: group.name })), { value: 'trending', label: t('conversation.gif.trending') }, { value: 'recent', label: t('conversation.gif.recent') }])

  function libraryItem(result: GifResult): ContentItem | undefined {
    return saved.get(result.url) ?? (result.content_id ? library.find((item) => item.id === result.content_id) : undefined)
  }

  function columns<T>(items: T[], count: number): T[][] {
    const out: T[][] = Array.from({ length: count }, () => [])
    items.forEach((item, index) => out[index % count].push(item))
    return out
  }

  function merge(item: ContentItem) {
    library = [item, ...library.filter((entry) => entry.id !== item.id)]
    if (menu?.item.id === item.id) menu = { ...menu, item }
  }

  async function load(more = false) {
    const token = ++request
    if (more) {
      loadingMore = true
    } else {
      loading = true
      results = []
      next = null
    }
    failure = null
    try {
      if (!more) api.getContent(undefined, query.trim(), 'relevance').then((items) => token === request && (yours = items.filter((item) => item.kind !== 'emoji' && isVisual(item)))).catch(() => (yours = []))
      const page = await api.getGifs(query.trim(), 30, more ? next : null)
      if (token !== request) return
      results = appendResults(more ? results : [], page.results)
      next = page.next
      notice = page.notice
    } catch (error) {
      if (token !== request) return
      const message = String(error)
      if (/key/i.test(message)) notice = message
      else failure = message
    } finally {
      if (token === request) {
        loading = false
        loadingMore = false
      }
    }
  }

  function handleInput() {
    menu = null
    clearTimeout(timer)
    if (!query.trim()) {
      request += 1
      results = []
      yours = []
      if (tab === 'trending') load()
      return
    }
    timer = setTimeout(() => load(), 300)
  }

  function chooseCategory(name: string) {
    query = name
    clearTimeout(timer)
    load()
  }

  function chooseTab(value: string) {
    tab = value
    tagFilter = null
    query = ''
    menu = null
    if (value === 'trending') load()
  }

  function handleScroll(e: Event) {
    const el = e.currentTarget as HTMLElement
    if (feed && next && !loading && !loadingMore && el.scrollTop + el.clientHeight > el.scrollHeight - 160) load(true)
  }

  async function pickResult(result: GifResult) {
    try {
      const known = result.content_id ? (library.find((item) => item.id === result.content_id) ?? (await api.getContent()).find((item) => item.id === result.content_id)) : undefined
      const item = known ?? (await api.postContent(favouriteInput(result, query, false)))
      if (item.kind !== 'emoji') merge(item)
      onpick(item)
    } catch (error) {
      failure = String(error)
    }
  }

  async function star(result: GifResult, e: MouseEvent) {
    e.stopPropagation()
    const key = resultKey(result)
    if (starring.includes(key)) return
    starring = [...starring, key]
    const known = libraryItem(result)
    try {
      if (known?.is_favourite) merge(await api.putContent({ content_id: known.id, is_favourite: false }))
      else if (known) merge(await api.putContent({ content_id: known.id, is_favourite: true, tags: parseTags([...known.tags, query].join(',')) }))
      else merge(await api.postContent(favouriteInput(result, query, true)))
    } catch (error) {
      failure = String(error)
    } finally {
      starring = starring.filter((entry) => entry !== key)
    }
  }

  function pickItem(item: ContentItem) {
    if (pressed) {
      pressed = false
      return
    }
    onpick(item)
  }

  function openMenu(item: ContentItem, e: MouseEvent) {
    e.preventDefault()
    const box = picker?.getBoundingClientRect()
    if (!box) return
    menu = { item, x: Math.min(e.clientX - box.left, box.width - 224), y: Math.min(e.clientY - box.top, box.height - 120), edit: null, text: '' }
  }

  function pressStart(item: ContentItem, e: PointerEvent) {
    pressed = false
    clearTimeout(pressTimer)
    pressTimer = setTimeout(() => {
      pressed = true
      openMenu(item, e)
    }, 500)
  }

  function pressEnd() {
    clearTimeout(pressTimer)
  }

  async function update(input: PutContentInput) {
    try {
      merge(await api.putContent(input))
    } catch (error) {
      failure = String(error)
    }
  }

  async function toggleGroup(item: ContentItem, group: ContentGroup) {
    const inside = item.group_ids.includes(group.id)
    try {
      const input = { content_group_id: group.id, content_ids: [item.id] }
      const changed = inside ? await api.deleteContentGroupItems(input) : await api.putContentGroupItems(input)
      groups = groups.map((entry) => (entry.id === changed.id ? changed : entry))
      merge({ ...item, group_ids: inside ? item.group_ids.filter((id) => id !== group.id) : [...item.group_ids, group.id] })
    } catch (error) {
      failure = String(error)
    }
  }

  async function saveEdit() {
    if (!menu?.edit) return
    const { item, edit, text } = menu
    if (edit === 'name' && text.trim()) await update({ content_id: item.id, name: text.trim() })
    if (edit === 'tags') await update({ content_id: item.id, tags: parseTags(text) })
    menu = null
  }

  async function remove(item: ContentItem) {
    menu = null
    try {
      await api.deleteContent(item.id)
      library = library.filter((entry) => entry.id !== item.id)
    } catch (error) {
      failure = String(error)
    }
  }

  async function drop(to: number) {
    const from = dragFrom
    dragFrom = null
    dragOver = null
    if (from === null || from === to) return
    const ordered = reorder(favourites, from, to)
    const positions = new Map(ordered.map((item, index) => [item.id, index]))
    library = library.map((item) => (positions.has(item.id) ? { ...item, favourite_position: positions.get(item.id) ?? 0 } : item))
    try {
      await api.putContentOrder(ordered.map((item) => item.id))
    } catch (error) {
      failure = String(error)
    }
  }

  onMount(() => {
    Promise.all([api.getContent(), api.getAppSettings(), api.getContentGroups()])
      .then(([items, settings, list]) => {
        library = items.filter((item) => item.kind !== 'emoji' && isVisual(item))
        groups = list.filter((group) => group.kind === 'manual' && !group.parent_id)
        const preferred = settings['gif.default_tab'] || 'mine'
        const empty = preferred === 'favourites' ? !items.some((item) => item.is_favourite) : preferred === 'mine' ? library.length === 0 : false
        chooseTab(empty ? 'trending' : preferred)
      })
      .catch((error) => (failure = String(error)))
    return () => {
      clearTimeout(timer)
      clearTimeout(pressTimer)
    }
  })
</script>

<div bind:this={picker} class="relative w-[26rem] max-w-[90vw]" use:anchored={{ at, side, align, onclose }}>
  <Sheet label={t('content.title')} width="100%" anchored onclose={() => (menu ? (menu = null) : onclose())}>
    <div class="flex flex-col gap-2 px-2 pt-2">
      <Field bind:value={query} size="sm" placeholder={t('content.picker.search')} oninput={handleInput} autofocus />
      <div class="flex items-center gap-3 overflow-x-auto px-1 text-[13px] whitespace-nowrap" role="tablist" aria-label={t('conversation.gif.shelves')}>
        {#each tabs as option (option.value)}
          <button type="button" role="tab" aria-selected={!searching && tab === option.value} class="shrink-0 font-medium {!searching && tab === option.value ? 'text-ink' : 'text-ink-3 hover:text-ink'}" onclick={() => chooseTab(option.value)}>{option.label}</button>
        {/each}
      </div>
      <div class="flex flex-wrap gap-1 px-0.5">
        {#if !searching && tab === 'mine'}
          {#each topTags as tag (tag)}
            <button type="button" class="h-6 px-1.5 text-[12px] {tagFilter === tag ? 'bg-accent-soft text-ink' : 'text-ink-3 hover:text-ink'}" aria-pressed={tagFilter === tag} onclick={() => (tagFilter = tagFilter === tag ? null : tag)}>#{tag}</button>
          {/each}
        {/if}
        {#each GIF_CATEGORIES as category (category)}
          <button type="button" class="h-6 rounded-[0.375rem] px-1.5 text-[12px] hairline {query.trim().toLowerCase() === category ? 'text-ink' : 'text-ink-3 hover:text-ink'}" onclick={() => chooseCategory(category)}>{category}</button>
        {/each}
      </div>
    </div>

    <div class="max-h-[22rem] min-h-[12rem] overflow-y-auto p-2" onscroll={handleScroll}>
      {#if failure}
        <p class="px-2 py-1 text-[12px] text-error-500">{failure}</p>
      {/if}
      {#if feed}
        {#if searching && yours.length}
          <p class="px-1 pb-1 text-[12px] font-medium text-ink-3">{t('conversation.gif.yours')}</p>
          <div class="mb-3 grid grid-cols-3 gap-1.5">
            {#each yours.slice(0, 9) as item (item.id)}
              <button type="button" class="relative aspect-square overflow-hidden rounded-[0.5rem] hairline" onclick={() => pickItem(item)} title={item.name}>
                {#if isVideo(item)}
                  <video src={contentSrc(item)} class="size-full object-cover" muted loop autoplay playsinline></video>
                {:else}
                  <img src={contentSrc(item)} alt={item.name} class="size-full object-cover" loading="lazy" />
                {/if}
              </button>
            {/each}
          </div>
          <p class="px-1 pb-1 text-[12px] font-medium text-ink-3">{t('conversation.gif.web')}</p>
        {/if}
        {#if notice}
          <p class="px-2 pb-2 text-[12px] text-ink-3">{noticeText}</p>
        {/if}
        {#if loading && results.length === 0}
          <div class="grid grid-cols-3 gap-1.5">
            {#each Array(9) as _, i (i)}
              <div class="aspect-square rounded-[0.5rem] bg-paper-2"></div>
            {/each}
          </div>
        {:else if results.length === 0}
          <p class="px-6 py-10 text-center text-[13px] text-ink-3">{searching ? t('conversation.gif.nothingFound') : t('conversation.gif.trendingNeedsKey')}</p>
        {:else}
          <div class="grid grid-cols-3 gap-1.5">
            {#each columns(searching && yours.length ? webResults : results, 3) as column, c (c)}
              <div class="flex flex-col gap-1.5">
                {#each column as result (resultKey(result))}
                  {@const known = libraryItem(result)}
                  <div class="group relative overflow-hidden rounded-[0.5rem] hairline {result.kind === 'sticker' ? 'bg-paper-2' : ''}">
                    <button type="button" class="block w-full hover:opacity-90" onclick={() => pickResult(result)} title={result.title}>
                      <img src={gifSrc(result)} alt={result.title} class="w-full" style:aspect-ratio={result.width && result.height ? `${result.width} / ${result.height}` : undefined} loading="lazy" />
                    </button>
                    <button
                      type="button"
                      class="absolute top-1 right-1 size-6 items-center justify-center rounded-full bg-paper/90 {known?.is_favourite ? 'flex text-signal' : 'hidden text-ink-3 group-hover:flex hover:text-signal'}"
                      onclick={(e) => star(result, e)}
                      aria-label={known?.is_favourite ? t('conversation.gif.unfavourite') : t('conversation.gif.favourite')}
                      aria-busy={starring.includes(resultKey(result))}
                    >
                      <Icon icon={starring.includes(resultKey(result)) ? 'lucide:loader-circle' : 'lucide:star'} class="text-[13px] {known?.is_favourite ? 'fill-current' : ''} {starring.includes(resultKey(result)) ? 'animate-spin' : ''}" />
                    </button>
                    {#if result.source === 'library'}
                      <span class="absolute bottom-1 left-1 rounded-[0.25rem] bg-paper/90 px-1 font-mono text-[10px] text-ink-3">{t('conversation.gif.saved')}</span>
                    {/if}
                  </div>
                {/each}
              </div>
            {/each}
          </div>
          {#if loadingMore}
            <p class="py-3 text-center text-[12px] text-ink-3">{t('conversation.gif.loadingMore')}</p>
          {/if}
        {/if}
      {:else if shelf.length === 0}
        <p class="px-6 py-10 text-center text-[13px] text-ink-3">
          {tab === 'mine' ? t('content.picker.emptyMine') : tab === 'favourites' ? t('conversation.gif.emptyFavourites') : tab === 'recent' ? t('conversation.gif.emptyRecent') : t('conversation.gif.emptyCollection')}
        </p>
      {:else}
        <div class="grid grid-cols-3 gap-1.5">
          {#each shelf as item, index (item.id)}
            <div
              class="group relative aspect-square overflow-hidden rounded-[0.5rem] hairline {dragOver === index && dragFrom !== index ? 'ring-2 ring-accent' : ''} {dragFrom === index ? 'opacity-40' : ''}"
              draggable={tab === 'favourites'}
              role="listitem"
              ondragstart={(e) => {
                dragFrom = index
                e.dataTransfer?.setData('text/plain', item.id)
              }}
              ondragover={(e) => {
                if (dragFrom === null) return
                e.preventDefault()
                dragOver = index
              }}
              ondragleave={() => (dragOver = null)}
              ondrop={(e) => {
                e.preventDefault()
                drop(index)
              }}
              ondragend={() => {
                dragFrom = null
                dragOver = null
              }}
              oncontextmenu={(e) => openMenu(item, e)}
            >
              <button type="button" class="block size-full" onclick={() => pickItem(item)} onpointerdown={(e) => pressStart(item, e)} onpointerup={pressEnd} onpointerleave={pressEnd} title={item.name}>
                {#if isVideo(item)}
                  <video src={contentSrc(item)} class="pointer-events-none size-full object-cover" muted loop autoplay playsinline></video>
                {:else}
                  <img src={contentSrc(item)} alt={item.name} class="size-full object-cover" loading="lazy" draggable="false" />
                {/if}
              </button>
              <button
                type="button"
                class="absolute top-1 right-1 size-6 items-center justify-center rounded-full bg-paper/90 {item.is_favourite ? 'text-signal' : 'text-ink-3'} hidden group-hover:flex"
                onclick={(e) => {
                  e.stopPropagation()
                  update({ content_id: item.id, is_favourite: !item.is_favourite })
                }}
                aria-label={item.is_favourite ? t('conversation.gif.unfavourite') : t('conversation.gif.favourite')}
              >
                <Icon icon="lucide:star" class="text-[13px] {item.is_favourite ? 'fill-current' : ''}" />
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </div>
    {#snippet footer()}
      <span class="text-[12px] text-ink-3">{tab === 'favourites' && !searching ? t('conversation.gif.helpSort') : t('conversation.gif.help')}</span>
      <Kbd keys="esc" class="ml-auto" />
    {/snippet}
  </Sheet>

  {#if menu}
    <div class="rise absolute z-50 flex w-56 flex-col gap-0.5 rounded-[0.625rem] p-1 glass" style:left="{Math.max(0, menu.x)}px" style:top="{Math.max(0, menu.y)}px" role="menu" aria-label={t('conversation.gif.edit', { name: menu.item.name })}>
      {#if menu.edit}
        <form class="p-1" onsubmit={(e) => { e.preventDefault(); saveEdit() }}>
          <Field bind:value={menu.text} size="sm" placeholder={menu.edit === 'name' ? t('common.name') : t('conversation.gif.tagsPlaceholder')} autofocus onkeydown={(e) => e.key === 'Escape' && (menu = null)} />
        </form>
      {:else}
        <MenuItem icon="lucide:pencil" onclick={() => menu && (menu = { ...menu, edit: 'name', text: menu.item.name })}>{t('common.rename')}</MenuItem>
        <MenuItem icon="lucide:tags" detail={menu.item.tags.join(', ')} onclick={() => menu && (menu = { ...menu, edit: 'tags', text: menu.item.tags.join(', ') })}>{t('conversation.gif.tags')}</MenuItem>
        <MenuItem icon="lucide:star" checked={menu.item.is_favourite} onclick={() => menu && update({ content_id: menu.item.id, is_favourite: !menu.item.is_favourite })}>{t('conversation.gif.favouriteItem')}</MenuItem>
        {#each groups as group (group.id)}
          <MenuItem icon={group.icon || 'lucide:folder'} checked={menu.item.group_ids.includes(group.id)} onclick={() => menu && toggleGroup(menu.item, group)}>{group.name}</MenuItem>
        {/each}
        <div class="my-0.5 border-t border-line"></div>
        <MenuItem icon="lucide:trash-2" danger onclick={() => menu && remove(menu.item)}>{t('conversation.gif.removeLibrary')}</MenuItem>
      {/if}
    </div>
  {/if}
</div>
