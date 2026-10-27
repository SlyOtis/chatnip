<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import { goto } from '$app/navigation'
  import { onMount } from 'svelte'
  import { modalsClose } from '$lib/modals'
  import { api, conversationsStore, providersStore, usersStore } from '$lib/tauri'
  import type { JoinableConversation } from '$lib/tauri/types'
  import { providerIcon } from '$lib/providerIcons'
  import { Button, Kbd, MenuItem, Segmented, Sheet, Switch } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'
  import { renderEmojiText } from '$lib/emoji'

  let searchQuery = $state('')
  let activeTab = $state<string>('all')
  let joinable = $state<JoinableConversation[]>([])
  let loadingJoinable = $state(true)
  let joining = $state<string | null>(null)
  let failure = $state<string | null>(null)
  let creating = $state(false)
  let draftName = $state('')
  let draftPrivate = $state(false)
  let draftProvider = $state<string>('')
  let memberQuery = $state('')
  let memberIds = $state<string[]>([])
  let busy = $state(false)
  let list = $state<HTMLDivElement | null>(null)

  const providers = $derived($providersStore.data.filter((p) => p.is_enabled))
  const creatable = $derived(providers.filter((p) => p.capabilities.supports_channel_create))
  const providerTabs = $derived([{ value: 'all', label: t('common.all') }, ...providers.filter((p) => $conversationsStore.data.some((c) => c.type === 'channel' && c.providers.some((cp) => cp.id === p.id)) || joinable.some((j) => j.provider_id === p.id)).map((p) => ({ value: p.id, label: p.name }))])

  const query = $derived(searchQuery.trim().toLowerCase())
  const synced = $derived(
    $conversationsStore.data
      .filter((c) => c.type === 'channel')
      .filter((c) => activeTab === 'all' || c.providers.some((p) => p.id === activeTab))
      .filter((c) => !query || c.name.toLowerCase().includes(query) || (c.summary ?? '').toLowerCase().includes(query))
  )
  const open = $derived(
    joinable
      .filter((j) => activeTab === 'all' || j.provider_id === activeTab)
      .filter((j) => !query || j.conversation.name.toLowerCase().includes(query) || String(j.conversation.metadata.purpose ?? j.conversation.metadata.topic ?? '').toLowerCase().includes(query))
  )
  const memberMatches = $derived(memberQuery.trim() ? $usersStore.data.filter((u) => !memberIds.includes(u.id) && (u.display_name ?? u.email ?? '').toLowerCase().includes(memberQuery.trim().toLowerCase())).slice(0, 6) : [])

  onMount(() => {
    api.getJoinableConversations().then((found) => (joinable = found)).catch(() => (joinable = [])).finally(() => (loadingJoinable = false))
  })

  function openChannel(id: string) {
    modalsClose()
    api.putCurrentConversation(id)
    goto(conversationHref(id))
  }

  async function join(entry: JoinableConversation) {
    joining = entry.conversation.external_id
    failure = null
    try {
      await api.postConversationMembership({ provider_id: entry.provider_id, external_id: entry.conversation.external_id })
      joinable = joinable.filter((j) => j !== entry)
    } catch (e) {
      failure = String(e)
    } finally {
      joining = null
    }
  }

  function startCreate() {
    creating = true
    failure = null
    draftProvider = activeTab !== 'all' && creatable.some((p) => p.id === activeTab) ? activeTab : (creatable[0]?.id ?? '')
    draftName = searchQuery.trim()
  }

  async function create() {
    if (!draftProvider || !draftName.trim() || busy) return
    busy = true
    failure = null
    try {
      const made = await api.postProviderConversation(draftProvider, { conversation_type: 'channel', name: draftName.trim(), members: [], private: draftPrivate }, memberIds)
      creating = false
      if (made) openChannel(made.id)
    } catch (e) {
      failure = String(e)
    } finally {
      busy = false
    }
  }

  function userName(id: string): string {
    const user = $usersStore.data.find((u) => u.id === id)
    return user?.display_name ?? user?.email ?? id
  }

  function move(e: KeyboardEvent) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
    const items = [...(list?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])]
    if (items.length === 0) return
    const at = items.indexOf(document.activeElement as HTMLElement)
    e.preventDefault()
    items[Math.max(0, Math.min(items.length - 1, at < 0 ? 0 : at + (e.key === 'ArrowDown' ? 1 : -1)))].focus()
  }
</script>

