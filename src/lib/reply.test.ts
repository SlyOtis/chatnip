import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import type { Attachment, Conversation, Message, Provider } from '$lib/tauri/types'
import { canForward, clearReply, closeForward, deliveryFor, excerptOf, forwardSet, forwarding, loadedOriginal, openForward, orderForForward, quotedReply, replyBody, replyMode, replyPreview, replyState, selection, startReply, toggleSelected } from './reply.svelte'

const file = (name: string, is_inline = false) => ({ id: name, name, content_type: 'application/pdf', size: 1, is_inline, content_id: null }) as unknown as Attachment

const message = (id: string, over: Partial<Message> = {}) =>
  ({ id, type: 'chat', conversation_id: 'c1', provider_id: 'teams', sender_id: 'u1', sender_name: 'Nora Gale', sender_email: 'nora@northwind.example', content: `<p>Message ${id}</p>`, content_type: 'html', timestamp: `2026-10-08T09:0${id.slice(-1)}:00Z`, attachments: [], thread_id: null, reply_to_id: null, ...over }) as unknown as Message

const provider = (id: string, forward: Provider['capabilities']['forward'], can_send_messages = true) => ({ id, capabilities: { can_send_messages, forward, reply_quote: 'native', supports_reply_quote: true, supports_forward: true } }) as unknown as Provider

const conversation = (id: string, type: Conversation['type'], providers: Provider[]) => ({ id, name: id, type, providers }) as unknown as Conversation

describe('reply preview', () => {
  beforeEach(() => clearReply())

  it('names the author, flattens the excerpt and counts real attachments', () => {
    const preview = replyPreview(message('m1', { content: '<p>Staging   is <b>green</b></p><p>ship?</p>', attachments: [file('notes.pdf'), file('logo.png', true)] }))
    expect(preview).toEqual({ id: 'm1', author: 'Nora Gale', excerpt: 'Staging is green ship?', attachments: 1, attachmentNames: ['notes.pdf'], mail: false })
    expect(replyPreview(message('m2', { sender_name: null, sender_email: null }), 'Unknown').author).toBe('Unknown')
    expect(excerptOf('word '.repeat(60), 12)).toBe('word word wo…')
  })

  it('keeps one reply at a time and clears it', () => {
    startReply(message('m1'))
    startReply(message('m2'))
    expect(replyState.replying?.id).toBe('m2')
    clearReply()
    expect(replyState.replying).toBeNull()
  })

  it('replies to mail through the compose sheet, to chats inline and never in read chats', () => {
    const teams = provider('teams', 'repost')
    expect(replyMode(message('m1'), teams, false)).toBe('chat')
    expect(replyMode(message('m1', { type: 'email' }), teams, false)).toBe('mail')
    expect(replyMode(message('m1'), teams, true)).toBeNull()
    expect(replyMode(message('m1', { type: 'system' }), teams, false)).toBeNull()
    expect(replyMode(message('m1'), provider('ro', 'repost', false), false)).toBeNull()
    expect(canForward(message('m1', { type: 'system' }))).toBe(false)
  })

  it('shows a quote header for quoted replies but not for thread replies', () => {
    expect(quotedReply(message('m1', { reply_to_id: 'x' }))).toBe(true)
    expect(quotedReply(message('m1', { reply_to_id: 'x', thread_id: 'x' }))).toBe(false)
    expect(quotedReply(message('m1'))).toBe(false)
  })

  it('strips the embedded quote of the replied message and the gap after it', () => {
    const original = message('m1', { sender_name: 'Milo Oakes', content: 'Vi må bestemme oss for leverandør innen fredag.', content_type: 'text' })
    const markdown = message('m2', { reply_to_id: 'm1', content: '> **Milo Oakes**: Vi må bestemme oss for leverandør innen fredag.\n\nFlott, da sier vi det!', content_type: 'text' })
    expect(loadedOriginal(markdown, [original, markdown])?.id).toBe('m1')
    const body = replyBody(markdown, original)
    expect(body.message.content).toBe('Flott, da sier vi det!')
    expect(body.quote).toEqual({ author: 'Milo Oakes', text: 'Vi må bestemme oss for leverandør innen fredag.', content: 'Flott, da sier vi det!' })
    const html = message('m3', { reply_to_id: 'm1', content: '<div><blockquote class="reply-quote"><strong>Milo Oakes</strong><p>Vi må bestemme oss</p></blockquote><p><br></p><p>Flott</p></div>' })
    expect(replyBody(html, null).message.content).toBe('<div><p>Flott</p></div>')
    expect(replyBody(html, null).quote?.text).toBe('Vi må bestemme oss')
    expect(replyBody(markdown, message('m9', { sender_name: 'Anna Berg', content: 'Noe annet' })).message.content).toBe(markdown.content)
    expect(replyBody(message('m4', { content: markdown.content, content_type: 'text' }), null).message.content).toBe(markdown.content)
  })
})

describe('forward dialog selection', () => {
  beforeEach(() => closeForward())

  it('forwards the selection in time order and adds the focused message once', () => {
    const loaded = [message('m3'), message('m1'), message('m2')]
    expect(forwardSet(loaded[0], loaded, []).map((m) => m.id)).toEqual(['m3'])
    toggleSelected('m3')
    toggleSelected('m1')
    expect(selection.ids).toEqual(['m3', 'm1'])
    expect(forwardSet(loaded[2], loaded, selection.ids).map((m) => m.id)).toEqual(['m1', 'm2', 'm3'])
    expect(forwardSet(loaded[1], loaded, selection.ids).map((m) => m.id)).toEqual(['m1', 'm3'])
    toggleSelected('m3')
    expect(selection.ids).toEqual(['m1'])
    expect(orderForForward([loaded[0], loaded[0], loaded[1]]).map((m) => m.id)).toEqual(['m1', 'm3'])
  })

  it('opens with the ordered messages and closing clears the selection', () => {
    toggleSelected('m2')
    openForward([message('m2'), message('m1')])
    expect(forwarding.messages.map((m) => m.id)).toEqual(['m1', 'm2'])
    closeForward()
    expect(forwarding.messages).toEqual([])
    expect(selection.ids).toEqual([])
  })

  it('tells per target whether it shares natively, re-posts or mails', () => {
    const slack = provider('slack', 'native')
    const teams = provider('teams', 'repost')
    const fromSlack = [message('m1', { provider_id: 'slack' })]
    expect(deliveryFor(fromSlack, conversation('s', 'channel', [slack]))).toBe('native')
    expect(deliveryFor([...fromSlack, message('m2')], conversation('s', 'channel', [slack]))).toBe('repost')
    expect(deliveryFor(fromSlack, conversation('t', 'direct_message', [teams]))).toBe('repost')
    expect(deliveryFor(fromSlack, conversation('g', 'email', [teams]))).toBe('mail')
    expect(deliveryFor(fromSlack, undefined)).toBe('unsupported')
  })
})
