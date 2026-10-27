<script lang="ts">
  import Icon from '@iconify/svelte'
  import { untrack } from 'svelte'
  import { goto } from '$app/navigation'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { api, loadTags, loadObjectTags, tagRecordsStore } from '$lib/tauri'
  import type { ContentGroup, ContentItem, ContentTag, ContentTemplateKind, ContentUse, RenderedContent } from '$lib/tauri/types'
  import { CONTENT_KINDS, SOURCE_LABELS, TEMPLATE_KINDS, contentSrc, isTemplate, isVideo, isVisual, kindIcon, nestValues, sourceLink, tagSuggestions, toggleWord } from '$lib/media'
  import { insertVariable } from '$lib/intelligence'
  import { buildTree, flattenTree } from '$lib/tauri/groups'
  import { Button, Field, IconButton, Segmented, TagMark } from '$lib/components/ui'
  import SignatureBindings from '$lib/components/settings/SignatureBindings.svelte'
  import { formatDate, t } from '$lib/i18n.svelte'

  interface Props {
    item: ContentItem
    tags: ContentTag[]
    groups: ContentGroup[]
    onchange: (item: ContentItem) => void
    ongroups: () => void
    onimported?: () => void
    ondelete: (item: ContentItem) => void
    onclose: () => void
  }

  let { item, tags, groups, onchange, ongroups, onimported, ondelete, onclose }: Props = $props()

  let name = $state('')
  let body = $state('')
  let tagText = $state('')
  let keywordText = $state('')
  let samples = $state<Record<string, string>>({})
  let rendered = $state<RenderedContent | null>(null)
  let renderError = $state<string | null>(null)
  let uses = $state<ContentUse[]>([])
  let busy = $state(false)
  let failure = $state<string | null>(null)
  let editor = $state<HTMLTextAreaElement | null>(null)
  let timer: ReturnType<typeof setTimeout> | undefined
  let request = 0

  const itemId = $derived(item.id)

  $effect(() => {
    itemId
    untrack(() => {
      body = item.body ?? ''
      tagText = ''
      keywordText = ''
      samples = {}
      failure = null
    })
  })

  $effect(() => {
    name = item.name
  })

  $effect(() => {
    const id = item.id
    let live = true
    api.getContentUses(id).then((list) => live && (uses = list)).catch(() => live && (uses = []))
    return () => {
      live = false
    }
  })

  $effect(() => {
    if (!isTemplate(item)) return
    const source = body
    const values = nestValues(samples)
    clearTimeout(timer)
    const token = ++request
    timer = setTimeout(() => {
      api
        .postContentRender({ body: source, values, sample: true })
        .then((result) => {
          if (token !== request) return
          rendered = result
          renderError = null
        })
        .catch((error) => token === request && (renderError = String(error)))
    }, 200)
    return () => clearTimeout(timer)
  })

  const template = $derived(isTemplate(item))
  const suggestions = $derived(tagSuggestions(tags, tagText, item.tags, 6))
  const link = $derived(sourceLink(item))
  const manual = $derived(flattenTree(buildTree(groups)).filter((node) => node.group.kind === 'manual'))
  const kindOptions = $derived(TEMPLATE_KINDS.map((kind) => ({ value: kind, label: t(CONTENT_KINDS.find((entry) => entry.value === kind)!.label), icon: kindIcon(kind) })))
  const fillable = $derived((rendered?.variables ?? []).filter((variable) => variable.kind !== 'list'))
  const size = $derived(item.size_bytes ? (item.size_bytes > 1048576 ? `${(item.size_bytes / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(item.size_bytes / 1024))} KB`) : null)
  const source = $derived(t(SOURCE_LABELS[item.source_kind ?? (template ? 'manual' : 'upload')]))

  async function save(patch: { name?: string; tags?: string[]; keywords?: string[]; is_favourite?: boolean; body?: string; kind?: ContentTemplateKind }) {
    busy = true
    failure = null
    try {
      onchange(await api.putContent({ content_id: item.id, ...patch }))
      if (item.kind === 'signature' && patch.body !== undefined) await api.putSignatureSync(item.id)
      if (patch.tags) await Promise.all([loadTags(), loadObjectTags()])
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }

  function recordOf(name: string) {
    return $tagRecordsStore.find((record) => record.name.toLowerCase() === name.toLowerCase()) ?? { name, color: '', icon: null }
  }

  function addTag(value: string) {
    if (!value.trim()) return
    save({ tags: toggleWord(item.tags.filter((tag) => tag !== value.trim().toLowerCase()), value) })
    tagText = ''
  }

  function addKeyword() {
    const value = keywordText.trim().toLowerCase()
    if (!value || item.keywords.includes(value)) return
    save({ keywords: [...item.keywords, value] })
    keywordText = ''
  }

  async function toggleGroup(group: ContentGroup) {
    failure = null
    try {
      const input = { content_group_id: group.id, content_ids: [item.id] }
      if (item.group_ids.includes(group.id)) await api.deleteContentGroupItems(input)
      else await api.putContentGroupItems(input)
      onchange({ ...item, group_ids: item.group_ids.includes(group.id) ? item.group_ids.filter((id) => id !== group.id) : [...item.group_ids, group.id] })
      ongroups()
    } catch (error) {
      failure = String(error)
    }
  }

  function insert(variable: string) {
    const next = insertVariable(body, variable, editor?.selectionStart ?? body.length)
    body = next.template
    requestAnimationFrame(() => {
      editor?.focus()
      editor?.setSelectionRange(next.cursor, next.cursor)
    })
  }

  function saveBody() {
    if (body !== (item.body ?? '')) save({ body })
  }

  function openSource() {
    if (!link) return
    if (link.external) openUrl(link.href).catch((error) => (failure = String(error)))
    else goto(link.href)
  }
</script>

<aside data-testid="content-detail" data-id={item.id} class="flex h-full min-h-0 flex-col border-l border-line" aria-label={t('content.detail.label')}>
  <header class="flex h-12 shrink-0 items-center gap-2 px-4">
    <Icon icon={kindIcon(item.kind)} class="shrink-0 text-[15px] text-ink-3" />
    <span data-testid="content-detail-name" class="min-w-0 flex-1 truncate text-[14px] font-semibold text-ink">{item.name}</span>
    <IconButton testid="content-detail-favourite" icon="lucide:heart" label={item.is_favourite ? t('content.detail.unfavourite') : t('content.detail.favourite')} active={item.is_favourite} tone="signal" size="sm" onclick={() => save({ is_favourite: !item.is_favourite })} />
    <IconButton testid="content-detail-close" icon="lucide:x" label={t('common.close')} keys="esc" size="sm" onclick={onclose} />
  </header>
  <div class="min-h-0 flex-1 overflow-y-auto px-4 pb-4">
    {#if isVisual(item)}
      <div class="overflow-hidden rounded-[0.5rem] bg-paper-3">
        {#if isVideo(item)}
          <video src={contentSrc(item)} class="max-h-[40vh] w-full object-contain" controls loop muted autoplay playsinline></video>
        {:else}
          <img src={contentSrc(item)} alt={item.name} class="max-h-[40vh] w-full object-contain" />
        {/if}
      </div>
    {:else if !template}
      <div class="flex items-center gap-3 rounded-[0.5rem] bg-paper-3 p-3">
        <Icon icon="lucide:file" class="shrink-0 text-[22px] text-ink-3" />
        <span class="min-w-0 flex-1 truncate font-mono text-[12px] text-ink-2">{item.mime_type}</span>
      </div>
    {/if}

    <label class="mt-4 block text-[12px] text-ink-3" for="content-name">{t('content.detail.title')}</label>
    <Field id="content-name" data-testid="content-name" bind:value={name} size="sm" onblur={() => name.trim() && name.trim() !== item.name && save({ name })} onkeydown={(e) => e.key === 'Enter' && (e.currentTarget as HTMLInputElement).blur()} />

    {#if template}
      <div class="mt-4">
        <Segmented testid="content-kind" label={t('content.detail.kind')} size="sm" value={item.kind} options={kindOptions} onchange={(kind) => save({ kind: kind as ContentTemplateKind })} />
      </div>
      <div class="mt-3 flex items-baseline justify-between gap-2">
        <span class="text-[12px] text-ink-3">{t('content.detail.template')}</span>
        <span class="text-[11.5px] text-ink-3">{t('content.detail.templateHint')}</span>
      </div>
      <textarea
        bind:this={editor}
        data-testid="content-body"
        bind:value={body}
        class="mt-1 min-h-[10rem] w-full resize-y border-b border-line bg-paper-2 px-2.5 py-2 font-mono text-[12.5px] leading-5 text-ink outline-none placeholder:text-ink-3 focus:border-ink-3"
        spellcheck="false"
        aria-label={t('content.detail.template')}
        placeholder={t('content.detail.templatePlaceholder')}
        onblur={saveBody}
        onkeydown={(e) => {
          if (e.key === 's' && (e.metaKey || e.ctrlKey)) {
            e.preventDefault()
            saveBody()
          }
        }}
      ></textarea>
      {#if renderError}<p data-testid="content-render-error" class="mt-1 text-[12px] text-error-600">{renderError}</p>{/if}
      {#if rendered?.variables.length}
        <p class="mt-3 text-[12px] text-ink-3">{t('content.detail.variables')}</p>
        <div class="mt-1 flex flex-wrap gap-0.5">
          {#each rendered.variables as variable (variable.name)}
            <button type="button" data-testid="content-variable" data-name={variable.name} class="h-6 rounded-[0.5rem] px-1.5 font-mono text-[11px] text-ink-2 hover:bg-paper-3 hover:text-ink" title={variable.kind === 'list' ? t('content.detail.listOf', { fields: variable.fields.join(', ') }) : variable.kind} onclick={() => insert(variable.name)}>.{variable.name}{variable.kind === 'list' ? ' []' : ''}</button>
          {/each}
        </div>
      {/if}
      {#if fillable.length}
        <p class="mt-3 text-[12px] text-ink-3">{t('content.detail.samples')}</p>
        <div class="mt-1 grid grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)] items-center gap-x-2">
          {#each fillable as variable (variable.name)}
            <span class="truncate font-mono text-[11.5px] text-ink-2">.{variable.name}</span>
            <Field data-testid="content-sample" data-name={variable.name} size="sm" value={samples[variable.name] ?? variable.sample} aria-label={variable.name} oninput={(e) => (samples = { ...samples, [variable.name]: e.currentTarget.value })} />
          {/each}
        </div>
      {/if}
      <p class="mt-3 text-[12px] text-ink-3">{t('content.detail.preview')}</p>
      <div data-testid="content-preview" class="mt-1 rounded-[0.5rem] bg-paper-3 px-3 py-2 text-[13px] leading-5 whitespace-pre-wrap text-ink">{rendered?.text || t('content.detail.previewEmpty')}</div>
      {#if item.kind === 'signature'}
        <p class="mt-2 text-[11.5px] text-ink-3">{t('content.signature.variables')}</p>
        <SignatureBindings contentId={item.id} body={item.body ?? ''} {onimported} />
      {/if}
    {/if}

    <p class="mt-4 text-[12px] text-ink-3">{t('content.detail.tags')}</p>
    <div class="mt-1 flex flex-wrap gap-1">
      {#each item.tags as tag (tag)}
        <button type="button" data-testid="content-tag" data-tag={tag} class="inline-flex h-6 items-center gap-1 bg-paper-3 px-2 text-[12.5px] text-ink-2 hover:text-ink" onclick={() => save({ tags: item.tags.filter((entry) => entry !== tag) })} aria-label={t('content.detail.removeTag', { tag })}>
          <TagMark tag={recordOf(tag)} size="xs" label ink="ink-2" /><Icon icon="lucide:x" class="text-[11px]" />
        </button>
      {/each}
    </div>
    <Field data-testid="content-tag-input" bind:value={tagText} size="sm" placeholder={t('content.detail.addTag')} class="mt-1" onkeydown={(e) => e.key === 'Enter' && addTag(tagText)} />
    {#if suggestions.length && tagText.trim()}
      <div class="mt-1 flex flex-wrap gap-1">
        {#each suggestions as tag (tag)}
          <button type="button" class="h-6 px-2 text-[12.5px] text-ink-3 hover:bg-paper-3 hover:text-ink" onclick={() => addTag(tag)}>{tag}</button>
        {/each}
      </div>
    {/if}

    <p class="mt-4 text-[12px] text-ink-3">{t('content.detail.keywords')} <span class="text-ink-3/80">· {t('content.detail.keywordsHint')}</span></p>
    <div class="mt-1 flex flex-wrap gap-1">
      {#each item.keywords as word (word)}
        <button type="button" data-testid="content-keyword" data-word={word} class="inline-flex h-6 items-center gap-1 px-2 text-[12.5px] text-ink-2 hairline hover:text-ink" onclick={() => save({ keywords: item.keywords.filter((entry) => entry !== word) })} aria-label={t('content.detail.removeKeyword', { word })}>
          {word}<Icon icon="lucide:x" class="text-[11px]" />
        </button>
      {/each}
    </div>
    <Field data-testid="content-keyword-input" bind:value={keywordText} size="sm" placeholder={t('content.detail.addKeyword')} class="mt-1" onkeydown={(e) => e.key === 'Enter' && addKeyword()} />

    <p class="mt-4 text-[12px] text-ink-3">{t('content.detail.groups')}</p>
    {#if manual.length}
      <div class="mt-1 flex flex-col">
        {#each manual as node (node.group.id)}
          {@const on = item.group_ids.includes(node.group.id)}
          <button type="button" data-testid="content-group-toggle" data-id={node.group.id} class="flex h-7 min-w-0 items-center gap-2 rounded-[0.375rem] pr-2 text-left text-[13px] {on ? 'text-ink' : 'text-ink-3 hover:text-ink'} hover:bg-paper-3" style:padding-inline-start="{0.5 + node.depth}rem" aria-pressed={on} onclick={() => toggleGroup(node.group)}>
            <Icon icon={on ? 'lucide:check' : node.group.icon || 'lucide:folder'} class="shrink-0 text-[13px] {on ? 'text-accent' : ''}" />
            <span class="truncate">{node.group.name}</span>
          </button>
        {/each}
      </div>
    {:else}
      <p class="mt-1 text-[12.5px] text-ink-3">{t('content.detail.noGroups')}</p>
    {/if}

    <p class="mt-4 text-[12px] text-ink-3">{t('content.detail.origin')}</p>
    <div data-testid="content-origin" data-source={item.source_kind} class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[12.5px] text-ink-2">
      <span>{source}</span>
      {#if item.width && item.height}<span class="font-mono text-ink-3">{item.width}×{item.height}</span>{/if}
      {#if size}<span class="font-mono text-ink-3">{size}</span>{/if}
      <span class="text-ink-3">{t('content.detail.created', { date: formatDate(item.created_at, { dateStyle: 'medium' }) })}</span>
    </div>
    {#if link}
      <Button testid="content-source-link" size="sm" icon={link.external ? 'lucide:external-link' : 'lucide:corner-down-right'} class="mt-1 -ml-2" onclick={openSource}>{t(link.external ? 'content.detail.openLink' : item.source_kind === 'person' ? 'content.detail.openPerson' : item.source_kind === 'action' ? 'content.detail.openAction' : 'content.detail.openConversation')}</Button>
    {/if}

    <p class="mt-4 text-[12px] text-ink-3">{t('content.detail.usedWhere')}</p>
    <p class="mt-1 text-[12.5px] text-ink-2">{t('content.detail.sent', { count: item.used_count })}</p>
    {#each uses as use (use.action_id + use.node_id)}
      <button type="button" data-testid="content-use" data-action={use.action_id} class="mt-0.5 flex h-7 w-full min-w-0 items-center gap-2 rounded-[0.375rem] px-1 text-left text-[13px] text-ink-2 hover:bg-paper-3 hover:text-ink" onclick={() => goto(`/main/actions?id=${encodeURIComponent(use.action_id)}`)}>
        <Icon icon="lucide:workflow" class="shrink-0 text-[13px] text-ink-3" />
        <span class="truncate">{use.action_name}</span>
        <span class="ml-auto shrink-0 font-mono text-[11px] text-ink-3">{use.node_label || use.node_id}</span>
      </button>
    {:else}
      <p class="text-[12.5px] text-ink-3">{t('content.detail.noActions')}</p>
    {/each}

    {#if failure}<p data-testid="content-detail-failure" class="mt-3 text-[12.5px] text-error-600">{failure}</p>{/if}
  </div>
  <footer class="flex shrink-0 items-center justify-between border-t border-line px-4 py-2">
    <span class="text-[12px] text-ink-3">{busy ? t('common.saving') : ' '}</span>
    <Button testid="content-delete" size="sm" variant="danger" icon="lucide:trash-2" onclick={() => ondelete(item)}>{t('common.delete')}</Button>
  </footer>
</aside>
