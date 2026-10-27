import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, cleanup, screen } from '@testing-library/svelte'
import { tick } from 'svelte'
import { launchSettled } from '$lib/window'

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onFocusChanged: () => Promise.resolve(() => {}),
    isMaximized: () => Promise.resolve(false),
    minimize: () => Promise.resolve(),
    toggleMaximize: () => Promise.resolve(),
    close: () => Promise.resolve()
  })
}))

vi.mock('$lib/tauri/api', () => ({ getPlatform: () => Promise.resolve('windows') }))

afterEach(() => {
  cleanup()
  launchSettled.set(false)
})

async function settle() {
  for (let i = 0; i < 5; i++) {
    await Promise.resolve()
    await tick()
  }
}

describe('WindowControls', () => {
  it('stays hidden while the window is the launch card', async () => {
    launchSettled.set(false)
    const { default: WindowControls } = await import('./WindowControls.svelte')
    render(WindowControls)
    await settle()
    expect(screen.queryByRole('toolbar')).toBeNull()
  })

  it('appears once the launch has grown into the app', async () => {
    launchSettled.set(false)
    const { default: WindowControls } = await import('./WindowControls.svelte')
    render(WindowControls)
    await settle()
    launchSettled.set(true)
    await settle()
    expect(screen.queryByRole('toolbar')).not.toBeNull()
  })
})
