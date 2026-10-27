<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { api, chatsStore, loadChats, putChatMode, putChatView, readStore } from '$lib/tauri'
  import { CHAT_MODES, CHAT_VIEWS, RECEIPTS_KEY } from '$lib/tauri/chats'
  import type { Action, ChatSource, Conversation } from '$lib/tauri/types'
  import { IconButton, Segmented } from '$lib/components/ui'
  import SourcePicker from '../SourcePicker.svelte'
  import ActionChoice from '../ActionChoice.svelte'
  import PipePanel from './PipePanel.svelte'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    conversation: Conversation
  }

  let { conversation }: Props = $props()

  let actions = $state<Action[]>([])
  let picking = $state(false)
  let editingSources = $state(false)
  let busy = $state(false)
  let failure = $state<string | null>(null)

  const chat = $derived($chatsStore.find((c) => c.conversation_id === conversation.id) ?? null)
  const reading = $derived($readStore.has(conversation.id))
  const inherited = $derived(reading && conversation.mode !== 'read')
  const attached = $derived(actions.filter((a) => a.chats.includes(conversation.id)))

  async function loadActions() {
    actions = await api.getActions().catch(() => actions)
  }

  onMount(() => {
    loadActions()
    const off = listen('actions-updated', loadActions)
    return () => off.then((stop) => stop())
  })

  async function run(work: () => Promise<unknown>) {
    busy = true
    failure = null
    try {
      await work()
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }

  function setSources(sources: ChatSource[]) {
    if (!chat) return
    run(async () => {
      await api.putChat({ chat_id: chat.id, sources })
      await loadChats()
    })
  }

  function attach(ids: string[]) {
    const added = ids.filter((id) => !attached.some((a) => a.id === id))
    const removed = attached.filter((a) => !ids.includes(a.id)).map((a) => a.id)
    run(async () => {
      for (const id of added) await api.putChatAction(conversation.id, id)
      for (const id of removed) await api.deleteChatAction(conversation.id, id)
      await Promise.all([loadActions(), loadChats()])
    })
  }

  function copyReceipts() {
    const receipts = actions.find((a) => a.key === RECEIPTS_KEY)
    if (!receipts) return
    run(async () => {
      const copy = await api.postActionCopy(receipts.id, true, t('chats.actions.copyName', { chat: conversation.name }))
      await api.putChatAction(conversation.id, copy.id)
      await Promise.all([loadActions(), loadChats()])
      picking = false
    })
  }
</script>

<div class="flex flex-col gap-3 border-b border-line py-3">
  <div class="flex items-center gap-3">
    <div class="min-w-0 flex-1">
      <p class="text-[13px] text-ink">{t('chats.mode.label')}</p>
      <p class="text-[12px] text-ink-3">{inherited ? t('chats.mode.inherited') : reading ? t('chats.mode.readHint') : t('chats.mode.writeHint')}</p>
    </div>
    <Segmented testid="chat-mode" size="sm" label={t('chats.mode.label')} value={reading ? 'read' : 'write'} options={CHAT_MODES.map((m) => ({ value: m.value, label: t(m.label), icon: m.icon }))} onchange={(v) => run(() => putChatMode(conversation.id, v as 'read' | 'write'))} />
  </div>
  {#if reading}
    <div class="flex items-center gap-3">
      <p class="min-w-0 flex-1 text-[13px] text-ink">{t('chats.view.label')}</p>
      <Segmented testid="chat-view" size="sm" label={t('chats.view.label')} value={conversation.view} options={CHAT_VIEWS.map((v) => ({ value: v.value, label: t(v.label), icon: v.icon }))} onchange={(v) => run(() => putChatView(conversation.id, v as 'chat' | 'grid'))} />
    </div>
  {/if}
</div>

{#if chat}
  <div class="border-b border-line py-3">
    <div class="flex h-7 items-center justify-between">
      <p class="text-[13px] text-ink-3">{chat.mode === 'read' ? t('chats.source.title') : t('chats.source.writesThrough')} <span class="font-mono text-[11px]">{chat.sources.length}</span></p>
      <IconButton testid="chat-sources-edit" size="sm" icon={editingSources ? 'lucide:check' : 'lucide:pencil'} label={editingSources ? t('common.done') : t('chats.source.edit')} onclick={() => (editingSources = !editingSources)} />
    </div>
    {#if editingSources}
      <div class="mt-1.5"><SourcePicker sources={chat.sources} self={conversation.id} writes={chat.mode === 'write'} onchange={setSources} /></div>
    {:else}
      <p class="mt-1 text-[12.5px] leading-5 text-ink-3">{chat.sources.length ? (chat.mode === 'read' ? t('chats.source.summary', { count: chat.sources.length }) : t('chats.source.throughSummary', { count: chat.sources.length })) : chat.mode === 'read' ? t('chats.source.none') : t('chats.source.notesOnly')}</p>
    {/if}
  </div>
{/if}

<div class="border-b border-line py-3">
  <div class="flex h-7 items-center justify-between">
    <p class="text-[13px] text-ink-3">{t('chats.pipe.title')} <span class="font-mono text-[11px]" data-testid="chat-actions-count">{attached.length}</span></p>
    <IconButton testid="chat-actions-attach" size="sm" icon={picking ? 'lucide:check' : 'lucide:plus'} label={picking ? t('common.done') : t('chats.actions.attach')} busy={busy} onclick={() => (picking = !picking)} />
  </div>
  <p class="text-[12px] leading-5 text-ink-3">{reading ? t('chats.actions.hintRead') : t('chats.actions.hintWrite')}</p>
  {#if picking}
    <div class="mt-1.5">
      <ActionChoice {actions} chosen={attached.map((a) => a.id)} copyReceipts={false} onchange={attach} oncopy={(on) => on && copyReceipts()} />
    </div>
  {/if}
  {#if attached.length}
    <div class="mt-2"><PipePanel conversationId={conversation.id} view={conversation.view} revision={attached.length} ondetach={(id) => attach(attached.filter((a) => a.id !== id).map((a) => a.id))} /></div>
  {/if}
  {#if failure}<p data-testid="chat-settings-error" class="mt-1 text-[12px] text-error-600">{failure}</p>{/if}
</div>
