<script lang="ts">
  import { goto } from '$app/navigation'
  import { myUserIds, peopleStore } from '$lib/tauri/people'
  import { openChatDetails } from '$lib/tauri/chats'
  import Icon from '@iconify/svelte'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { Avatar, IconButton } from '$lib/components/ui'
  import { rowDate, rowModel, itemActivity, itemUnread, itemConversations, type InboxItem } from '$lib/inbox'
  import { t } from '$lib/i18n.svelte'
  import { renderEmojiText } from '$lib/emoji'

  interface Props {
    item: InboxItem
    focused: boolean
    active: boolean
    picked: boolean
    onopen: () => void
    onfocus: () => void
    onpick: () => void
    ondone: () => void
    onsnooze: () => void
    onmore: () => void
    ondelete: () => void
    expanded?: boolean
    onexpand?: () => void
  }

  let { item, focused, active, picked, onopen, onfocus, onpick, ondone, onsnooze, onmore, ondelete, expanded = false, onexpand }: Props = $props()

  const conversations = $derived(itemConversations(item))
  const latest = $derived([...conversations].sort((a, b) => b.last_activity.localeCompare(a.last_activity))[0])
  const unread = $derived(itemUnread(item) > 0)
  const model = $derived(rowModel(item, $peopleStore, $myUserIds))
  const isMail = $derived(latest.type === 'email')
  const isDirect = $derived(item.kind === 'conversation' && item.conversation.type === 'direct_message')
  const sender = $derived(model.title)
  const channels = $derived(conversations.flatMap((c) => c.providers.map((p) => ({ ...p, conversation_type: c.type }))))
  const preview = $derived.by(() => {
    const text = renderEmojiText(latest.summary ?? '').replace(/\s+/g, ' ').trim()
    const fold = (value: string) => value.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[._-]+/g, ' ').trim()
    const names = new Set([sender, ...(item.kind === 'person' ? item.person.identities.flatMap((i) => [i.display_name ?? '', (i.email ?? '').split('@')[0]]) : [])].filter(Boolean).map(fold))
    const colon = text.indexOf(': ')
    if (colon > 0 && colon < 48 && names.has(fold(text.slice(0, colon)))) return text.slice(colon + 2)
    return text
  })
  const subject = $derived.by(() => {
    if (isMail && latest.name !== sender) return latest.name
    if (!isMail && !isDirect && item.kind === 'conversation') return latest.name
    const words = preview.split(' ')
    return words.slice(0, 6).join(' ')
  })
  const pinned = $derived(conversations.some((c) => c.is_pinned))
  const muted = $derived(conversations.every((c) => c.is_muted))
  const deleted = $derived(conversations.every((c) => !!c.deleted_at))
  const done = $derived(conversations.every((c) => c.is_archived))
  const showActions = $derived(focused)
  function image() {
    if (model.image.kind === 'profile') goto(model.image.path)
    else openChatDetails(model.image.conversationId)
  }
</script>

<div
  class="group relative grid h-[var(--row)] cursor-default grid-cols-[2.25rem_minmax(0,1fr)] items-center gap-x-3 pl-3 pr-4 {active ? 'bg-accent-soft/60' : focused || picked ? 'bg-ink/8' : 'hover:bg-ink/5'}"
  role="option"
  data-testid="inbox-row"
  data-id={item.kind === 'person' ? item.person.id : item.kind === 'merged' ? item.id : item.conversation.id}
  data-kind={item.kind}
  data-unread={unread}
  data-focused={focused}
  aria-selected={active}
  tabindex="-1"
  onclick={(e) => (e.metaKey || e.shiftKey ? onpick() : onopen())}
  onmouseenter={onfocus}
  onkeydown={(e) => e.key === 'Enter' && onopen()}
>
  <Avatar testid="inbox-row-avatar" source={model.avatar} size="lg" {picked} label={model.image.kind === 'profile' ? t('inbox.row.openProfile', { name: sender }) : t('inbox.row.chatDetails', { name: sender })} hint={model.image.kind === 'profile' ? t('inbox.row.openProfile', { name: sender }) : t('inbox.row.chatDetails', { name: sender })} onclick={(e) => (e.metaKey || e.shiftKey || picked ? onpick() : image())} />

  <div class="min-w-0">
    <div class="flex min-w-0 items-center gap-1.5">
      {#if unread}
        <span data-testid="inbox-row-unread" class="size-1.5 shrink-0 rounded-full bg-accent"></span>
      {/if}
      <span data-testid="inbox-row-name" class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[14px] leading-5 {unread ? 'font-semibold text-ink' : 'font-medium text-ink'}">{sender}</span>
      <ProviderIcon providers={channels} size="xs" max={3} />
      {#if pinned}
        <Icon icon="lucide:star" class="shrink-0 fill-current text-[11px] text-signal" />
      {/if}
      {#if muted}
        <Icon icon="lucide:bell-off" class="shrink-0 text-[11px] text-ink-3" />
      {/if}
      <span class="relative ml-auto flex h-5 w-[5.75rem] shrink-0 items-center justify-end pl-2">
        {#if showActions}
          <span class="absolute -right-1 top-1/2 flex -translate-y-1/2 items-center gap-0.5">
            {#if deleted}
              <IconButton testid="inbox-row-restore" size="sm" icon="lucide:archive-restore" label={t('inbox.restore')} keys="#" onclick={(e) => { e.stopPropagation(); ondelete() }} />
            {:else}
              <IconButton testid="inbox-row-done" size="sm" icon={done ? 'lucide:archive-restore' : 'lucide:check'} label={done ? t('inbox.moveToInbox') : t('inbox.view.done')} keys="E" onclick={(e) => { e.stopPropagation(); ondone() }} />
              <IconButton testid="inbox-row-snooze" size="sm" icon="lucide:clock" label={t('inbox.snooze')} keys="H" onclick={(e) => { e.stopPropagation(); onsnooze() }} />
              <IconButton testid="inbox-row-more" size="sm" icon="lucide:ellipsis" label={t('common.more')} keys="L" onclick={(e) => { e.stopPropagation(); onmore() }} />
            {/if}
          </span>
        {:else}
          <span class="font-mono text-[11px] {unread ? 'text-accent' : 'text-ink-3'}">{rowDate(itemActivity(item))}</span>
        {/if}
      </span>
    </div>
    <div class="flex min-w-0 items-center gap-1">
      <div class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-[12.5px] leading-[1.125rem] text-ink-3">
        {#if isMail && subject && subject !== preview}
          <span class="{unread ? 'text-ink-2' : ''}">{subject}</span><span>&nbsp;·&nbsp;</span>
        {/if}{preview || latest.name}
      </div>
      {#if onexpand}
        <button type="button" data-testid="inbox-row-expand" class="flex size-5 shrink-0 items-center justify-center rounded text-ink-3 hover:text-ink" aria-expanded={expanded} aria-label={expanded ? t('inbox.collapse') : t('inbox.expand')} onclick={(e) => { e.stopPropagation(); onexpand() }}><Icon icon={expanded ? 'lucide:chevron-down' : 'lucide:chevron-right'} class="text-[13px]" /></button>
      {/if}
    </div>
  </div>
</div>
