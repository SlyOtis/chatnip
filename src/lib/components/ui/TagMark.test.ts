import { describe, it, expect, afterEach } from 'vitest'
import { render, cleanup } from '@testing-library/svelte'
import TagMark from './TagMark.svelte'
import { contrast, tagTone } from '$lib/tauri/tags'

afterEach(() => cleanup())

function mark(container: HTMLElement) {
  return container.querySelector<HTMLElement>('.tag-mark')!
}

describe('TagMark', () => {
  it('draws a dot when the tag has no icon', () => {
    const { container } = render(TagMark, { tag: { name: 'Client', color: '#1f9d55', icon: null } })
    expect(mark(container).dataset.tagMark).toBe('dot')
    expect(container.querySelector('.rounded-full.bg-current')).not.toBeNull()
    expect(container.querySelector('svg')).toBeNull()
  })

  it('draws the icon instead of the dot when one is set', () => {
    const { container } = render(TagMark, { tag: { name: 'Client', color: '#1f9d55', icon: 'lucide:star' } })
    expect(mark(container).dataset.tagMark).toBe('icon')
    expect(container.querySelector('.rounded-full.bg-current')).toBeNull()
  })

  it('tints the mark with the tag colour in both themes', () => {
    const { container } = render(TagMark, { tag: { name: 'Client', color: '#1f9d55', icon: 'lucide:star' } })
    expect(mark(container).style.getPropertyValue('--tag-day')).toBe('#1f9d55')
    expect(mark(container).style.getPropertyValue('--tag-night')).toBe('#1f9d55')
  })

  it('swaps in a contrast safe variant for a light colour on day paper', () => {
    const { container } = render(TagMark, { tag: { name: 'Sun', color: '#ffe680', icon: null } })
    const day = mark(container).style.getPropertyValue('--tag-day')
    expect(day).toMatch(/^color-mix\(in srgb, #ffe680 \d+%, var\(--ink\)\)$/)
    expect(mark(container).style.getPropertyValue('--tag-night')).toBe('#ffe680')
  })

  it('swaps in a contrast safe variant for a dark colour on night paper', () => {
    const { container } = render(TagMark, { tag: { name: 'Ink', color: '#1b2a3a', icon: 'lucide:anchor' } })
    expect(mark(container).style.getPropertyValue('--tag-day')).toBe('#1b2a3a')
    expect(mark(container).style.getPropertyValue('--tag-night')).toMatch(/^color-mix\(in srgb, #1b2a3a \d+%, var\(--ink\)\)$/)
  })

  it('shows the name only when asked', () => {
    const quiet = render(TagMark, { tag: { name: 'Client', color: '#1f9d55', icon: null } })
    expect(quiet.queryByText('Client')).toBeNull()
    cleanup()
    const labelled = render(TagMark, { tag: { name: 'Client', color: '#1f9d55', icon: null }, label: true })
    expect(labelled.getByText('Client')).toBeInTheDocument()
  })
})

describe('tagTone', () => {
  it('keeps colours that already clear three to one and mixes the rest until they do', () => {
    expect(tagTone('#1f9d55', 'day')).toBe('#1f9d55')
    const tone = tagTone('#ffe680', 'day')
    const share = Number(/(\d+)%/.exec(tone)![1]) / 100
    const mixed = '#' + [[0xff, 0x1d], [0xe6, 0x1a], [0x80, 0x17]].map(([c, i]) => Math.round(c * share + i * (1 - share)).toString(16).padStart(2, '0')).join('')
    expect(contrast(mixed, '#faf8f4')).toBeGreaterThanOrEqual(3)
    expect(contrast('#ffe680', '#faf8f4')).toBeLessThan(3)
  })

  it('passes through values it cannot read', () => {
    expect(tagTone('var(--accent)', 'night')).toBe('var(--accent)')
  })
})
