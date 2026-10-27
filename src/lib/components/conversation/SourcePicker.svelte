<script lang="ts">
  import Icon from '@iconify/svelte'
  import { conversationsById, conversationsStore, groupsStore, peopleStore, tagRecordsStore, chatsStore, myUserIds } from '$lib/tauri'
  import { SOURCE_KINDS, chatChoices, participants, sameSource, toggleSource } from '$lib/tauri/chats'
  import { Avatar } from '$lib/components/ui'
  import { chatAvatar, personAvatar, type AvatarSource } from '$lib/avatar'
  import type { ChatSource } from '$lib/tauri/types'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    sources: ChatSource[]
    self?: string | null
    writes?: boolean
    chosen?: boolean
    onchange: (sources: ChatSource[]) => void
  }

  let { sources, self = null, writes = false, chosen = true, onchange }: Props = $props()

  let query = $state('')
  let kind = $state<(typeof SOURCE_KINDS)[number]['value']>('conversation')

  interface Choice {
    source: ChatSource
    label: string
    detail: string
    icon: string
    avatar?: AvatarSource
  }

  const local = $derived(new Set($chatsStore.map((c) => c.conversation_id)))
  const merged = $derived(writes ? chatChoices($conversationsStore.data, query, local, $myUserIds) : [])

  const choices = $derived.by((): Choice[] => {
    const conversation = (id: string, detail: string, icon: string): Choice => ({ source: { kind: 'conversation', id }, label: $conversationsById.get(id)?.name ?? id, detail, icon })
    switch (kind) {
      case 'conversation':
        return $conversationsStore.data.filter((c) => !local.has(c.id) && !c.deleted_at).map((c) => conversation(c.id, c.providers[0]?.name ?? '', c.mode === 'read' ? 'lucide:book-open' : 'lucide:message-square'))
      case 'chat':
        return $chatsStore.filter((c) => c.conversation_id !== self).map((c) => ({ source: { kind: 'conversation', id: c.conversation_id }, label: c.name, detail: c.mode === 'read' ? t('chats.mode.read') : t('chats.mode.write'), icon: c.icon ?? 'lucide:book-open' }))
      case 'group':
        return $groupsStore.map((g) => ({ source: { kind: 'group', id: g.id }, label: g.name, detail: g.kind === 'query' ? t('chats.source.savedSearch') : t('chats.source.group'), icon: g.icon ?? (g.kind === 'query' ? 'lucide:search' : 'lucide:folder') }))
      case 'tag':
        return $tagRecordsStore.filter((tag) => tag.kind === 'tag').map((tag) => ({ source: { kind: 'tag', id: tag.id }, label: tag.name, detail: '', icon: 'lucide:tag' }))
      case 'person':
        return $peopleStore.filter((p) => p.is_human).map((p) => ({ source: { kind: 'person', id: p.id }, label: p.display_name, detail: p.primary_email ?? '', icon: 'lucide:user', avatar: personAvatar(p) }))
      default: {
        const text = query.trim()
        return text ? [{ source: { kind: 'query', query: { text, sort: 'activity', direction: 'desc' } }, label: t('chats.source.searchFor', { text }), detail: '', icon: 'lucide:search' }] : []
      }
    }
  })

  const shown = $derived.by(() => {
    if (kind === 'query') return choices
    const needle = query.trim().toLowerCase()
    return (needle ? choices.filter((c) => `${c.label} ${c.detail}`.toLowerCase().includes(needle)) : choices).slice(0, 60)
  })

  function describe(source: ChatSource): { label: string; icon: string } {
    switch (source.kind) {
      case 'conversation': {
        const chat = $chatsStore.find((c) => c.conversation_id === source.id)
        if (chat) return { label: chat.name, icon: chat.icon ?? 'lucide:book-open' }
        return { label: $conversationsById.get(source.id ?? '')?.name ?? source.id ?? '', icon: 'lucide:message-square' }
      }
      case 'group': {
        const group = $groupsStore.find((g) => g.id === source.id)
        return { label: group?.name ?? source.id ?? '', icon: group?.icon ?? 'lucide:folder' }
      }
      case 'tag':
        return { label: $tagRecordsStore.find((tag) => tag.id === source.id)?.name ?? source.id ?? '', icon: 'lucide:tag' }
      case 'person':
        return { label: $peopleStore.find((p) => p.id === source.id)?.display_name ?? source.id ?? '', icon: 'lucide:user' }
      default:
        return { label: source.query?.text ? t('chats.source.searchFor', { text: source.query.text }) : t('chats.source.search'), icon: 'lucide:search' }
    }
  }
</script>

