import { get } from 'svelte/store'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import * as api from '$lib/tauri/api'
import { messagesStore } from '$lib/tauri'
import { plainText } from '$lib/messageKind'
import { isHtml } from '$lib/links'
import { selection } from '$lib/reply.svelte'
import type { Message } from '$lib/tauri/types'

export type ExportFormat = 'markdown' | 'pdf'

export const chatExport = $state<{ ids: string[]; title: string }>({ ids: [], title: '' })

function bodyOf(message: Message): string {
  return isHtml(message.content_type) ? plainText(message.content) : (message.content ?? '').trim()
}

function senderOf(message: Message): string {
  return message.sender_name ?? message.sender_email ?? ''
}

function stampOf(message: Message): string {
  return new Date(message.timestamp).toLocaleString()
}

function filesOf(message: Message): string[] {
  return (message.attachments ?? []).filter((a) => !a.is_inline).map((a) => a.name)
}

function escaped(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
}

export function markdownOf(messages: Message[], title?: string): string {
  const blocks = messages.map((message) => {
    const lines = [`**${senderOf(message)}** · ${stampOf(message)}`]
    if (message.subject) lines.push('', `### ${message.subject}`)
    const body = bodyOf(message)
    if (body) lines.push('', body)
    const files = filesOf(message)
    if (files.length) lines.push('', ...files.map((name) => `- ${name}`))
    return lines.join('\n')
  })
  return [...(title ? [`# ${title}`, ''] : []), blocks.join('\n\n---\n\n'), ''].join('\n')
}

function htmlOf(messages: Message[], title: string): string {
  const articles = messages.map((message) => {
    const files = filesOf(message)
    return [
      `<article><header><strong>${escaped(senderOf(message))}</strong> · ${escaped(stampOf(message))}</header>`,
      message.subject ? `<h3>${escaped(message.subject)}</h3>` : '',
      `<p>${escaped(bodyOf(message))}</p>`,
      files.length ? `<ul>${files.map((name) => `<li>${escaped(name)}</li>`).join('')}</ul>` : '',
      '</article>',
    ].join('')
  })
  return `<h1>${escaped(title)}</h1>${articles.join('')}`
}

function ordered(messages: Message[]): Message[] {
  return [...messages].sort((a, b) => a.timestamp.localeCompare(b.timestamp))
}

async function exportedMessages(): Promise<Message[]> {
  const stored = chatExport.ids.length ? await api.getConversationMessages(chatExport.ids) : []
  if (!selection.ids.length) return ordered(stored)
  const ids = new Set(selection.ids)
  const pool = new Map([...(get(messagesStore).data ?? []), ...stored].map((m) => [m.id, m]))
  return ordered([...pool.values()].filter((m) => ids.has(m.id)))
}

export async function copySelected(): Promise<void> {
  const text = exportedMessages().then((messages) => new Blob([markdownOf(messages)], { type: 'text/plain' }))
  await navigator.clipboard.write([new ClipboardItem({ 'text/plain': text })])
}

export async function exportChat(format: ExportFormat): Promise<void> {
  const messages = await exportedMessages()
  const title = chatExport.title || 'Chatnip'
  if (format === 'markdown') {
    const path = await api.postExport(`${title}.md`, markdownOf(messages, title))
    await revealItemInDir(path)
    return
  }
  document.getElementById('print-root')?.remove()
  const root = document.createElement('div')
  root.id = 'print-root'
  root.innerHTML = htmlOf(messages, title)
  document.body.append(root)
  window.print()
}
