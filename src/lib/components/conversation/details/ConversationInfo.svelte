<script lang="ts">
  import Icon from '@iconify/svelte';
  import { conversationStore } from '$lib/tauri/conversations';
  import { api, tagRecordsStore, usersStore } from '$lib/tauri';
  import { requestPresence } from '$lib/tauri/presence';
  import PresenceDot from '$lib/components/PresenceDot.svelte';
  import ChatSettings from './ChatSettings.svelte';
  import type { Conversation, ObjectTag, Tag } from '$lib/tauri/types';
  import { providerIcon, providerLabel } from '$lib/providerIcons';
  import { Avatar, Field, IconButton, Kbd, TagMark } from '$lib/components/ui';
  import { goto } from '$app/navigation';
  import { peopleStore, personHref, personOf, personProfileHref } from '$lib/tauri/people';
  import { userAvatar } from '$lib/avatar';
  import type { User } from '$lib/tauri/types';
  import { t } from '$lib/i18n.svelte';

  let { conversation, people = true }: { conversation: Conversation; people?: boolean } = $props();
  const chips = $derived(
    (conversation?.tags ?? [])
      .map(l => ({ link: l, label: $tagRecordsStore.find(label => label.id === l.tag_id) }))
      .filter((c): c is { link: ObjectTag; label: Tag } => !!c.label)
  );

  const meta = $derived.by((): Record<string, unknown> => {
    if (!conversation?.metadata) return {};
    try {
      return JSON.parse(conversation.metadata) ?? {};
    } catch {
      return {};
    }
  });
  const currentTopic = $derived(String(meta.topic ?? ''));
  const currentPurpose = $derived(String(meta.purpose ?? ''));
  const provider = $derived(conversation?.providers[0] ?? null);
  const canInvite = $derived(!!provider?.capabilities.can_add_members && conversation?.type !== 'direct_message' && conversation?.type !== 'email');

  let topic = $state('');
  let purpose = $state('');
  let topicNote = $state<string | null>(null);
  let inviteQuery = $state('');
  let memberFailure = $state<string | null>(null);
  let naming = $state<string | null>(null);
  let nickname = $state('');
  let nameFailure = $state<string | null>(null);

  const inviteMatches = $derived(inviteQuery.trim() ? $usersStore.data.filter((u) => !conversation?.members.some((m) => m.id === u.id) && (u.display_name ?? u.email ?? '').toLowerCase().includes(inviteQuery.trim().toLowerCase())).slice(0, 6) : []);

  $effect(() => {
    const ids = (conversation?.members ?? []).slice(0, 20).map((m) => m.id);
    if (ids.length) requestPresence(ids);
  });

  function startNaming(member: User) {
    naming = member.id;
    nickname = conversation?.nicknames[member.id] ?? member.display_name ?? '';
    nameFailure = null;
  }

  async function saveNickname(member: User, value: string | null) {
    const person = personOf($peopleStore, { user_id: member.id });
    if (!conversation || !person) return;
    const target = { object_type: 'person' as const, object_id: person.id, scope_type: 'conversation' as const, scope_id: conversation.id, field: 'display_name' as const };
    try {
      if (value?.trim()) await api.putObjectOverride({ ...target, value: value.trim() });
      else await api.deleteObjectOverride(target);
      conversationStore.set(await api.getCurrentConversation());
      naming = null;
    } catch (e) {
      nameFailure = String(e);
    }
  }

  $effect(() => {
    topic = currentTopic;
  });

  $effect(() => {
    purpose = currentPurpose;
  });

  async function refresh() {
    if ($conversationStore.data?.id === conversation?.id) conversationStore.set(await api.getCurrentConversation());
  }

  async function saveTopic(field: 'topic' | 'purpose') {
    if (!conversation) return;
    const next = (field === 'topic' ? topic : purpose).trim();
    if (next === (field === 'topic' ? currentTopic : currentPurpose)) return;
    try {
      const written = await api.putConversationTopic(conversation.id, field === 'topic' ? next : null, field === 'purpose' ? next : null);
      topicNote = provider ? t(written ? 'modals.channel.topicWritten' : 'modals.channel.topicLocal', { provider: provider.name }) : null;
      await refresh();
    } catch (e) {
      topicNote = String(e);
    }
  }

  async function invite(userId: string) {
    if (!conversation) return;
    memberFailure = null;
    try {
      await api.postConversationMember({ conversation_id: conversation.id, user_id: userId });
      inviteQuery = '';
      await refresh();
    } catch (e) {
      memberFailure = String(e);
    }
  }

  async function removeMember(userId: string) {
    if (!conversation) return;
    memberFailure = null;
    try {
      await api.deleteConversationMember({ conversation_id: conversation.id, user_id: userId });
      await refresh();
    } catch (e) {
      memberFailure = String(e);
    }
  }
