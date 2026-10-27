<script lang="ts">
  import { isTyping } from '$lib/shortcuts'
  import { onMount } from 'svelte'
  import Icon from '@iconify/svelte'
  import { api, providersStore } from '$lib/tauri'
  import type { ContentGroup, ContentItem, ContentSort, ContentTag, ContentTemplateKind, GifResult } from '$lib/tauri/types'
  import { CONTENT_KINDS, TEMPLATE_KINDS, contentSections, gifSrc, isVisual, kindIcon, placementIndex, readFileAsBase64, type ContentSection } from '$lib/media'
  import { Button, Field, IconButton, MenuItem, PaneGrid, Segmented, Select, Sheet, SplitButton } from '$lib/components/ui'
  import ContentTile from '$lib/components/content/ContentTile.svelte'
  import ContentDetail from '$lib/components/content/ContentDetail.svelte'
  import GroupEditor from '$lib/components/settings/GroupEditor.svelte'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  const SORTS: { value: ContentSort; label: MessageKey }[] = [
    { value: 'relevance', label: 'content.sort.best' },
    { value: 'used', label: 'content.sort.used' },
    { value: 'recent', label: 'content.sort.recent' },
    { value: 'newest', label: 'content.sort.newest' }
  ]
  const NEW_NAMES: Record<ContentTemplateKind, MessageKey> = { reply: 'content.new.reply', email: 'content.new.email', prompt: 'content.new.prompt', snippet: 'content.new.snippet', signature: 'content.new.signature' }
  const CREATE_LABELS: Record<ContentTemplateKind, MessageKey> = { reply: 'content.create.reply', email: 'content.create.email', prompt: 'content.create.prompt', snippet: 'content.create.snippet', signature: 'content.create.signature' }
  const MEDIA_TYPE = /^(image|video)\//
  const DRAG_TYPE = 'application/x-chatnip-content'

  let library = $state<ContentItem[]>([])
  let ranked = $state<ContentItem[]>([])
  let remote = $state<GifResult[]>([])
  let groups = $state<ContentGroup[]>([])
  let tags = $state<ContentTag[]>([])
  let query = $state('')
  let sort = $state<ContentSort>('used')
  let favouritesOnly = $state(false)
  let importing = $state(false)
  let importUrl = $state('')
  let importBusy = $state(false)
  let selected = $state<string[]>([])
  let openId = $state<string | null>(null)
  let dropping = $state(false)
  let uploading = $state(0)
  let notice = $state<string | null>(null)
  let failure = $state<string | null>(null)
  let autoplay = $state(true)
  let bulkTag = $state('')
  let bulkGroup = $state('')
  let savingRemote = $state<string[]>([])
  let fileInput = $state<HTMLInputElement>()
  let editing = $state<{ group: ContentGroup | null; parentId: string | null } | null>(null)
  let dragging = $state<{ ids: string[]; from: string | null } | { group: string } | null>(null)
  let dropAt = $state<string | null>(null)
  let timer: ReturnType<typeof setTimeout> | undefined
  let request = 0

  const searching = $derived(query.trim().length > 0)
  const sortOptions = $derived((searching ? SORTS : SORTS.filter((option) => option.value !== 'relevance')).map((option) => ({ value: option.value, label: t(option.label) })))
  const shown = $derived((searching ? ranked : library).filter((item) => !favouritesOnly || item.is_favourite))
  const sections = $derived(searching ? [] : contentSections(groups, shown))
  const manualGroups = $derived(groups.filter((group) => group.kind === 'manual'))
  const open = $derived(library.find((item) => item.id === openId) ?? null)
  const savedUrls = $derived(new Set(library.map((item) => item.source_url).filter(Boolean)))
  const remoteVisible = $derived(remote.filter((result) => result.source !== 'library' && !savedUrls.has(result.url)))

  async function loadLibrary() {
    try {
      const [items, tagList, groupList] = await Promise.all([api.getContent(undefined, undefined, sort), api.getContentTags(), api.getContentGroups()])
      library = items.filter((item) => item.kind !== 'emoji')
      tags = tagList
      groups = groupList
      if (!searching) ranked = library
    } catch (error) {
      failure = String(error)
    }
  }

  const signatureAccounts = $derived(($providersStore.data ?? []).filter((provider) => provider.is_connected && provider.capabilities.signature_sync))

  async function importSignatures(provider_id: string) {
    importBusy = true
    try {
      await api.postSignatureImport(provider_id)
      await loadLibrary()
      failure = null
    } catch (error) {
      failure = String(error)
    } finally {
      importBusy = false
    }
  }

  async function loadGroups() {
    groups = await api.getContentGroups().catch((error) => {
      failure = String(error)
      return groups
    })
  }

  async function search() {
    const token = ++request
    const q = query.trim()
    if (!q && sort === 'relevance') sort = 'used'
    try {
      const items = await api.getContent(undefined, q || undefined, q ? (sort === 'used' ? 'relevance' : sort) : sort)
      if (token !== request) return
      if (q) ranked = items.filter((item) => item.kind !== 'emoji')
      else library = items.filter((item) => item.kind !== 'emoji')
    } catch (error) {
      failure = String(error)
    }
    if (!q) {
      remote = []
      notice = null
      return
    }
    try {
      const page = await api.getGifs(q, 30)
      if (token !== request) return
      remote = page.results
      notice = page.notice && /key/i.test(page.notice) ? t('content.web.noKey') : page.notice
    } catch (error) {
      if (token !== request) return
      remote = []
      notice = /key/i.test(String(error)) ? t('content.web.noKey') : String(error)
    }
  }

  function handleInput() {
    clearTimeout(timer)
    timer = setTimeout(search, 250)
  }

  function chooseSort(value: string) {
    sort = value as ContentSort
    search()
  }

  function merge(items: ContentItem[]) {
    const byId = new Map(items.map((item) => [item.id, item]))
    const known = new Set(library.map((item) => item.id))
    library = [...items.filter((item) => !known.has(item.id)), ...library.map((item) => byId.get(item.id) ?? item)]
    ranked = ranked.map((item) => byId.get(item.id) ?? item)
  }

  async function refreshTags() {
    tags = await api.getContentTags().catch(() => tags)
  }

  async function importFromUrl() {
    const url = importUrl.trim()
    if (!/^https?:\/\//i.test(url)) return
    importBusy = true
    try {
      merge([await api.postContentFromUrl(url)])
      importing = false
      importUrl = ''
    } catch (error) {
      failure = String(error)
    } finally {
      importBusy = false
    }
  }

  async function uploadFiles(files: File[]) {
    uploading += files.length
    for (const file of files) {
      try {
        const media = MEDIA_TYPE.test(file.type)
        const data = await readFileAsBase64(file)
        merge([await api.postContent({ kind: 'auto', name: media ? file.name.replace(/\.[^.]+$/, '') : file.name, data, mime_type: file.type || undefined, source_kind: 'upload' })])
      } catch (error) {
        failure = String(error)
      } finally {
        uploading -= 1
      }
    }
    refreshTags()
  }

  async function createTemplate(kind: ContentTemplateKind) {
    try {
      const item = await api.postContent({ kind, name: t(NEW_NAMES[kind]), body: '', source_kind: 'manual' })
      merge([item])
      openId = item.id
    } catch (error) {
      failure = String(error)
    }
  }

  async function saveRemote(result: GifResult) {
    savingRemote = [...savingRemote, result.url]
    try {
      merge([await api.postContent({ kind: result.kind === 'sticker' ? 'sticker' : 'gif', name: result.title || query.trim(), source_url: result.url, query: query.trim(), source_kind: result.source === 'giphy' ? 'giphy' : 'tenor' })])
      refreshTags()
    } catch (error) {
      failure = String(error)
    } finally {
      savingRemote = savingRemote.filter((url) => url !== result.url)
    }
  }

  function openItem(item: ContentItem, e: MouseEvent) {
    if (e.metaKey || e.shiftKey || selected.length > 0) {
      toggle(item)
      return
    }
    openId = openId === item.id ? null : item.id
  }

  function toggle(item: ContentItem) {
    selected = selected.includes(item.id) ? selected.filter((id) => id !== item.id) : [...selected, item.id]
  }

  async function bulk(patch: { add_tags?: string[]; add_group_ids?: string[]; is_favourite?: boolean }) {
    try {
      merge(await api.putContentBulk({ content_ids: selected, ...patch }))
      if (patch.add_group_ids) await loadGroups()
      refreshTags()
    } catch (error) {
      failure = String(error)
    }
  }

  async function remove(ids: string[]) {
    if (!confirm(t('content.confirmDelete', { count: ids.length }))) return
    for (const id of ids) {
      await api.deleteContent(id).catch((error) => (failure = String(error)))
    }
    library = library.filter((item) => !ids.includes(item.id))
    ranked = ranked.filter((item) => !ids.includes(item.id))
    selected = selected.filter((id) => !ids.includes(id))
    if (openId && ids.includes(openId)) openId = null
    await loadGroups()
    refreshTags()
  }

  async function syncMembership() {
    await loadGroups()
    const items = await api.getContent(undefined, undefined, sort).catch(() => null)
    if (items) library = items.filter((item) => item.kind !== 'emoji')
  }

  function siblings(group: ContentGroup): ContentGroup[] {
    return groups.filter((other) => other.parent_id === group.parent_id).sort((a, b) => a.position - b.position || a.name.localeCompare(b.name))
  }

  async function putGroup(input: Parameters<typeof api.putContentGroup>[0]) {
    try {
      await api.putContentGroup(input)
      await loadGroups()
    } catch (error) {
      failure = String(error)
    }
  }

  function moveGroup(group: ContentGroup, step: number) {
    const list = siblings(group)
    const index = list.findIndex((other) => other.id === group.id)
    const next = Math.max(0, Math.min(list.length - 1, index + step))
    if (next !== index) putGroup({ content_group_id: group.id, position: next })
  }

  function dragItem(item: ContentItem, section: ContentSection | null, e: DragEvent) {
    const ids = selected.includes(item.id) ? selected : [item.id]
    dragging = { ids, from: section?.group?.kind === 'manual' ? section.group.id : null }
    e.dataTransfer?.setData(DRAG_TYPE, JSON.stringify(ids))
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'copyMove'
  }

  function dragGroup(group: ContentGroup, e: DragEvent) {
    dragging = { group: group.id }
    e.dataTransfer?.setData(DRAG_TYPE, group.id)
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
  }

  function accepts(section: ContentSection): boolean {
    if (!dragging || !section.group) return false
    if ('group' in dragging) return dragging.group !== section.group.id
    return section.group.kind === 'manual'
  }

  async function dropOn(section: ContentSection, before: ContentItem | null, e: DragEvent) {
    const held = dragging
    dragging = null
    dropAt = null
    const target = section.group
    if (!held || !target) return
    if ('group' in held) {
      if (held.group === target.id) return
      const nest = e.altKey && target.kind === 'manual'
      await putGroup(nest ? { content_group_id: held.group, parent_id: target.id } : { content_group_id: held.group, parent_id: target.parent_id, position: siblings(target).findIndex((other) => other.id === target.id) })
      return
    }
    if (target.kind !== 'manual') return
    try {
      await api.putContentGroupItems({ content_group_id: target.id, content_ids: held.ids, position: placementIndex(target, before?.id ?? null, held.ids) })
      if (e.altKey && held.from && held.from !== target.id) await api.deleteContentGroupItems({ content_group_id: held.from, content_ids: held.ids })
      await syncMembership()
    } catch (error) {
      failure = String(error)
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    const typing = isTyping(e.target)
    if (e.key === 'Escape') {
      if (selected.length) selected = []
      else openId = null
    } else if (e.key === '/' && !typing) {
      e.preventDefault()
      document.getElementById('content-search')?.focus()
    }
  }

  onMount(() => {
    loadLibrary()
    api.getAppSettings().then((settings) => (autoplay = (settings as Record<string, string>)['messages.autoplay'] !== 'off')).catch(() => {})
  })
</script>

<svelte:window
  onkeydown={handleKeydown}
  ondragend={() => {
    dragging = null
    dropAt = null
  }}
  ondragover={(e) => {
    if (dragging || !e.dataTransfer?.types.includes('Files')) return
    e.preventDefault()
    dropping = true
  }}
  ondragleave={(e) => {
    if (!e.relatedTarget) dropping = false
  }}
  ondrop={(e) => {
    if (dragging || !e.dataTransfer?.files.length) return
    e.preventDefault()
    dropping = false
    uploadFiles([...e.dataTransfer.files])
  }}
/>

{#snippet grid(items: ContentItem[], section: ContentSection | null)}
  {@const visuals = items.filter((item) => isVisual(item))}
  {@const texts = items.filter((item) => !isVisual(item))}
  {@const droppable = !!section && accepts(section) && !!dragging && !('group' in dragging)}
  {#if visuals.length}
    <div class="grid grid-cols-[repeat(auto-fill,minmax(clamp(6.5rem,16%,10rem),1fr))] gap-2">
      {#each visuals as item (item.id)}
        <ContentTile {item} {autoplay} dropping={droppable} selected={selected.includes(item.id)} active={openId === item.id} onopen={openItem} ontoggle={toggle} ondragstart={(entry, e) => dragItem(entry, section, e)} ondrop={(entry, e) => section && dropOn(section, entry, e)} />
      {/each}
    </div>
  {/if}
  {#if texts.length}
    <div class="grid grid-cols-[repeat(auto-fill,minmax(clamp(12rem,30%,20rem),1fr))] gap-2 {visuals.length ? 'mt-2' : ''}">
      {#each texts as item (item.id)}
        <ContentTile {item} {autoplay} dropping={droppable} selected={selected.includes(item.id)} active={openId === item.id} onopen={openItem} ontoggle={toggle} ondragstart={(entry, e) => dragItem(entry, section, e)} ondrop={(entry, e) => section && dropOn(section, entry, e)} />
      {/each}
    </div>
  {/if}
{/snippet}

<PaneGrid name="content" class="bg-paper" panes={[{ key: 'library' }, { key: 'detail', size: '26rem', min: '20rem', max: '42vw', open: !!open, edge: 'start' }]}>
  {#snippet pane(key)}
  {#if key === 'library'}
  <div class="relative flex min-h-0 min-w-0 flex-1 flex-col">
    <header class="shrink-0 border-b border-line px-6 pt-5 pb-3">
      <div class="flex items-center gap-3">
        <h1 class="text-[18px] font-semibold tracking-tight text-ink">{t('content.title')}</h1>
        <span data-testid="content-count" class="font-mono text-[12px] text-ink-3">{library.length}</span>
        <div class="ml-auto flex items-center gap-1">
          <Segmented testid="content-sort" label={t('content.sort.label')} size="sm" value={sort} options={sortOptions} onchange={chooseSort} />
          <IconButton testid="content-favourites" icon="lucide:heart" label={favouritesOnly ? t('content.favouritesAll') : t('content.favouritesOnly')} active={favouritesOnly} tone="signal" size="sm" onclick={() => (favouritesOnly = !favouritesOnly)} />
          <IconButton testid="content-new-group" icon="lucide:folder-plus" label={t('content.group.new')} size="sm" onclick={() => (editing = { group: null, parentId: null })} />
          <SplitButton testid="content-upload" label={t('common.upload')} menuLabel={t('content.addMore')} busy={uploading > 0 || importBusy} onclick={() => fileInput?.click()}>
            {#snippet menu(close)}
              <MenuItem testid="content-import-url" icon="lucide:link" onclick={() => { close(); importing = true }}>{t('content.importUrl')}</MenuItem>
              <div class="menu-sep"></div>
              {#each TEMPLATE_KINDS as kind (kind)}
                <MenuItem testid="content-create-{kind}" icon={kindIcon(kind)} onclick={() => { close(); createTemplate(kind) }}>{t(CREATE_LABELS[kind])}</MenuItem>
              {/each}
              {#each signatureAccounts as provider (provider.id)}
                <MenuItem testid="content-import-signatures" icon="lucide:signature" onclick={() => { close(); importSignatures(provider.id) }}>{t('content.signature.importFrom', { name: provider.name })}</MenuItem>
              {/each}
            {/snippet}
          </SplitButton>
          <input bind:this={fileInput} data-testid="content-file" type="file" multiple class="hidden" onchange={(e) => uploadFiles([...(e.currentTarget.files ?? [])])} />
        </div>
      </div>
      <div class="mt-3 flex items-center gap-2">
        <Icon icon="lucide:search" class="shrink-0 text-[15px] text-ink-3" />
        <Field id="content-search" data-testid="content-search" bind:value={query} placeholder={t('content.searchPlaceholder')} oninput={handleInput} />
        <span class="hidden shrink-0 text-[12px] text-ink-3 sm:inline">/</span>
      </div>
    </header>

    {#if selected.length}
      <div data-testid="content-bulk" class="flex shrink-0 items-center gap-2 border-b border-line bg-paper-2 px-6 py-2">
        <span class="text-[13px] text-ink">{t('common.selected', { count: selected.length })}</span>
        <Field data-testid="content-bulk-tag" bind:value={bulkTag} size="sm" placeholder={t('content.bulk.tag')} class="max-w-[10rem]" onkeydown={(e) => {
          if (e.key === 'Enter' && bulkTag.trim()) {
            bulk({ add_tags: [bulkTag] })
            bulkTag = ''
          }
        }} />
        {#if manualGroups.length}
          <Select testid="content-bulk-group" size="sm" class="max-w-[14rem]" bind:value={bulkGroup} options={[{ value: '', label: t('content.bulk.group') }, ...manualGroups.map((group) => ({ value: group.id, label: group.name }))]} onchange={(id) => {
            if (id) bulk({ add_group_ids: [id] })
            bulkGroup = ''
          }} />
        {/if}
        <IconButton testid="content-bulk-favourite" icon="lucide:heart" label={t('content.detail.favourite')} size="sm" onclick={() => bulk({ is_favourite: true })} />
        <IconButton testid="content-bulk-delete" icon="lucide:trash-2" label={t('common.delete')} tone="danger" size="sm" onclick={() => remove(selected)} />
        <Button testid="content-bulk-clear" size="sm" class="ml-auto" keys="esc" onclick={() => (selected = [])}>{t('common.clear')}</Button>
      </div>
    {/if}

    <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-8">
      {#if failure}
        <p data-testid="content-failure" class="mt-3 flex items-center gap-2 text-[12.5px] text-error-600">
          {failure}
          <button type="button" class="text-ink-3 hover:text-ink" aria-label={t('common.dismiss')} onclick={() => (failure = null)}><Icon icon="lucide:x" /></button>
        </p>
      {/if}

      {#if library.length === 0 && !searching}
        <div class="mt-16 flex flex-col items-center text-center">
          <Icon icon="lucide:library" class="text-[32px] text-ink-3" />
          <p class="mt-3 text-[14px] text-ink">{t('content.empty.title')}</p>
          <p class="mt-1 max-w-[30rem] text-[13px] text-ink-3">{t('content.empty.body')}</p>
        </div>
      {:else if searching}
        <h2 data-testid="content-matches" data-count={shown.length} class="mt-5 mb-2 flex items-center gap-2 text-[13px] font-medium text-ink-2">
          <Icon icon="lucide:search" class="text-[13px] text-ink-3" />{t('content.section.matches')}<span class="font-mono text-[11.5px] text-ink-3">{shown.length}</span>
        </h2>
        {#if shown.length === 0}
          <p class="text-[13px] text-ink-3">{t('content.noMatches')}</p>
        {:else}
          {@render grid(shown, null)}
        {/if}
        <h2 data-testid="content-web" class="mt-6 mb-2 flex items-center gap-2 text-[13px] font-medium text-ink-2">
          <Icon icon="lucide:globe" class="text-[13px] text-ink-3" />{t('content.section.web')}{#if remoteVisible.length}<span class="font-mono text-[11.5px] text-ink-3">{remoteVisible.length}</span>{/if}
        </h2>
        {#if notice}<p class="mb-2 text-[12.5px] text-ink-3">{notice}</p>{/if}
        <div class="grid grid-cols-[repeat(auto-fill,minmax(clamp(6.5rem,16%,10rem),1fr))] gap-2">
          {#each remoteVisible as result (result.source + result.id)}
            {@const saving = savingRemote.includes(result.url)}
            <button type="button" class="group relative aspect-square overflow-hidden rounded-[0.5rem] bg-paper-3" title={t('content.web.save')} aria-label={t('content.web.saveNamed', { name: result.title || 'GIF' })} disabled={saving} onclick={() => saveRemote(result)}>
              <img src={gifSrc(result)} alt={result.title} loading="lazy" class="size-full object-cover" />
              <span class="absolute inset-x-0 bottom-0 flex items-center justify-between bg-black/50 px-1.5 py-0.5 text-[10.5px] text-white opacity-0 group-hover:opacity-100">
                <span class="uppercase">{result.source}</span>
                <span class="inline-flex items-center gap-1"><Icon icon={saving ? 'lucide:loader-circle' : 'lucide:download'} class={saving ? 'animate-spin' : ''} />{t('common.save')}</span>
              </span>
            </button>
          {/each}
        </div>
      {:else}
        {#each sections as section (section.key)}
          {@const group = section.group}
          {@const target = accepts(section)}
          <section data-testid="content-section" data-key={section.key} data-group={group?.id} data-kind={group?.kind} data-count={section.items.length} class="mt-5" style:padding-inline-start="{section.depth * 1.25}rem">
            <header
              class="group/head mb-2 flex h-7 items-center gap-1.5 rounded-[0.375rem] {dropAt === section.key && target ? 'bg-accent-soft' : ''}"
              draggable={!!group}
              role="group"
              aria-label={group?.name ?? t('content.section.ungrouped')}
              ondragstart={(e) => group && dragGroup(group, e)}
              oncontextmenu={(e) => {
                if (!group) return
                e.preventDefault()
                editing = { group, parentId: group.parent_id }
              }}
              ondragover={(e) => {
                if (!target) return
                e.preventDefault()
                dropAt = section.key
              }}
              ondragleave={() => dropAt === section.key && (dropAt = null)}
              ondrop={(e) => {
                if (!target) return
                e.preventDefault()
                dropOn(section, null, e)
              }}
            >
              {#if group}
                <button type="button" data-testid="content-section-toggle" class="flex size-5 shrink-0 items-center justify-center text-ink-3 hover:text-ink" aria-label={group.is_collapsed ? t('app.rail.expand') : t('app.rail.collapse')} aria-expanded={!group.is_collapsed} onclick={() => putGroup({ content_group_id: group.id, is_collapsed: !group.is_collapsed })}>
                  <Icon icon={group.is_collapsed ? 'lucide:chevron-right' : 'lucide:chevron-down'} class="text-[13px]" />
                </button>
              {/if}
              <Icon icon={group?.icon || (group?.kind === 'kind' && group.rule ? kindIcon(group.rule as ContentItem['kind']) : group?.kind === 'tag' ? 'lucide:tag' : group ? 'lucide:folder' : 'lucide:inbox')} class="shrink-0 text-[14px] text-ink-3" />
              <h2 data-testid="content-section-name" class="min-w-0 truncate text-[13.5px] font-medium text-ink">{group?.name ?? t('content.section.ungrouped')}</h2>
              <span class="font-mono text-[11.5px] text-ink-3">{section.items.length}</span>
              {#if group && group.kind !== 'manual'}
                <span class="text-[11.5px] text-ink-3">{group.kind === 'kind' ? t(CONTENT_KINDS.find((entry) => entry.value === group.rule)?.label ?? 'content.group.kind.kind') : `#${group.rule}`}</span>
              {/if}
              {#if group}
                <span class="ml-auto flex items-center opacity-0 group-hover/head:opacity-100 focus-within:opacity-100">
                  <IconButton testid="content-section-up" icon="lucide:arrow-up" label={t('content.group.up')} size="sm" onclick={() => moveGroup(group, -1)} />
                  <IconButton testid="content-section-down" icon="lucide:arrow-down" label={t('content.group.down')} size="sm" onclick={() => moveGroup(group, 1)} />
                  <IconButton testid="content-section-more" icon="lucide:ellipsis" label={t('app.rail.moreFor', { name: group.name })} size="sm" onclick={() => (editing = { group, parentId: group.parent_id })} />
                </span>
              {/if}
            </header>
            {#if !group?.is_collapsed}
              {#if section.items.length}
                {@render grid(section.items, section)}
              {:else if group?.kind === 'manual'}
                <p class="rounded-[0.5rem] px-2 py-3 text-[12.5px] text-ink-3 hairline">{t('content.section.emptyGroup')}</p>
              {/if}
            {/if}
          </section>
        {/each}
        <p class="mt-6 text-[12px] text-ink-3">{t('content.dragHint')}</p>
      {/if}
    </div>

    {#if dropping}
      <div class="pointer-events-none absolute inset-3 z-10 flex items-center justify-center rounded-[0.75rem] border-2 border-dashed border-accent bg-accent-soft">
        <p class="text-[14px] text-ink">{t('content.dropHere')}</p>
      </div>
    {/if}
  </div>
  {:else if open}
    <ContentDetail item={open} {tags} {groups} onchange={(item) => { merge([item]); refreshTags() }} ongroups={loadGroups} onimported={loadLibrary} ondelete={(item) => remove([item.id])} onclose={() => (openId = null)} />
  {/if}
  {/snippet}
</PaneGrid>

{#if importing}
  <Sheet testid="content-import" label={t('content.importUrl')} width="28rem" onclose={() => (importing = false)}>
    <form class="flex flex-col gap-3 p-4" onsubmit={(e) => { e.preventDefault(); importFromUrl() }}>
      <p class="text-[14px] font-medium text-ink">{t('content.importTitle')}</p>
      <Field data-testid="content-import-field" bind:value={importUrl} placeholder={t('content.importPlaceholder')} aria-label={t('content.importLabel')} />
      <div class="flex justify-end gap-2">
        <Button size="sm" onclick={() => (importing = false)}>{t('common.cancel')}</Button>
        <Button testid="content-import-submit" size="sm" variant="primary" type="submit" icon="lucide:download" busy={importBusy} disabled={!/^https?:\/\//i.test(importUrl.trim())}>{t('content.import')}</Button>
      </div>
    </form>
  </Sheet>
{/if}

{#if editing}
  {@const context = editing}
  {#key context}
    <GroupEditor
      subject={{ scope: 'content', group: context.group, groups, tags }}
      parentId={context.parentId}
      onsaved={async () => {
        editing = null
        await syncMembership()
      }}
      oncancel={() => (editing = null)}
      onnewgroup={(parentId) => (editing = { group: null, parentId })}
      ondeleted={async () => {
        editing = null
        await syncMembership()
      }}
    />
  {/key}
{/if}
