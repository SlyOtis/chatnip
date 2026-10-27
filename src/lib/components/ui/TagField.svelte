<script lang="ts">
  import Icon from '@iconify/svelte'
  import TagMark from './TagMark.svelte'
  import TagPicker from './TagPicker.svelte'
  import { objectTagsStore, tagIdsOn, tagRecordsStore, toggleObjectTag } from '$lib/tauri/tags'
  import type { ObjectType, Tag } from '$lib/tauri/types'
  import { boxOf, type Box } from '$lib/popover'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    objectType: ObjectType
    objectId: string
  }

  let { objectType, objectId }: Props = $props()

  let at = $state<Box | null>(null)

  const tags = $derived(tagIdsOn($objectTagsStore, objectType, objectId).map((id) => $tagRecordsStore.find((tag) => tag.id === id)).filter((tag): tag is Tag => !!tag))
</script>

<div class="flex min-w-0 flex-wrap items-center gap-1.5" data-tag-field={objectType}>
  {#each tags as tag (tag.id)}
    <span class="flex h-6 items-center gap-1 rounded-full bg-paper-2 pl-2 pr-1 text-[12.5px]">
      <TagMark {tag} size="xs" label />
      <button type="button" class="flex size-4 items-center justify-center rounded-full text-ink-3 hover:bg-paper-3 hover:text-ink" aria-label={t('people.tags.remove', { name: tag.name })} onclick={() => toggleObjectTag(objectType, [objectId], tag.id).catch(() => {})}>
        <Icon icon="lucide:x" class="text-[11px]" />
      </button>
    </span>
  {/each}
  <button type="button" class="flex h-6 items-center gap-1.5 rounded-full px-2 text-[12.5px] text-ink-2 hover:bg-paper-3 hover:text-ink" aria-haspopup="dialog" aria-expanded={!!at} onclick={(e) => (at = at ? null : boxOf(e.currentTarget))}>
    <Icon icon="lucide:tag" class="text-[12px] text-ink-3" />{tags.length ? t('common.add') : t('people.tags.add')}
  </button>
</div>

{#if at}
  <TagPicker {objectType} ids={[objectId]} {at} onclose={() => (at = null)} />
{/if}
