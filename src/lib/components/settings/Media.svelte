<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '$lib/tauri'
  import type { ContentGroup, ContentItem, ContentKind, LinkPreviewMode } from '$lib/tauri/types'
  import { contentSrc, filterLibrary, isVisual, parseTags, readFileAsBase64, sortLibrary } from '$lib/media'
  import { putPreviewMode } from '$lib/links'
  import { Button, Field, IconButton, Row, Section, Segmented, Select, Switch } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  let settings = $state<Record<string, string>>({})
  let tenorKey = $state('')
  let giphyKey = $state('')
  let locale = $state('')
  let maxSize = $state('')
  let items = $state<ContentItem[]>([])
  let groups = $state<ContentGroup[]>([])
  let kind = $state<ContentKind | 'all'>('all')
  let query = $state('')
  let failure = $state<string | null>(null)
  let importing = $state<string | null>(null)
  let uploadInput = $state<HTMLInputElement>()
  let folderInput = $state<HTMLInputElement>()
  let editingTags = $state<string | null>(null)
  let tagDraft = $state('')

  const shelves = $derived(groups.filter((group) => group.kind === 'manual' && !group.parent_id))
  const visible = $derived(filterLibrary(sortLibrary(items).filter((item) => kind === 'all' || item.kind === kind), query))
  const counts = $derived({
    all: items.length,
    gif: items.filter((i) => i.kind === 'gif').length,
    emoji: items.filter((i) => i.kind === 'emoji').length,
    image: items.filter((i) => i.kind === 'image').length
  })

  onMount(async () => {
    try {
      settings = await api.getAppSettings()
      tenorKey = settings['gif.tenor_key'] ?? ''
      giphyKey = settings['gif.giphy_key'] ?? ''
      locale = settings['gif.locale'] ?? ''
      maxSize = settings['gif.max_size'] ?? ''
      const [library, list] = await Promise.all([api.getContent(), api.getContentGroups()])
      items = library.filter(isVisual)
      groups = list
    } catch (error) {
      failure = String(error)
    }
  })

  async function put(key: string, value: string) {
    if ((settings[key] ?? '') === value) return
    try {
      settings = await api.putAppSetting(key, value)
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  async function putPreviews(value: string) {
    await put('links.previews', value === 'on' ? '' : value)
    putPreviewMode(value as LinkPreviewMode)
  }

  function replace(updated: ContentItem) {
    items = items.map((entry) => (entry.id === updated.id ? updated : entry))
  }

  async function toggleFavourite(item: ContentItem) {
    try {
      replace(await api.putContent({ content_id: item.id, is_favourite: !item.is_favourite }))
    } catch (error) {
      failure = String(error)
    }
  }

  async function saveTags(item: ContentItem) {
    editingTags = null
    const tags = parseTags(tagDraft)
    if (tags.join(',') === item.tags.join(',')) return
    try {
      replace(await api.putContent({ content_id: item.id, tags }))
    } catch (error) {
      failure = String(error)
    }
  }

  async function rename(item: ContentItem, name: string) {
    const next = name.trim()
    if (!next || next === item.name) return
    try {
      replace(await api.putContent({ content_id: item.id, name: next }))
    } catch (error) {
      failure = String(error)
    }
  }

  async function remove(item: ContentItem) {
    try {
      await api.deleteContent(item.id)
      items = items.filter((entry) => entry.id !== item.id)
    } catch (error) {
      failure = String(error)
    }
  }

  async function store(files: File[], group?: ContentGroup) {
    const images = files.filter((file) => file.type.startsWith('image/'))
    let failed = 0
    for (const [index, file] of images.entries()) {
      if (group) importing = t('settings.media.importProgress', { done: index + 1, total: images.length })
      try {
        const saved = await api.postContent({
          kind: file.type === 'image/gif' ? 'gif' : 'image',
          name: file.name.replace(/\.[^.]+$/, ''),
          data: await readFileAsBase64(file),
          mime_type: file.type || 'application/octet-stream',
          tags: group ? [group.name.toLowerCase()] : [],
          group_ids: group ? [group.id] : undefined,
          source_kind: 'upload'
        })
        items = [saved, ...items.filter((entry) => entry.id !== saved.id)]
      } catch (error) {
        failed += 1
        failure = String(error)
      }
    }
    importing = null
    if (failed > 0) failure = t('settings.media.importFailed', { failed, total: images.length, error: failure ?? '' })
  }

  async function upload(e: Event) {
    const input = e.target as HTMLInputElement
    const files = Array.from(input.files ?? [])
    input.value = ''
    await store(files)
  }

  async function importFolder(e: Event) {
    const input = e.target as HTMLInputElement
    const files = Array.from(input.files ?? [])
    input.value = ''
    if (files.length === 0) return
    const folder = (files[0].webkitRelativePath.split('/')[0] || t('settings.media.imported')).trim()
    try {
      const group = shelves.find((entry) => entry.name === folder) ?? (await api.postContentGroup({ name: folder, icon: 'lucide:folder' }))
      if (!groups.some((entry) => entry.id === group.id)) groups = [...groups, group]
      await store(files, group)
    } catch (error) {
      failure = String(error)
    }
  }

  const kinds = $derived([
    { value: 'all', label: t('settings.media.kindAll', { count: counts.all }) },
    { value: 'gif', label: t('settings.media.kindGif', { count: counts.gif }) },
    { value: 'emoji', label: t('settings.media.kindEmoji', { count: counts.emoji }) },
    { value: 'image', label: t('settings.media.kindImage', { count: counts.image }) }
  ])
</script>

<div>
  <Section id="content.gif-search" title={t('settings.media.gifTitle')} description={t('settings.media.gifDescription')}>
    {#if failure}
      <p class="mb-2 text-[13px] text-error-500">{failure}</p>
    {/if}
    <Row id="content.tenor-key" label={t('settings.media.tenorKey')} hint={t('settings.media.tenorHint')} for="gif-tenor">
      <Field id="gif-tenor" type="password" mono class="w-64" placeholder={t('settings.media.pasteKey')} bind:value={tenorKey} onblur={() => put('gif.tenor_key', tenorKey.trim())} autocomplete="off" spellcheck="false" />
    </Row>
    <Row id="content.giphy-key" label={t('settings.media.giphyKey')} hint={t('settings.media.giphyHint')} for="gif-giphy">
      <Field id="gif-giphy" type="password" mono class="w-64" placeholder={t('settings.media.pasteKey')} bind:value={giphyKey} onblur={() => put('gif.giphy_key', giphyKey.trim())} autocomplete="off" spellcheck="false" />
    </Row>
    <Row id="content.rating" label={t('settings.media.rating')} hint={t('settings.media.ratingHint')}>
      <Segmented label={t('settings.media.rating')} size="sm" value={settings['gif.rating'] || 'pg-13'} options={[{ value: 'g', label: 'G' }, { value: 'pg', label: 'PG' }, { value: 'pg-13', label: 'PG-13' }, { value: 'r', label: 'R' }]} onchange={(v) => put('gif.rating', v === 'pg-13' ? '' : v)} />
    </Row>
    <Row id="content.locale" label={t('settings.media.language')} hint={t('settings.media.languageHint')} for="gif-locale">
      <Field id="gif-locale" mono class="w-32" placeholder="en_US" bind:value={locale} onblur={() => put('gif.locale', locale.trim())} autocomplete="off" spellcheck="false" />
    </Row>
    <Row id="content.opens-on" label={t('settings.media.opensOn')} hint={t('settings.media.opensOnHint')}>
      <Select size="sm" class="w-[clamp(9rem,30%,13rem)]" testid="opens-on" dialog={t('settings.media.opensOn')} value={settings['gif.default_tab'] || 'favourites'} options={[{ value: 'favourites', label: t('conversation.gif.favourites') }, { value: 'trending', label: t('conversation.gif.trending') }, { value: 'recent', label: t('conversation.gif.recent') }, ...shelves.map((group) => ({ value: `g:${group.id}`, label: group.name }))]} onchange={(v) => put('gif.default_tab', v === 'favourites' ? '' : v)} />
    </Row>
    <Row id="content.max-size" label={t('settings.media.largest')} hint={t('settings.media.largestHint')} for="gif-max">
      <Field id="gif-max" mono class="w-20" inputmode="numeric" placeholder="8" bind:value={maxSize} onblur={() => put('gif.max_size', maxSize.replace(/\D/g, ''))} />
    </Row>
    <Row id="content.stickers" label={t('settings.media.stickers')} hint={t('settings.media.stickersHint')}>
      <Switch label={t('settings.media.showStickers')} checked={settings['gif.stickers'] === 'true'} onchange={(on) => put('gif.stickers', on ? 'true' : '')} />
    </Row>
  </Section>

  <Section id="content.previews" title={t('settings.media.linksTitle')} description={t('settings.media.linksDescription')}>
    <Row label={t('settings.media.previews')} hint={t('settings.media.previewsHint')}>
      <Segmented label={t('settings.media.linksTitle')} size="sm" value={settings['links.previews'] || 'on'} options={[{ value: 'on', label: t('common.on') }, { value: 'known-only', label: t('settings.media.knownSites') }, { value: 'off', label: t('common.off') }]} onchange={putPreviews} />
    </Row>
  </Section>

  <Section id="content.library" title={t('settings.media.libraryTitle')} description={t('settings.media.libraryDescription')} total={visible.length}>
    {#snippet actions()}
      <Button icon="lucide:folder-input" busy={importing !== null} onclick={() => folderInput?.click()}>{importing ?? t('settings.media.importFolder')}</Button>
      <input bind:this={folderInput} type="file" webkitdirectory multiple class="hidden" onchange={importFolder} />
      <Button variant="primary" icon="lucide:upload" onclick={() => uploadInput?.click()}>{t('common.upload')}</Button>
      <input bind:this={uploadInput} type="file" accept="image/*" multiple class="hidden" onchange={upload} />
    {/snippet}
    {#snippet children(shown)}
    <div class="flex items-center gap-3 border-b border-line pb-3">
      <Segmented label={t('settings.media.kind')} size="sm" value={kind} options={kinds} onchange={(v) => (kind = v as ContentKind | 'all')} />
      <Field size="sm" class="max-w-xs" bind:value={query} placeholder={t('settings.media.filter')} />
    </div>
    {#if visible.length === 0}
      <p class="py-4 text-[13px] text-ink-3">{items.length === 0 ? t('settings.media.empty') : t('settings.media.noMatch')}</p>
    {:else}
      <ul>
        {#each visible.slice(0, shown) as item (item.id)}
          <li class="group grid h-12 grid-cols-[2.5rem_minmax(0,1fr)_auto] items-center gap-3 border-b border-line last:border-b-0">
            <span class="flex size-9 items-center justify-center overflow-hidden rounded-[0.5rem] bg-paper-3">
              <img src={contentSrc(item)} alt={item.name} class={item.kind === 'emoji' ? 'size-6 object-contain' : 'size-full object-cover'} loading="lazy" />
            </span>
            <div class="flex min-w-0 items-baseline gap-2">
              <input class="w-40 shrink-0 bg-transparent text-[14px] font-medium text-ink outline-none" value={item.name} onchange={(e) => rename(item, e.currentTarget.value)} aria-label={t('common.name')} />
              <span class="shrink-0 font-mono text-[11px] text-ink-3">{item.kind}</span>
              {#if editingTags === item.id}
                <input class="min-w-0 flex-1 bg-transparent font-mono text-[12px] text-ink outline-none" bind:value={tagDraft} onblur={() => saveTags(item)} onkeydown={(e) => { if (e.key === 'Enter') saveTags(item); if (e.key === 'Escape') editingTags = null }} placeholder={t('conversation.gif.tagsPlaceholder')} aria-label={t('conversation.gif.tags')} />
              {:else}
                <button class="min-w-0 flex-1 truncate text-left font-mono text-[12px] text-ink-3 hover:text-ink" onclick={() => { editingTags = item.id; tagDraft = item.tags.join(', ') }} title={t('settings.media.editTags')}>{item.tags.length ? item.tags.join(', ') : t('settings.media.addTags')}</button>
              {/if}
              <span class="shrink-0 font-mono text-[11px] text-ink-3">{t('settings.media.used', { count: item.used_count })}</span>
            </div>
            <div class="flex items-center gap-0.5 {item.is_favourite ? '' : 'opacity-0 group-hover:opacity-100 focus-within:opacity-100'}">
              <IconButton size="sm" icon="lucide:star" label={item.is_favourite ? t('settings.media.unstar') : t('settings.media.star')} tone="signal" active={item.is_favourite} onclick={() => toggleFavourite(item)} />
              <IconButton size="sm" icon="lucide:trash-2" label={t('common.delete')} tone="danger" class={item.is_favourite ? 'opacity-0 group-hover:opacity-100' : ''} onclick={() => remove(item)} />
            </div>
          </li>
        {/each}
      </ul>
    {/if}
    {/snippet}
  </Section>
</div>
