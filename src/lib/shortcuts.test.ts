import { describe, expect, it, vi } from 'vitest'
import { armSequence, comboKeys, formatShortcutKeys, handleKeydown, registerShortcut } from './shortcuts'

function press(key: string) {
  handleKeydown(new KeyboardEvent('keydown', { key, cancelable: true }))
}

describe('shortcut sequences', () => {
  it('a key after a prefix runs its sequence and the same key alone runs its own shortcut', () => {
    const done = vi.fn()
    const goDone = vi.fn()
    registerShortcut({ key: 'g', description: 'app.shortcut.goTo', category: 'app.shortcutCategory.navigation', action: () => armSequence('g') })
    registerShortcut({ key: 'e', after: 'g', description: 'app.shortcut.goToDone', category: 'app.shortcutCategory.navigation', action: goDone })
    registerShortcut({ key: 'e', description: 'app.shortcut.markDone', category: 'app.shortcutCategory.inbox', action: done })
    press('e')
    expect(done).toHaveBeenCalledTimes(1)
    expect(goDone).not.toHaveBeenCalled()
    press('g')
    press('e')
    expect(goDone).toHaveBeenCalledTimes(1)
    expect(done).toHaveBeenCalledTimes(1)
  })

  it('a sequence shows both keys', () => {
    const shortcut = { key: 'i', after: 'g', description: 'app.shortcut.goToInbox' as const, category: 'app.shortcutCategory.navigation' as const, action: () => {} }
    expect(formatShortcutKeys(shortcut)).toEqual(['G', 'I'])
    expect(comboKeys('mod+shift+a')).toEqual(['⌘', '⇧', 'A'])
  })
})
