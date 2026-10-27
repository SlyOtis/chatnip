<script lang="ts">
  import Icon from '@iconify/svelte'
  import { api, tagRecordsStore, groupsStore, loadGroups } from '$lib/tauri'
  import type { ActionGroup, ContentGroup, ContentGroupKind, ContentTag, Group, GroupKind, GroupQuery } from '$lib/tauri/types'
  import { emptyQuery, groupFields, parentChoices, serializeQuery, validRegex } from '$lib/tauri/groups'
  import { CONTENT_KINDS } from '$lib/media'
  import { layoutSettings, putLayoutSetting, sortKey, countKey, SORT_CHOICES } from '$lib/layoutSettings'
  import { Button, ColourSwatch, Field, IconPicker, Segmented, Select, Sheet, Switch, TagField } from '$lib/components/ui'
  import { anchored, boxOf } from '$lib/popover'
  import QueryBuilder from './QueryBuilder.svelte'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  type Subject =
    | { scope: 'conversation'; group: Group | null }
    | { scope: 'action'; group: ActionGroup | null; groups: ActionGroup[] }
    | { scope: 'content'; group: ContentGroup | null; groups: ContentGroup[]; tags: ContentTag[] }

  type Saved = Group | ActionGroup | ContentGroup

  interface Props {
    subject: Subject
    parentId?: string | null
    onsaved: (group: Saved) => void
    oncancel: () => void
    onnewgroup?: (parentId: string) => void
    onnewaction?: (groupId: string) => void
    ondeleted?: (group: Saved) => void
  }

  let { subject, parentId = null, onsaved, oncancel, onnewgroup, onnewaction, ondeleted }: Props = $props()

  const initial = subject.group
  let picking = $state(false)
  let name = $state(initial?.name ?? '')
  let kind = $state<string>(subject.scope === 'action' ? 'manual' : (subject.group?.kind ?? 'manual'))
  let parent = $state(initial?.parent_id ?? parentId ?? '')
  let tagId = $state(subject.scope === 'conversation' ? (subject.group?.tag_id ?? '') : '')
  let rule = $state(subject.scope === 'content' ? (subject.group?.rule ?? '') : '')
  let query = $state<GroupQuery>(subject.scope === 'conversation' ? (subject.group?.query ?? emptyQuery()) : emptyQuery())
  let icon = $state<string | null>(initial?.icon ?? null)
  let color = $state<string | null>(subject.scope === 'conversation' ? (subject.group?.color ?? null) : null)
  let reading = $state(subject.scope === 'conversation' && subject.group?.mode === 'read')
  let sort = $state(initial ? ($layoutSettings[sortKey(initial.id)] ?? 'manual') : 'manual')
  let counting = $state(initial ? $layoutSettings[countKey(initial.id)] !== 'off' : true)
  let search = $state('')
  let parentOpen = $state(false)
  let parentButton = $state<HTMLButtonElement | null>(null)
  let confirming = $state(false)
  let saving = $state(false)
  let deleting = $state(false)
  let failure = $state<string | null>(null)

  const KIND_ICONS: Record<string, string> = { manual: 'lucide:folder', tag: 'lucide:tag', query: 'lucide:filter', kind: 'lucide:shapes' }

  const fields = $derived(groupFields(subject.scope, !!subject.group, subject.scope === 'action' && !!subject.group?.builtin))
  const kinds = $derived(
    subject.scope === 'content'
      ? [
          { value: 'manual', label: t('content.group.kind.manual'), icon: KIND_ICONS.manual },
          { value: 'kind', label: t('content.group.kind.kind'), icon: KIND_ICONS.kind },
          { value: 'tag', label: t('content.group.kind.tag'), icon: KIND_ICONS.tag }
        ]
      : [
          { value: 'manual', label: t('settings.group.kind.manual'), icon: KIND_ICONS.manual },
          { value: 'tag', label: t('settings.group.kind.tag'), icon: KIND_ICONS.tag },
          { value: 'query', label: t('settings.group.kind.query'), icon: KIND_ICONS.query }
        ]
  )
  const help = $derived<Record<string, MessageKey>>(
    subject.scope === 'content'
      ? { manual: 'content.group.help.manual', kind: 'content.group.help.kind', tag: 'content.group.help.tag' }
      : { manual: 'settings.group.help.manual', tag: 'settings.group.help.tag', query: 'settings.group.help.query' }
  )
  const kindNames: Record<GroupKind, MessageKey> = { manual: 'settings.group.kindName.manual', tag: 'settings.group.kindName.tag', query: 'settings.group.kindName.query' }
  const confirmText = $derived<MessageKey>(subject.scope === 'action' ? 'actions.group.removeConfirm' : subject.scope === 'content' ? 'content.group.confirmDelete' : 'settings.group.confirmDelete')

  const tree = $derived<Saved[]>(subject.scope === 'conversation' ? $groupsStore : subject.groups)
  const choices = $derived(parentChoices(tree, subject.group?.id ?? null, search))
  const offered = $derived(choices.filter((entry) => !entry.disabled))
  const chosen = $derived(tree.find((entry) => entry.id === parent) ?? null)
  const trail = $derived.by(() => {
    const names: string[] = []
    let cursor = chosen
    while (cursor) {
      names.unshift(cursor.name)
      cursor = cursor.parent_id ? (tree.find((entry) => entry.id === cursor?.parent_id) ?? null) : null
    }
    return names.join(' / ')
  })
  const ruleOptions = $derived(
    subject.scope !== 'content' ? [] : kind === 'kind' ? CONTENT_KINDS.map((entry) => ({ value: entry.value, label: t(entry.label) })) : subject.tags.map((tag) => ({ value: tag.name, label: `#${tag.name}` }))
  )
  const valid = $derived(
    name.trim().length > 0 &&
      (subject.scope !== 'conversation' || ((kind !== 'tag' || !!tagId) && (kind !== 'query' || !validRegex(query.regex)))) &&
      (subject.scope !== 'content' || kind === 'manual' || rule.trim().length > 0)
  )

  function iconOf(entry: Saved): string {
    return entry.icon || KIND_ICONS['kind' in entry ? entry.kind : 'manual']
  }

  async function persist(): Promise<Saved> {
    const trimmed = name.trim()
    const parent_id = parent || null
    if (subject.scope === 'action') {
      return subject.group ? api.putActionGroup({ group_id: subject.group.id, name: trimmed, parent_id, icon }) : api.postActionGroup({ name: trimmed, parent_id, icon })
    }
    if (subject.scope === 'content') {
      const payload = { name: trimmed, parent_id, rule: kind === 'manual' ? null : rule, icon }
      return subject.group ? api.putContentGroup({ content_group_id: subject.group.id, ...payload }) : api.postContentGroup({ kind: kind as ContentGroupKind, ...payload })
    }
    const payload = { name: trimmed, parent_id, tag_id: kind === 'tag' ? tagId || null : null, query: kind === 'query' ? serializeQuery(query) : null, icon, color }
    const group = subject.group
    if (group) {
      const saved = await api.putGroup({ group_id: group.id, ...payload, mode: reading ? 'read' : 'write' })
      await putLayoutSetting(sortKey(group.id), sort === 'manual' ? null : sort)
      await putLayoutSetting(countKey(group.id), counting ? null : 'off')
      await loadGroups()
      return saved
    }
    const created = await api.postGroup({ kind: kind as GroupKind, ...payload })
    const saved = reading ? await api.putGroup({ group_id: created.id, mode: 'read' }) : created
    await loadGroups()
    return saved
  }

  async function save() {
    if (!valid || saving) return
    saving = true
    try {
      const saved = await persist()
      failure = null
      onsaved(saved)
    } catch (error) {
      failure = String(error)
    } finally {
      saving = false
    }
  }

  async function remove() {
    const group = subject.group
    if (!group || deleting) return
    deleting = true
    try {
      if (subject.scope === 'action') await api.deleteActionGroup(group.id)
      else if (subject.scope === 'content') await api.deleteContentGroup(group.id)
      else {
        await api.deleteGroup(group.id)
        await loadGroups()
      }
      ondeleted?.(group)
    } catch (error) {
      failure = String(error)
      confirming = false
    } finally {
      deleting = false
    }
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) save()
  }

  function pick(id: string) {
    parent = id
    parentOpen = false
    search = ''
  }

  function searchKeydown(e: KeyboardEvent) {
    if (e.key !== 'Enter' || e.metaKey || e.ctrlKey) return
    e.preventDefault()
    const first = offered[0]
    if (first) pick(first.group.id)
  }
