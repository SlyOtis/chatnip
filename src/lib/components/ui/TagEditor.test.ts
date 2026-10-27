import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, screen, fireEvent, cleanup, waitFor } from '@testing-library/svelte'
import { invoke } from '@tauri-apps/api/core'
import TagEditor from './TagEditor.svelte'
import { t } from '$lib/i18n.svelte'

vi.mock('$app/navigation', () => ({ goto: vi.fn() }))
vi.mock('$lib/icons', () => ({
  loadIconBank: () => Promise.resolve(['lucide:star', 'lucide:folder']),
  searchIcons: (names: string[]) => names,
  ensureIcons: () => {}
}))

const tag = { id: 't1', profile_id: 'p', name: 'Client', color: '#1f9d55', kind: 'tag' as const, position: 0, description: '', icon: 'lucide:star', favourite: false, favourite_position: null, created_at: '', updated_at: '' }

afterEach(() => {
  cleanup()
  vi.mocked(invoke).mockReset()
})

function mark() {
  return document.querySelector<HTMLElement>('form .tag-mark')!
}

describe('TagEditor', () => {
  it('goes back to the dot from the icon picker reset entry and saves an empty icon', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'put_tag') return Promise.resolve({ ...tag, icon: null })
      if (cmd === 'get_tags') return Promise.resolve([{ ...tag, icon: null }])
      return Promise.reject(new Error(cmd))
    })
    const onclose = vi.fn()
    render(TagEditor, { tag, onclose })
    expect(mark().dataset.tagMark).toBe('icon')

    await fireEvent.click(screen.getByRole('button', { name: t('app.iconPicker.change') }))
    await fireEvent.click(await screen.findByRole('button', { name: t('app.tagEditor.useDot') }))
    expect(mark().dataset.tagMark).toBe('dot')

    await fireEvent.click(screen.getByRole('button', { name: new RegExp(t('common.save')) }))
    await waitFor(() => expect(onclose).toHaveBeenCalled())
    expect(invoke).toHaveBeenCalledWith('put_tag', { input: { tag_id: 't1', name: 'Client', color: '#1f9d55', icon: '' } })
  })

  it('creates a tag with the chosen colour', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'post_tag') return Promise.resolve({ ...tag, id: 'new', icon: null })
      if (cmd === 'put_tag') return Promise.resolve({ ...tag, id: 'new', icon: null })
      if (cmd === 'get_tags') return Promise.resolve([])
      return Promise.reject(new Error(cmd))
    })
    const onclose = vi.fn()
    render(TagEditor, { onclose })
    await fireEvent.input(screen.getByRole('textbox', { name: t('common.name') }), { target: { value: 'Travel' } })
    await fireEvent.click(screen.getByRole('button', { name: t('app.colour.title') }))
    await fireEvent.click(await screen.findByRole('radio', { name: '#e8743b' }))
    await fireEvent.click(screen.getByRole('button', { name: new RegExp(t('common.create')) }))
    await waitFor(() => expect(onclose).toHaveBeenCalled())
    expect(invoke).toHaveBeenCalledWith('post_tag', { input: { name: 'Travel', color: '#e8743b', kind: 'tag' } })
  })
})
