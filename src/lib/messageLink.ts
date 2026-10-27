import { scrollToMessage } from '$lib/reply.svelte'
import { conversationHref } from '$lib/tauri/conversations'

export const MESSAGE_ANCHOR = 'message-'

export function messageHref(conversationId: string, messageId: string): string {
  return `${conversationHref(conversationId)}#${MESSAGE_ANCHOR}${encodeURIComponent(messageId)}`
}

export function messageAnchor(hash: string | null | undefined): string | null {
  const raw = (hash ?? '').replace(/^#/, '')
  if (!raw.startsWith(MESSAGE_ANCHOR)) return null
  try {
    return decodeURIComponent(raw.slice(MESSAGE_ANCHOR.length)) || null
  } catch {
    return null
  }
}

export interface RevealOptions {
  attempts?: number
  delay?: number
  interval?: number
  reveal?: (id: string) => boolean
}

export function revealMessage(id: string, options: RevealOptions = {}): { done: Promise<boolean>; cancel: () => void } {
  const { attempts = 16, delay = 250, interval = 150, reveal = scrollToMessage } = options
  let timer: ReturnType<typeof setTimeout> | null = null
  let settle: (found: boolean) => void = () => {}
  const done = new Promise<boolean>((resolve) => (settle = resolve))
  const attempt = (left: number) => {
    if (reveal(id)) return settle(true)
    if (left <= 1) return settle(false)
    timer = setTimeout(() => attempt(left - 1), interval)
  }
  timer = setTimeout(() => attempt(attempts), delay)
  return {
    done,
    cancel: () => {
      if (timer) clearTimeout(timer)
    }
  }
}

export function linkTargetHref(target: { conversation_id: string; message_id: string | null }): string {
  return target.message_id ? messageHref(target.conversation_id, target.message_id) : conversationHref(target.conversation_id)
}

export async function openLink(href: string): Promise<void> {
  if (/^https?:/i.test(href)) {
    const { getLinkTarget } = await import('$lib/tauri/api')
    const target = await getLinkTarget(href).catch(() => null)
    if (target) {
      const { goto } = await import('$app/navigation')
      await goto(linkTargetHref(target))
      return
    }
  }
  if (!/^(https?:|mailto:)/i.test(href)) return
  const { openUrl } = await import('@tauri-apps/plugin-opener')
  await openUrl(href)
}