<div class="flex flex-col gap-2">
  {#if chosen && sources.length}
    <ul class="flex flex-col">
      {#each sources as source, i (i)}
        {@const shownSource = describe(source)}
        <li data-testid="source-chosen" data-kind={source.kind} data-id={source.id ?? ''} class="group/source grid h-7 grid-cols-[1.25rem_minmax(0,1fr)_auto] items-center gap-1.5 text-[13px] text-ink">
          <Icon icon={shownSource.icon} class="text-[13px] text-ink-3" />
          <span class="overflow-hidden text-ellipsis whitespace-nowrap">{shownSource.label}</span>
          <button type="button" class="flex size-6 items-center justify-center rounded-[0.375rem] text-ink-3 opacity-0 hover:bg-paper-3 hover:text-ink group-hover/source:opacity-100 focus-visible:opacity-100" data-testid="source-remove" aria-label={t('chats.source.remove', { name: shownSource.label })} onclick={() => onchange(sources.filter((s) => !sameSource(s, source)))}>
            <Icon icon="lucide:x" class="text-[13px]" />
          </button>
        </li>
      {/each}
    </ul>
  {:else if chosen}
    <p class="text-[12.5px] text-ink-3">{t('chats.source.none')}</p>
  {/if}
  {#if writes}
    <input class="h-8 w-full border-b border-line bg-transparent text-[13px] text-ink outline-none placeholder:text-ink-3 focus:border-ink-3" data-testid="source-find" placeholder={t('chats.source.findChats')} aria-label={t('chats.source.findChats')} bind:value={query} />
    <ul class="max-h-[16rem] overflow-y-auto">
      {#each merged as conversation (conversation.id)}
        {@const on = sources.some((s) => s.kind === 'conversation' && s.id === conversation.id)}
        <li>
          <button type="button" class="grid min-h-9 w-full grid-cols-[1.5rem_minmax(0,1fr)_1rem] items-center gap-1.5 rounded-[0.375rem] px-1 py-0.5 text-left text-[13px] {on ? 'text-ink' : 'text-ink-2'} hover:bg-paper-3 hover:text-ink" data-testid="source-merge-choice" data-id={conversation.id} aria-pressed={on} onclick={() => onchange(toggleSource(sources, { kind: 'conversation', id: conversation.id }))}>
            <Avatar source={chatAvatar(conversation, $peopleStore, $myUserIds)} size="sm" />
            <span class="flex min-w-0 flex-col leading-4">
              <span class="overflow-hidden text-ellipsis whitespace-nowrap">{conversation.name}</span>
              <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[11.5px] text-ink-3">{participants(conversation, $myUserIds) || conversation.providers[0]?.name || ''}</span>
            </span>
            <Icon icon={on ? 'lucide:check' : 'lucide:plus'} class="text-[13px] {on ? 'text-accent' : 'text-ink-3'}" />
          </button>
        </li>
      {:else}
        <li class="px-1 py-2 text-[12.5px] text-ink-3">{t('chats.source.nothing')}</li>
      {/each}
    </ul>
  {:else}
  <div class="flex flex-wrap gap-1" role="tablist" aria-label={t('chats.source.kinds')}>
    {#each SOURCE_KINDS as option (option.value)}
      <button type="button" role="tab" data-testid="source-kind" data-value={option.value} aria-selected={kind === option.value} class="flex h-7 items-center gap-1.5 whitespace-nowrap rounded-[0.375rem] px-2 text-[12.5px] {kind === option.value ? 'bg-accent-soft text-ink' : 'text-ink-2 hover:bg-paper-3 hover:text-ink'}" onclick={() => (kind = option.value)}>
        <Icon icon={option.icon} class="text-[13px]" />{t(option.label)}
      </button>
    {/each}
  </div>
  <input class="h-8 w-full border-b border-line bg-transparent text-[13px] text-ink outline-none placeholder:text-ink-3 focus:border-ink-3" placeholder={kind === 'query' ? t('chats.source.searchPlaceholder') : t('chats.source.filter')} data-testid="source-filter" aria-label={t('chats.source.filter')} bind:value={query} />
  <ul class="max-h-[16rem] overflow-y-auto">
    {#each shown as choice (JSON.stringify(choice.source))}
      {@const on = sources.some((s) => sameSource(s, choice.source))}
      <li>
        <button type="button" class="grid h-8 w-full grid-cols-[1.25rem_minmax(0,1fr)_auto_1rem] items-center gap-1.5 rounded-[0.375rem] px-1 text-left text-[13px] {on ? 'text-ink' : 'text-ink-2'} hover:bg-paper-3 hover:text-ink" data-testid="source-choice" data-kind={choice.source.kind} data-id={choice.source.id ?? ''} aria-pressed={on} onclick={() => onchange(toggleSource(sources, choice.source))}>
          {#if choice.avatar}<Avatar source={choice.avatar} size="xs" />{:else}<Icon icon={choice.icon} class="text-[13px] text-ink-3" />{/if}
          <span class="overflow-hidden text-ellipsis whitespace-nowrap">{choice.label}</span>
          <span class="max-w-[10rem] overflow-hidden text-ellipsis whitespace-nowrap text-[12px] text-ink-3">{choice.detail}</span>
          <Icon icon={on ? 'lucide:check' : 'lucide:plus'} class="text-[13px] {on ? 'text-accent' : 'text-ink-3'}" />
        </button>
      </li>
    {:else}
      <li class="px-1 py-2 text-[12.5px] text-ink-3">{kind === 'query' ? t('chats.source.typeToSearch') : t('chats.source.nothing')}</li>
    {/each}
  </ul>
  {/if}
</div>
