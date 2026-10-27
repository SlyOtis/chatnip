<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { untrack } from 'svelte'
  import { modalsClose } from '$lib/modals'
  import { anchored, boxOf } from '$lib/popover'
  import { api, providersStore, usersStore } from '$lib/tauri'
  import type { Message, Provider, User } from '$lib/tauri/types'
  import { providerIcon } from '$lib/providerIcons'
  import { Button, Segmented, Sheet } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'


  interface Props {
    message: Message
    conversation_id: string
  }

  let { message, conversation_id }: Props = $props()

  let conversationName = $state(t('modals.discuss.defaultName', { subject: extractSubject(message) }))
  let selectedProviderId = $state('')
  let selectedUsers = $state<User[]>([])
  let userSearchQuery = $state('')
  let userSearchFocused = $state(false)
  let peopleRow = $state<HTMLDivElement | null>(null)
  let isCreating = $state(false)
  let error = $state('')

  const chatProviders = $derived(
    $providersStore.data.filter((p) => p.is_connected && p.is_enabled && p.capabilities.can_create_conversations && p.capabilities.can_send_messages && p.provider_type !== 'microsoft_email' && p.provider_type !== 'google_email')
  )
  const selectedProvider = $derived<Provider | null>(chatProviders.find((p) => p.id === selectedProviderId) ?? null)
  const providerOptions = $derived(chatProviders.map((p) => ({ value: p.id, label: p.name, icon: providerIcon(p, 'group_message') })))

  const filteredUsers = $derived.by(() => {
    if (!userSearchQuery.trim()) return []
    const q = userSearchQuery.toLowerCase()
    return $usersStore.data
      .filter((u) => {
        if (selectedUsers.some((s) => s.id === u.id)) return false
        const name = (u.display_name || '').toLowerCase()
        const email = (u.email || '').toLowerCase()
        return name.includes(q) || email.includes(q)
      })
      .slice(0, 8)
  })

  $effect(() => {
    const providers = chatProviders
    untrack(() => {
      if (providers.length > 0 && !selectedProviderId) selectedProviderId = providers[0].id
    })
  })

  function extractSubject(msg: Message): string {
    if (msg.metadata) {
      try {
        const meta = JSON.parse(msg.metadata)
        if (meta.subject) return meta.subject.slice(0, 60)
      } catch {}
    }
    return msg.content?.slice(0, 50) || t('modals.discuss.email')
  }

  function formatEmailPreview(msg: Message): string {
    const content = msg.content || ''
    const lines = content.split('\n').filter((l) => l.trim())
    return lines.slice(0, 5).join('\n')
  }

  function addUser(user: User) {
    selectedUsers = [...selectedUsers, user]
    userSearchQuery = ''
  }

  function removeUser(user: User) {
    selectedUsers = selectedUsers.filter((u) => u.id !== user.id)
  }

  async function handleCreate() {
    if (!selectedProvider) {
      error = t('modals.discuss.selectProvider')
      return
    }
    if (selectedUsers.length === 0) {
      error = t('modals.discuss.addParticipant')
      return
    }
    isCreating = true
    error = ''
    try {
      const newConversation = await api.postConversation({
        name: conversationName,
        conversation_type: 'group_message',
        group_type: 'custom',
        member_ids: selectedUsers.map((u) => u.id)
      })
      await api.postConversationLink({ source_conversation_id: conversation_id, target_conversation_id: newConversation.id, link_type: 'discuss' })
      const subject = extractSubject(message)
      const emailContent = t('modals.discuss.content', { subject, sender: message.sender_name || t('common.unknown'), content: message.content || '' })
      await api.postMessage({ conversation_id: newConversation.id, content: emailContent, provider_id: selectedProvider.id })
      modalsClose()
      await api.putCurrentConversation(newConversation.id)
      await goto(conversationHref(newConversation.id))
    } catch (e: any) {
      error = e?.message || t('modals.discuss.failed')
    } finally {
      isCreating = false
    }
  }

  const line = 'flex items-center gap-3 border-b border-line py-2'
  const key = 'w-12 shrink-0 font-mono text-[11px] text-ink-3'
