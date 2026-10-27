<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import { anchored, boxOf, type Box } from '$lib/popover'
  import { myUserIds, peopleStore } from '$lib/tauri/people'
  import { goto } from '$app/navigation'
  import { page } from '$app/state'
  import { onMount, untrack } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import Icon from '@iconify/svelte'
  import { conversationStore, conversationsById, conversationsStore, messagesStore } from '$lib/tauri/conversations'
  import { api, providersStore, chatsStore, openNewChat, putAccountUsed } from '$lib/tauri'
  import { accountName, accountOf } from '$lib/accounts'
  import { profileSettingsStore, profileStore } from '$lib/tauri/profile'
  import type { Action, ContentItem, MessageSchedule, RemoteAlias, RenderedSignature } from '$lib/tauri/types'
  import { addContent, addFiles, composerVariables, fromOutgoing, isTemplate, removeAttachment, toOutgoing, type PendingAttachment } from '$lib/media'
  import { Button, IconButton, MenuItem, SplitButton, TagPicker } from '$lib/components/ui'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import AttachmentUpload from './AttachmentUpload.svelte'
  import EmojiPicker from '$lib/components/EmojiPicker.svelte'
  import GifPicker from './GifPicker.svelte'
  import ContentPicker from '$lib/components/content/ContentPicker.svelte'
  import Composer from '$lib/components/composer/Composer.svelte'
  import MicButton from '$lib/components/composer/MicButton.svelte'
  import type { VoiceTarget } from '$lib/voice.svelte'
  import { emptyDoc, isEmpty, toMarkdown, type Doc } from '$lib/components/composer/markdown'
  import { defaultSendOption, getSendChoice, personFor, putSendChoice, replyActivity, sendOptions, viaOptions, type SendChoice, type SendOption } from '$lib/composerTargets'
  import type { ComposerCommand } from '$lib/composerCommands'
  import { mergedItems, snoozeUntil } from '$lib/inbox'
  import { layoutSettings, messageSetting } from '$lib/layoutSettings'
  import { formatDate, t } from '$lib/i18n.svelte'
  import { summarisable, summarizeSelection } from '$lib/sections.svelte'
  import SummarizePopover from './messages/SummarizePopover.svelte'
  import { clearReply, clearSelection, editRequest, highlighted, replyPreview, replyState, scrollToMessage, selection, shortcutForward } from '$lib/reply.svelte'
  import { copySelected } from '$lib/export.svelte'
  import { composerText, discardHold, holdsFor, outbound, secondsLeft, stopHold, type ComposerDraft } from '$lib/outbound.svelte'
  import { sendKeys } from '$lib/shortcuts'
  import { requirementBadge, requirementsFor, type NodeRequirement } from '$lib/actions'
  import { settingHref } from '$lib/settings'

  let doc = $state<Doc>(emptyDoc())
  let selectionTagAt = $state<Box | null>(null)
  const targets = $derived(selection.ids)
  let copied = $state(false)

  async function copyChecked() {
    await copySelected()
    copied = true
    setTimeout(() => (copied = false), 1600)
  }
  const targetMessages = $derived(($messagesStore.data ?? []).filter((m) => targets.includes(m.id)))
  let summarizingAt = $state<Box | null>(null)
  let editor = $state<ReturnType<typeof Composer>>()
  let choice = $state<SendChoice | null>(null)
  let sentTick = $state(0)
  let openMenu = $state<'sendAs' | 'more' | 'heading' | 'commands' | 'signature' | null>(null)
  let menuAt = $state<Box | null>(null)
  let deliveryMode = $state<'selected' | 'all'>('selected')
  let selectedKeys = $state<string[]>([])
  let pinned = $state<string | null>(null)
  let pending = $state<PendingAttachment[]>([])
  let gifAt = $state<Box | null>(null)
  let contentAt = $state<Box | null>(null)
  let filling = $state(false)
  let fileInputElement = $state<HTMLInputElement>()
  let clipInputElement = $state<HTMLInputElement>()
  let isDragging = $state(false)
  let emojiAt = $state<Box | null>(null)
  let formatting = $state(false)
  let sending = $state(false)
  let drafting = $state(false)
  let draftError = $state<string | null>(null)
  let aiNeeds = $state.raw<NodeRequirement[]>([])
  const aiBadge = $derived(requirementBadge(aiNeeds))
  let replyAll = $state(false)
  $effect(() => {
    conversation?.id
    replyAll = messageSetting($layoutSettings, 'messages.default_reply') === 'all'
  })
  let intelligence = $state(true)
  let actions = $state<Action[]>([])
  let schedules = $state<MessageSchedule[]>([])
  let scheduleAt = $state('')
  let composer = $state<HTMLDivElement | null>(null)

  const maxLines = $derived($profileSettingsStore?.max_input_lines || 10)
  const conversation = $derived($conversationStore.data)
  const routePerson = $derived(page.url.pathname.startsWith('/main/person/') ? decodeURIComponent(page.url.pathname.split('/')[3]) : null)
  const routeMerged = $derived(page.url.pathname.startsWith('/main/merged/') ? decodeURIComponent(page.url.pathname.split('/')[3]) : null)
  const choiceKey = $derived(routeMerged ?? conversation?.id ?? null)
  const person = $derived(personFor(conversation, $peopleStore, routePerson))
  const localChat = $derived(conversation?.type === 'local' ? ($chatsStore.find((c) => c.conversation_id === conversation.id) ?? null) : null)
  const through = $derived(localChat ? localChat.sources.filter((s) => s.kind === 'conversation').map((s) => $conversationsById.get(s.id ?? '')).filter((c): c is NonNullable<typeof c> => !!c).sort((a, b) => b.last_activity.localeCompare(a.last_activity)) : routeMerged ? [...($mergedItems.get(routeMerged)?.conversations ?? [])].sort((a, b) => b.last_activity.localeCompare(a.last_activity)) : [])
  const viasHere = $derived(conversation?.type === 'local' || routeMerged ? through.flatMap((c) => viaOptions(c, null, $conversationsStore.data).slice(0, 1)) : viaOptions(conversation, person, $conversationsStore.data))
  const replyConversation = $derived(replyState.replying ? ($conversationsById.get(replyState.replying?.conversation_id) ?? null) : null)
  const vias = $derived(replyConversation && (routePerson || routeMerged) && !viasHere.some((v) => v.conversation_id === replyConversation.id) ? [...viaOptions(replyConversation, null, $conversationsStore.data).slice(0, 1), ...viasHere] : viasHere)
  const options = $derived(sendOptions(vias, $providersStore.data, $conversationsStore.data))
  const activity = $derived.by(() => {
    sentTick
    return replyActivity($messagesStore.data ?? [], $myUserIds)
  })
  const routes = $derived.by<SendOption[]>(() => {
    const picked = selectedKeys.map((key) => options.find((o) => o.key === key)).filter((o): o is SendOption => !!o)
    if (picked.length) return picked
    const answer = aimed ? options.filter((o) => o.via.conversation_id === aimed.conversation_id) : []
    if (answer.length) return [answer.find((o) => o.from.provider.id === aimed?.provider_id) ?? answer[0]]
    const held = pinned ? options.find((o) => o.key === pinned) : undefined
    if (held) return [held]
    const fallback = defaultSendOption(options, activity, choice)
    return fallback ? [fallback] : []
  })
  const sendAs = $derived<SendOption | null>(routes[0] ?? null)
  const via = $derived(sendAs?.via ?? null)
  const from = $derived(sendAs?.from ?? null)
  const sender = $derived(accountOf($providersStore.data, from?.provider.id))
  const onCurrent = $derived(!!via && !!conversation && via.conversation_id === conversation.id)
  const isMail = $derived(via ? via.channel === 'mail' : conversation?.type === 'email')
  const replying = $derived(replyState.replying && conversation && (replyState.replying.conversation_id === conversation.id || conversation.type === 'local' || !!routePerson || !!routeMerged) ? replyState.replying : null)
  const replyShown = $derived(replying ? replyPreview(replying, t('common.unknown')) : null)
  const aimed = $derived(replying ?? (highlighted.id ? (($messagesStore.data ?? []).find((m) => m.id === highlighted.id) ?? null) : null))
  const replyCapabilities = $derived(replying ? (accountOf($providersStore.data, replying.provider_id)?.capabilities ?? null) : null)
  const replyStyles = $derived([...(replyCapabilities?.supports_reply_quote && replyCapabilities.reply_quote !== 'unsupported' ? ['quote' as const] : []), ...(replyCapabilities?.supports_threads && replying?.type !== 'email' ? ['thread' as const] : [])])
  let replyStyle = $state<'quote' | 'thread'>('quote')
  $effect(() => {
    replying?.id
    replyStyle = untrack(() => (replyStyles.includes('quote') ? 'quote' : (replyStyles[0] ?? 'quote')))
  })
  const threading = $derived(!!replying && replyStyle === 'thread' && replyStyles.includes('thread'))
  const hasMultipleProviders = $derived(onCurrent && (conversation?.providers.length ?? 0) > 1)
  const fillHeight = true
  const sendsOnEnter = $derived(!isMail && messageSetting($layoutSettings, 'messages.enter_sends') === 'enter')
  const sendKey = $derived(sendKeys(sendsOnEnter ? 'enter' : 'mod'))
  const held = $derived(holdsFor(outbound.holds, conversation?.id))

  let broadcast = $state(false)
  const broadcastable = $derived(threading && !!replyCapabilities && replyCapabilities.thread_broadcast !== 'unsupported')
  $effect(() => {
    replying?.id
    broadcast = false
  })

  let aliases = $state<RemoteAlias[]>([])
  let alias = $state<string | null>(null)
  const senderCapabilities = $derived(sender?.capabilities ?? null)
  $effect(() => {
    const id = from?.provider.id
    const allowed = !!senderCapabilities?.supports_aliases && isMail
    untrack(() => {
      aliases = []
      alias = null
    })
    if (!id || !allowed) return
    let live = true
    api.getAliases(id).then((found) => live && (aliases = found)).catch(() => {})
    return () => {
      live = false
    }
  })

  let signature = $state<RenderedSignature | null>(null)
  let signatureOff = $state(false)
  let signatureId = $state<string | null>(null)
  let signatureChoices = $state<ContentItem[]>([])
  $effect(() => {
    conversation?.id
    untrack(() => {
      signatureOff = false
      signatureId = null
    })
  })
  $effect(() => {
    const id = from?.provider.id
    const conversationId = conversation?.id ?? null
    const chosen = signatureId
    const mode = lastInbound ? 'reply' : 'new'
    const address = alias
    signature = null
    if (!id || !isMail) return
    let live = true
    api.getSignature({ provider_id: id, alias: address, mode, conversation_id: conversationId, content_id: chosen }).then((found) => live && (signature = found)).catch(() => {})
    return () => {
      live = false
    }
  })

  async function pickSignature(e: MouseEvent) {
    menuAt = boxOf(e.currentTarget)
    openMenu = openMenu === 'signature' ? null : 'signature'
    if (openMenu === 'signature') signatureChoices = await api.getContent('signature').catch(() => [])
  }

  let restoredFor = $state<string | null>(null)
  let skipSave = false
  $effect(() => {
    const id = conversation?.id
    if (!id || !editor || id === restoredFor) return
    untrack(() => {
      restoredFor = null
      api
        .getDrafts(id)
        .then((rows) => {
          const row = rows.find((r) => !r.thread_id)
          if (row && conversation?.id === id && editor?.isBlank() && row.markdown.trim()) {
            skipSave = true
            editor?.insertMarkdown(row.markdown)
          }
        })
        .catch(() => {})
        .finally(() => {
          if (conversation?.id === id) restoredFor = id
        })
    })
  })

  $effect(() => {
    const id = conversation?.id
    const markdown = toMarkdown(doc)
    if (!id || restoredFor !== id || conversation?.type === 'local') return
    if (skipSave) {
      skipSave = false
      return
    }
    const copy = $state.snapshot(doc)
    const metadata = untrack(() => ({ provider_id: from?.provider.id ?? null, ...(isMail && mailMetadata ? { subject: mailMetadata.subject, to: mailMetadata.to, cc: mailMetadata.cc } : {}), ...(alias ? { from: alias } : {}) }))
    const timer = setTimeout(() => {
      if (markdown.trim()) api.putDraft({ conversation_id: id, doc: copy, markdown, metadata }).catch(() => {})
      else api.deleteDraft(id).catch(() => {})
    }, 800)
    return () => clearTimeout(timer)
  })

  function forgetDraft(id: string) {
    skipSave = true
    api.deleteDraft(id).catch(() => {})
  }

  $effect(() => {
    const hold = outbound.restoring
    const id = conversation?.id
    if (!hold || !id || !editor || (hold.conversation_id && hold.conversation_id !== id)) return
    untrack(() => {
      outbound.restoring = null
      const draft = hold.draft as Partial<ComposerDraft> | null
      const files = draft?.files ?? (hold.payload.kind === 'forward' ? [] : (hold.payload.input as { attachments?: ComposerDraft['files'] }).attachments ?? [])
      editor?.clear()
      editor?.insertMarkdown(composerText(hold))
      pending = fromOutgoing(draft?.content ?? [], files)
    })
  })

  const lastInbound = $derived.by(() => {
    const mine = $myUserIds
    const messages = $messagesStore.data ?? []
    for (let i = messages.length - 1; i >= 0; i--) {
      if (!mine.has(messages[i].sender_id)) return messages[i]
    }
    return messages[messages.length - 1] ?? null
  })

  const placeholder = $derived.by(() => {
    if (via && !onCurrent) return `${via.label} · ${via.detail}`
    if (!isMail) return t('conversation.composer.reply')
    const who = lastInbound?.sender_name || lastInbound?.sender_email
    return who ? t('conversation.composer.replyTo', { name: who }) : t('conversation.composer.reply')
  })

  const mailMetadata = $derived.by(() => {
    if (!isMail) return undefined
    if (via?.to) return { in_reply_to: undefined, subject: conversation?.name ?? '', to: [via.to], cc: [] as string[] }
    if (!lastInbound) return undefined
    const subject = lastInbound.subject?.trim() || conversation?.name || ''
    const to = lastInbound.sender_email ? [lastInbound.sender_email] : []
    const cc = replyAll ? [...(lastInbound.recipients ?? []), ...(lastInbound.cc_recipients ?? [])].map((r) => r.address).filter((a) => a && !to.includes(a)) : []
    return { in_reply_to: lastInbound.id as string | undefined, subject: /^re:/i.test(subject) ? subject : `Re: ${subject}`, to, cc }
  })

  const builtins = $derived<ComposerCommand[]>(
      [
        { id: 'draft', label: t('conversation.composer.draftAi'), icon: 'lucide:sparkles', keywords: ['ai', 'draft', 'reply'], run: draftReply, hidden: !intelligence },
        { id: 'table', label: t('conversation.composer.table'), icon: 'lucide:table', keywords: ['table', 'grid'], run: () => editor?.block('table') },
        { id: 'code', label: t('conversation.composer.codeBlock'), icon: 'lucide:square-code', keywords: ['code', 'snippet'], run: () => editor?.block('code') },
        { id: 'task', label: t('conversation.composer.taskList'), icon: 'lucide:list-todo', keywords: ['todo', 'task', 'check'], run: () => editor?.block('task') },
        { id: 'heading', label: t('conversation.composer.heading'), icon: 'lucide:heading', keywords: ['title', 'heading', 'h1'], run: () => editor?.block('heading', { level: 2 }) },
        { id: 'divider', label: t('conversation.composer.divider'), icon: 'lucide:minus', keywords: ['rule', 'line', 'hr'], run: () => editor?.block('rule') },
        { id: 'later', label: t('conversation.composer.laterToday'), icon: 'lucide:clock-3', keywords: ['schedule', 'later', 'send'], run: () => schedule(snoozeUntil('later')) },
        { id: 'chat', label: t('conversation.composer.newChat'), icon: 'lucide:messages-square', keywords: ['chat', 'read', 'write', 'new'], run: newChat }
      ]
  )
  const actionCommands = $derived<ComposerCommand[]>(actions.map((action) => ({ id: 'action:' + action.id, label: action.name, icon: action.icon || 'lucide:workflow', keywords: [action.name], run: () => runAction(action) })))

  $effect(() => {
    const id = choiceKey
    choice = id ? getSendChoice(id) : null
    deliveryMode = 'selected'
    selectedKeys = []
  })

  $effect(() => {
    const id = conversation?.id
    if (!id) return
    let live = true
    const load = () => api.getMessageSchedules(id).then((rows) => live && (schedules = rows.filter((row) => row.status === 'pending'))).catch(() => {})
    load()
    const unlisten = listen<string>('schedules-updated', (event) => {
      if (!event.payload || event.payload === id) load()
    })
    return () => {
      live = false
      unlisten.then((stop) => stop())
    }
  })

  onMount(() => {
    api.getAppSettings().then((settings) => (intelligence = settings['assistant.enabled'] !== 'false')).catch(() => {})
    api.getActions().then((rows) => (actions = rows.filter((row) => row.enabled && (row.usage.chat || row.usage.step)))).catch(() => {})
  })

  function toggleMenu(kind: 'sendAs' | 'more' | 'heading' | 'commands' | 'signature', e: MouseEvent) {
    if (openMenu === kind) {
      openMenu = null
      return
    }
    menuAt = boxOf(e.currentTarget)
    if (kind === 'commands') {
      openMenu = null
      editor?.openCommands(menuAt)
      return
    }
    openMenu = kind
  }

  function chooseSendAs(option: SendOption) {
    if (!choiceKey) return
    const current = routes.map((r) => r.key)
    const next = current.includes(option.key) ? current.filter((key) => key !== option.key) : [...current, option.key]
    if (!next.length) return
    selectedKeys = next
    choice = putSendChoice(choiceKey, next[0])
    deliveryMode = 'selected'
  }

  function handleEmojiSelect(emoji: string) {
    emojiAt = null
    editor?.insertText(emoji)
  }

  let pinnedFor: string | null = null
  $effect(() => {
    const id = conversation?.id ?? null
    const typing = !isEmpty(doc) || pending.length > 0
    untrack(() => {
      if (!typing) pinned = null
      else if (!pinned || pinnedFor !== id) pinned = sendAs?.key ?? null
      pinnedFor = id
    })
  })

  const canSend = $derived(!!conversation && (!!via || conversation.type === 'local') && !sending && (!isEmpty(doc) || pending.length > 0))

  function attachContent(item: ContentItem) {
    pending = addContent(pending, item)
    gifAt = null
    emojiAt = null
    contentAt = null
    editor?.focus()
  }

  async function useContent(item: ContentItem) {
    if (!isTemplate(item)) {
      attachContent(item)
      return
    }
    if (!conversation) return
    filling = true
    draftError = null
    try {
      const values = composerVariables({ conversation, person, me: { name: sender ? accountName(sender) : $profileStore?.data?.name, email: sender?.account?.email ?? (from?.label.includes('@') ? from.label : null) }, message: lastInbound, subject: mailMetadata?.subject })
      const rendered = await api.postContentRender({ content_id: item.id, conversation_id: conversation.id, message_id: lastInbound?.id, values, used: true })
      contentAt = null
      editor?.insertMarkdown(rendered.text)
      editor?.focus()
    } catch (e) {
      draftError = String(e)
    } finally {
      filling = false
    }
  }

  async function targetConversation(route: SendOption): Promise<string> {
    if (!conversation) throw new Error(t('conversation.composer.nothingToSend'))
    const target = route.via
    if (target.conversation_id) return target.conversation_id
    if (target.to) return ''
    const created = await api.postConversation({ name: person?.display_name ?? target.detail, conversation_type: target.conversation_type, group_type: 'auto', member_ids: target.member_ids })
    return created.id
  }

  function mailExtras() {
    return { ...(alias ? { from: alias } : {}), ...(signatureOff ? { signature: false } : {}), ...(signatureId ? { signature_id: signatureId } : {}) }
  }

  function metadataFor(route: SendOption) {
    if (route.via.channel === 'mail' && route.via.to) return { in_reply_to: replying?.id, subject: conversation?.name ?? '', to: [route.via.to], cc: [] as string[], ...mailExtras() }
    if (route.via.channel === 'mail' && mailMetadata) return { in_reply_to: replying?.id ?? mailMetadata.in_reply_to, subject: mailMetadata.subject, to: mailMetadata.to, cc: mailMetadata.cc, ...mailExtras() }
    return replying ? { in_reply_to: replying.id } : undefined
  }

  function remember(originId: string, targetId: string | null, route: SendOption | null = sendAs) {
    putAccountUsed(route?.from.provider.id)
    if (choice && sendAs && choice.key === sendAs.key) choice = putSendChoice(routeMerged ?? originId, choice.key)
    sentTick++
  }

  async function send(andDone = false) {
    if (!conversation || !canSend) return
    sending = true
    draftError = null
    const origin = conversation
    try {
      if (origin.type === 'local') {
        await api.postChatMessage(origin.id, toMarkdown(doc), replying ? null : (via?.conversation_id ?? null), replying?.id ?? null)
        clearReply()
        remember(origin.id, via?.conversation_id ?? null)
        editor?.clear()
        pending = []
        forgetDraft(origin.id)
        return
      }
      if (threading && replying) {
        const root = replying
        const existing = root.thread_id ?? $conversationsById.get(root.conversation_id)?.threads?.find((thread) => thread.root_message_id === root.id)?.id ?? null
        const thread = existing ?? (await api.postThread({ message_id: root.id })).id
        const files = await toOutgoing(pending)
        await api.postThreadReply({ thread_id: thread, content: toMarkdown(doc), content_type: 'markdown', broadcast: broadcastable && broadcast, attachments: files.length ? files : undefined })
        editor?.clear()
        clearReply()
        pending = []
        forgetDraft(origin.id)
        return
      }
      const attachments = await toOutgoing(pending)
      const markdown = toMarkdown(doc)
      const draft: ComposerDraft = { markdown, content: pending.flatMap((p) => (p.content ? [$state.snapshot(p.content)] : [])), files: attachments.filter((a) => !!a.data), reply_to: replying?.id ?? null }
      let target = ''
      for (const route of routes) {
        const sentTo = await targetConversation(route)
        await api.postMessage(
          {
            conversation_id: sentTo,
            content: markdown,
            content_type: 'markdown',
            provider_id: route.from.provider.id,
            delivery_mode: hasMultipleProviders && routes.length === 1 ? deliveryMode : 'selected',
            metadata: metadataFor(route),
            attachments: attachments.length ? attachments : undefined
          },
          { surface: 'composer', conversation_id: origin.id, draft }
        )
        remember(origin.id, sentTo || null, route)
        if (!target) target = sentTo
      }
      forgetDraft(origin.id)
      editor?.clear()
      clearReply()
      pending = []
      if (andDone) {
        await api.putConversation({ conversation_id: origin.id, is_archived: true, snoozed_until: null })
        await goto('/main/conversation')
      } else if (target && target !== origin.id && !routePerson && !routeMerged) {
        await goto(conversationHref(target))
      }
    } catch (e) {
      draftError = String(e)
    } finally {
      sending = false
    }
  }

  async function schedule(at: string | null, actionId: string | null = null) {
    if (!conversation || !canSend) return
    sending = true
    try {
      const attachments = await toOutgoing(pending)
      const thread = threading && replying ? { thread_message_id: replying.id } : {}
      for (const route of routes) {
        const target = await targetConversation(route)
        const metadata = { ...(metadataFor(route) ?? {}), ...thread }
        const row = await api.postMessageSchedule({ conversation_id: target, provider_id: route.from.provider.id ?? null, content: toMarkdown(doc), content_type: 'markdown', at, action_id: actionId, metadata: Object.keys(metadata).length ? metadata : null, attachments: attachments.length ? attachments : null })
        if (row.conversation_id === conversation.id) schedules = [...schedules, row]
      }
      editor?.clear()
      clearReply()
      pending = []
      forgetDraft(conversation.id)
    } catch (e) {
      draftError = String(e)
    } finally {
      sending = false
    }
  }

  async function cancelSchedule(id: string) {
    await api.deleteMessageSchedule(id)
    schedules = schedules.filter((row) => row.id !== id)
  }

  function configureAi() {
    const href = aiBadge?.href
    aiNeeds = []
    if (href) goto(href)
  }

  async function draftReply() {
    if (aiBadge) return configureAi()
    if (!intelligence) {
      aiNeeds = [{ setting: 'ai.enabled', label: t('conversation.composer.aiNeeded'), href: settingHref('ai.enabled') }]
      return
    }
    if (!conversation || drafting) return
    drafting = true
    draftError = null
    try {
      const run = await api.postActionRun('draft_reply', { conversation: conversation.id, instruction: toMarkdown(doc) })
      aiNeeds = requirementsFor([run.requirement])
      if (aiNeeds.length) return
      if (run.status === 'failed') throw new Error(t('conversation.composer.draftFailed'))
      const reply = typeof run.outputs?.reply === 'string' ? run.outputs.reply : ''
      if (reply) {
        editor?.clear()
        editor?.insertMarkdown(reply)
      }
    } catch {
      draftError = t('conversation.composer.draftFailed')
    } finally {
      drafting = false
    }
  }

  async function runAction(action: Action) {
    if (!conversation || drafting) return
    drafting = true
    draftError = null
    try {
      const run = await api.postActionRun(action.id, { conversation: conversation.id, instruction: toMarkdown(doc) })
      aiNeeds = requirementsFor([run.requirement])
      if (aiNeeds.length) return
      if (run.status === 'failed') throw new Error(t('conversation.composer.draftFailed'))
      const text = typeof run.outputs?.reply === 'string' ? run.outputs.reply : typeof run.outputs?.text === 'string' ? run.outputs.text : ''
      if (text) {
        editor?.clear()
        editor?.insertMarkdown(text)
      }
    } catch {
      draftError = t('conversation.composer.draftFailed')
    } finally {
      drafting = false
    }
  }

  function newChat() {
    openNewChat({ sources: conversation ? [{ kind: 'conversation', id: conversation.id }] : [] })
  }

  const dictation: VoiceTarget = {
    insert: (text) => editor?.insertText(text),
    replace: (previous, next) => editor?.replaceText(previous, next) ?? false
  }

  function handleEditorKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && replying) {
      e.preventDefault()
      clearReply()
      return
    }
    if (e.key === 'ArrowUp' && !e.shiftKey && !e.metaKey && !e.ctrlKey && !e.altKey && isEmpty(doc) && pending.length === 0) {
      const mine = $myUserIds
      const messages = $messagesStore.data ?? []
      for (let i = messages.length - 1; i >= 0; i--) {
        const message = messages[i]
        if (message.type !== 'chat' || message.deleted_datetime || !mine.has(message.sender_id ?? '')) continue
        e.preventDefault()
        editRequest.id = message.id
        scrollToMessage(message.id)
        return
      }
    }
  }

  function handleFileInputChange(e: Event) {
    const input = e.target as HTMLInputElement
    if (input.files) {
      pending = addFiles(pending, Array.from(input.files))
      input.value = ''
    }
  }

  function handleDragOver(e: DragEvent) {
    if (!e.dataTransfer?.types.includes('Files')) return
    e.preventDefault()
    isDragging = true
  }

  function handleDrop(e: DragEvent) {
    isDragging = false
    if (!e.dataTransfer?.files.length) return
    e.preventDefault()
    pending = addFiles(pending, Array.from(e.dataTransfer.files))
  }

  function when(iso: string | null): string {
    return iso ? formatDate(iso, { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : ''
  }
</script>


{#if conversation}
  {#if held.length}
    <div class="flex shrink-0 flex-col gap-0.5 border-t border-line px-3 py-1" role="status" aria-live="polite" data-testid="composer-holds">
      {#each held as hold (hold.id)}
        {@const waiting = hold.status === 'pending'}
        <div class="flex min-w-0 items-center gap-2 text-[12.5px] text-ink-2" data-testid="composer-hold" data-status={hold.status}>
          <Icon icon={waiting ? 'lucide:send-horizontal' : hold.status === 'failed' ? 'lucide:circle-alert' : 'lucide:file-pen-line'} class="shrink-0 text-[13px] {hold.status === 'failed' ? 'text-error-600' : 'text-ink-3'}" />
          <span class="shrink-0 font-mono text-[11px] text-ink-3">{waiting ? t('conversation.composer.holdIn', { seconds: secondsLeft(hold, outbound.now) }) : hold.status === 'failed' ? t('conversation.composer.holdFailed') : t('conversation.composer.holdDraft')}</span>
          <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap" title={hold.error ?? undefined}>{hold.subject || hold.recipient}</span>
          <Button size="sm" testid="composer-hold-undo" icon={waiting ? 'lucide:undo-2' : 'lucide:pencil'} onclick={() => stopHold(hold.id).catch((e) => (draftError = String(e)))}>{waiting ? t('conversation.composer.undo') : t('conversation.composer.restore')}</Button>
          {#if !waiting}
            <IconButton icon="lucide:x" testid="composer-hold-discard" label={t('conversation.composer.discard')} size="sm" onclick={() => discardHold(hold.id).catch((e) => (draftError = String(e)))} />
          {/if}
        </div>
      {/each}
    </div>
  {/if}
  {#if schedules.length}
    <div class="flex shrink-0 flex-col gap-0.5 border-t border-line px-3 py-1">
      {#each schedules as row (row.id)}
        <div class="flex min-w-0 items-center gap-2 text-[12.5px] text-ink-2" data-testid="composer-schedule">
          <Icon icon={row.action_id ? 'lucide:workflow' : 'lucide:clock'} class="shrink-0 text-[13px] text-ink-3" />
          <span class="shrink-0 font-mono text-[11px] text-ink-3">{row.action_id ? (row.action_name ?? t('conversation.composer.whenAction')) : when(row.at)}</span>
          <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{row.content}</span>
          <IconButton icon="lucide:x" testid="composer-schedule-cancel" label={t('conversation.composer.cancelScheduled')} size="sm" onclick={() => cancelSchedule(row.id)} />
        </div>
      {/each}
    </div>
  {/if}
  <div
    bind:this={composer}
    class="relative flex flex-col border-t border-line bg-paper px-3 pt-1 {fillHeight ? 'min-h-0 flex-1' : 'shrink-0'}"
    ondragover={handleDragOver}
    ondragleave={(e) => {
      e.preventDefault()
      isDragging = false
    }}
    ondrop={handleDrop}
    role="group"
    data-testid="composer"
    aria-label={t('conversation.composer.label')}
  >
    {#if isDragging}
      <div class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center bg-accent-soft">
        <p class="text-[13px] font-medium text-accent">{t('conversation.composer.drop')}</p>
      </div>
    {/if}

    {#if isMail && mailMetadata}
      <div class="flex h-7 shrink-0 items-center gap-1 text-[12px] text-ink-3">
        <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-mono" data-testid="composer-mail-to">{t('conversation.composer.to', { list: mailMetadata.to.join(', ') || '—' })}{replyAll && mailMetadata.cc.length ? t('conversation.composer.cc', { list: mailMetadata.cc.join(', ') }) : ''}</span>
        {#if onCurrent}
          <button type="button" data-testid="composer-reply-all" data-active={replyAll} class="ml-auto shrink-0 inline-flex items-center gap-1 font-mono hover:text-ink {replyAll ? 'text-accent' : ''}" onclick={() => (replyAll = !replyAll)}><Icon icon={replyAll ? 'lucide:reply-all' : 'lucide:reply'} class="text-[12px]" />{replyAll ? t('conversation.composer.replyAllToggle') : t('conversation.composer.replyToggle')}</button>
        {/if}
      </div>
    {/if}

    {#if targets.length}
      {#if summarizingAt}
        <SummarizePopover at={summarizingAt} title={t('conversation.summarize.selection', { count: targetMessages.length })} onsubmit={(options) => summarizeSelection(targetMessages, options)} onclose={() => (summarizingAt = null)} />
      {/if}
      {#if selectionTagAt}
        <TagPicker objectType="message" ids={targets} at={selectionTagAt} onclose={() => (selectionTagAt = null)} />
      {/if}
    {/if}
    {#if replyShown}
      <div class="flex h-7 min-w-0 shrink-0 items-center gap-2 border-l-2 border-accent pl-2 text-[12.5px]" data-testid="composer-reply-bar" role="status" aria-label={t('conversation.reply.label', { name: replyShown.author })}>
        <Icon icon={threading ? 'lucide:messages-square' : 'lucide:reply'} class="shrink-0 text-[12px] text-accent" />
        <span class="shrink-0 font-medium text-ink-2">{t(threading ? 'conversation.reply.threadLabel' : 'conversation.reply.label', { name: replyShown.author })}</span>
        <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-ink-3">{replyShown.excerpt}</span>
        {#if replyShown.attachments}
          <span class="flex shrink-0 items-center gap-1 font-mono text-[11px] text-ink-3" title={replyShown.attachmentNames.join(', ')}><Icon icon="lucide:paperclip" class="text-[11px]" />{t('conversation.reply.attachments', { count: replyShown.attachments })}</span>
        {/if}
        <IconButton icon="lucide:x" testid="composer-reply-cancel" label={t('conversation.reply.cancel')} keys="esc" size="sm" onclick={clearReply} />
      </div>
    {/if}

    {#if formatting}
      <div class="flex h-8 shrink-0 items-center gap-0.5 overflow-x-auto" data-testid="composer-toolbar" role="toolbar" aria-label={t('conversation.composer.formatting')}>
        <IconButton icon="lucide:bold" testid="composer-format-bold" label={t('conversation.composer.bold')} keys="⌘B" size="sm" onclick={() => editor?.format('bold')} />
        <IconButton icon="lucide:italic" testid="composer-format-italic" label={t('conversation.composer.italic')} keys="⌘I" size="sm" onclick={() => editor?.format('italic')} />
        <IconButton icon="lucide:underline" testid="composer-format-underline" label={t('conversation.composer.underline')} keys="⌘U" size="sm" onclick={() => editor?.format('underline')} />
        <IconButton icon="lucide:strikethrough" testid="composer-format-strike" label={t('conversation.composer.strike')} keys="⌘⇧X" size="sm" onclick={() => editor?.format('strike')} />
        <span class="mx-1 h-4 w-px shrink-0 bg-line" aria-hidden="true"></span>
        <IconButton icon="lucide:link" testid="composer-format-link" label={t('conversation.composer.link')} keys="⌘K" size="sm" onclick={() => editor?.format('link')} />
        <IconButton icon="lucide:list-ordered" testid="composer-block-ordered" label={t('conversation.composer.orderedList')} keys="⌘⇧7" size="sm" onclick={() => editor?.block('ordered')} />
        <IconButton icon="lucide:list" testid="composer-block-bullet" label={t('conversation.composer.bulletList')} keys="⌘⇧8" size="sm" onclick={() => editor?.block('bullet')} />
        <IconButton icon="lucide:list-todo" testid="composer-block-task" label={t('conversation.composer.taskList')} size="sm" onclick={() => editor?.block('task')} />
        <span class="mx-1 h-4 w-px shrink-0 bg-line" aria-hidden="true"></span>
        <IconButton icon="lucide:text-quote" testid="composer-block-quote" label={t('conversation.composer.quote')} keys="⌘⇧9" size="sm" onclick={() => editor?.block('quote')} />
        <IconButton icon="lucide:code" testid="composer-format-code" label={t('conversation.composer.code')} keys="⌘E" size="sm" onclick={() => editor?.format('code')} />
        <IconButton icon="lucide:square-code" testid="composer-block-code" label={t('conversation.composer.codeBlock')} keys="⌘⌥⇧C" size="sm" onclick={() => editor?.block('code')} />
        <span class="mx-1 h-4 w-px shrink-0 bg-line" aria-hidden="true"></span>
        <IconButton icon="lucide:heading" testid="composer-format-heading" label={t('conversation.composer.heading')} size="sm" active={openMenu === 'heading'} onclick={(e) => toggleMenu('heading', e)} />
        {#if openMenu === 'heading'}
          <div class="menu w-48" use:anchored={{ at: menuAt, side: 'below', align: 'start', onclose: () => (openMenu = null) }} role="menu">
            {#each [1, 2, 3] as level (level)}
              <MenuItem icon={`lucide:heading-${level}`} testid="composer-heading-level" value={String(level)} keys={`${'#'.repeat(level)} `} onclick={() => { openMenu = null; editor?.block('heading', { level }) }}>{t('conversation.composer.headingLevel', { level })}</MenuItem>
            {/each}
            <MenuItem icon="lucide:pilcrow" testid="composer-heading-normal" onclick={() => { openMenu = null; editor?.block('paragraph') }}>{t('conversation.composer.normalText')}</MenuItem>
          </div>
        {/if}
        <IconButton icon="lucide:table" testid="composer-block-table" label={t('conversation.composer.table')} size="sm" onclick={() => editor?.block('table')} />
        <IconButton icon="lucide:minus" testid="composer-block-rule" label={t('conversation.composer.divider')} size="sm" onclick={() => editor?.block('rule')} />
      </div>
    {/if}

    <AttachmentUpload items={pending} onremove={(id) => (pending = removeAttachment(pending, id))} />

    <Composer bind:this={editor} bind:doc conversationId={conversation.id} providerId={from?.provider.id ?? null} mail={isMail} {placeholder} {maxLines} fill={fillHeight} submitOnEnter={sendsOnEnter} {builtins} actions={actionCommands} onsubmit={send} onfiles={(files) => (pending = addFiles(pending, files))} onkeydown={handleEditorKeydown} />

    {#if broadcastable}
      <label class="flex h-7 shrink-0 items-center gap-2 text-[12.5px] text-ink-2" data-testid="composer-broadcast">
        <input type="checkbox" bind:checked={broadcast} class="accent-[var(--accent)]" />
        <span>{t('conversation.thread.broadcast')}</span>
      </label>
    {/if}

    {#if isMail && (signature || signatureOff)}
      <div class="flex h-7 shrink-0 items-center gap-1.5 text-[12px] text-ink-3" data-testid="composer-signature">
        <Icon icon="lucide:signature" class="shrink-0 text-[13px]" />
        {#if signatureOff}
          <button type="button" class="hover:text-ink" data-testid="composer-signature-add" onclick={() => (signatureOff = false)}>{t('conversation.composer.signatureAdd')}</button>
        {:else if signature}
          <button type="button" class="min-w-0 truncate rounded-[0.5rem] bg-paper-2 px-2 py-0.5 text-ink-2 hover:text-ink" title={signature.text} data-testid="composer-signature-pick" aria-haspopup="menu" aria-expanded={openMenu === 'signature'} onclick={pickSignature}>{signature.name}</button>
          <IconButton icon="lucide:x" size="sm" testid="composer-signature-remove" label={t('conversation.composer.signatureRemove')} onclick={() => (signatureOff = true)} />
        {/if}
      </div>
      {#if openMenu === 'signature'}
        <div class="menu w-64" use:anchored={{ at: menuAt, side: 'above', align: 'start', onclose: () => (openMenu = null) }} role="menu" data-testid="composer-signature-menu" aria-label={t('conversation.composer.signatureChoose')}>
          {#each signatureChoices as item (item.id)}
            <MenuItem checked={item.id === (signatureId ?? signature?.content_id)} testid="composer-signature-option" value={item.id} onclick={() => { signatureId = item.id; openMenu = null }}>{item.name}</MenuItem>
          {/each}
          {#if !signatureChoices.length}
            <p class="px-2.5 py-1.5 text-[12.5px] text-ink-3">{t('conversation.composer.signatureNone')}</p>
          {/if}
        </div>
      {/if}
    {/if}

    {#if aiBadge}
      <span class="flex shrink-0 pt-1"><Button size="sm" variant="warn" testid="composer-needs" icon="lucide:triangle-alert" title={aiBadge.title} onclick={configureAi}>{aiBadge.title}</Button></span>
    {/if}
    {#if draftError}
      <p class="shrink-0 pt-1 text-[12px] text-error-600" data-testid="composer-error">{draftError}</p>
    {/if}

    <div class="-mx-3 mt-1 flex h-10 shrink-0 items-center gap-0.5 border-t border-line bg-paper-2 px-2" role="toolbar" aria-label={t('conversation.composer.label')}>
      <IconButton icon="lucide:plus" testid="composer-more" label={t('conversation.composer.more')} size="sm" active={openMenu === 'more'} busy={drafting || filling} onclick={(e) => toggleMenu('more', e)} />
      {#if openMenu === 'more'}
        <div class="menu w-64" use:anchored={{ at: menuAt, side: 'above', align: 'start', onclose: () => (openMenu = null) }} role="menu">
          <MenuItem icon="lucide:paperclip" testid="composer-more-files" keys="⌘⇧A" onclick={() => { openMenu = null; fileInputElement?.click() }}>{t('conversation.composer.files')}</MenuItem>
          <MenuItem icon="lucide:library" testid="composer-more-content" onclick={() => { openMenu = null; contentAt = menuAt }}>{t('content.picker.open')}</MenuItem>
          <MenuItem icon="lucide:image-play" testid="composer-more-gif" onclick={() => { openMenu = null; gifAt = menuAt }}>{t('conversation.composer.gif')}</MenuItem>
          <div class="menu-sep"></div>
          <MenuItem icon={aiBadge ? 'lucide:triangle-alert' : 'lucide:sparkles'} warn={!!aiBadge} testid="composer-more-draft" value={intelligence ? 'on' : 'off'} onclick={() => { openMenu = null; draftReply() }}>{aiBadge ? aiBadge.title : intelligence ? t('conversation.composer.draftAi') : t('conversation.composer.aiOff')}</MenuItem>
          <MenuItem icon="lucide:messages-square" testid="composer-more-chat" onclick={() => { openMenu = null; newChat() }}>{t('conversation.composer.newChat')}</MenuItem>
        </div>
      {/if}
      <input bind:this={fileInputElement} type="file" multiple class="hidden" onchange={handleFileInputChange} />
      <input bind:this={clipInputElement} type="file" accept="video/*" class="hidden" onchange={handleFileInputChange} />
      {#if gifAt}
        <GifPicker at={gifAt} side="above" align="start" onpick={attachContent} onclose={() => (gifAt = null)} />
      {/if}
      {#if contentAt}
        <ContentPicker at={contentAt} mail={isMail} busy={filling} onpick={useContent} onclose={() => (contentAt = null)} />
      {/if}
      <IconButton icon="lucide:a-large-small" testid="composer-formatting" label={t('conversation.composer.formatting')} size="sm" active={formatting} onclick={() => (formatting = !formatting)} />
      <IconButton icon="lucide:smile" testid="composer-emoji" label={t('conversation.composer.emoji')} size="sm" active={!!emojiAt} onclick={(e) => (emojiAt = emojiAt ? null : boxOf(e.currentTarget))} />
      {#if emojiAt}
        <EmojiPicker at={emojiAt} side="above" align="start" onselect={handleEmojiSelect} oncontent={attachContent} onclose={() => (emojiAt = null)} />
      {/if}
      <IconButton icon="lucide:at-sign" testid="composer-mention" label={t('conversation.composer.mention')} keys="@" size="sm" onclick={() => editor?.insertText('@')} />
      <span class="mx-1 h-4 w-px shrink-0 bg-line" aria-hidden="true"></span>
      <IconButton icon="lucide:video" label={t('conversation.composer.clip')} size="sm" onclick={() => clipInputElement?.click()} />
      <MicButton target={dictation} conversationId={conversation.id} size="sm" />
      <span class="mx-1 h-4 w-px shrink-0 bg-line" aria-hidden="true"></span>
      <IconButton icon="lucide:square-slash" testid="composer-commands-button" label={t('conversation.composer.commands')} keys="/" size="sm" active={openMenu === 'commands'} onclick={(e) => toggleMenu('commands', e)} />

      {#if targets.length}
        <div class="flex min-w-0 items-center gap-0.5 rounded-[0.5rem] bg-paper-2 pl-2 text-[12px] text-ink-2" role="status" data-testid="composer-selection">
          <span class="whitespace-nowrap pr-1">{t('conversation.forward.selected', { count: targets.length })}</span>
          <IconButton size="sm" testid="composer-selection-tag" icon="lucide:tag" label={t('conversation.menu.tagSelected', { count: targets.length })} onclick={(e) => (selectionTagAt = selectionTagAt ? null : boxOf(e.currentTarget))} />
          {#if summarisable(targetMessages)}
            <IconButton size="sm" testid="composer-selection-summarize" icon="lucide:sparkles" label={t('conversation.section.summarize')} onclick={(e) => (summarizingAt = summarizingAt ? null : boxOf(e.currentTarget))} />
          {/if}
          <IconButton size="sm" testid="composer-selection-copy" icon={copied ? 'lucide:check' : 'lucide:copy'} label={copied ? t('conversation.export.copied') : t('conversation.export.copy')} keys="⌘C" onclick={copyChecked} />
          <IconButton size="sm" testid="composer-selection-forward" icon="lucide:forward" label={t('conversation.forward.send')} keys="F" onclick={shortcutForward} />
          <IconButton size="sm" icon="lucide:x" label={t('conversation.forward.clear')} keys="Esc" onclick={clearSelection} />
        </div>
      {/if}
      <div class="ml-auto flex min-w-0 items-center gap-1">
      {#if replying && replyStyles.length > 1}
        <IconButton size="sm" testid="composer-reply-style" icon={threading ? 'lucide:messages-square' : 'lucide:reply'} active={threading} label={threading ? t('conversation.reply.asThread') : t('conversation.reply.asReply')} onclick={() => (replyStyle = threading ? 'quote' : 'thread')} />
      {/if}
        <button type="button" data-testid="composer-send-as" data-value={sendAs?.key} data-mode={deliveryMode} class="flex h-7 min-w-0 items-center gap-1.5 rounded-[0.5rem] px-2 text-[12px] text-ink-2 hover:bg-paper-3 hover:text-ink" onclick={(e) => toggleMenu('sendAs', e)} aria-haspopup="menu" aria-expanded={openMenu === 'sendAs'} aria-label={t('conversation.composer.sendAs')}>
          {#if from && via}
            <ProviderIcon providers={[{ ...from.provider, conversation_type: via.conversation_type }]} size="xs" max={1} />
            <span class="min-w-0 truncate">{deliveryMode === 'all' ? t('conversation.composer.everyAccount') : from.label}</span>
            {#if !from.label.includes(via.detail || via.label)}
              <span class="shrink-0 text-ink-3" aria-hidden="true">·</span>
              <span class="min-w-0 truncate text-ink-3">{via.detail || via.label}</span>
            {/if}
            {#if routes.length > 1}<span class="shrink-0 rounded-full bg-accent-soft px-1.5 font-mono text-[11px] text-accent" data-testid="composer-send-as-more">+{routes.length - 1}</span>{/if}
          {:else}
            <span class="truncate">{t('conversation.composer.noAccount')}</span>
          {/if}
          <Icon icon="lucide:chevron-down" class="shrink-0 text-[11px]" />
        </button>
        {#if openMenu === 'sendAs'}
          <div class="menu w-80" use:anchored={{ at: menuAt, side: 'above', align: 'end', onclose: () => (openMenu = null) }} role="menu" data-testid="composer-send-as-menu" aria-label={t('conversation.composer.sendAs')}>
            {#each options as option (option.key)}
              <MenuItem checked={routes.some((r) => r.key === option.key) && deliveryMode !== 'all'} testid="composer-send-as-option" value={option.key} detail={option.via.label} onclick={() => chooseSendAs(option)}>
                <span class="inline-flex min-w-0 items-center gap-2"><ProviderIcon providers={[{ ...option.from.provider, conversation_type: option.via.conversation_type }]} size="sm" max={1} /><span class="truncate">{option.from.label}</span><span class="truncate text-ink-3">{option.via.detail}</span></span>
              </MenuItem>
            {/each}
            {#if aliases.length > 1}
              <p class="menu-label">{t('conversation.composer.sendFromAddress')}</p>
              {#each aliases as entry (entry.email)}
                <MenuItem checked={(alias ?? aliases.find((a) => a.is_default)?.email) === entry.email} testid="composer-alias-option" value={entry.email} detail={entry.name ?? undefined} onclick={() => { alias = entry.is_default ? null : entry.email; openMenu = null }}>{entry.email}</MenuItem>
              {/each}
            {/if}
            {#if hasMultipleProviders}
              <div class="menu-sep"></div>
              <MenuItem icon="lucide:list-checks" testid="composer-send-every" checked={deliveryMode === 'all'} onclick={() => { deliveryMode = deliveryMode === 'all' ? 'selected' : 'all'; openMenu = null }}>{t('conversation.composer.sendFromEvery')}</MenuItem>
            {/if}
            {#if !options.length}
              <p class="px-2.5 py-1.5 text-[12.5px] text-ink-3">{t('conversation.composer.noSender')}</p>
            {/if}
          </div>
        {/if}
        <SplitButton icon="lucide:send-horizontal" testid="composer-send" variant={canSend ? 'primary' : 'ghost'} label={t('common.send')} keys={sendKey} menuLabel={t('conversation.composer.sendOptions')} disabled={!canSend} busy={sending} onclick={() => send(false)}>
          {#snippet menu(close)}
            <MenuItem icon="lucide:send" testid="composer-send-now" keys={sendKey} onclick={() => { close(); send(false) }}>{t('conversation.composer.sendNow')}</MenuItem>
            <MenuItem icon="lucide:archive" testid="composer-send-done" keys="⌘⇧↵" onclick={() => { close(); send(true) }}>{t('conversation.composer.sendDone')}</MenuItem>
            <p class="menu-label">{t('conversation.composer.sendAt')}</p>
            <MenuItem icon="lucide:clock-3" testid="composer-send-later" detail={when(snoozeUntil('later'))} onclick={() => { close(); schedule(snoozeUntil('later')) }}>{t('conversation.composer.laterToday')}</MenuItem>
            <MenuItem icon="lucide:sunrise" testid="composer-send-tomorrow" detail={when(snoozeUntil('tomorrow'))} onclick={() => { close(); schedule(snoozeUntil('tomorrow')) }}>{t('conversation.composer.tomorrowMorning')}</MenuItem>
            <MenuItem icon="lucide:calendar-days" testid="composer-send-monday" detail={when(snoozeUntil('week'))} onclick={() => { close(); schedule(snoozeUntil('week')) }}>{t('conversation.composer.mondayMorning')}</MenuItem>
            <div class="flex items-center gap-2 px-2.5 py-1">
              <input type="datetime-local" data-testid="composer-send-at" bind:value={scheduleAt} class="min-w-0 flex-1 bg-paper-2 px-2 py-1 font-mono text-[12px] text-ink outline-none" aria-label={t('conversation.composer.sendAtLabel')} />
              <Button size="sm" testid="composer-send-at-set" disabled={!scheduleAt} onclick={() => { close(); schedule(new Date(scheduleAt).toISOString()) }}>{t('conversation.composer.set')}</Button>
            </div>
            {#if actions.length}
              <p class="menu-label">{t('conversation.composer.sendWhen')}</p>
              {#each actions as action (action.id)}
                <MenuItem icon={action.icon || 'lucide:workflow'} testid="composer-send-when" value={action.id} onclick={() => { close(); schedule(null, action.id) }}>{action.name}</MenuItem>
              {/each}
            {/if}
            <div class="menu-sep"></div>
            <MenuItem icon="lucide:plus" testid="composer-send-new-action" onclick={() => { close(); goto('/main/actions?conversation=' + conversation.id) }}>{t('conversation.composer.newAction')}</MenuItem>
          {/snippet}
        </SplitButton>
      </div>
    </div>
  </div>
{/if}