</script>

{#snippet choice(id: string, label: string, glyph: string, depth: number, disabled: boolean)}
  {@const on = parent === id}
  <button
    type="button"
    role="option"
    data-testid="group-editor-parent"
    data-id={id}
    aria-selected={on}
    {disabled}
    class="grid h-7 w-full shrink-0 grid-cols-[1.75rem_minmax(0,1fr)_auto] items-center rounded-[0.4rem] pr-2 text-left text-[13px] {disabled ? 'cursor-default text-ink-3 line-through decoration-ink-3' : on ? 'bg-accent-soft text-ink' : 'text-ink-2 hover:bg-paper-3 hover:text-ink'}"
    style:padding-left="{depth * 1.25}rem"
    title={disabled ? t('settings.group.cannotNest') : label}
    onclick={() => pick(id)}
  >
    <Icon icon={glyph} class="justify-self-center text-[13px] {on ? 'text-accent' : 'text-ink-3'}" />
    <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{label}</span>
    {#if on}
      <Icon icon="lucide:check" class="text-[13px] text-accent" />
    {/if}
  </button>
{/snippet}

<Sheet label={subject.group ? t('settings.group.edit') : t('settings.group.new')} width="36rem" testid="group-editor" movable onclose={oncancel}>
  <form
    class="flex flex-col gap-5 px-5 py-4"
    onsubmit={(e) => {
      e.preventDefault()
      save()
    }}
    onkeydown={keydown}
  >
    <div class="flex items-center gap-3">
      <Field data-testid="group-editor-name" class="text-[14px] font-medium" placeholder={t('settings.group.name')} aria-label={t('settings.group.name')} icon={icon ?? KIND_ICONS[kind]} onicon={() => (picking = true)} bind:value={name} />
      {#if fields.has('colour')}
        <ColourSwatch value={color} clearable label={t('settings.group.colour')} onpick={(next) => (color = next)} />
      {/if}
    </div>

    {#if subject.group && subject.scope !== 'content'}
      <TagField objectType={subject.scope === 'action' ? 'action_group' : 'group'} objectId={subject.group.id} />
    {/if}

    {#if fields.has('read')}
      <div class="flex items-center gap-3">
        <div class="min-w-0 flex-1">
          <p class="text-[13px] text-ink">{t('settings.group.read')}</p>
          <p class="text-[12px] text-ink-3">{t('settings.group.readHint')}</p>
        </div>
        <Switch testid="group-editor-read" label={t('settings.group.read')} bind:checked={reading} />
      </div>
    {/if}

    {#if fields.has('kind')}
      <div class="flex flex-wrap items-center gap-3">
        <Segmented
          testid="group-editor-kind"
          label={t('settings.group.kindLabel')}
          value={kind}
          options={kinds}
          onchange={(value) => {
            kind = value
            rule = ''
          }}
        />
        <span class="text-[13px] text-ink-3">{t(help[kind])}</span>
      </div>
    {/if}

    {#if fields.has('rule') && subject.scope === 'conversation' && kind === 'tag'}
      <label class="flex flex-col gap-1 text-[13px] text-ink-2">
        {t('settings.group.kind.tag')}
        <Select testid="group-editor-tag" bind:value={tagId} options={[{ value: '', label: t('settings.group.pickTag') }, ...$tagRecordsStore.map((label) => ({ value: label.id, label: label.kind === 'split' ? t('settings.group.splitTag', { name: label.name }) : label.name }))]} />
      </label>
    {:else if fields.has('rule') && subject.scope === 'content' && kind !== 'manual'}
      <label class="flex flex-col gap-1 text-[13px] text-ink-2">
        {kind === 'kind' ? t('content.group.kind.kind') : t('content.group.kind.tag')}
        <Select bind:value={rule} options={[{ value: '', label: kind === 'kind' ? t('content.group.pickKind') : t('settings.group.pickTag') }, ...ruleOptions]} />
      </label>
    {/if}

    {#if fields.has('query') && kind === 'query'}
      <QueryBuilder {query} onchange={(next) => (query = next)} />
    {/if}

    {#if fields.has('sort')}
      <div class="flex items-end gap-4">
        <label class="flex min-w-0 flex-1 flex-col gap-1 text-[13px] text-ink-2">
          {t('app.rail.sortOrder')}
          <Select testid="group-editor-sort" bind:value={sort} options={SORT_CHOICES.map((entry) => ({ value: entry.value, label: t(entry.label) }))} />
        </label>
        {#if fields.has('count')}
          <span class="flex h-8 items-center gap-2 text-[13px] text-ink">
            {t('app.rail.showCount')}
            <Switch testid="group-editor-count" label={t('app.rail.showCount')} bind:checked={counting} />
          </span>
        {/if}
      </div>
    {/if}

    <div class="flex flex-col gap-1">
      <p class="text-[13px] text-ink-2">{t('settings.group.inside')}</p>
      <button
        type="button"
        data-testid="group-editor-parent-open"
        data-parent={parent}
        aria-haspopup="listbox"
        aria-expanded={parentOpen}
        class="flex h-8 w-full items-center gap-2 rounded-[0.5rem] bg-paper-3 px-2 text-left text-[13px] text-ink hover:bg-paper-2 {parentOpen ? 'bg-paper-2' : ''}"
        bind:this={parentButton}
        onclick={() => (parentOpen = !parentOpen)}
      >
        <Icon icon={chosen ? iconOf(chosen) : 'lucide:corner-left-up'} class="shrink-0 text-[13px] text-ink-3" />
        <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{chosen ? trail : t('settings.group.topLevel')}</span>
        <Icon icon="lucide:chevron-down" class="shrink-0 text-[13px] text-ink-3" />
      </button>
      {#if parentOpen}
        <div class="menu flex w-[min(24rem,90vw)] flex-col gap-1" role="presentation" use:anchored={{ at: () => boxOf(parentButton), align: 'start', onclose: () => (parentOpen = false) }}>
          <Field data-testid="group-editor-parent-search" size="sm" placeholder={t('settings.group.findParent')} aria-label={t('settings.group.findParent')} autocomplete="off" bind:value={search} onkeydown={searchKeydown} />
          <div class="flex max-h-[40vh] flex-col gap-px overflow-y-auto py-1" role="listbox" aria-label={t('settings.group.inside')}>
            {@render choice('', t('settings.group.topLevel'), 'lucide:corner-left-up', 0, false)}
            {#each offered as entry (entry.group.id)}
              {@render choice(entry.group.id, entry.group.name, iconOf(entry.group), entry.depth, false)}
            {/each}
            {#if search.trim() && !offered.length}
              <p class="px-2 py-1.5 text-[12.5px] text-ink-3">{t('settings.group.noMatch')}</p>
            {/if}
          </div>
        </div>
      {/if}
    </div>

    {#if subject.group && (fields.has('newGroup') || fields.has('newAction'))}
      {@const current = subject.group}
      <div class="flex flex-wrap items-center gap-2">
        {#if fields.has('newAction') && onnewaction}
          <Button size="sm" icon="lucide:plus" testid="group-editor-new-action" onclick={() => onnewaction(current.id)}>{t('settings.group.newAction')}</Button>
        {/if}
        {#if fields.has('newGroup') && onnewgroup}
          <Button size="sm" icon="lucide:folder-plus" testid="group-editor-new-group" onclick={() => onnewgroup(current.id)}>{t('app.rail.newGroupInside')}</Button>
        {/if}
      </div>
    {/if}

    {#if failure}
      <p class="text-[13px] text-error-500">{failure}</p>
    {/if}

    {#if subject.group && fields.has('delete')}
      <div class="flex min-h-8 items-center gap-2 border-t border-line pt-4">
        {#if confirming}
          <p class="min-w-0 flex-1 text-[13px] text-ink-2">{t(confirmText, { name: subject.group.name })}</p>
          <Button size="sm" onclick={() => (confirming = false)}>{t('common.cancel')}</Button>
          <Button size="sm" variant="danger" icon="lucide:trash-2" testid="group-editor-delete-confirm" busy={deleting} onclick={remove}>{t('common.delete')}</Button>
        {:else}
          <Button size="sm" variant="danger" icon="lucide:trash-2" testid="group-editor-delete" onclick={() => (confirming = true)}>{t('settings.group.delete')}</Button>
        {/if}
      </div>
    {/if}
  </form>
  {#snippet footer()}
    <span class="mr-auto text-[12px] text-ink-3">{subject.scope === 'conversation' ? t(kindNames[kind as GroupKind]) : ''}</span>
    <Button testid="group-editor-cancel" onclick={oncancel}>{t('common.cancel')}</Button>
    <Button testid="group-editor-save" variant="primary" keys="⌘↵" busy={saving} disabled={!valid} onclick={save}>{subject.group ? t('common.save') : t('common.create')}</Button>
  {/snippet}
</Sheet>

{#if picking}
  <IconPicker value={icon} fallback={KIND_ICONS[kind]} onpick={(next) => (icon = next)} onclose={() => (picking = false)} />
{/if}