</script>

<Sheet label={t('modals.discuss.label')} width="40rem" onclose={modalsClose}>
  <div class="px-5 pb-3 pt-5">
    <h2 class="text-[15px] font-semibold tracking-tight text-ink">{t('modals.discuss.title')}</h2>
    <p class="mt-0.5 text-[13.5px] text-ink-3">{t('modals.discuss.description')}</p>

    <div class="mt-4">
      <div class={line}>
        <span class={key}>{t('modals.field.via')}</span>
        {#if chatProviders.length === 0}
          <span class="text-[13.5px] text-ink-3">{t('modals.discuss.noConnections')}</span>
        {:else}
          <Segmented bind:value={selectedProviderId} options={providerOptions} size="sm" label={t('modals.discuss.sendThrough')} />
        {/if}
      </div>
      <div class={line}>
        <span class={key}>{t('modals.field.name')}</span>
        <input type="text" bind:value={conversationName} class="flex-1 bg-transparent text-[14px] text-ink outline-none placeholder:text-ink-3" />
      </div>
      <div class="relative {line}" bind:this={peopleRow}>
        <span class={key}>{t('modals.field.with')}</span>
        <div class="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">
          {#each selectedUsers as user (user.id)}
            <button type="button" class="inline-flex items-center gap-1 text-[14px] text-ink hover:text-error-600" onclick={() => removeUser(user)} aria-label={t('modals.removeToken', { name: user.display_name || user.email || '' })}>
              {user.display_name || user.email || t('common.unknown')}<Icon icon="lucide:x" class="text-[12px] text-ink-3" />
            </button>
          {/each}
          <input
            type="text"
            bind:value={userSearchQuery}
            onfocus={() => (userSearchFocused = true)}
            onblur={() => setTimeout(() => (userSearchFocused = false), 200)}
            placeholder={selectedUsers.length === 0 ? t('modals.discuss.searchPeople') : ''}
            class="min-w-[8rem] flex-1 bg-transparent text-[14px] text-ink outline-none placeholder:text-ink-3"
          />
        </div>
        {#if filteredUsers.length > 0 && userSearchFocused}
          <div class="menu max-h-44 w-[min(24rem,80vw)] overflow-y-auto" use:anchored={{ at: () => boxOf(peopleRow), align: 'start', onclose: () => (userSearchFocused = false) }} role="menu">
            {#each filteredUsers as user (user.id)}
              <button type="button" role="menuitem" class="flex h-8 w-full items-center gap-2.5 rounded-[0.5rem] px-2.5 text-left text-[14px] text-ink hover:bg-paper-3" onmousedown={() => addUser(user)}>
                <span class="min-w-0 flex-1 truncate">{user.display_name || t('common.unknown')}<span class="text-ink-3"> {user.email ?? ''}</span></span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <div class="mt-4 border-l-2 border-line pl-3">
      <div class="text-[14px] font-medium text-ink">{extractSubject(message)}</div>
      <div class="text-[13px] text-ink-3">{message.sender_name || t('common.unknown')}</div>
      <div class="mt-1 line-clamp-4 whitespace-pre-line text-[13px] text-ink-2">{formatEmailPreview(message)}</div>
    </div>

    {#if error}
      <p class="mt-3 text-[13px] text-error-500">{error}</p>
    {/if}
  </div>
  {#snippet footer()}
    <Button class="ml-auto" onclick={modalsClose}>{t('common.cancel')}</Button>
    <Button variant="primary" icon="lucide:messages-square" busy={isCreating} disabled={chatProviders.length === 0} onclick={handleCreate}>{t('modals.discuss.start')}</Button>
  {/snippet}
</Sheet>
