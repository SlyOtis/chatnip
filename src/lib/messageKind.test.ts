import { describe, expect, it } from 'vitest'
import { collapsedEmails, emojiCount, inviteOf, isEmojiOnly, linksOf, messageKind, plainText, presented, reactionsOf, type KindSource } from './messageKind'

const base: KindSource = { type: 'chat', content: 'Hei', content_type: 'text', metadata: null, has_attachments: false, attachments: [] }

describe('messageKind', () => {
  it('classifies system messages first', () => {
    expect(messageKind({ ...base, type: 'system', content: 'Anna joined' })).toBe('system')
  })
  it('detects invites from content type, metadata and ics bodies', () => {
    expect(messageKind({ ...base, type: 'email', content_type: 'text/calendar' })).toBe('invite')
    expect(messageKind({ ...base, metadata: JSON.stringify({ event: { title: 'Sync', starts_at: '2026-10-12T09:00:00Z' } }) })).toBe('invite')
    expect(messageKind({ ...base, type: 'email', content: 'BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20261012T090000Z\nEND:VEVENT' })).toBe('invite')
  })
  it('treats bot type and non human senders as bot', () => {
    expect(messageKind({ ...base, type: 'bot' })).toBe('bot')
    expect(messageKind({ ...base, type: 'email' }, { senderHuman: false })).toBe('bot')
  })
  it('keeps email as email when the sender is human or unknown', () => {
    expect(messageKind({ ...base, type: 'email' }, { senderHuman: true })).toBe('email')
    expect(messageKind({ ...base, type: 'email' })).toBe('email')
  })
  it('detects media only messages', () => {
    expect(messageKind({ ...base, content: '', has_attachments: true, attachments: [{ content_type: 'image/png' }, { content_type: 'image/jpeg' }] })).toBe('media')
    expect(messageKind({ ...base, content: 'Look', has_attachments: true, attachments: [{ content_type: 'image/png' }] })).toBe('chat')
    expect(messageKind({ ...base, content: '', has_attachments: true, attachments: [{ content_type: 'application/pdf' }] })).toBe('chat')
  })
  it('detects emoji only messages up to three', () => {
    expect(messageKind({ ...base, content: '👍' })).toBe('emoji')
    expect(messageKind({ ...base, content: ' 🎉🎉🎉 ' })).toBe('emoji')
    expect(messageKind({ ...base, content: '👍🏽' })).toBe('emoji')
    expect(messageKind({ ...base, content: '🎉🎉🎉🎉' })).toBe('chat')
    expect(messageKind({ ...base, content: 'ok 👍' })).toBe('chat')
  })
  it('distinguishes channel and chat by conversation type', () => {
    expect(messageKind(base, { conversationType: 'channel' })).toBe('channel')
    expect(messageKind(base, { conversationType: 'group_message' })).toBe('channel')
    expect(messageKind(base, { conversationType: 'direct_message' })).toBe('chat')
  })
})

