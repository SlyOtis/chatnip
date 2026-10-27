<script lang="ts">
  import Icon from '@iconify/svelte'
  import { untrack } from 'svelte'
  import { modalsClose } from '$lib/modals'
  import { api, providersStore } from '$lib/tauri'
  import type { Message } from '$lib/tauri/types'
  import type { EmailComposeModalData } from '$lib/types'
  import { Button, Kbd, Select, Sheet } from '$lib/components/ui'
  import { formatDate, t, type MessageKey } from '$lib/i18n.svelte'


  let { mode, original_message, to: initialTo, subject: initialSubject, conversation_id, provider_id: initialProviderId, body: initialBody, cc: initialCc, bcc: initialBcc, references }: EmailComposeModalData = $props()

  type Field = 'to' | 'cc' | 'bcc'

  let chips = $state<Record<Field, string[]>>({ to: initialTo ?? [], cc: initialCc ?? [], bcc: initialBcc ?? [] })
  let inputs = $state<Record<Field, string>>({ to: '', cc: '', bcc: '' })
  let subject = $state(initialSubject ?? '')
  let body = $state(initialBody ?? '')
  let showCc = $state(!!initialCc?.length)
  let showBcc = $state(!!initialBcc?.length)
  let isSending = $state(false)
  let failure = $state<string | null>(null)
  let selectedProviderId = $state(initialProviderId ?? '')
  let toInputEl = $state<HTMLInputElement | null>(null)

  const emailProviders = $derived(($providersStore?.data ?? []).filter((p) => p.is_connected && p.is_enabled))
  const providerOptions = $derived(emailProviders.map((p) => ({ value: p.id, label: p.name })))

  const title = $derived.by(() => {
    switch (mode) {
      case 'reply':
        return t('modals.compose.reply')
      case 'reply_all':
        return t('modals.compose.replyAll')
      case 'forward':
        return t('modals.compose.forward')
      default:
        return t('modals.compose.newMail')
    }
  })

  $effect(() => {
    if (!original_message) return
    if (mode === 'reply' || mode === 'reply_all') {
      subject = prefixed('Re', extractSubject(original_message))
      body = buildQuotedBody(original_message)
      chips.to = senderOf(original_message)
      if (mode === 'reply_all') chips.cc = extractRecipients(original_message).filter((a) => !chips.to.includes(a))
      showCc = chips.cc.length > 0
    }
    if (mode === 'forward') {
      subject = prefixed('Fwd', extractSubject(original_message))
      body = ''
    }
  })

  $effect(() => {
    const providers = emailProviders
    untrack(() => {
      if (!selectedProviderId && providers.length > 0) selectedProviderId = providers[0].id
    })
  })

  $effect(() => {
    toInputEl?.focus()
  })

  function prefixed(prefix: string, value: string): string {
    return value.toLowerCase().startsWith(`${prefix.toLowerCase()}:`) ? value : `${prefix}: ${value}`
  }

  function extractSubject(message: Message): string {
    if (message.subject?.trim()) return message.subject.trim()
    const lines = message.content.split('\n')
    const subjectLine = lines.find((l) => l.toLowerCase().startsWith('subject:'))
    if (subjectLine) return subjectLine.replace(/^subject:\s*/i, '').trim()
    return lines[0]?.slice(0, 80) || ''
  }

  function senderOf(message: Message): string[] {
    const who = message.sender_email || message.sender_name
    return who ? [who] : []
  }

  function extractRecipients(message: Message): string[] {
    const mine = ($providersStore?.data ?? []).find((p) => p.id === message.provider_id)?.namespace?.toLowerCase() ?? ''
    const all = [...(message.recipients ?? []), ...(message.cc_recipients ?? [])].map((r) => r.address).filter((a) => !!a && a.toLowerCase() !== mine)
    return [...new Set(all)]
  }

  function buildQuotedBody(message: Message): string {
    const timestamp = formatDate(message.timestamp, { dateStyle: 'medium', timeStyle: 'short' })
    return t('modals.compose.quoted', { sender: message.sender_name || t('common.unknown'), when: timestamp, content: message.content })
  }

  function addChip(target: Field) {
    const value = inputs[target].trim()
    if (!value) return
    chips[target] = [...chips[target], value]
    inputs[target] = ''
  }

  function removeChip(target: Field, index: number) {
    chips[target] = chips[target].filter((_, i) => i !== index)
  }

  function handleChipKeydown(e: KeyboardEvent, target: Field) {
    if (e.key === 'Enter') {
      e.preventDefault()
      addChip(target)
    }
    if (e.key === 'Backspace' && !inputs[target] && chips[target].length > 0) removeChip(target, chips[target].length - 1)
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault()
      handleSend()
    }
  }

  const canSend = $derived(!isSending && chips.to.length > 0 && (mode === 'forward' ? !!original_message : !!references?.forward_of || body.trim().length > 0))
  const forwardedFiles = $derived(mode === 'forward' ? (original_message?.attachments ?? []).filter((a) => !a.is_inline).map((a) => a.name) : [])

  async function handleSend() {
    if (!canSend) return
    isSending = true
    failure = null
    try {
      const linked: Record<string, unknown> = { ...(references ?? {}) }
      if (original_message && mode === 'forward') linked.forward_of = [original_message.id]
      else if (original_message) linked.in_reply_to = original_message.id
      const metadata: Record<string, unknown> = { ...linked, to: chips.to, subject, content_type: 'email' }
      if (chips.cc.length > 0) metadata.cc = chips.cc
      if (chips.bcc.length > 0) metadata.bcc = chips.bcc
      const target = conversation_id || original_message?.conversation_id || ''
      const draft: EmailComposeModalData = { mode: 'new', to: chips.to, cc: chips.cc, bcc: chips.bcc, subject, body, conversation_id: target || undefined, provider_id: selectedProviderId || undefined, references: linked }
      await api.postMessage(
        {
          conversation_id: target,
          content: body,
          content_type: 'email',
          provider_id: selectedProviderId || undefined,
          metadata
        },
        { surface: 'compose', conversation_id: target || null, draft }
      )
      modalsClose()
    } catch (error) {
      failure = String(error)
      isSending = false
    }
  }

  const FIELDS: Record<Field, MessageKey> = { to: 'modals.field.to', cc: 'modals.field.cc', bcc: 'modals.field.bcc' }
  const line = 'flex items-center gap-3 border-b border-line py-1.5'
  const key = 'w-12 shrink-0 font-mono text-[11px] text-ink-3'
