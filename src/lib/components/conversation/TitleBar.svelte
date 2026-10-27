<script lang="ts">
  import { goto } from '$app/navigation'
  import { displayName, myUserIds, peopleStore, personOf, personProfileHref } from '$lib/tauri/people'
  import { openChatDetails } from '$lib/tauri/chats'
  import { chatAvatar, personAvatar, userAvatar } from '$lib/avatar'
  import Icon from '@iconify/svelte';
  import { conversationStore, conversationsStore, tagRecordsStore, objectTagsStore, tagIdsOn, casesStore, openCasePicker, readStore, loadPinned, pinnedStore } from '$lib/tauri';
  import { memberships } from '$lib/tauri/cases';
  import { profileSettingsStore, putDetailsOpen } from '$lib/tauri/profile';
  import { shell } from '$lib/appearance';
  import { FEATURES } from '$lib/settings';
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte';
  import { anchored, boxOf, sheetAnchor } from '$lib/popover';
  import { revealMessage } from '$lib/messageLink';
  import { showTooltip } from '$lib/tooltip';
  import { counterpartOf, inboxSheet, listCollapsed, markDone, openTag, remove, toggleList, type RowModel } from '$lib/inbox';
  import type { Conversation, Person } from '$lib/tauri/types';
  import { Avatar, IconButton, TagMark } from '$lib/components/ui';
  import NotificationBell from './NotificationBell.svelte';
  import ExportMenu from './ExportMenu.svelte';
  import { chatExport } from '$lib/export.svelte';
  import { selection } from '$lib/reply.svelte';
  import PinnedMessages from './details/PinnedMessages.svelte';
  import { t, type MessageKey } from '$lib/i18n.svelte';

  interface Props {
    person?: Person | null;
    merged?: RowModel | null;
    mergeGroupId?: string | null;
    personConversations?: Conversation[];
  }

  let { person = null, merged = null, mergeGroupId = null, personConversations = [] }: Props = $props();

  const conversation = $derived($conversationStore.data);
  const provider = $derived(conversation?.providers?.[0] ?? null);

  const counterpart = $derived(conversation ? counterpartOf(conversation, $myUserIds) : null)

  const title = $derived(merged ? merged.title : person ? person.display_name : counterpart && conversation ? displayName($peopleStore, conversation, counterpart.id, counterpart.display_name ?? counterpart.email ?? conversation.name) : (conversation?.name ?? ''));

  const counterpartPerson = $derived(counterpart ? personOf($peopleStore, { user_id: counterpart.id }) : null);
  const avatar = $derived(merged ? merged.avatar : person ? personAvatar(person) : counterpart ? userAvatar(counterpart, $peopleStore, title) : conversation ? chatAvatar(conversation, $peopleStore, $myUserIds) : null);
  const profileTarget = $derived(merged ? null : (person ?? counterpartPerson));

  function openImage() {
    if (profileTarget) goto(personProfileHref(profileTarget.id));
    else if (merged?.image.kind === 'chat') openChatDetails(merged.image.conversationId);
    else if (conversation) openChatDetails(conversation.id);
  }

  const typeLabel: Record<string, MessageKey> = {
    direct_message: 'conversation.type.direct',
    group_message: 'conversation.type.group',
    channel: 'conversation.type.channel',
    email: 'conversation.type.email',
    local: 'chats.type.local',
    unknown: 'conversation.type.unknown',
  };

  const targets = $derived(person || merged ? personConversations : conversation ? [conversation] : []);
  const targetIds = $derived(targets.map((c) => c.id));

  $effect(() => {
    chatExport.ids = targetIds;
    chatExport.title = title;
    return () => {
      chatExport.ids = [];
    };
  });
  const done = $derived(targets.length > 0 && targets.every((c) => c.is_archived));
  const deleted = $derived(targets.length > 0 && targets.every((c) => !!c.deleted_at));
  const inCases = $derived(memberships($casesStore, targetIds).size);

  const sources = $derived.by(() => {
    const seen = new Map<string, string>()
    for (const c of targets) for (const p of c.providers) seen.set(p.id, p.name)
    return [...seen.values()]
  })

  const reading = $derived(!person && !merged && !!conversation && $readStore.has(conversation.id));
  const meta = $derived.by(() => {
    const parts: string[] = []
    if (merged) {
      parts.push(...sources)
      parts.push(t('conversation.title.conversations', { count: personConversations.length }))
      return parts
    }
    if (person) {
      if (person.primary_email) parts.push(person.primary_email)
      parts.push(...sources)
      if (personConversations.length > 1) parts.push(t('conversation.title.conversations', { count: personConversations.length }))
      if (!person.is_human) parts.push(t('conversation.title.service'))
      return parts
    }
    if (!conversation) return parts
    parts.push(typeLabel[conversation.type] ? t(typeLabel[conversation.type]) : conversation.type)
    if (counterpart?.email) parts.push(counterpart.email)
    if (provider) parts.push(provider.name)
    if ((conversation.type === 'channel' || conversation.type === 'group_message') && conversation.members.length > 0) parts.push(t('conversation.overview.members', { count: conversation.members.length }))
    if (reading) parts.push(t('chats.mode.read'))
    return parts
  });

  const chips = $derived(
    [...(merged && mergeGroupId ? tagIdsOn($objectTagsStore, 'merge_group', mergeGroupId) : []), ...targets.flatMap((c) => c.tags).map((l) => l.tag_id)]
      .map(id => $tagRecordsStore.find(label => label.id === id))
      .filter((l): l is NonNullable<typeof l> => !!l)
      .filter((l, i, all) => all.findIndex((x) => x.id === l.id) === i)
  );

  const pinTarget = $derived(!person && !merged && conversation ? conversation.id : null);
  const pins = $derived(pinTarget && $pinnedStore.conversation_id === pinTarget ? $pinnedStore.data : []);
  let pinsButton = $state<HTMLButtonElement | null>(null);
  let pinsFor = $state<string | null>(null);
  const pinsOpen = $derived(!!pinTarget && pinsFor === pinTarget && pins.length > 0);

  $effect(() => {
    if (pinTarget) loadPinned(pinTarget);
  });

  function pickPin(message_id: string) {
    pinsFor = null;
    revealMessage(message_id, { delay: 0 });
  }

  const marks = $derived(targets.flatMap((c) => c.providers.map((p) => ({ ...p, conversation_type: c.type }))))