describe('content helpers', () => {
  it('strips html to plain text', () => {
    expect(plainText('<p>Hei&nbsp;Milo,</p><p>Takk &amp; ha det</p><style>p{}</style>')).toBe('Hei Milo,\nTakk & ha det')
  })
  it('counts emoji graphemes including flags and zwj sequences', () => {
    expect(emojiCount('🇳🇴')).toBe(1)
    expect(emojiCount('👨‍👩‍👧')).toBe(1)
    expect(isEmojiOnly('1')).toBe(false)
  })
  it('treats shortcode only text as emoji only', () => {
    expect(emojiCount(':tada: :+1::skin-tone-3:')).toBe(2)
    expect(isEmojiOnly(':rocket:')).toBe(true)
    expect(isEmojiOnly('ship :rocket:')).toBe(false)
    expect(isEmojiOnly(':not_an_emoji_here:')).toBe(false)
  })
  it('groups reactions by emoji', () => {
    const reactions = reactionsOf(JSON.stringify({ reactions: [{ emoji: '👍', user: 'A' }, { emoji: '👍', user: 'B' }, { emoji: ':tada:', user: 'A', original: 'tada' }] }))
    expect(reactions).toEqual([{ emoji: ':+1:', count: 2, users: ['A', 'B'], names: ['A', 'B'] }, { emoji: ':tada:', count: 1, users: ['A'], names: ['A'] }])
    expect(reactionsOf('not json')).toEqual([])
  })
  it('parses invites from ics with folded lines and escapes', () => {
    const invite = inviteOf({ ...base, content: 'BEGIN:VEVENT\nSUMMARY:Board\\, Q4\nDTSTART;TZID=Europe/Oslo:20261012T090000\nDTEND:20261012T100000\nLOCATION:Oslo\n office\nORGANIZER:mailto:anna@client.example\nEND:VEVENT' })
    expect(invite).toEqual({ title: 'Board, Q4', starts_at: '2026-10-12T09:00:00', ends_at: '2026-10-12T10:00:00', location: 'Oslooffice', organizer: 'anna@client.example' })
    expect(inviteOf(base)).toBeNull()
  })
  it('extracts links from html and text without trailing punctuation', () => {
    expect(linksOf('<a href="https://a.no/x">x</a> see https://b.no/y.')).toEqual(['https://a.no/x', 'https://b.no/y'])
  })
  it('collapses all but the last two emails per conversation', () => {
    const items = ['a', 'b', 'c', 'd'].map((id) => ({ id, kind: 'email' as const, conversationId: 'c1' }))
    expect([...collapsedEmails([...items, { id: 'e', kind: 'chat', conversationId: 'c1' }])]).toEqual(['a', 'b'])
  })
})

