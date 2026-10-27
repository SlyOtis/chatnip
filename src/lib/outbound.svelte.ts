import { goto } from '$app/navigation'
import { deleteOutboundHold, getOutboundHolds, getOutboundOpened } from '$lib/tauri/api'
import type { ContentItem, Message, OutboundHold, OutgoingAttachmentInput } from '$lib/tauri/types'
import type { EmailComposeModalData } from '$lib/types'
import { modalsShow } from '$lib/modals'
import { openForward } from '$lib/reply.svelte'
import {  } from '$lib/tauri/conversations'
import { conversationHref } from '$lib/tauri/conversations'

export const HOLD_EVENT = 'outbound-hold-updated'
export const OPEN_EVENT = 'outbound-hold-open'
const SHOWN = ['pending', 'draft', 'failed']

export interface ComposerDraft {
  markdown: string
  content: ContentItem[]
  files: OutgoingAttachmentInput[]
  reply_to: string | null
}

export interface ForwardDraft {
  messages: Message[]
  comment: string
  targets: string[]
}

export const outbound = $state<{ holds: OutboundHold[]; restoring: OutboundHold | null; now: number }>({ holds: [], restoring: null, now: Date.now() })

export function putHold(list: OutboundHold[], hold: OutboundHold): OutboundHold[] {
  const rest = list.filter((h) => h.id !== hold.id)
  return SHOWN.includes(hold.status) ? [...rest, hold].sort((a, b) => a.due_at.localeCompare(b.due_at)) : rest
}

export function secondsLeft(hold: OutboundHold, now: number): number {
  return Math.max(0, Math.ceil((new Date(hold.due_at).getTime() - now) / 1000))
}

export function holdsFor(list: OutboundHold[], conversationId: string | null | undefined): OutboundHold[] {
  return conversationId ? list.filter((h) => h.conversation_id === conversationId || !h.conversation_id) : []
}

export function composerText(hold: OutboundHold): string {
  const draft = hold.draft as Partial<ComposerDraft> | null
  if (typeof draft?.markdown === 'string') return draft.markdown
  if (hold.payload.kind === 'forward') return hold.payload.input.comment ?? ''
  return hold.payload.input.content
}

export async function restoreHold(hold: OutboundHold): Promise<void> {
  outbound.holds = outbound.holds.filter((h) => h.id !== hold.id)
  if (hold.conversation_id) await goto(conversationHref(hold.conversation_id))
  if (hold.surface === 'compose' && hold.draft) {
    modalsShow(hold.draft as EmailComposeModalData)
    return
  }
  if (hold.surface === 'forward' && hold.draft) {
    const draft = hold.draft as ForwardDraft
    openForward(draft.messages, draft.comment, draft.targets)
    return
  }
  outbound.restoring = hold
}

export async function stopHold(id: string): Promise<void> {
  await restoreHold(await deleteOutboundHold(id))
}

export async function discardHold(id: string): Promise<void> {
  await deleteOutboundHold(id)
  outbound.holds = outbound.holds.filter((h) => h.id !== id)
}

export async function loadHolds(): Promise<void> {
  outbound.holds = await getOutboundHolds()
}

async function opened(): Promise<void> {
  const hold = await getOutboundOpened()
  if (hold) await restoreHold(hold)
}

export async function listenHolds(): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event')
  const offs = await Promise.all([
    listen<OutboundHold>(HOLD_EVENT, (event) => (outbound.holds = putHold(outbound.holds, event.payload))),
    listen<string>(OPEN_EVENT, () => void opened().catch(() => {}))
  ])
  const timer = setInterval(() => {
    if (outbound.holds.some((h) => h.status === 'pending')) outbound.now = Date.now()
  }, 1000)
  await Promise.all([loadHolds(), opened()]).catch(() => {})
  return () => {
    clearInterval(timer)
    for (const off of offs) off()
  }
}