</script>

{#if person || merged || conversation}
  <header data-testid="chat-header" class="group/bar bar-frost {$shell.glass ? `frost-${$shell.tint}` : ''} relative z-40 flex shrink-0 flex-wrap items-start gap-x-4 gap-y-2 px-4 pb-2.5 pt-2.5">
    {#if $listCollapsed}
      <IconButton icon="lucide:panel-left-open" label={t('conversation.overview.showList')} keys="⌘⇧L" onclick={toggleList} class="-ml-2 mt-1" />
    {/if}
    {#if avatar}
      <Avatar testid="chat-avatar" source={avatar} size="xl" label={profileTarget ? t('inbox.row.openProfile', { name: title }) : t('inbox.row.chatDetails', { name: title })} hint={profileTarget ? t('inbox.row.openProfile', { name: title }) : t('inbox.row.chatDetails', { name: title })} onclick={openImage} />
    {/if}
    <div class="min-w-0 flex-1">
      <div class="flex items-center gap-2.5">
        <h1 data-testid="chat-title" class="min-w-0 truncate text-[18px] font-semibold leading-7 tracking-tight text-ink">{title}</h1>
        <ProviderIcon providers={marks} size="md" />
      </div>
      <div class="mt-0.5 flex min-w-0 items-center gap-3 text-[13px] leading-5 text-ink-3">
        <p class="min-w-0 truncate">
          {#each meta as part, i (i)}
            {#if i > 0}<span class="mx-1.5">·</span>{/if}<span>{part}</span>
          {/each}
        </p>
        {#if pins.length}
          <div data-testid="chat-header-marks" class="ml-auto flex shrink-0 items-center">
            {#if pins.length}
              <button
                bind:this={pinsButton}
                type="button"
                data-testid="chat-pinned"
                class="flex shrink-0 items-center gap-1 rounded-[0.4rem] px-1.5 hover:bg-paper-3 hover:text-ink {pinsOpen ? 'bg-paper-3 text-ink' : ''}"
                aria-label={`${t('conversation.pinned.title')} ${pins.length}`}
                aria-haspopup="dialog"
                aria-expanded={pinsOpen}
                onmouseenter={(e) => showTooltip(e.currentTarget, t('conversation.pinned.title'))}
                onclick={() => (pinsFor = pinsOpen ? null : pinTarget)}
              >
                <Icon icon="lucide:pin" class="text-[13px]" />
                <span class="font-mono text-[11px]">{pins.length}</span>
              </button>
            {/if}
          </div>
        {/if}
      </div>
      {#if pinsOpen}
        <div class="menu w-[min(22rem,80vw)] px-3" use:anchored={{ at: () => boxOf(pinsButton), align: 'end', onclose: () => (pinsFor = null) }} role="dialog" aria-label={t('conversation.pinned.title')} data-testid="chat-pinned-list">
          <PinnedMessages collapsible={false} onpick={pickPin} />
        </div>
      {/if}
    </div>
    <div class="flex h-7 shrink-0 items-center gap-0.5 opacity-60 group-hover/bar:opacity-100 focus-within:opacity-100">
      <IconButton testid="chat-done" icon={done ? 'lucide:archive-restore' : 'lucide:check'} label={done ? t('conversation.title.moveToInbox') : t('conversation.title.done')} keys="E" onclick={() => markDone(targetIds, !done)} />
      <IconButton testid="chat-snooze" icon="lucide:clock" label={t('conversation.title.snooze')} keys="H" onclick={(e) => { sheetAnchor.set(boxOf(e.currentTarget)); inboxSheet.set('snooze') }} />
      <IconButton testid="chat-tag" icon="lucide:tag" label={t('conversation.title.tag')} keys="L" onclick={(e) => { sheetAnchor.set(boxOf(e.currentTarget)); inboxSheet.set('tag') }} />
      {#if FEATURES.cases}
        <IconButton icon="lucide:briefcase" label={inCases ? t('conversation.title.inCases', { count: inCases }) : t('conversation.title.addToCase')} keys="⇧C" active={inCases > 0} onclick={() => openCasePicker({ conversationIds: targetIds, messageIds: [] })} />
      {/if}
      <IconButton testid="chat-delete" icon={deleted ? 'lucide:undo-2' : 'lucide:trash-2'} label={deleted ? t('conversation.title.restore') : t('common.delete')} keys="#" tone="danger" onclick={() => remove(targetIds, $conversationsStore.data)} />
      {#if selection.ids.length}
        <ExportMenu />
      {/if}
      <NotificationBell />
      <IconButton testid="chat-info" icon="lucide:info" label={t('conversation.details.title')} keys="⌘⇧D" active={!!$profileSettingsStore?.details_panel_open} onclick={() => putDetailsOpen(null)} />
    </div>
    {#if chips.length}
      <div data-testid="chat-header-tags" class="flex basis-full flex-wrap items-center gap-1">
        {#each chips as chip (chip.id)}
          <button type="button" data-testid="chat-header-tag" data-tag={chip.id} class="inline-flex min-w-0 cursor-pointer items-center rounded-full bg-paper-3/60 px-2 py-px text-[11px] leading-4 hover:bg-paper-3" onclick={() => openTag(chip.id)}><TagMark tag={chip} size="xs" label ink="ink-2" /></button>
        {/each}
      </div>
    {/if}
    {#if $conversationStore.is_loading}
      <div class="absolute inset-x-0 bottom-0 h-px overflow-hidden">
        <div class="loading-marquee h-full w-[30%] bg-accent"></div>
      </div>
    {/if}
  </header>
{/if}

<style>
  @keyframes loading-marquee {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(400%); }
  }
  .loading-marquee {
    animation: loading-marquee 1.5s linear infinite;
  }
</style>
