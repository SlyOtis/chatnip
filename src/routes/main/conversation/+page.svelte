<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import { myUserIds, peopleStore } from '$lib/tauri/people'
  import { goto } from '$app/navigation'
  import { conversationsStore } from '$lib/tauri'
  import { Button, Kbd } from '$lib/components/ui'
  import { personInitials } from '$lib/tauri/people'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { listCollapsed, toggleList, selectedItem, itemConversations, itemPath, rowDate, rowModel } from '$lib/inbox'
  import { t } from '$lib/i18n.svelte'

  const item = $derived($selectedItem)
  const conversations = $derived(item ? [...itemConversations(item)].sort((a, b) => b.last_activity.localeCompare(a.last_activity)) : [])
  const name = $derived.by(() => {
    if (!item) return ''
    if (item.kind === 'person') return item.person.display_name
    if (item.kind === 'merged') return rowModel(item, $peopleStore, $myUserIds).title
    const c = item.conversation
    if (c.type === 'direct_message') {
      const other = c.members.find((m) => !$myUserIds.has(m.id))
      return other?.display_name ?? other?.email ?? c.name
    }
    return c.name
  })
  const line = $derived.by(() => {
    if (!item) return ''
    if (item.kind === 'person') return item.person.primary_email ?? t('conversation.overview.identities', { count: item.person.identities.length })
    if (item.kind === 'merged') return t('conversation.overview.chatsTogether', { count: item.conversations.length })
    const c = item.conversation
    if (c.type === 'direct_message') return c.members.find((m) => !$myUserIds.has(m.id))?.email ?? ''
    if (c.type === 'email') return c.members.map((m) => m.display_name ?? m.email).filter(Boolean).slice(0, 3).join(', ')
    return t('conversation.overview.members', { count: c.members.length })
  })
  const marks = $derived(conversations.flatMap((c) => c.providers.map((p) => ({ ...p, conversation_type: c.type }))))
  const recent = $derived(conversations.slice(0, 3))
</script>

<div class="relative flex h-full flex-col bg-paper">
  {#if $listCollapsed}
    <div class="absolute left-3 top-3">
      <Button size="sm" icon="lucide:panel-left-open" keys="⌘⇧L" onclick={toggleList}>{t('conversation.overview.showList')}</Button>
    </div>
  {/if}
  {#if item}
    <div class="mx-auto flex w-full max-w-[26rem] flex-1 flex-col justify-center px-8">
      <span class="flex size-14 items-center justify-center rounded-[0.875rem] bg-paper-3 text-[16px] font-medium text-ink-2">{personInitials(name)}</span>
      <h1 class="mt-5 text-[18px] font-medium leading-7 tracking-tight text-ink">{name}</h1>
      {#if line}
        <p class="mt-0.5 text-[14px] text-ink-3">{line}</p>
      {/if}
      {#if marks.length}
        <div class="mt-3 flex items-center gap-2 text-ink-3">
          <ProviderIcon providers={marks} size="md" max={6} />
          <span class="font-mono text-[11px]">{rowDate(conversations[0].last_activity)}</span>
        </div>
      {/if}
      <ul class="mt-7 flex flex-col">
        {#each recent as c (c.id)}
          <li>
            <button class="flex h-9 w-full items-center gap-3 text-left text-[14px] text-ink-2 hover:text-ink" onclick={() => goto(conversationHref(c.id))}>
              <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{c.name}</span>
              <span class="shrink-0 font-mono text-[11px] text-ink-3">{rowDate(c.last_activity)}</span>
            </button>
          </li>
        {/each}
      </ul>
      <div class="mt-8">
        <Button variant="secondary" keys="↵" onclick={() => goto(itemPath(item))}>{t('common.open')}</Button>
      </div>
    </div>
  {:else}
    <div class="empty-host relative flex flex-1 flex-col items-center justify-center gap-3 px-6 text-center">
      <div class="empty-frost" aria-hidden="true"></div>
      <p class="text-[15px] font-medium tracking-tight text-ink">
        {#if $conversationsStore.data.length === 0 && !$conversationsStore.is_loading}
          {t('conversation.overview.nothingToRead')}
        {:else}
          {t('conversation.empty.pick')}
        {/if}
      </p>
      <div class="flex items-center gap-5 text-[12px] text-ink-3">
        <span class="flex items-center gap-1.5"><Kbd keys="J K" /> {t('conversation.empty.move')}</span>
        <span class="flex items-center gap-1.5"><Kbd keys="↵" /> {t('conversation.empty.open')}</span>
        <span class="flex items-center gap-1.5"><Kbd keys="⌘K" /> {t('conversation.empty.search')}</span>
      </div>
    </div>
  {/if}
</div>
