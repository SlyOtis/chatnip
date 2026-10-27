import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte'
import IconPicker from './IconPicker.svelte'
import { t } from '$lib/i18n.svelte'

vi.mock('$lib/icons', () => ({
  loadIconBank: () => Promise.resolve(['lucide:folder', 'lucide:star']),
  searchIcons: (names: string[]) => names
}))

afterEach(() => cleanup())

function reset() {
  return screen.queryByRole('button', { name: t('app.iconPicker.useDefault') })
}

describe('IconPicker reset entry', () => {
  it('shows when a custom icon is set and a fallback exists', () => {
    render(IconPicker, { value: 'lucide:star', fallback: 'lucide:folder', onpick: () => {}, onclose: () => {} })
    expect(reset()).toBeInTheDocument()
  })

  it('hides when no custom icon is set', () => {
    render(IconPicker, { value: null, fallback: 'lucide:folder', onpick: () => {}, onclose: () => {} })
    expect(reset()).toBeNull()
  })

  it('hides when there is no fallback', () => {
    render(IconPicker, { value: 'lucide:star', onpick: () => {}, onclose: () => {} })
    expect(reset()).toBeNull()
  })

  it('picks null and closes', async () => {
    const onpick = vi.fn()
    const onclose = vi.fn()
    render(IconPicker, { value: 'lucide:star', fallback: 'lucide:folder', onpick, onclose })
    await fireEvent.click(reset()!)
    expect(onpick).toHaveBeenCalledWith(null)
    expect(onclose).toHaveBeenCalled()
  })
})