</script>

{#if conversation}
  <ChatSettings {conversation} />

  <div class="border-b border-line py-3">
    <p class="text-[13px] text-ink-3">{t('conversation.info.topic')}</p>
    <Field data-testid="chat-topic" bind:value={topic} size="sm" placeholder={t('conversation.info.topicPlaceholder')} class="mt-1.5" onblur={() => saveTopic('topic')} onkeydown={(e) => e.key === 'Enter' && saveTopic('topic')} />
    {#if provider?.capabilities.supports_purpose || currentPurpose}
      <p class="mt-3 text-[13px] text-ink-3">{t('modals.channel.purpose')}</p>
      {#if provider?.capabilities.supports_purpose}
        <Field data-testid="chat-purpose" bind:value={purpose} size="sm" placeholder={t('modals.channel.purposePlaceholder')} class="mt-1.5" onblur={() => saveTopic('purpose')} onkeydown={(e) => e.key === 'Enter' && saveTopic('purpose')} />
      {:else}
        <p data-testid="chat-purpose" class="mt-1.5 text-[13px] text-ink">{currentPurpose}</p>
      {/if}
    {/if}
    {#if topicNote}<p data-testid="chat-topic-note" class="mt-1.5 text-[12px] text-ink-3">{topicNote}</p>{/if}
  </div>

  <div class="border-b border-line py-3">
    <p class="text-[13px] text-ink-3">{t('conversation.info.where')}</p>
    <ul class="mt-1.5">
      {#each conversation.providers as provider (provider.id)}
        <li class="flex h-7 items-center gap-2 text-[13px] text-ink">
          <Icon icon={providerIcon(provider, conversation.type)} class="text-[14px] grayscale" />
          <span class="truncate">{provider.name}</span>
          <span class="ml-auto text-[12px] text-ink-3">{providerLabel(provider)}</span>
        </li>
      {/each}
    </ul>
  </div>

  <div class="border-b border-line py-3">
    <p class="text-[13px] text-ink-3">{t('conversation.info.tags')}</p>
    {#if chips.length === 0}
      <p class="mt-1.5 text-[13px] text-ink-3">{t('conversation.info.tagsNone')} <Kbd keys="L" /> {t('conversation.info.tagsNoneAfter')}</p>
    {:else}
      <ul class="mt-1.5">
        {#each chips as chip (chip.label.id)}
          <li class="flex h-7 items-center gap-2 text-[13px] text-ink">
            <TagMark tag={chip.label} label />
            <span class="ml-auto text-[12px] text-ink-3" title={chip.link.reason ?? ''}>{chip.link.source}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#if people}
  <div class="border-b border-line py-3">
    <p class="text-[13px] text-ink-3">{t('conversation.info.people')} <span class="font-mono text-[11px]">{conversation.members.length}</span></p>
    <ul class="mt-1.5">
      {#each conversation.members.slice(0, 20) as member (member.id)}
        {@const person = personOf($peopleStore, { user_id: member.id })}
        {@const label = member.display_name ?? member.email ?? member.id}
        <li data-testid="member" data-id={member.id} data-nickname={conversation.nicknames[member.id] ?? ''} class="group/member grid min-h-8 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2 text-[13px] text-ink">
          <span class="relative"><Avatar source={userAvatar(member, $peopleStore, label)} size="sm" label={t('inbox.row.openProfile', { name: label })} onclick={person ? () => goto(personProfileHref(person.id)) : undefined} /><PresenceDot user_id={member.id} size="sm" /></span>
          {#if naming === member.id}
            <form class="min-w-0" onsubmit={(e) => { e.preventDefault(); saveNickname(member, nickname) }}>
              <Field data-testid="member-nickname-field" bind:value={nickname} size="sm" aria-label={t('conversation.info.nickname', { name: label })} placeholder={t('conversation.info.nicknamePlaceholder')} onkeydown={(e) => e.key === 'Escape' && (naming = null)} autofocus />
            </form>
            <IconButton testid="member-nickname-save" size="sm" icon="lucide:check" label={t('common.save')} onclick={() => saveNickname(member, nickname)} />
          {:else}
            <span class="flex min-w-0 items-baseline gap-2">
              {#if person}
                <button type="button" data-testid="member-open" class="truncate text-left hover:underline" onclick={() => goto(personHref(person.id))}>{label}</button>
              {:else}
                <span class="truncate">{label}</span>
              {/if}
              {#if conversation.nicknames[member.id]}
                <span class="shrink-0 text-[11px] text-ink-3" title={t('conversation.info.nicknameHint')}>{t('conversation.info.inThisChat')}</span>
              {:else if member.email}
                <span class="truncate font-mono text-[11px] text-ink-3">{member.email}</span>
              {/if}
            </span>
            {#if person}
              <span class="flex items-center opacity-0 focus-within:opacity-100 group-hover/member:opacity-100">
                {#if conversation.nicknames[member.id]}
                  <IconButton testid="member-nickname-reset" size="sm" icon="lucide:rotate-ccw" label={t('conversation.info.resetNickname')} onclick={() => saveNickname(member, null)} />
                {/if}
                <IconButton testid="member-nickname-edit" size="sm" icon="lucide:pencil" label={t('conversation.info.nickname', { name: label })} onclick={() => startNaming(member)} />
                {#if canInvite}<IconButton testid="member-remove" size="sm" tone="danger" icon="lucide:user-minus" label={t('modals.channel.removeMember', { name: label })} onclick={() => removeMember(member.id)} />{/if}
              </span>
            {:else}
              <span></span>
            {/if}
          {/if}
        </li>
      {/each}
      {#if nameFailure}<li class="text-[12px] text-error-600">{nameFailure}</li>{/if}
      {#if memberFailure}<li data-testid="member-error" class="text-[12px] text-error-600">{memberFailure}</li>{/if}
      {#if conversation.members.length > 20}
        <li class="text-[13px] text-ink-3">{t('conversation.info.andMore', { count: conversation.members.length - 20 })}</li>
      {/if}
    </ul>
    {#if canInvite}
      <div class="mt-2 flex flex-col gap-1" data-testid="member-invite">
        <Field data-testid="member-invite-field" bind:value={inviteQuery} size="sm" placeholder={t('modals.channel.invite', { provider: provider?.name ?? '' })} aria-label={t('modals.channel.invite', { provider: provider?.name ?? '' })} onkeydown={(e) => e.key === 'Enter' && inviteMatches[0] && invite(inviteMatches[0].id)} />
        {#each inviteMatches as user (user.id)}
          <button type="button" data-testid="member-invite-option" class="flex h-7 items-center gap-2 rounded-[0.4rem] px-2 text-left text-[13px] text-ink hover:bg-paper-3" onclick={() => invite(user.id)}><Icon icon="lucide:user-plus" class="text-[13px] text-ink-3" />{user.display_name ?? user.email}</button>
        {/each}
      </div>
    {/if}
  </div>
  {/if}
{/if}
