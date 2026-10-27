<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import { goto } from '$app/navigation'
  import { api, closeNewChat, loadChats, newChatStore } from '$lib/tauri'
  import { CHAT_MODES, CHAT_VIEWS, RECEIPTS_KEY } from '$lib/tauri/chats'
  import type { Action, ChatMode, ChatSource, ChatView } from '$lib/tauri/types'
  import { Button, Field, IconPicker, Segmented, Sheet } from '$lib/components/ui'
  import SourcePicker from '$lib/components/conversation/SourcePicker.svelte'
  import ActionChoice from '$lib/components/conversation/ActionChoice.svelte'
  import { defaultRoutes } from '$lib/pipe'
  import Icon from '@iconify/svelte'
  import { t } from '$lib/i18n.svelte'

  let name = $state('')
  let icon = $state<string | null>(null)
  let picking = $state(false)
  let mode = $state<ChatMode>('read')
  let view = $state<ChatView>('chat')
  let sources = $state<ChatSource[]>([])
  let chosen = $state<string[]>([])
  let copyReceipts = $state(false)
  let actions = $state<Action[]>([])
  let busy = $state(false)
  let failure = $state<string | null>(null)

  const target = $derived($newChatStore)
  const ready = $derived(name.trim().length > 0 && !busy)
  const preview = $derived.by(() => {
    const picked = actions.filter((a) => chosen.includes(a.id))
    const receipts = copyReceipts ? actions.find((a) => a.key === RECEIPTS_KEY) : undefined
    return [...(receipts ? [receipts] : []), ...picked].map((a) => ({ name: a.name, outputs: a.outputs.map((p) => `${p.name}: ${p.type}`).join(', '), lands: defaultRoutes(a.outputs).length > 0 && mode === 'read' }))
  })

  $effect(() => {
    const opened = target
    if (!opened) return
    name = ''
    mode = opened.mode ?? 'read'
    view = 'chat'
    sources = opened.sources ?? []
    chosen = opened.actionId ? [opened.actionId] : []
    copyReceipts = false
    failure = null
    api.getActions().then((list) => (actions = list)).catch(() => {})
  })

  async function create() {
    if (!ready) return
    busy = true
    failure = null
    try {
      const ids = [...chosen]
      const receipts = actions.find((a) => a.key === RECEIPTS_KEY)
      if (copyReceipts && receipts) ids.push((await api.postActionCopy(receipts.id, true, t('chats.actions.copyName', { chat: name.trim() }))).id)
      const chat = await api.postChat({ name: name.trim(), icon, mode, view: mode === 'read' ? view : 'chat', sources: mode === 'read' ? sources : sources.filter((s) => s.kind === 'conversation'), action_ids: ids })
      await loadChats()
      closeNewChat()
      await api.putCurrentConversation(chat.conversation_id).catch(() => {})
      goto(conversationHref(chat.conversation_id))
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }
</script>

{#if target}
  <Sheet testid="new-chat" label={t('chats.new.title')} width="36rem" emphasis="strong" onclose={closeNewChat}>
    <form class="flex flex-col gap-4 px-5 pb-4 pt-5" onsubmit={(e) => { e.preventDefault(); create() }}>
      <h2 class="text-[16px] font-semibold tracking-tight text-ink">{t('chats.new.title')}</h2>
      <label class="flex flex-col gap-1 text-[12px] text-ink-3">
        {t('chats.new.name')}
        <Field data-testid="new-chat-name" bind:value={name} placeholder={t('chats.new.namePlaceholder')} icon={icon ?? (mode === 'read' ? 'lucide:book-open' : 'lucide:message-circle')} onicon={() => (picking = true)} />
      </label>
      <div class="flex items-center gap-3">
        <div class="min-w-0 flex-1">
          <p class="text-[13px] text-ink">{t('chats.mode.label')}</p>
          <p class="text-[12px] text-ink-3">{mode === 'read' ? t('chats.new.readHint') : t('chats.new.writeHint')}</p>
        </div>
        <Segmented testid="new-chat-mode" size="sm" label={t('chats.mode.label')} value={mode} options={CHAT_MODES.map((m) => ({ value: m.value, label: t(m.label), icon: m.icon }))} onchange={(v) => (mode = v as ChatMode)} />
      </div>
      <div class="flex flex-col gap-1.5">
        <p class="text-[13px] text-ink">{mode === 'read' ? t('chats.source.title') : t('chats.source.writesThrough')}</p>
        {#if mode === 'write'}<p class="text-[12px] leading-5 text-ink-3">{t('chats.new.throughHint')}</p>{/if}
        {#key mode}<SourcePicker {sources} writes={mode === 'write'} onchange={(next) => (sources = next)} />{/key}
      </div>
      {#if mode === 'read'}
        <div class="flex items-center gap-3">
          <p class="min-w-0 flex-1 text-[13px] text-ink">{t('chats.view.label')}</p>
          <Segmented testid="new-chat-view" size="sm" label={t('chats.view.label')} value={view} options={CHAT_VIEWS.map((v) => ({ value: v.value, label: t(v.label), icon: v.icon }))} onchange={(v) => (view = v as ChatView)} />
        </div>
      {/if}
      <div class="flex flex-col gap-1.5">
        <p class="text-[13px] text-ink">{t('chats.actions.title')} <span class="text-[12px] text-ink-3">{t('chats.new.optional')}</span></p>
        <p class="text-[12px] leading-5 text-ink-3">{mode === 'read' ? t('chats.actions.hintRead') : t('chats.actions.hintWrite')}</p>
        <ActionChoice {actions} {chosen} {copyReceipts} onchange={(next) => (chosen = next)} oncopy={(on) => (copyReceipts = on)} />
        {#if preview.length}
          <ul class="flex flex-col gap-1 border-t border-line pt-2">
            <li class="flex items-center gap-1.5 text-[12px] text-ink-3"><Icon icon="lucide:folder-input" class="text-[13px]" />{t('chats.pipe.sources', { count: sources.length })}</li>
            {#each preview as step (step.name)}
              <li class="grid grid-cols-[1.25rem_minmax(0,1fr)] gap-x-1.5 text-[12px]">
                <Icon icon="lucide:arrow-down" class="mt-0.5 text-[12px] text-ink-3" />
                <span class="flex min-w-0 flex-col">
                  <span class="text-ink">{step.name}</span>
                  <span class="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11px] text-ink-3">{t('chats.pipe.out')} {step.outputs || t('chats.pipe.noOutputs')}</span>
                  <span class="{step.lands ? 'text-ink-2' : 'text-ink-3'}">{step.lands ? t('chats.pipe.landsHere') : t('chats.pipe.nowhere')}</span>
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
      {#if failure}<p data-testid="new-chat-failure" class="text-[12.5px] text-error-600">{failure}</p>{/if}
      <button type="submit" class="hidden" aria-hidden="true" tabindex="-1"></button>
    </form>
    {#snippet footer()}
      <span class="flex-1"></span>
      <Button testid="new-chat-cancel" onclick={closeNewChat}>{t('common.cancel')}</Button>
      <Button testid="new-chat-create" variant="primary" icon="lucide:plus" busy={busy} disabled={!ready} onclick={create}>{t('chats.new.create')}</Button>
    {/snippet}
  </Sheet>
{/if}
{#if picking}
  <IconPicker value={icon} fallback={mode === 'read' ? 'lucide:book-open' : 'lucide:message-circle'} onpick={(next) => (icon = next)} onclose={() => (picking = false)} />
{/if}
