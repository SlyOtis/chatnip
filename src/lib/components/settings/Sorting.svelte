<script lang="ts">
  import Icon from '@iconify/svelte'
  import { api, loadTags, splitsStore, tagsStore, groupsStore, TAG_COLOURS } from '$lib/tauri'
  import type { Tag, TagKind } from '$lib/tauri/types'
  import { Button, ColourSwatch, Field, IconButton, IconPicker, Row, Section, Switch, TagMark } from '$lib/components/ui'
  import { onMount } from 'svelte'
  import { goto } from '$app/navigation'
  import Groups from './Groups.svelte'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    part: 'organise' | 'actions'
  }

  let { part }: Props = $props()

  let newSplit = $state('')
  let newTag = $state('')
  let failure = $state<string | null>(null)
  let sorting = $state(false)
  let sorted = $state<number | null>(null)
  let dragging = $state<number | null>(null)
  let dropAt = $state<number | null>(null)
  let actionsOn = $state(true)
  let drafts = $state<Record<TagKind, { icon: string | null; color: string }>>({ split: { icon: null, color: TAG_COLOURS[0] }, tag: { icon: null, color: TAG_COLOURS[0] } })
  let picking = $state<{ tag: Tag } | { kind: TagKind } | null>(null)

  onMount(() => {
    if (part === 'actions') api.getAppSettings().then((all) => (actionsOn = all['actions.enabled'] !== 'false')).catch((error) => (failure = String(error)))
  })

  async function putActions(on: boolean) {
    actionsOn = on
    try {
      await api.putAppSetting('actions.enabled', on ? 'true' : 'false')
      failure = null
    } catch (error) {
      actionsOn = !on
      failure = String(error)
    }
  }

  async function run(action: () => Promise<unknown>) {
    try {
      await action()
      await loadTags()
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  async function add(kind: TagKind) {
    const name = (kind === 'split' ? newSplit : newTag).trim()
    if (!name) return
    const position = (kind === 'split' ? $splitsStore : $tagsStore).length
    const draft = drafts[kind]
    await run(async () => {
      const made = await api.postTag({ name, kind, position, color: draft.color })
      if (draft.icon) await api.putTag({ tag_id: made.id, icon: draft.icon })
    })
    drafts[kind] = { icon: null, color: TAG_COLOURS[0] }
    if (kind === 'split') newSplit = ''
    else newTag = ''
  }

  async function rename(label: Tag, name: string) {
    if (name.trim() === label.name || !name.trim()) return
    await run(() => api.putTag({ tag_id: label.id, name: name.trim() }))
  }

  async function describe(label: Tag, description: string) {
    if (description.trim() === label.description) return
    await run(() => api.putTag({ tag_id: label.id, description: description.trim() }))
  }

  async function recolor(label: Tag, color: string) {
    if (color === label.color) return
    await run(() => api.putTag({ tag_id: label.id, color }))
  }

  async function reicon(label: Tag, icon: string | null) {
    await run(() => api.putTag({ tag_id: label.id, icon: icon ?? '' }))
  }

  async function reorder(list: Tag[], from: number, to: number) {
    if (from === to) return
    const order = [...list]
    const [item] = order.splice(from, 1)
    order.splice(to, 0, item)
    await run(() => Promise.all(order.map((label, position) => api.putTag({ tag_id: label.id, position }))))
  }

  async function move(list: Tag[], index: number, delta: number) {
    const target = index + delta
    if (target < 0 || target >= list.length) return
    await reorder(list, index, target)
  }

  async function remove(label: Tag) {
    if (!confirm(t('settings.sorting.removeTag', { name: label.name }))) return
    await run(() => api.deleteTag(label.id))
  }

  async function resort(withAssistant: boolean) {
    sorting = true
    sorted = null
    try {
      sorted = (await api.postActionBackfill(withAssistant ? null : 'sort_splits', 200)).runs
      failure = null
    } catch (error) {
      failure = String(error)
    } finally {
      sorting = false
    }
  }

  const tagGroupCount = $derived((label: Tag) => $groupsStore.filter((g) => g.tag_id === label.id).length)
</script>

{#snippet splitMark()}
  <TagMark tag={{ name: newSplit, color: drafts.split.color, icon: drafts.split.icon }} />
{/snippet}

{#snippet tagMark()}
  <TagMark tag={{ name: newTag, color: drafts.tag.color, icon: drafts.tag.icon }} />
{/snippet}

{#snippet tagRow(label: Tag, index: number, list: Tag[], kind: TagKind)}
  <li
    class="group grid h-10 grid-cols-[1rem_auto_minmax(0,1fr)_auto] items-center gap-3 border-b border-line last:border-b-0 {dropAt === index && dragging !== index ? 'shadow-[inset_0_2px_0_var(--accent)]' : ''}"
    draggable="true"
    ondragstart={(e) => {
      dragging = index
      e.dataTransfer?.setData('text/plain', label.id)
    }}
    ondragover={(e) => {
      e.preventDefault()
      dropAt = index
    }}
    ondragleave={() => (dropAt = null)}
    ondrop={(e) => {
      e.preventDefault()
      if (dragging !== null) reorder(list, dragging, index)
      dragging = null
      dropAt = null
    }}
    ondragend={() => {
      dragging = null
      dropAt = null
    }}
  >
    <Icon icon="lucide:grip-vertical" class="cursor-grab text-[14px] text-ink-3 opacity-0 group-hover:opacity-100" />
    <span class="flex items-center gap-0.5">
      <button type="button" class="flex size-7 items-center justify-center rounded-[0.5rem] bg-paper-3 hover:bg-paper-2" aria-label={t('app.iconPicker.change')} onclick={() => (picking = { tag: label })}><TagMark tag={label} /></button>
      <ColourSwatch size="sm" value={label.color} onpick={(next) => next && recolor(label, next)} />
    </span>
    <div class="flex min-w-0 items-baseline gap-2">
      <input class="w-40 shrink-0 bg-transparent text-[14px] font-medium text-ink outline-none placeholder:text-ink-3" value={label.name} onchange={(e) => rename(label, e.currentTarget.value)} aria-label={t('common.name')} />
      <input class="min-w-0 flex-1 truncate bg-transparent text-[13px] text-ink-3 outline-none placeholder:text-ink-3 focus:text-ink-2" value={label.description} placeholder={kind === 'split' ? t('settings.sorting.splitPlaceholder') : t('settings.sorting.tagPlaceholder')} onchange={(e) => describe(label, e.currentTarget.value)} aria-label={t('settings.sorting.description')} />
      {#if kind === 'tag' && tagGroupCount(label) > 0}
        <span class="shrink-0 font-mono text-[11px] text-ink-3">{t('settings.sorting.group')}</span>
      {/if}
      <span class="shrink-0 font-mono text-[11px] text-ink-3">{index + 1}</span>
    </div>
    <div class="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 focus-within:opacity-100">
      <IconButton size="sm" icon="lucide:chevron-up" label={t('settings.moveUp')} disabled={index === 0} onclick={() => move(list, index, -1)} />
      <IconButton size="sm" icon="lucide:chevron-down" label={t('settings.moveDown')} disabled={index === list.length - 1} onclick={() => move(list, index, 1)} />
      <IconButton size="sm" icon="lucide:x" label={t('common.remove')} tone="danger" onclick={() => remove(label)} />
    </div>
  </li>
{/snippet}

<div>
  {#if failure}
    <div class="flex items-center gap-3 border-b border-line py-3 text-[13px] text-ink">
      <Icon icon="lucide:circle-alert" class="text-[15px] text-error-500" />
      <p class="flex-1">{failure}</p>
      <Button size="sm" onclick={() => (failure = null)}>{t('common.dismiss')}</Button>
    </div>
  {/if}

  {#if part === 'organise'}
  <Section id="organise.splits" title={t('settings.sorting.tabs')} description={t('settings.sorting.tabsDescription')} total={$splitsStore.length}>
    {#snippet children(shown)}
      <ul role="list" aria-label={t('settings.sorting.splits')}>
        {#each $splitsStore.slice(0, shown) as split, index (split.id)}
          {@render tagRow(split, index, $splitsStore, 'split')}
        {/each}
      </ul>
    {/snippet}
    {#snippet footer()}
      <form
        class="flex h-10 items-center gap-2 pl-7"
        onsubmit={(e) => {
          e.preventDefault()
          add('split')
        }}
      >
        <span class="flex w-full max-w-xs items-center gap-1"><Field size="sm" placeholder={t('settings.sorting.newSplit')} bind:value={newSplit} glyph={splitMark} onicon={() => (picking = { kind: 'split' })} /><ColourSwatch size="sm" value={drafts.split.color} onpick={(next) => next && (drafts.split.color = next)} /></span>
        <Button type="submit" size="sm" variant="secondary" disabled={!newSplit.trim()}>{t('common.add')}</Button>
      </form>
    {/snippet}
  </Section>

  <Section id="organise.tags" title={t('settings.inbox.tags')} description={t('settings.sorting.tagsDescription')} total={$tagsStore.length}>
    {#snippet children(shown)}
      <ul role="list" aria-label={t('settings.inbox.tags')}>
        {#each $tagsStore.slice(0, shown) as label, index (label.id)}
          {@render tagRow(label, index, $tagsStore, 'tag')}
        {/each}
      </ul>
    {/snippet}
    {#snippet footer()}
      <form
        class="flex h-10 items-center gap-2 pl-7"
        onsubmit={(e) => {
          e.preventDefault()
          add('tag')
        }}
      >
        <span class="flex w-full max-w-xs items-center gap-1"><Field size="sm" placeholder={t('settings.sorting.newTag')} bind:value={newTag} glyph={tagMark} onicon={() => (picking = { kind: 'tag' })} /><ColourSwatch size="sm" value={drafts.tag.color} onpick={(next) => next && (drafts.tag.color = next)} /></span>
        <Button type="submit" size="sm" variant="secondary" disabled={!newTag.trim()}>{t('common.add')}</Button>
      </form>
    {/snippet}
  </Section>

  <Groups />
  {:else}
  <Row id="actions.enabled" label={t('settings.actions.enabled')} hint={t('settings.actions.enabledHint')}>
    <Switch label={t('settings.actions.enabled')} checked={actionsOn} onchange={putActions} />
  </Row>

  <Section id="actions.run" title={t('settings.sorting.title')} description={t('settings.sorting.sortingDescription')}>
    <div class="flex flex-wrap items-center gap-2">
      <Button variant="primary" icon="lucide:zap" onclick={() => goto('/main/actions')}>{t('settings.sorting.openActions')}</Button>
      <Button busy={sorting} onclick={() => resort(true)}>{t('settings.sorting.runAll')}</Button>
      <Button disabled={sorting} onclick={() => resort(false)}>{t('settings.sorting.runSplits')}</Button>
      {#if sorted !== null && !sorting}
        <span class="font-mono text-[12px] text-ink-2">{t('settings.sorting.runs', { count: sorted })}</span>
      {/if}
    </div>
  </Section>
  {/if}
</div>

{#if picking}
  {@const target = picking}
  <IconPicker
    value={'tag' in target ? target.tag.icon : drafts[target.kind].icon}
    fallback="lucide:circle-small"
    resetLabel={t('app.tagEditor.useDot')}
    onpick={(icon) => ('tag' in target ? reicon(target.tag, icon) : (drafts[target.kind].icon = icon))}
    onclose={() => (picking = null)}
  />
{/if}
