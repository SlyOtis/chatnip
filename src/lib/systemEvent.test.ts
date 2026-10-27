import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render } from '@testing-library/svelte'
import { applyLocale } from '$lib/i18n.svelte'
import { peopleStore } from '$lib/tauri/people'
import type { Message, Person, SystemEvent, SystemPerson } from '$lib/tauri/types'
import { systemEventOf } from './systemEvent'
import { linkMentions } from './content'
import SystemMessage from './components/conversation/messages/SystemMessage.svelte'

afterEach(() => {
  cleanup()
  applyLocale('en')
  peopleStore.set([])
})

const who = (name: string, user_id: string | null = `user-${name}`): SystemPerson => ({ user_id, external_id: `ext-${name}`, name })
const anna = who('Anna Berg')
const mia = who('Mia Sund')
const sam = who('Sam Lie')
const nora = who('Nora Gale')
const added = (targets: SystemPerson[], actors: SystemPerson[] = [anna]): SystemEvent => ({ event: 'member_added', actors, targets, detail: null })

function person(id: string, name: string, user_id: string | null, external_id: string | null = null): Person {
  return { id, profile_id: 'p1', display_name: name, provider_name: name, primary_email: null, avatar_url: null, avatar_data: null, overrides: {}, nicknames: {}, is_human: true, favourite_position: null, identities: [{ id: `${id}-i`, person_id: id, user_id, provider_type: 'microsoft', external_id, email: null, display_name: name, manual: false }], conversation_ids: [], provider_types: ['microsoft'], last_activity: null, unread_count: 0, created_at: '', updated_at: '' }
}

function message(system: unknown, content = 'fallback'): Message {
  return { id: 'm1', type: 'system', conversation_id: 'c1', provider_id: 'pr', provider_name: 'Teams', sender_id: 'user-Anna Berg', sender_name: 'Anna Berg', sender_email: null, subject: null, recipients: [], cc_recipients: [], bcc_recipients: [], content, content_type: 'text', timestamp: '2026-10-12T09:00:00Z', metadata: system === null ? null : JSON.stringify({ system }), cached_at: null, thread_id: null, has_attachments: false, attachments: [], last_modified_datetime: '', deleted_datetime: null, is_read: true, created_at: '', updated_at: '' }
}

describe('systemEventOf', () => {
  it('reads the stored event and refuses anything else', () => {
    expect(systemEventOf(JSON.stringify({ system: { event: 'member_added', actors: [{ user_id: 'u1', external_id: 'x1', name: 'Anna' }], targets: [{ user_id: null, external_id: 'x2', name: 'Mia' }], detail: ' ' } }))).toEqual({ event: 'member_added', actors: [{ user_id: 'u1', external_id: 'x1', name: 'Anna' }], targets: [{ user_id: null, external_id: 'x2', name: 'Mia' }], detail: null })
    expect(systemEventOf(JSON.stringify({ system: { event: 'exploded' } }))).toBeNull()
    expect(systemEventOf(null)).toBeNull()
    expect(systemEventOf('not json')).toBeNull()
  })
})

describe('SystemMessage', () => {
  it('links resolved people to their page and leaves the unresolved bold', () => {
    peopleStore.set([person('p-anna', 'Anna Berg', 'user-Anna Berg'), person('p-sam', 'Sam Lie', null, 'ext-Sam Lie')])
    const ghost = who('Ghost', null)
    const { container } = render(SystemMessage, { props: { message: message(added([who('Sam Lie', null), ghost])) } })
    expect(container.textContent).toBe('Anna Berg added Sam Lie and Ghost')
    const links = [...container.querySelectorAll('a')]
    expect(links.map((a) => [a.textContent, a.getAttribute('href')])).toEqual([['Anna Berg', '/main/person/p-anna'], ['Sam Lie', '/main/person/p-sam']])
    expect(links.every((a) => a.className.includes('font-semibold') && a.className.includes('hover:underline'))).toBe(true)
    expect([...container.querySelectorAll('strong')].map((s) => s.textContent)).toEqual(['Ghost'])
  })

  it('renders the Norwegian sentence', () => {
    applyLocale('nb')
    const { container } = render(SystemMessage, { props: { message: message(added([mia, sam, nora])) } })
    expect(container.textContent).toBe('Anna Berg la til Mia Sund, Sam Lie og Nora Gale')
    expect(container.querySelectorAll('strong')).toHaveLength(4)
  })

  it('falls back to the content when there is no structured event', () => {
    const { container } = render(SystemMessage, { props: { message: message(null, 'Mia Sund joined the conversation') } })
    expect(container.textContent).toBe('Mia Sund joined the conversation')
    expect(container.querySelector('a, strong')).toBeNull()
  })
})

describe('linkMentions', () => {
  it('gives inline mentions the person link look and links them when the person is known', () => {
    const people = [person('p-mia', 'Mia Sund', 'user-mia', 'users/110')]
    const html = linkMentions('<p>Hei <at id="0">Mia Sund</at> og <span class="msg-mention">@Ola</span></p>', [{ id: 'users/110', name: 'Mia Sund' }], people)
    const div = document.createElement('div')
    div.innerHTML = html
    const link = div.querySelector('a[data-person]')
    expect([link?.textContent, link?.getAttribute('href')]).toEqual(['Mia Sund', '/main/person/p-mia'])
    expect(link?.className).toContain('font-semibold')
    expect(div.querySelector('strong')?.textContent).toBe('@Ola')
    expect(div.querySelector('at, .msg-mention')).toBeNull()
  })
})
