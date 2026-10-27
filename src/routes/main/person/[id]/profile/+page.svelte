<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import Icon from '@iconify/svelte'
  import { page } from '$app/state'
  import { goto } from '$app/navigation'
  import { get } from 'svelte/store'
  import { api, peopleStore, loadPeople, loadTags, tagsStore, objectTagsStore, toggleObjectTag, identityLabel, loadChats } from '$lib/tauri'
  import { openPerson } from '$lib/tauri/people'
  import type { IdentityProfile, LocalField, PersonProfile, PersonThread, Stats, StatsRangeInput, Tag } from '$lib/tauri/types'
  import { Avatar, Button, ColourSwatch, Field, IconButton, PaneGrid, TagMark, TagPicker } from '$lib/components/ui'
  import { boxOf, type Box } from '$lib/popover'
  import { personAvatar } from '$lib/avatar'
  import { modalsShow } from '$lib/modals'
  import { rowDate } from '$lib/inbox'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import RangePicker from '$lib/components/stats/RangePicker.svelte'
  import Timeline from '$lib/components/stats/Timeline.svelte'
  import Heatmap from '$lib/components/stats/Heatmap.svelte'
  import Distribution from '$lib/components/stats/Distribution.svelte'
  import Bars from '$lib/components/stats/Bars.svelte'
  import Panel from '$lib/components/stats/Panel.svelte'
  import { bytes, compact, conversationTypeLabel, fastestWeekday, fileLabel, fileRows, habits, hourLabel, kindLabel, kindRows, leadSentence, minutes, percent, presetRange, rangeInput, volumePeak, volumeSeries, weekdayLabel, weekdayReplyRows, type Range } from '$lib/components/stats/stats'
  import { formatDate, t, type MessageKey } from '$lib/i18n.svelte'
  import type { PageProps } from './$types'

  let { data }: PageProps = $props()

  let profile = $state<PersonProfile | null>(null)
  let failure = $state('')
  let busy = $state(false)
  let name = $state('')
  let tagging = $state<{ objectType: 'person' | 'identity'; objectId: string; at: Box | null } | null>(null)
  let range = $state<Range>(presetRange('30d'))
  let stats = $state<Stats | null>(null)
  let counting = $state(true)
  let statsFailure = $state('')
  let ticket: StatsRangeInput | null = null

  const personId = $derived(decodeURIComponent(data.personId))
  const selected = $derived(page.url.searchParams.get('identity'))
  const chosen = $derived(profile?.identities.find((identity) => identity.key === selected) ?? null)

  $effect(() => {
    const id = personId
    load(id)
    loadTags().catch(() => {})
    if (!get(peopleStore).length) loadPeople().catch(() => {})
  })

  $effect(() => {
    const input = rangeInput(range, { person_ids: [personId] })
    ticket = input
    counting = true
    api
      .getStats(input)
      .then((next) => {
        if (ticket !== input) return
        stats = next
        statsFailure = ''
      })
      .catch((e) => {
        if (ticket === input) statsFailure = String(e)
      })
      .finally(() => {
        if (ticket === input) counting = false
      })
  })

  async function load(id: string) {
    try {
      profile = await api.getPersonProfile(id)
      name = profile.person.display_name
      failure = ''
    } catch (e) {
      failure = String(e)
    }
  }

  async function run(action: () => Promise<unknown>) {
    busy = true
    failure = ''
    try {
      await action()
      await Promise.all([load(personId), loadPeople()])
    } catch (e) {
      failure = String(e)
    } finally {
      busy = false
    }
  }

  function pick(key: string | null) {
    goto(key ? `?identity=${encodeURIComponent(key)}` : page.url.pathname, { replaceState: true, keepFocus: true, noScroll: true })
  }

  const local = $derived(profile?.person.overrides ?? {})
  const synced = $derived.by(() => {
    const first = (pick: (identity: IdentityProfile) => string | null) => profile?.identities.map(pick).find((value) => !!value) ?? null
    return { job_title: first((i) => i.job_title), company: first((i) => i.company) }
  })
  let title = $state('')
  let company = $state('')
  let notes = $state('')

  $effect(() => {
    title = local.job_title ?? ''
    company = local.company ?? ''
    notes = local.notes ?? ''
  })

  const KINDS: Record<string, MessageKey> = {
    direct_message: 'conversation.type.direct',
    group_message: 'conversation.type.group',
    channel: 'conversation.type.channel',
    email: 'conversation.type.email',
    local: 'chats.type.local',
    unknown: 'conversation.type.unknown'
  }

  function threadIcon(thread: PersonThread): string {
    if (thread.local) return thread.mode === 'read' ? 'lucide:book-open' : 'lucide:pen-line'
    if (thread.kind === 'email') return 'lucide:mail'
    if (thread.kind === 'channel') return 'lucide:hash'
    if (thread.kind === 'group_message') return 'lucide:users'
    return 'lucide:message-circle'
  }

  function putLocal(field: LocalField, value: string | null) {
    const target = { object_type: 'person' as const, object_id: personId, field }
    run(() => (value?.trim() ? api.putObjectOverride({ ...target, value: value.trim() }) : api.deleteObjectOverride(target)))
  }

  function saveField(field: LocalField, value: string) {
    if (value.trim() === (local[field] ?? '')) return
    putLocal(field, value)
  }

  function pickPhoto() {
    if (!profile) return
    modalsShow({ currentImage: profile.person.avatar_data, title: t('people.local.photo'), library: true, resettable: !!local.avatar_data, onpick: (image) => putLocal('avatar_data', image) })
  }

  function openChat() {
    if (!profile) return
    const person = profile.person
    loadChats()
      .then((chats) => openPerson(person, chats, goto))
      .catch((e) => (failure = String(e)))
  }

  function rename() {
    const next = name.trim()
    if (!profile || !next || next === profile.person.display_name) return
    run(() => api.putPerson({ person_id: personId, display_name: next }))
  }

  function resetName() {
    if (!profile) return
    name = profile.person.provider_name
    putLocal('display_name', null)
  }

  function split(identity: IdentityProfile) {
    run(async () => {
      await api.deletePersonIdentity(identity.identity.id)
      pick(null)
    })
  }

  function tagsOn(objectType: 'person' | 'identity', objectId: string): Tag[] {
    return $objectTagsStore
      .filter((link) => link.object_type === objectType && link.object_id === objectId)
      .map((link) => $tagsStore.find((candidate) => candidate.id === link.tag_id))
      .filter((record): record is Tag => !!record)
  }

  function untag(objectType: 'person' | 'identity', objectId: string, tagId: string) {
    run(() => toggleObjectTag(objectType, [objectId], tagId))
  }

  function when(timestamp: string | null): string {
    return timestamp ? formatDate(timestamp, { day: 'numeric', month: 'short', year: 'numeric' }) : t('common.never')
  }

  function facts(identity: IdentityProfile): { icon: string; value: string }[] {
    return [
      { icon: 'lucide:mail', value: identity.email },
      { icon: 'lucide:phone', value: identity.phone },
      { icon: 'lucide:briefcase', value: identity.job_title },
      { icon: 'lucide:users', value: identity.department },
      { icon: 'lucide:building-2', value: identity.company },
      { icon: 'lucide:map-pin', value: identity.office_location },
      { icon: 'lucide:message-square-quote', value: identity.status_text }
    ].filter((fact): fact is { icon: string; value: string } => !!fact.value)
  }

  const heading = $derived(profile ? (local.display_name ?? profile.person.display_name) : '')
  const role = $derived([local.job_title ?? synced.job_title, local.company ?? synced.company].filter((value): value is string => !!value).join(' · '))
  const channels = $derived.by(() => {
    const seen = new Map<string, { identity: IdentityProfile; source: string; presence: string | null }>()
    for (const identity of profile?.identities ?? []) {
      const source = identityLabel(identity.identity).source
      const known = seen.get(source)
      if (!known) seen.set(source, { identity, source, presence: identity.presence })
      else if (!known.presence) known.presence = identity.presence
    }
    return [...seen.values()]
  })

  const lead = $derived(stats ? leadSentence(stats) : null)
  const rhythm = $derived(stats ? habits(stats, range) : null)
  const volume = $derived(stats ? volumeSeries({ points: stats.timeline, range }, null) : [])
  const volumeTakeaway = $derived.by(() => {
    const peak = volumePeak(volume)
    return peak ? t('stats.timeline.takeaway', { date: formatDate(new Date(`${peak.date}T00:00:00`), { weekday: 'long', day: 'numeric', month: 'short' }), count: peak.values[0] + peak.values[1] }) : t('stats.timeline.quiet')
  })
  const weekdayRows = $derived(stats ? weekdayReplyRows(stats.reply_by_weekday ?? [], null).map((r) => ({ key: r.key, label: weekdayLabel(r.weekday), value: r.value, other: r.other })) : [])
  const fastest = $derived(stats ? fastestWeekday(stats.reply_by_weekday ?? []) : null)
  const kinds = $derived(stats ? kindRows(stats.by_kind ?? [], null).map((r) => ({ ...r, label: kindLabel(r.key) })) : [])
  const kindTotal = $derived(kinds.reduce((s, r) => s + r.value, 0))
  const files = $derived(stats ? fileRows(stats.by_file_type ?? [], null).map((r) => ({ ...r, label: fileLabel(r.key) })) : [])
  const fileCount = $derived(files.reduce((s, r) => s + r.value, 0))
  const fileBytes = $derived((stats?.by_file_type ?? []).reduce((s, f) => s + f.bytes, 0))
  const topThree = $derived(stats && stats.totals.messages ? percent(stats.by_conversation.slice(0, 3).reduce((s, c) => s + c.messages, 0), stats.totals.messages) : null)
