<script lang="ts">
  import { mergedHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { api, groupsStore } from '$lib/tauri'
  import { OBJECT_ICONS, groupTagged, objectTagsStore } from '$lib/tauri/tags'
  import type { ObjectType, TaggedObject } from '$lib/tauri/types'
  import { messageHref } from '$lib/messageLink'
  import { excerptOf } from '$lib/reply.svelte'
  import { openGroup, openInbox } from '$lib/inbox'
  import { formatDate, t, type MessageKey } from '$lib/i18n.svelte'

  interface Props {
    tagId: string
    count?: number
  }

  let { tagId, count = $bindable(0) }: Props = $props()

  let objects = $state<TaggedObject[]>([])
  let open = $state<Record<string, boolean>>({})

  const groups = $derived(groupTagged(objects, ['conversation']))

  $effect(() => {
    const id = tagId
    void $objectTagsStore
    let live = true
    api
      .getTaggedObjects(id)
      .then((found) => {
        if (live) objects = found
      })
      .catch(() => {
        if (live) objects = []
      })
    return () => {
      live = false
    }
  })

  $effect(() => {
    count = groups.reduce((sum, group) => sum + group.objects.length, 0)
  })

  function label(type: ObjectType): string {
    return t(`inbox.tagged.${type}` as MessageKey)
  }

  function when(value: string | null): string {
    return value ? formatDate(value, { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : ''
  }

  function visit(object: TaggedObject) {
    if ((object.object_type === 'message' || object.object_type === 'attachment') && object.conversation_id && object.message_id) goto(messageHref(object.conversation_id, object.message_id))
    else if ((object.object_type === 'person' || object.object_type === 'identity') && object.person_id) goto(`/main/person/${object.person_id}/profile`)
    else if (object.object_type === 'content') goto('/main/content')
    else if (object.object_type === 'event') goto('/main/calendar')
    else if (object.object_type === 'action' || object.object_type === 'action_group') goto('/main/actions')
    else if (object.object_type === 'inbox') openInbox(object.object_id)
    else if (object.object_type === 'merge_group' && object.conversation_id) goto(mergedHref(object.conversation_id))
    else if (object.object_type === 'group') openGroup($groupsStore.find((group) => group.id === object.object_id) ?? null)
  }
</script>

{#each groups as group, g (group.type)}
  {@const shut = open[group.type] === false}
  <div class="group/section" data-tagged-type={group.type}>
    <div class="sticky top-0 z-[2] flex h-7 items-center gap-2 bg-paper pl-3 pr-4 {g === 0 ? 'mt-1' : 'mt-5'}">
      <button class="flex min-w-0 flex-1 items-center gap-2 text-left text-[13px] text-ink-3 hover:text-ink-2" onclick={() => (open = { ...open, [group.type]: shut })} aria-expanded={!shut}>
        <Icon icon={OBJECT_ICONS[group.type]} class="shrink-0 text-[13px]" />
        <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{label(group.type)}</span>
        <span class="font-mono text-[11px]">{group.objects.length}</span>
      </button>
      <Icon icon={shut ? 'lucide:chevron-right' : 'lucide:chevron-down'} class="shrink-0 text-[13px] text-ink-3" />
    </div>
    {#if !shut}
      {#each group.objects as object (object.object_type + object.object_id)}
        {@const messageLike = object.object_type === 'message' || object.object_type === 'attachment'}
        <button type="button" class="flex w-full min-w-0 flex-col gap-0.5 rounded-[0.5rem] px-3 py-1.5 text-left hover:bg-ink/5" onclick={() => visit(object)} data-tagged-object={object.object_id}>
          <span class="flex min-w-0 items-baseline gap-2 text-[14px]">
            <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-medium text-ink">{messageLike ? (object.author ?? object.title) : object.title || t('common.unknown')}</span>
            {#if messageLike && object.conversation_name}
              <span class="min-w-0 shrink overflow-hidden text-ellipsis whitespace-nowrap text-[12.5px] text-ink-3">{object.conversation_name}</span>
            {:else if object.detail}
              <span class="min-w-0 shrink overflow-hidden text-ellipsis whitespace-nowrap text-[12.5px] text-ink-3">{object.detail}</span>
            {/if}
            <span class="ml-auto shrink-0 font-mono text-[11px] text-ink-3">{when(object.timestamp)}</span>
          </span>
          {#if messageLike && (object.excerpt || object.title)}
            <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[13px] text-ink-2">{object.object_type === 'attachment' ? object.title : excerptOf(object.excerpt, 160)}</span>
          {:else if object.excerpt}
            <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[13px] text-ink-2">{excerptOf(object.excerpt, 160)}</span>
          {/if}
        </button>
      {/each}
    {/if}
  </div>
{/each}
