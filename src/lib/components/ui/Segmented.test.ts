import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, cleanup } from '@testing-library/svelte'
import { tick } from 'svelte'
import Segmented from './Segmented.svelte'

const options = [
  { value: 'day', label: 'Light', icon: 'lucide:sun' },
  { value: 'night', label: 'Dark', icon: 'lucide:moon' }
]

function layout(width: number, natural: number) {
  vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockImplementation(function (this: HTMLElement) {
    return this.dataset.segmented === 'labels' ? natural : 0
  })
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(function (this: HTMLElement) {
    return this.classList.contains('segmented-fit') ? width : 0
  })
}

async function settle() {
  await new Promise((resolve) => requestAnimationFrame(() => resolve(null)))
  await tick()
}

function focusable(container: HTMLElement) {
  return Array.from(container.querySelectorAll<HTMLButtonElement>('button')).filter((button) => !inert(button))
}

function inert(node: Element | null): boolean {
  for (let current = node; current; current = current.parentElement) if ((current as HTMLElement).inert) return true
  return false
}

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

describe('Segmented', () => {
  it('lets only the visible icon buttons take focus when compact, each with its name', async () => {
    layout(40, 200)
    const { container } = render(Segmented, { value: 'day', options, label: 'Theme' })
    await settle()
    const icons = container.querySelector<HTMLElement>('[data-segmented="icons"]')!
    const labels = container.querySelector<HTMLElement>('[data-segmented="labels"]')!
    expect(icons.inert).toBe(false)
    expect(labels.inert).toBe(true)
    const buttons = focusable(container)
    expect(buttons).toHaveLength(2)
    for (const button of buttons) expect(icons.contains(button)).toBe(true)
    expect(buttons.map((button) => button.getAttribute('aria-label'))).toEqual(['Light', 'Dark'])
    expect(buttons.every((button) => button.textContent?.trim() === '')).toBe(true)
  })

  it('lets only the labelled buttons take focus when the labels fit', async () => {
    layout(400, 200)
    const { container } = render(Segmented, { value: 'day', options, label: 'Theme' })
    await settle()
    const labels = container.querySelector<HTMLElement>('[data-segmented="labels"]')!
    expect(labels.inert).toBe(false)
    expect(container.querySelector<HTMLElement>('[data-segmented="icons"]')!.inert).toBe(true)
    const buttons = focusable(container)
    expect(buttons).toHaveLength(2)
    for (const button of buttons) expect(labels.contains(button)).toBe(true)
    expect(buttons.map((button) => button.getAttribute('aria-label'))).toEqual(['Light', 'Dark'])
  })

  it('keeps a single focusable row when options have no icons', () => {
    const { container } = render(Segmented, { value: 'a', options: [{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }], label: 'Pick' })
    expect(container.querySelectorAll('button')).toHaveLength(2)
    expect(Array.from(container.querySelectorAll('button')).some((button) => inert(button))).toBe(false)
  })
})
