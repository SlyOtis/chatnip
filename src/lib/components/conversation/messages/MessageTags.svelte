<script lang="ts">
  import { TagMark } from '$lib/components/ui'
  import { objectTagsStore, tagIdsOn, tagRecordsStore } from '$lib/tauri/tags'
  import type { Tag } from '$lib/tauri/types'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    messageId: string
  }

  let { messageId }: Props = $props()

  const tags = $derived(tagIdsOn($objectTagsStore, 'message', messageId).map((id) => $tagRecordsStore.find((tag) => tag.id === id)).filter((tag): tag is Tag => !!tag))
</script>

{#if tags.length}
  <span class="mt-0.5 flex flex-wrap items-center gap-1.5 px-1" role="list" aria-label={t('conversation.info.tags')} data-message-tags>
    {#each tags as tag (tag.id)}
      <span role="listitem" class="inline-flex items-center rounded-full bg-paper-3/60 px-2 py-px text-[11px] leading-4"><TagMark {tag} size="xs" label ink="ink-2" /></span>
    {/each}
  </span>
{/if}