</script>

<svelte:window onkeydown={handleKeydown} />

<Sheet label={t('modals.compose.label')} width="44rem" testid="email-compose" onclose={modalsClose}>
  <div class="flex flex-col px-5 pt-4">
    <h2 class="pb-2 text-[15px] font-semibold tracking-tight text-ink">{title}</h2>
    {#each ['to', 'cc', 'bcc'] as const as target (target)}
      {#if target === 'to' || (target === 'cc' && showCc) || (target === 'bcc' && showBcc)}
        <div class={line}>
          <span class={key}>{t(FIELDS[target])}</span>
          <div class="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">
            {#each chips[target] as chip, i (chip + i)}
              <button type="button" class="inline-flex items-center gap-1 text-[14px] text-ink hover:text-error-600" onclick={() => removeChip(target, i)} aria-label={t('modals.removeToken', { name: chip })}>
                {chip}<Icon icon="lucide:x" class="text-[12px] text-ink-3" />
              </button>
            {/each}
            {#if target === 'to'}
              <input bind:this={toInputEl} data-testid="email-compose-to" bind:value={inputs.to} onkeydown={(e) => handleChipKeydown(e, 'to')} onblur={() => addChip('to')} placeholder={chips.to.length === 0 ? t('modals.compose.recipients') : ''} class="min-w-[8rem] flex-1 bg-transparent text-[14px] text-ink outline-none placeholder:text-ink-3" />
            {:else}
              <input bind:value={inputs[target]} onkeydown={(e) => handleChipKeydown(e, target)} onblur={() => addChip(target)} class="min-w-[8rem] flex-1 bg-transparent text-[14px] text-ink outline-none" />
            {/if}
          </div>
          {#if target === 'to'}
            <span class="flex shrink-0 items-center gap-2 font-mono text-[11px] text-ink-3">
              {#if !showCc}<button type="button" class="hover:text-ink" onclick={() => (showCc = true)}>{t('modals.field.cc')}</button>{/if}
              {#if !showBcc}<button type="button" class="hover:text-ink" onclick={() => (showBcc = true)}>{t('modals.field.bcc')}</button>{/if}
            </span>
          {/if}
        </div>
      {/if}
    {/each}
    <div class={line}>
      <span class={key}>{t('modals.field.subject')}</span>
      <input bind:value={subject} data-testid="email-compose-subject" placeholder={t('modals.compose.subject')} class="flex-1 bg-transparent text-[14px] text-ink outline-none placeholder:text-ink-3" />
    </div>
    {#if emailProviders.length > 1}
      <div class={line}>
        <span class={key}>{t('modals.field.via')}</span>
        <Select bind:value={selectedProviderId} options={providerOptions} size="sm" testid="email-compose-via" />
      </div>
    {/if}
    {#if mode === 'forward' && original_message}
      <div class="flex min-w-0 flex-col gap-0.5 border-b border-line py-2 text-[12.5px] text-ink-3">
        <span class="font-mono text-[11px]">{t('modals.compose.forward')}</span>
        <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-ink-2">{original_message.sender_name || original_message.sender_email || t('common.unknown')} · {extractSubject(original_message)}</span>
        {#if forwardedFiles.length}
          <span class="flex min-w-0 items-center gap-1"><Icon icon="lucide:paperclip" class="shrink-0 text-[11px]" /><span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{t('modals.compose.attachments', { list: forwardedFiles.join(', ') })}</span></span>
        {/if}
      </div>
    {/if}
    <textarea bind:value={body} data-testid="email-compose-body" placeholder={t('modals.compose.body')} class="min-h-[14rem] flex-1 resize-none bg-transparent py-3 text-[14px] leading-6 text-ink outline-none placeholder:text-ink-3"></textarea>
    {#if failure}
      <p class="pb-2 text-[12px] text-error-600" data-testid="email-compose-error">{failure}</p>
    {/if}
  </div>
  {#snippet footer()}
    {#if emailProviders.length === 1}
      <span class="font-mono text-[11px] text-ink-3">{t('modals.compose.via', { name: emailProviders[0].name })}</span>
    {/if}
    <span class="ml-auto flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="esc" /> {t('modals.footer.close')}</span>
    <Button variant="primary" keys="⌘↵" testid="email-compose-send" busy={isSending} disabled={!canSend} onclick={handleSend}>{t('common.send')}</Button>
  {/snippet}
</Sheet>
