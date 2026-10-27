import { describe, it, expect, vi, afterEach } from 'vitest'
import { messageAnchor, messageHref, revealMessage } from './messageLink'

afterEach(() => {
  vi.useRealTimers()
  document.body.innerHTML = ''
})

describe('message deep links', () => {
  it('builds a link to the chat the message lives in and reads it back', () => {
    const href = messageHref('c 1', 'm/2')
    expect(href).toBe('/main/conversation/c%201#message-m%2F2')
    expect(messageAnchor(new URL(href, 'http://x').hash)).toBe('m/2')
  })

  it('ignores hashes that are not message anchors', () => {
    expect(messageAnchor('')).toBeNull()
    expect(messageAnchor(null)).toBeNull()
    expect(messageAnchor('#section-2')).toBeNull()
    expect(messageAnchor('#message-')).toBeNull()
    expect(messageAnchor('#message-%E0%A4%A')).toBeNull()
    expect(messageAnchor('message-abc')).toBe('abc')
  })

  it('retries until the message is on screen', async () => {
    vi.useFakeTimers()
    let calls = 0
    const reveal = vi.fn(() => ++calls === 3)
    const run = revealMessage('m1', { reveal, delay: 10, interval: 10 })
    await vi.advanceTimersByTimeAsync(100)
    expect(await run.done).toBe(true)
    expect(reveal).toHaveBeenCalledTimes(3)
    expect(reveal).toHaveBeenCalledWith('m1')
  })

  it('gives up after the attempts and stops when cancelled', async () => {
    vi.useFakeTimers()
    const reveal = vi.fn(() => false)
    const run = revealMessage('m1', { reveal, attempts: 4, delay: 0, interval: 5 })
    await vi.advanceTimersByTimeAsync(100)
    expect(await run.done).toBe(false)
    expect(reveal).toHaveBeenCalledTimes(4)
    const stopped = vi.fn(() => false)
    revealMessage('m2', { reveal: stopped, delay: 10 }).cancel()
    await vi.advanceTimersByTimeAsync(100)
    expect(stopped).not.toHaveBeenCalled()
  })

  it('scrolls to and highlights the message through scrollToMessage', async () => {
    vi.useFakeTimers()
    const node = document.createElement('div')
    node.dataset.messageId = 'm9'
    const scroll = vi.fn()
    node.scrollIntoView = scroll
    document.body.appendChild(node)
    const run = revealMessage('m9', { delay: 0 })
    await vi.advanceTimersByTimeAsync(1)
    expect(await run.done).toBe(true)
    expect(scroll).toHaveBeenCalledWith({ block: 'center', behavior: 'smooth' })
    expect(node.classList.contains('reveal-pulse')).toBe(true)
  })
})