describe('messageKind for the mock content', () => {
  const email = (content: string, metadata: Record<string, unknown> | null = null, attachments: { content_type: string }[] = []): KindSource => ({ type: 'email', content, content_type: 'html', metadata: metadata ? JSON.stringify(metadata) : null, has_attachments: attachments.length > 0, attachments })
  const chatMessage = (content: string, extra: Partial<KindSource> = {}): KindSource => ({ ...base, content, ...extra })

  it('classifies service mail as bot cards', () => {
    for (const sender of ['TLDR newsletter', 'Nordhus retailer', 'Nordlys receipt', 'GitHub diff', 'Linear issue', 'tracking summit', 'payslip']) {
      expect(messageKind(email(`<table><tr><td>${sender}</td></tr></table>`, { structure: { is_bulk: true } }), { senderHuman: false }), sender).toBe('bot')
    }
  })

  it('classifies human mail threads and replies as email', () => {
    for (const html of ['<div id="divRplyFwdMsg"><b>From:</b> Anna</div>', '<div class="gmail_quote"><blockquote class="gmail_quote">x</blockquote></div>', '<blockquote type="cite">On 7 Oct wrote:</blockquote>', '<h2>Bakgrunn</h2><ul><li>x</li></ul>']) {
      expect(messageKind(email(html), { senderHuman: true })).toBe('email')
    }
    expect(messageKind({ ...email('> sitert'), content_type: 'text' }, { senderHuman: true })).toBe('email')
  })

  it('classifies the cid mail with four attachments as email, not media', () => {
    const attachments = ['image/png', 'image/png', 'application/pdf', 'application/zip'].map((content_type) => ({ content_type }))
    expect(messageKind(email('<img src="cid:chart-uke41@tidewater.example">', null, attachments), { senderHuman: true })).toBe('email')
  })

  it('classifies the html invite with a calendar part as an invite before the sender check', () => {
    const invite = email('<p>Sprintdemo</p>', { event: { title: 'Sprintdemo', starts_at: '2026-10-16T12:00:00+00:00' } })
    expect(messageKind(invite, { senderHuman: true })).toBe('invite')
    expect(messageKind(invite, { senderHuman: false })).toBe('invite')
  })

  it('classifies teams html, slack rich text and google chat cards by conversation', () => {
    expect(messageKind(chatMessage('<table><tr><th>Leveranse</th></tr></table>', { content_type: 'html' }), { conversationType: 'group_message' })).toBe('channel')
    expect(messageKind(chatMessage('<blockquote class="reply-quote"><strong>Nora</strong><p>PR?</p></blockquote><p>Ja</p>', { content_type: 'html' }), { conversationType: 'direct_message' })).toBe('chat')
    expect(messageKind(chatMessage('Release 0.5 is cut 📦'), { conversationType: 'channel' })).toBe('channel')
    expect(messageKind(chatMessage('Standup 09:15', { type: 'bot' }))).toBe('bot')
  })

  it('classifies gif and image only messages as media', () => {
    expect(messageKind(chatMessage('', { has_attachments: true, attachments: [{ content_type: 'image/gif' }] }))).toBe('media')
    expect(messageKind(chatMessage('', { has_attachments: true, attachments: [{ content_type: 'image/jpeg' }] }))).toBe('media')
    expect(messageKind(chatMessage('Board deck', { has_attachments: true, attachments: [{ content_type: 'application/pdf' }, { content_type: 'image/jpeg' }] }), { conversationType: 'channel' })).toBe('channel')
  })

  it('classifies emoji only, link preview, join, leave and huddle messages', () => {
    expect(messageKind(chatMessage('🎉🎉🎉'))).toBe('emoji')
    expect(messageKind(chatMessage('Tre lenker til helgen: https://github.com/tauri-apps/tauri https://www.youtube.com/watch?v=jfKfPfyJRdk https://svelte.dev/blog/runes'))).toBe('chat')
    expect(messageKind(chatMessage('Mia Sund joined the conversation', { type: 'system' }))).toBe('system')
    expect(messageKind(chatMessage('Hugo Brandt left the conversation', { type: 'system' }))).toBe('system')
    expect(messageKind(chatMessage('A huddle happened in #dev', { type: 'system' }), { conversationType: 'channel' })).toBe('system')
  })
})

describe('presentation set by actions', () => {
  const mail = { type: 'email', content: '<p>Teams-møte: https://teams.microsoft.com/l/meetup-join/x</p>', content_type: 'html', metadata: null }

  it('takes the stored presentation before detection, then the conversation default', () => {
    expect(messageKind(mail)).toBe('email')
    expect(messageKind({ ...mail, presentation: { kind: 'invite' } })).toBe('invite')
    expect(messageKind(mail, { conversationPresentation: { kind: 'notification' } })).toBe('bot')
    expect(messageKind({ ...mail, presentation: { kind: 'chat' } }, { conversationPresentation: { kind: 'hidden' } })).toBe('chat')
    expect(messageKind({ ...mail, presentation: { kind: 'auto' } })).toBe('email')
    expect(messageKind({ ...mail, presentation: { kind: 'hidden' } })).toBe('hidden')
  })

  it('builds the invite card from presented invite details when there is no calendar file', () => {
    const invite = inviteOf({ ...mail, presentation: { kind: 'invite', invite: { title: 'Sprintdemo', starts_at: '2026-10-16T12:00:00Z', organizer: 'Lea', join_url: 'https://teams.microsoft.com/l/meetup-join/x' } } })
    expect(invite).toEqual({ title: 'Sprintdemo', starts_at: '2026-10-16T12:00:00Z', ends_at: null, location: '', organizer: 'Lea', join_url: 'https://teams.microsoft.com/l/meetup-join/x' })
    expect(presented({ presentation: { highlight: '#e0612f' } }, { conversationPresentation: { highlight: '#1f9d55', badge: 'Kunde' } })).toEqual({ highlight: '#e0612f', badge: 'Kunde' })
  })
})