</script>

{#snippet avatar(src: string | null, label: string, size: 'sm' | 'lg')}
  <Avatar source={{ shape: 'person', id: label, name: label, src, color: null, faces: [], icon: null }} {size} />
{/snippet}

{#snippet localMark(field: LocalField)}
  {#if local[field]}
    <span class="flex shrink-0 items-center gap-0.5 text-[11.5px] text-ink-3" data-testid="person-local" data-field={field}>
      <Icon icon="lucide:hard-drive" class="text-[11px]" />{t('people.local.badge')}
      <IconButton size="sm" icon="lucide:rotate-ccw" testid="person-reset-{field}" label={t('people.local.reset')} disabled={busy} onclick={() => putLocal(field, null)} />
    </span>
  {:else}
    <span></span>
  {/if}
{/snippet}

{#snippet tagRow(objectType: 'person' | 'identity', objectId: string, hint: string)}
  {@const applied = tagsOn(objectType, objectId)}
  <div class="flex flex-wrap items-center gap-1.5">
    {#each applied as record (record.id)}
      <span class="flex h-6 items-center gap-1.5 rounded-full bg-paper-2 pl-2 pr-1 text-[12.5px] text-ink">
        <TagMark tag={record} size="xs" label />
        <button type="button" class="flex size-4 items-center justify-center rounded-full text-ink-3 hover:bg-paper-3 hover:text-ink" aria-label={t('people.tags.remove', { name: record.name })} disabled={busy} onclick={() => untag(objectType, objectId, record.id)}>
          <Icon icon="lucide:x" class="text-[11px]" />
        </button>
      </span>
    {/each}
    <Button size="sm" icon="lucide:tag" onclick={(e) => (tagging = { objectType, objectId, at: boxOf(e.currentTarget) })}>{applied.length ? t('common.add') : t('people.tags.add')}</Button>
    {#if !applied.length}<span class="text-[12.5px] text-ink-3">{hint}</span>{/if}
  </div>
{/snippet}

{#snippet identityCard(identity: IdentityProfile, full: boolean)}
  {@const label = identityLabel(identity.identity)}
  <article data-testid="person-identity-card" data-key={identity.key} data-full={full} class="flex min-w-0 flex-col gap-3 rounded-[var(--radius-sheet)] bg-paper-2/60 p-4 hairline">
    <header class="grid grid-cols-[2.5rem_minmax(0,1fr)_auto] items-center gap-3">
      {@render avatar(identity.avatar_data ?? identity.avatar_url, identity.display_name ?? label.primary, 'lg')}
      <span class="flex min-w-0 flex-col">
        <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[14px] font-medium text-ink">{identity.display_name ?? label.primary}</span>
        <span class="flex items-center gap-1.5 overflow-hidden text-ellipsis whitespace-nowrap text-[12px] text-ink-3">
          <ProviderIcon providers={[{ ...identity.identity, conversation_type: identity.identity.email ? 'email' : 'direct_message' }]} size="sm" max={1} />
          {label.source}{identity.presence ? ` · ${identity.presence}` : ''}
        </span>
      </span>
      {#if full}
        <Button size="sm" icon="lucide:split" testid="person-split" disabled={busy || (profile?.identities.length ?? 0) < 2} onclick={() => split(identity)}>{t('people.profile.splitOut')}</Button>
      {:else}
        <IconButton size="sm" icon="lucide:arrow-up-right" testid="person-open-identity" label={t('people.profile.openIdentity')} onclick={() => pick(identity.key)} />
      {/if}
    </header>
    {#if facts(identity).length}
      <ul class="flex flex-col gap-1.5">
        {#each facts(identity) as fact (fact.icon)}
          <li class="grid grid-cols-[1rem_minmax(0,1fr)] items-center gap-2.5">
            <Icon icon={fact.icon} class="text-[13px] text-ink-3" />
            <span class="selectable overflow-hidden text-ellipsis whitespace-nowrap text-[12.5px] text-ink-2">{fact.value}</span>
          </li>
        {/each}
      </ul>
    {/if}
    <dl class="grid grid-cols-3 gap-2 border-t border-line pt-3">
      <div><dt class="text-[11.5px] text-ink-3">{t('people.profile.messages')}</dt><dd class="font-mono text-[13px] tabular-nums text-ink">{identity.message_count}</dd></div>
      <div><dt class="text-[11.5px] text-ink-3">{t('people.profile.firstSeen')}</dt><dd class="text-[13px] text-ink">{when(identity.first_seen)}</dd></div>
      <div><dt class="text-[11.5px] text-ink-3">{t('people.profile.lastSeen')}</dt><dd class="text-[13px] text-ink">{when(identity.last_seen)}</dd></div>
    </dl>
    {#if full}
      <div class="flex flex-col gap-2 border-t border-line pt-3">
        <h3 class="text-[12.5px] text-ink-3">{t('people.profile.tagsAddressOnly')}</h3>
        {@render tagRow('identity', identity.identity.id, t('people.profile.tagsAddressHint'))}
      </div>
      <div class="flex flex-col gap-1 border-t border-line pt-3">
        <h3 class="pb-1 text-[12.5px] text-ink-3">{t('people.profile.conversations')}</h3>
        {#each identity.conversations as conversation (conversation.id)}
          <a href="/main/conversation/{conversation.id}" class="grid h-8 grid-cols-[1rem_minmax(0,1fr)] items-center gap-2.5 rounded-[0.5rem] px-1.5 text-[13px] text-ink-2 hover:bg-paper-3 hover:text-ink">
            <Icon icon={conversation.kind === 'email' ? 'lucide:mail' : conversation.kind === 'direct_message' ? 'lucide:message-circle' : 'lucide:hash'} class="text-[13px] text-ink-3" />
            <span class="overflow-hidden text-ellipsis whitespace-nowrap">{conversation.name}</span>
          </a>
        {:else}
          <p class="text-[12.5px] text-ink-3">{t('people.profile.noConversations')}</p>
        {/each}
      </div>
    {/if}
  </article>
{/snippet}

<PaneGrid name="person-profile" class="bg-paper" panes={[{ key: 'identities', size: '20rem', min: '14rem', max: '36vw', edge: 'end' }, { key: 'detail' }]}>
  {#snippet pane(key)}
    {#if !profile}
      <div class="flex h-full items-center justify-center px-6 text-center text-[13px] text-ink-3">{failure || t('people.profile.loading')}</div>
    {:else if key === 'identities'}
      <header class="flex h-14 shrink-0 items-center gap-2 px-3">
        <IconButton size="sm" icon="lucide:arrow-left" label={t('people.profile.back')} onclick={() => goto(`/main/person/${encodeURIComponent(personId)}`)} />
        <h1 class="min-w-0 flex-1 text-[15px] font-medium tracking-tight text-ink">{t('people.profile.title')}</h1>
      </header>
      <div class="no-scrollbar flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-3 pb-4">
        <div class="flex items-center gap-3">
          <Avatar source={personAvatar(profile.person)} size="xl" label={t('people.local.photo')} hint={t('people.local.photo')} onclick={pickPhoto} />
          <div class="flex min-w-0 flex-1 flex-col">
            <h2 data-testid="person-heading" class="overflow-hidden text-ellipsis whitespace-nowrap text-[16px] font-semibold tracking-tight text-ink">{profile.person.display_name}</h2>
            <p class="overflow-hidden text-ellipsis whitespace-nowrap text-[12.5px] text-ink-3">{local.job_title ?? synced.job_title ?? profile.person.primary_email ?? t('people.profile.identityCount', { count: profile.identities.length })}</p>
          </div>
        </div>
        <Button variant="primary" icon="lucide:message-circle" testid="person-open-chat" onclick={openChat}>{t('people.profile.openChat')}</Button>
        <section class="flex flex-col gap-px" data-testid="person-threads" aria-label={t('people.profile.chats')}>
          <h2 class="flex h-7 items-center gap-2 px-2 text-[12.5px] text-ink-3"><Icon icon="lucide:messages-square" class="text-[13px]" />{t('people.profile.chats')}<span class="ml-auto font-mono text-[11px]">{profile.threads.length}</span></h2>
          {#each profile.threads as thread (thread.id)}
            <a href="/main/conversation/{thread.id}" data-testid="person-thread" data-id={thread.id} class="grid min-h-11 grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-2.5 rounded-[0.5rem] px-2 py-1 text-ink-2 hover:bg-paper-2 hover:text-ink">
              <span class="flex items-center justify-center text-ink-3"><Icon icon={threadIcon(thread)} class="text-[14px]" /></span>
              <span class="flex min-w-0 flex-col">
                <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[13.5px] {thread.unread_count ? 'font-semibold text-ink' : ''}">{thread.name}</span>
                <span class="flex min-w-0 items-center gap-1.5 overflow-hidden whitespace-nowrap text-[12px] text-ink-3">
                  {#if thread.provider_types.length}<ProviderIcon providers={thread.provider_types.map((provider_type, i) => ({ provider_type, name: thread.provider_names[i] ?? provider_type, conversation_type: thread.kind }))} size="xs" max={2} />{/if}
                  <span class="min-w-0 overflow-hidden text-ellipsis">{thread.direct ? t('people.profile.direct') : t(KINDS[thread.kind] ?? 'conversation.type.unknown')}{thread.local ? ` · ${thread.mode === 'read' ? t('chats.mode.read') : t('chats.mode.write')}` : ''}</span>
                </span>
              </span>
              <span class="flex flex-col items-end gap-0.5 font-mono text-[11px] tabular-nums text-ink-3">
                <span class={thread.unread_count ? 'text-accent' : ''}>{rowDate(thread.last_activity)}</span>
                <span title={t('people.profile.messages')}>{thread.unread_count ? `${thread.unread_count} / ${thread.message_count}` : thread.message_count}</span>
              </span>
            </a>
          {:else}
            <p class="px-2 text-[12.5px] text-ink-3">{t('people.profile.noChats')}</p>
          {/each}
        </section>
        {#if failure}
          <p data-testid="person-failure" class="text-[12.5px] text-error-600">{failure}</p>
        {/if}
      </div>
    {:else}
      <div class="board no-scrollbar flex min-h-0 flex-1 flex-col overflow-y-auto">
        <div class="sheet stats-tokens">
          <header data-testid="person-hero" class="full grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-[clamp(1rem,2.5cqi,1.75rem)]">
            <Avatar source={personAvatar(profile.person)} size="xxl" testid="person-hero-photo" label={t('people.local.photo')} hint={t('people.local.choosePhoto')} onclick={pickPhoto} />
            <div class="flex min-w-0 flex-col gap-1.5">
              <h2 data-testid="person-detail-heading" class="overflow-hidden text-ellipsis whitespace-nowrap text-[clamp(1.25rem,2.4cqi,1.75rem)] font-semibold tracking-tight text-ink">{heading}</h2>
              {#if role}
                <p data-testid="person-role" class="overflow-hidden text-ellipsis whitespace-nowrap text-[14px] text-ink-2">{role}</p>
              {/if}
              {#if channels.length}
                <ul data-testid="person-channels" class="flex flex-wrap items-center gap-1.5">
                  {#each channels as channel (channel.source)}
                    <li class="flex h-6 items-center gap-1.5 rounded-full bg-paper-2 px-2 text-[12px] text-ink-2">
                      <ProviderIcon providers={[{ ...channel.identity.identity, conversation_type: channel.identity.identity.email ? 'email' : 'direct_message' }]} size="xs" max={1} />
                      {channel.source}{#if channel.presence}<span class="text-ink-3">· {channel.presence}</span>{/if}
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
          </header>
          {#if chosen}
            <div class="full flex flex-col gap-3">
              <span><Button size="sm" icon="lucide:arrow-left" testid="person-identities-all" onclick={() => pick(null)}>{t('people.profile.allIdentities')}</Button></span>
              {@render identityCard(chosen, true)}
            </div>
          {:else}
            <dl data-testid="person-totals" class="full grid grid-cols-[repeat(auto-fit,minmax(min(100%,9rem),1fr))] gap-4 border-b border-line pb-5">
              <div><dt class="text-[12px] text-ink-3">{t('people.profile.messages')}</dt><dd class="font-mono text-[20px] tabular-nums text-ink">{profile.message_count}</dd></div>
              <div><dt class="text-[12px] text-ink-3">{t('people.profile.conversations')}</dt><dd class="font-mono text-[20px] tabular-nums text-ink">{profile.conversation_count}</dd></div>
              <div><dt class="text-[12px] text-ink-3">{t('people.profile.firstSeen')}</dt><dd class="text-[15px] text-ink">{when(profile.first_seen)}</dd></div>
              <div><dt class="text-[12px] text-ink-3">{t('people.profile.lastSeen')}</dt><dd class="text-[15px] text-ink">{when(profile.last_seen)}</dd></div>
            </dl>
            <section data-testid="person-signatures" class="full flex flex-col gap-2 border-b border-line pb-5" aria-label={t('people.profile.signatures')}>
              <h3 class="flex items-center gap-2 text-[15px] font-semibold tracking-tight text-ink">{t('people.profile.signatures')}<span class="font-mono text-[11px] text-ink-3">{profile.signatures.length}</span></h3>
              {#if profile.signatures.length}
                <ul class="grid grid-cols-[repeat(auto-fit,minmax(min(100%,18rem),1fr))] gap-3">
                  {#each profile.signatures as signature (signature.id)}
                    <li data-testid="person-signature" data-hash={signature.hash} class="flex min-w-0 flex-col gap-1.5 rounded-[0.75rem] bg-paper-2 p-3">
                      <pre class="whitespace-pre-wrap font-sans text-[12.5px] leading-5 text-ink">{signature.text}</pre>
                      <p class="flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[11.5px] text-ink-3">
                        {#if signature.provider_name}<span>{signature.provider_name}</span>{/if}
                        {#if signature.sender_email}<span class="font-mono">{signature.sender_email}</span>{/if}
                        <span>{t('people.profile.signatureUses', { count: signature.use_count })}</span>
                        <span>{when(signature.last_seen)}</span>
                        {#if signature.conversation_id}<button type="button" class="hover:underline" onclick={() => goto(conversationHref(signature.conversation_id ?? ''))}>{t('people.profile.openSource')}</button>{/if}
                      </p>
                    </li>
                  {/each}
                </ul>
              {:else}
                <p class="text-[12.5px] text-ink-3">{t('people.profile.noSignatures')}</p>
              {/if}
            </section>
            <div class="full flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
              <h3 class="text-[15px] font-semibold tracking-tight text-ink">{t('people.profile.statistics')}</h3>
              <RangePicker {range} onchange={(next) => (range = next)} />
            </div>
            {#if !stats || !lead}
              <p data-testid="person-stats-counting" class="full font-mono text-[11px] text-ink-3">{statsFailure || t('stats.counting')}</p>
            {:else}
              <div data-testid="person-stats" data-loading={counting} class="contents">
                <div data-testid="stats-panel" data-key="timeline" class="wide min-w-0 transition-opacity {counting ? 'opacity-60' : ''}">
                  <Timeline points={stats.timeline} {range} takeaway={volumeTakeaway} onbrush={(start, end) => (range = { start, end, preset: 'custom' })} />
                </div>
                <div data-testid="stats-panel" data-key="heatmap" class="min-w-0 transition-opacity {counting ? 'opacity-60' : ''}">
                  <Heatmap grid={stats.heatmap} takeaway={rhythm ? t('stats.heatmap.takeaway', { hour: hourLabel(lead.peakHour), weekday: weekdayLabel(lead.peakDay, 'long'), share: percent(rhythm.afterHours, 1) }) : null} />
                </div>
                <div data-testid="stats-panel" data-key="distribution" class="min-w-0 transition-opacity {counting ? 'opacity-60' : ''}">
                  <Distribution buckets={stats.response_times} median={stats.totals.median_response_minutes} mine={stats.totals.my_median_response_minutes} />
                </div>
                <div data-testid="stats-panel" data-key="weekday-replies" class="min-w-0 transition-opacity {counting ? 'opacity-60' : ''}">
                  <Bars
                    title={t('stats.weekdayReplies.title')}
                    rows={weekdayRows}
                    legend={[t('stats.legend.theirs'), t('stats.legend.mine')]}
                    aside={t('stats.weekdayReplies.median')}
                    format={(v) => (v > 0 ? minutes(v) : '—')}
                    empty={t('stats.weekdayReplies.empty')}
                    takeaway={fastest && fastest.mine !== null ? t('stats.weekdayReplies.takeaway', { weekday: weekdayLabel(fastest.weekday, 'long'), time: minutes(fastest.mine) }) : null}
                  />
                </div>
                <div data-testid="stats-panel" data-key="kinds" class="min-w-0 transition-opacity {counting ? 'opacity-60' : ''}">
                  <Bars title={t('stats.kinds.title')} rows={kinds} takeaway={kinds[0] && kindTotal ? t('stats.kinds.takeaway', { kind: kinds[0].label.toLowerCase(), share: percent(kinds[0].value, kindTotal) }) : null} aside={kindTotal ? t('stats.count.messages', { count: kindTotal, value: compact(kindTotal) }) : null} />
                </div>
                <div data-testid="stats-panel" data-key="conversations" class="min-w-0 transition-opacity {counting ? 'opacity-60' : ''}">
                  <Panel title={t('stats.section.busiestConversations')} aside={t('stats.section.byMessages')} takeaway={topThree ? t('stats.conversations.takeaway', { share: topThree }) : null}>
                    <ol class="divide-y divide-line">
                      {#each stats.by_conversation.slice(0, 8) as c (c.conversation_id)}
                        <li>
                          <button data-testid="stats-conversation" data-id={c.conversation_id} class="grid w-full grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-3 py-1.5 text-left hover:bg-paper-2" onclick={() => goto(conversationHref(c.conversation_id))}>
                            <span class="min-w-0">
                              <span class="block overflow-hidden text-ellipsis whitespace-nowrap text-[13px] font-medium text-ink">{c.name}</span>
                              <span class="block font-mono text-[10px] text-ink-3">{t('stats.conversation.meta', { type: conversationTypeLabel(c.type), count: c.people })}</span>
                            </span>
                            <span class="font-mono text-[12px] tabular-nums text-ink">{compact(c.messages)}</span>
                            <span class="font-mono text-[10px] tabular-nums text-ink-3">{t('stats.conversation.characters', { value: compact(c.characters) })}</span>
                          </button>
                        </li>
                      {/each}
                    </ol>
                  </Panel>
                </div>
              </div>
            {/if}
            <div class="stack flex min-w-0 flex-col">
              {#if stats && lead}
                <div data-testid="stats-panel" data-key="files" class="min-w-0 transition-opacity {counting ? 'opacity-60' : ''}">
                  <Bars
                    title={t('stats.files.title')}
                    rows={files}
                    aside={fileCount ? bytes(fileBytes) : null}
                    empty={t('stats.files.empty')}
                    unit={(count) => t('stats.count.times', { count })}
                    takeaway={files[0] ? t('stats.files.takeaway', { count: fileCount, size: bytes(fileBytes), kind: files[0].label.toLowerCase() }) : null}
                  />
                </div>
              {/if}
              <section class="flex flex-col gap-2 border-t border-line pt-5">
                <h3 class="text-[13px] font-medium text-ink">{t('people.profile.tags')}</h3>
                {@render tagRow('person', personId, t('people.profile.tagsEveryConversation', { name: profile.person.display_name }))}
              </section>
            </div>
            <section class="local flex flex-col gap-3 border-t border-line pt-5">
              <div class="flex flex-col gap-0.5">
                <h3 class="flex items-center gap-1.5 text-[13px] font-medium text-ink"><Icon icon="lucide:hard-drive" class="text-[13px] text-ink-3" />{t('people.local.title')}</h3>
                <p class="text-[12.5px] text-ink-3">{t('people.local.hint')}</p>
              </div>
              <div class="grid grid-cols-[minmax(6rem,0.3fr)_minmax(0,1fr)_auto] items-center gap-x-3 gap-y-2.5">
                <span class="text-[12.5px] text-ink-3">{t('common.name')}</span>
                <form class="min-w-0" onsubmit={(e) => { e.preventDefault(); rename() }}>
                  <Field bind:value={name} data-testid="person-name" size="sm" aria-label={t('common.name')} placeholder={profile.person.provider_name} onblur={rename} onicon={pickPhoto}>
                    {#snippet glyph()}<Avatar source={personAvatar(profile!.person)} size="sm" />{/snippet}
                  </Field>
                </form>
                {#if local.display_name}
                  <span class="flex shrink-0 items-center gap-0.5 text-[11.5px] text-ink-3" title={t('people.local.nameFrom', { name: profile.person.provider_name })}>
                    <Icon icon="lucide:hard-drive" class="text-[11px]" />{t('people.local.badge')}
                    <IconButton size="sm" icon="lucide:rotate-ccw" testid="person-reset-display_name" label={t('people.local.reset')} disabled={busy} onclick={resetName} />
                  </span>
                {:else}<span></span>{/if}

                <span class="text-[12.5px] text-ink-3">{t('people.local.photo')}</span>
                <span class="flex min-w-0 items-center gap-2">
                  <Button size="sm" icon="lucide:image-up" testid="person-photo" onclick={pickPhoto}>{t('people.local.choosePhoto')}</Button>
                  <span class="text-[12px] text-ink-3">{local.avatar_data ? t('people.local.photoLocal') : profile.person.avatar_data || profile.person.avatar_url ? t('people.local.photoSynced') : t('people.local.photoGenerated')}</span>
                </span>
                {@render localMark('avatar_data')}

                <span class="text-[12.5px] text-ink-3">{t('people.local.jobTitle')}</span>
                <Field bind:value={title} data-testid="person-job_title" size="sm" aria-label={t('people.local.jobTitle')} placeholder={synced.job_title ?? ''} onblur={() => saveField('job_title', title)} onkeydown={(e) => e.key === 'Enter' && saveField('job_title', title)} />
                {@render localMark('job_title')}

                <span class="text-[12.5px] text-ink-3">{t('people.local.company')}</span>
                <Field bind:value={company} data-testid="person-company" size="sm" aria-label={t('people.local.company')} placeholder={synced.company ?? ''} onblur={() => saveField('company', company)} onkeydown={(e) => e.key === 'Enter' && saveField('company', company)} />
                {@render localMark('company')}

                <span class="text-[12.5px] text-ink-3">{t('people.local.notes')}</span>
                <Field bind:value={notes} data-testid="person-notes" size="sm" aria-label={t('people.local.notes')} placeholder={t('people.local.notesPlaceholder')} onblur={() => saveField('notes', notes)} onkeydown={(e) => e.key === 'Enter' && saveField('notes', notes)} />
                {@render localMark('notes')}

                <span class="text-[12.5px] text-ink-3">{t('people.local.colour')}</span>
                <span class="flex items-center"><ColourSwatch size="sm" value={local.color ?? null} clearable label={t('people.local.colour')} onpick={(color) => putLocal('color', color)} /></span>
                {@render localMark('color')}
              </div>
            </section>
            <section class="full flex flex-col gap-3">
              <h3 class="text-[13px] font-medium text-ink">{t('people.profile.identities')}</h3>
              <div class="grid grid-cols-[repeat(auto-fill,minmax(min(100%,18rem),1fr))] gap-3">
                {#each profile.identities as identity (identity.identity.id)}
                  {@render identityCard(identity, false)}
                {/each}
              </div>
            </section>
          {/if}
        </div>
      </div>
    {/if}
  {/snippet}
</PaneGrid>

<style>
  .board {
    container: board / inline-size;
  }
  .sheet {
    --cols: 1;
    --wide: 1;
    --local: 1;
    display: grid;
    grid-template-columns: repeat(var(--cols), minmax(0, 1fr));
    grid-auto-flow: row dense;
    align-items: start;
    gap: clamp(1.5rem, 3cqi, 2.5rem) clamp(1.5rem, 3cqi, 3rem);
    padding: clamp(1rem, 2.5cqi, 2rem) clamp(1rem, 3cqi, 2.5rem) 4rem;
  }
  .full {
    grid-column: 1 / -1;
  }
  .wide {
    grid-column: span var(--wide);
  }
  .local {
    grid-column: span var(--local);
  }
  .stack {
    gap: clamp(1.5rem, 3cqi, 2.5rem);
  }
  @container board (min-width: 44rem) {
    .sheet {
      --cols: 2;
      --wide: 2;
      --local: 2;
    }
  }
  @container board (min-width: 72rem) {
    .sheet {
      --cols: 3;
      --wide: 2;
      --local: 1;
    }
  }
  @container board (min-width: 108rem) {
    .sheet {
      --cols: 4;
      --wide: 2;
      --local: 4;
    }
  }
</style>

{#if tagging}
  <TagPicker objectType={tagging.objectType} ids={[tagging.objectId]} at={tagging.at} onclose={() => (tagging = null)} />
{/if}
