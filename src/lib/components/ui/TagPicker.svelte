<script lang="ts">
  import Icon from '@iconify/svelte'
  import Field from './Field.svelte'
  import IconPicker from './IconPicker.svelte'
  import ColourSwatch from './ColourSwatch.svelte'
  import TagMark from './TagMark.svelte'
  import { api } from '$lib/tauri'
  import { TAG_COLOURS, loadTags, objectTagsStore, tagRecordsStore, toggleObjectTag } from '$lib/tauri/tags'
  import type { ObjectTag, ObjectType, Tag } from '$lib/tauri/types'
  import { anchored, type Box } from '$lib/popover'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    objectType: ObjectType
    ids: string[]
    at?: Box | (() => Box | null) | null
    current?: ObjectTag[]
    onclose: () => void
  }

  let { objectType, ids, at = null, current, onclose }: Props = $props()

  let query = $state('')
  let icon = $state<string | null>(null)
  let color = $state(TAG_COLOURS[0])
  let picking = $state(false)
  let failure = $state<string | null>(null)
  let panel = $state<HTMLDivElement | null>(null)

  $effect(() => {
    panel?.querySelector<HTMLInputElement>('input')?.focus()
  })

  const assigned = $derived(current ?? $objectTagsStore)
  const choices = $derived($tagRecordsStore.filter((tag) => tag.kind === 'tag' || objectType === 'conversation'))
  const needle = $derived(query.trim().toLowerCase())
  const shown = $derived(needle ? choices.filter((tag) => tag.name.toLowerCase().includes(needle)) : choices)
  const creatable = $derived(!!needle && !choices.some((tag) => tag.name.toLowerCase() === needle))

  function count(tag: Tag): number {
    return ids.filter((id) => assigned.some((link) => link.object_type === objectType && link.object_id === id && link.tag_id === tag.id)).length
  }

  async function toggle(tag: Tag) {
    try {
      await toggleObjectTag(objectType, ids, tag.id, assigned)
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  async function create() {
    const name = query.trim()
    if (!name) return
    try {
      const made = await api.postTag({ name, color, kind: 'tag' })
      if (icon) await api.putTag({ tag_id: made.id, icon })
      await loadTags()
      await toggleObjectTag(objectType, ids, made.id, assigned)
      query = ''
      icon = null
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      onclose()
      return
    }
    if (e.key !== 'Enter') return
    e.preventDefault()
    if (creatable && !shown.length) create()
    else if (shown[0]) toggle(shown[0])
  }
</script>

{#snippet mark()}
  <TagMark tag={{ name: query, color, icon }} size="sm" />
{/snippet}

<div bind:this={panel} class="menu flex w-[min(19rem,90vw)] flex-col" use:anchored={{ at, align: 'start', onclose }} role="dialog" data-testid="tag-picker" aria-label={t('app.tagPicker.title')} tabindex="-1" onkeydown={keydown}>
  <div class="px-1 pb-1">
    <Field data-testid="tag-picker-search" size="sm" bind:value={query} placeholder={t('app.tagPicker.search')} aria-label={t('app.tagPicker.search')} autocomplete="off" />
  </div>
  <div class="flex max-h-[min(20rem,50vh)] flex-col overflow-y-auto" role="menu">
    {#each shown as tag (tag.id)}
      {@const on = count(tag)}
      <button type="button" role="menuitemcheckbox" data-testid="tag-picker-item" data-id={tag.id} aria-checked={on === ids.length ? 'true' : on > 0 ? 'mixed' : 'false'} class="flex h-8 w-full shrink-0 items-center gap-2.5 rounded-[0.5rem] px-2.5 text-left text-[14px] text-ink hover:bg-paper-3" onclick={() => toggle(tag)}>
        <span class="flex w-4 shrink-0 justify-center"><TagMark {tag} /></span>
        <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{tag.name}</span>
        {#if on === ids.length && on > 0}
          <Icon icon="lucide:check" class="shrink-0 text-[15px] text-accent" />
        {:else if on > 0}
          <Icon icon="lucide:minus" class="shrink-0 text-[15px] text-ink-3" />
        {/if}
      </button>
    {/each}
    {#if !shown.length && !creatable}
      <p class="px-2.5 py-1.5 text-[13px] text-ink-3">{t('app.tagPicker.empty')}</p>
    {/if}
  </div>
  {#if creatable}
    <div class="mt-1 flex items-center gap-1 border-t border-line px-1 pt-1.5">
      <Field size="sm" bind:value={query} glyph={mark} onicon={() => (picking = true)} aria-label={t('app.tagEditor.name')} autocomplete="off" />
      <ColourSwatch size="sm" value={color} onpick={(next) => next && (color = next)} />
      <button type="button" data-testid="tag-picker-create" class="flex h-7 shrink-0 items-center gap-1.5 rounded-[0.4rem] px-2 text-[13px] font-medium text-accent hover:bg-accent-soft" onclick={create}>
        <Icon icon="lucide:plus" class="text-[13px]" />{t('common.create')}
      </button>
    </div>
  {/if}
  {#if failure}
    <p class="px-2.5 pt-1 text-[12.5px] text-error-600">{failure}</p>
  {/if}
</div>

{#if picking}
  <IconPicker value={icon} fallback="lucide:circle-small" resetLabel={t('app.tagEditor.useDot')} onpick={(next) => (icon = next)} onclose={() => (picking = false)} />
{/if}
