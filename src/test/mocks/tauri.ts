import { vi } from 'vitest'

type InvokeHandler = (cmd: string, args?: unknown) => unknown

const handlers = new Map<string, InvokeHandler>()

export function mockInvoke(cmd: string, handler: InvokeHandler) {
  handlers.set(cmd, handler)
}

export function clearMocks() {
  handlers.clear()
}

export function createInvokeMock() {
  return vi.fn(async (cmd: string, args?: unknown) => {
    const handler = handlers.get(cmd)
    if (handler) {
      return handler(cmd, args)
    }
    throw new Error(`No mock handler for invoke: ${cmd}`)
  })
}
