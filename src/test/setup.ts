import '@testing-library/jest-dom'
import { vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn((cmd: string, _args?: unknown) => {
    return Promise.reject(new Error(`Unmocked invoke call: ${cmd}`))
  })
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}))
