import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { render, screen, fireEvent, cleanup, waitFor } from '@testing-library/svelte'
import { invoke } from '@tauri-apps/api/core'
import TagPicker from './TagPicker.svelte'
import { objectTagsStore, tagRecordsStore } from '$lib/tauri/tags'
import type { ObjectTag, Tag } from '$lib/tauri/types'
import { t } from '$lib/i18n.svelte'

vi.mock('$app/navigation', () => ({ goto: vi.fn() }))

function record(id: string, name: string, kind: Tag['kind'] = 'tag'): Tag {
  return { id, profile_id: 'p', name, color: '#1f9d55', kind, position: 0, description: '', icon: null, favourite: false, favourite_position: null, created_at: '', updated_at: '' }
}

function link(object_type: ObjectTag['object_type'], object_id: string, tag_id: string): ObjectTag {
  return { object_type, object_id, tag_id, source: 'manual', rule_id: null, reason: null, created_at: '' }
}

beforeEach(() => {
  tagRecordsStore.set([record('a', 'Client'), record('b', 'Travel'), record('s', 'Important', 'split')])
  vi.mocked(invoke).mockImplementation((cmd: string) => {
    if (cmd === 'put_object_tag' || cmd === 'delete_object_tag' || cmd === 'get_object_tags') return Promise.resolve([])
    if (cmd === 'post_tag') return Promise.resolve(record('n', 'Urgent'))
    if (cmd === 'get_tags') return Promise.resolve([record('a', 'Client'), record('n', 'Urgent')])
    return Promise.reject(new Error(cmd))
  })
})

afterEach(() => {
  cleanup()
  vi.mocked(invoke).mockReset()
  objectTagsStore.set([])
})

function row(name: string) {
  return screen.getByRole('menuitemcheckbox', { name: new RegExp(name) })
}

describe('TagPicker on any object type', () => {
  it('tags a message and leaves splits out', async () => {
    objectTagsStore.set([link('message', 'm1', 'a')])
    render(TagPicker, { objectType: 'message', ids: ['m1'], onclose: () => {} })
    expect(row('Client')).toHaveAttribute('aria-checked', 'true')
    expect(row('Travel')).toHaveAttribute('aria-checked', 'false')
    expect(screen.queryByRole('menuitemcheckbox', { name: /Important/ })).toBeNull()
    await fireEvent.click(row('Travel'))
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('put_object_tag', { input: { object_type: 'message', object_id: 'm1', tag_id: 'b' } }))
    expect(invoke).toHaveBeenCalledWith('get_object_tags', { objectType: null, objectId: null })
  })

  it('shows a mixed state across people and completes it for the rest', async () => {
    objectTagsStore.set([link('person', 'p1', 'a'), link('message', 'p2', 'a')])
    render(TagPicker, { objectType: 'person', ids: ['p1', 'p2'], onclose: () => {} })
    expect(row('Client')).toHaveAttribute('aria-checked', 'mixed')
    await fireEvent.click(row('Client'))
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('put_object_tag', { input: { object_type: 'person', object_id: 'p2', tag_id: 'a' } }))
    expect(invoke).not.toHaveBeenCalledWith('put_object_tag', { input: { object_type: 'person', object_id: 'p1', tag_id: 'a' } })
  })

  it('removes a tag every selected event already carries', async () => {
    objectTagsStore.set([link('event', 'e1', 'b'), link('event', 'e2', 'b')])
    render(TagPicker, { objectType: 'event', ids: ['e1', 'e2'], onclose: () => {} })
    await fireEvent.click(row('Travel'))
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('delete_object_tag', { input: { object_type: 'event', object_id: 'e2', tag_id: 'b' } }))
    expect(invoke).toHaveBeenCalledWith('delete_object_tag', { input: { object_type: 'event', object_id: 'e1', tag_id: 'b' } })
  })

  it('offers splits for conversations', () => {
    render(TagPicker, { objectType: 'conversation', ids: ['c1'], onclose: () => {} })
    expect(row('Important')).toBeInTheDocument()
  })

  it('creates a tag from the search and puts it on the content item', async () => {
    render(TagPicker, { objectType: 'content', ids: ['gif'], onclose: () => {} })
    await fireEvent.input(screen.getByRole('textbox', { name: t('app.tagPicker.search') }), { target: { value: 'Urgent' } })
    await fireEvent.click(screen.getByRole('button', { name: new RegExp(t('common.create')) }))
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('put_object_tag', { input: { object_type: 'content', object_id: 'gif', tag_id: 'n' } }))
    expect(invoke).toHaveBeenCalledWith('post_tag', { input: { name: 'Urgent', color: '#1f9d55', kind: 'tag' } })
  })
})