<Sheet testid="channel-browser" label={t('modals.channels.title')} width="38rem" onclose={modalsClose}>
  {#if creating}
    <form class="flex flex-col gap-3 px-5 py-4" data-testid="channel-create" onsubmit={(e) => { e.preventDefault(); create() }}>
      <h2 class="text-[15px] font-semibold tracking-tight text-ink">{t('modals.channels.create')}</h2>
      {#if creatable.length > 1}
        <Segmented bind:value={draftProvider} options={creatable.map((p) => ({ value: p.id, label: p.name }))} size="sm" label={t('modals.channels.source')} />
      {/if}
      <label class="flex flex-col gap-1 text-[13px] text-ink-3">
        {t('modals.channels.name')}
        <input data-testid="channel-create-name" bind:value={draftName} class="border-b border-line bg-transparent py-1.5 text-[15px] text-ink outline-none focus:border-accent" />
      </label>
      <div class="flex items-center justify-between text-[13.5px] text-ink">
        <span>{t('modals.channels.private')}</span>
        <Switch bind:checked={draftPrivate} label={t('modals.channels.private')} testid="channel-create-private" />
      </div>
      <label class="flex flex-col gap-1 text-[13px] text-ink-3">
        {t('modals.channels.members')}
        <input data-testid="channel-create-members" bind:value={memberQuery} placeholder={t('modals.channels.addMember')} class="border-b border-line bg-transparent py-1.5 text-[14px] text-ink outline-none focus:border-accent" />
      </label>
      {#if memberMatches.length}
        <div class="flex flex-col" role="menu">
          {#each memberMatches as user (user.id)}
            <MenuItem icon="lucide:user-plus" onclick={() => { memberIds = [...memberIds, user.id]; memberQuery = '' }}>{user.display_name ?? user.email}</MenuItem>
          {/each}
        </div>
      {/if}
      {#if memberIds.length}
        <div class="flex flex-wrap gap-1.5">
          {#each memberIds as id (id)}
            <button type="button" class="inline-flex h-6 items-center gap-1 rounded-[0.4rem] bg-accent-soft px-2 text-[12.5px] text-ink" onclick={() => (memberIds = memberIds.filter((m) => m !== id))}>{userName(id)} ×</button>
          {/each}
        </div>
      {/if}
      {#if failure}<p class="text-[13px] text-error-500" data-testid="channel-browser-error">{failure}</p>{/if}
      <div class="flex justify-end gap-2">
        <Button onclick={() => (creating = false)}>{t('common.back')}</Button>
        <Button variant="primary" type="submit" keys="↵" busy={busy} disabled={!draftName.trim() || !draftProvider} testid="channel-create-submit">{t('modals.channels.createSubmit')}</Button>
      </div>
    </form>
  {:else}
    <div class="flex items-center gap-3 border-b border-line px-5 py-3">
      <input type="text" data-testid="channel-browser-search" bind:value={searchQuery} placeholder={t('modals.channels.search')} class="flex-1 bg-transparent text-[15px] text-ink outline-none placeholder:text-ink-3" onkeydown={(e) => { if (e.key === 'ArrowDown') { e.preventDefault(); list?.querySelector<HTMLElement>('[role="menuitem"]')?.focus() } }} />
      <Segmented bind:value={activeTab} options={providerTabs} size="sm" label={t('modals.channels.source')} />
    </div>
    <div bind:this={list} class="px-2.5 py-2" role="menu" tabindex="-1" onkeydown={move}>
      {#if failure}<p class="px-2.5 pb-2 text-[13px] text-error-500" data-testid="channel-browser-error">{failure}</p>{/if}
      {#if synced.length}
        <div class="px-2.5 pb-1 pt-1 text-[12px] font-medium text-ink-3">{t('modals.channels.joined')}</div>
        {#each synced as channel (channel.id)}
          {@const channelProvider = channel.providers[0]}
          <MenuItem testid="channel-joined" value={channel.id} icon={channelProvider ? providerIcon(channelProvider, 'channel') : 'lucide:hash'} detail={channel.members.length ? String(channel.members.length) : undefined} onclick={() => openChannel(channel.id)}>
            {channel.name}{#if channel.summary}<span class="text-ink-3"> {renderEmojiText(channel.summary)}</span>{/if}
          </MenuItem>
        {/each}
      {/if}
      {#if open.length}
        <div class="px-2.5 pb-1 pt-3 text-[12px] font-medium text-ink-3">{t('modals.channels.open')}</div>
        {#each open as entry (entry.provider_id + entry.conversation.external_id)}
          <MenuItem testid="channel-joinable" value={entry.conversation.external_id} icon="lucide:hash" detail={joining === entry.conversation.external_id ? t('modals.channels.joining') : t('modals.channels.join')} onclick={() => join(entry)}>
            {entry.conversation.name}<span class="text-ink-3"> {String(entry.conversation.metadata.purpose ?? entry.conversation.metadata.topic ?? '')} · {entry.provider_name}</span>
          </MenuItem>
        {/each}
      {/if}
      {#if synced.length === 0 && open.length === 0}
        <div class="py-10 text-center text-[13px] text-ink-3">{loadingJoinable ? t('modals.channels.loading') : t(searchQuery ? 'modals.channels.noMatch' : 'modals.channels.none')}</div>
      {/if}
    </div>
  {/if}
  {#snippet footer()}
    {#if !creating}
      <span class="text-[12px] text-ink-3">{t('modals.channels.count', { count: synced.length + open.length })}</span>
      {#if creatable.length}
        <Button size="sm" icon="lucide:plus" testid="channel-create-open" onclick={startCreate}>{t('modals.channels.create')}</Button>
      {/if}
    {/if}
    <span class="ml-auto flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="esc" /> {t('modals.footer.close')}</span>
  {/snippet}
</Sheet>
