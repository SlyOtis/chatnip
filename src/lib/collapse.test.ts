import { beforeEach, describe, expect, it } from 'vitest'
import { get } from 'svelte/store'
import { chatRuns, foldAll, foldPlan, focusFold, folds, isOpen, markerLabel, registerFold, resetFolds, setFold, toggleFocused, toggleFold, type FoldGroup } from './collapse'

const mail = (id: string, conversationId = 'c1'): FoldGroup => ({ id, conversationId, email: true, mine: false, chat: false, size: 1 })
const chat = (id: string, size = 1, mine = false): FoldGroup => ({ id, conversationId: 'c2', email: false, mine, chat: true, size })

describe('thread folding', () => {
  it('adds no folds inside shown day sections', () => {
    const plan = foldPlan(['a', 'b', 'c', 'd', 'e'].map((id) => ({ ...mail(id), section: 'monday' })))
    expect(plan.hidden.size).toBe(0)
    expect(plan.markers.size).toBe(0)
    expect(plan.summary.size).toBe(0)
    expect(foldPlan(['a', 'b', 'c', 'd', 'e'].map((id) => mail(id))).hidden.size).toBe(3)
  })

  it('folds every mail older than the last two behind one earlier marker per conversation', () => {
    const plan = foldPlan(['a', 'b', 'c', 'd', 'e', 'f', 'g'].map((id) => mail(id)))
    expect([...plan.folded]).toEqual(['a', 'b', 'c', 'd', 'e'])
    expect(plan.markers.get('a')).toEqual({ key: 'earlier:c1', kind: 'earlier', count: 5 })
    expect([...plan.hidden.keys()]).toEqual(['a', 'b', 'c', 'd', 'e'])
    expect(plan.hidden.has('f') || plan.hidden.has('g')).toBe(false)
    expect(markerLabel(plan.markers.get('a')!)).toBe('5 earlier messages')
  })

  it('keeps short threads open and counts conversations apart', () => {
    expect(foldPlan([mail('a'), mail('b')]).markers.size).toBe(0)
    const plan = foldPlan([mail('a', 'x'), mail('b', 'y'), mail('c', 'x'), mail('d', 'x'), mail('e', 'y')])
    expect(plan.markers.get('a')).toEqual({ key: 'earlier:x', kind: 'earlier', count: 1 })
    expect(markerLabel(plan.markers.get('a')!)).toBe('1 earlier message')
    expect(plan.markers.size).toBe(1)
  })

  it('shows every mail as a summary except the last group in the view', () => {
    expect([...foldPlan([mail('a'), mail('b'), mail('c')]).summary]).toEqual(['a', 'b'])
    expect([...foldPlan([mail('a'), chat('b'), mail('c'), chat('d')]).summary]).toEqual(['a', 'c'])
    expect(foldPlan([mail('a')]).summary.size).toBe(0)
  })
})

describe('chat run folding', () => {
  it('collapses the middle of a run longer than twelve messages from others', () => {
    const groups = Array.from({ length: 22 }, (_, i) => chat(`m${i}`))
    const runs = chatRuns(groups)
    expect(runs).toHaveLength(1)
    expect(runs[0].hidden[0]).toBe('m3')
    expect(runs[0].hidden.at(-1)).toBe('m18')
    expect(runs[0].count).toBe(16)
    const plan = foldPlan(groups)
    expect(plan.markers.get('m3')).toEqual({ key: 'run:m3', kind: 'run', count: 16 })
    expect(markerLabel(plan.markers.get('m3')!)).toBe('Show 16 more')
  })

  it('counts messages inside groups and keeps whole groups at the edges', () => {
    const runs = chatRuns([chat('a', 4), chat('b', 5), chat('c', 2), chat('d', 4)])
    expect(runs).toEqual([{ key: 'run:b', hidden: ['b', 'c'], count: 7 }])
  })

  it('leaves runs of twelve or fewer and breaks runs at own messages and mail', () => {
    expect(chatRuns(Array.from({ length: 12 }, (_, i) => chat(`m${i}`)))).toEqual([])
    const split = [...Array.from({ length: 8 }, (_, i) => chat(`a${i}`)), chat('mine', 1, true), ...Array.from({ length: 8 }, (_, i) => chat(`b${i}`))]
    expect(chatRuns(split)).toEqual([])
    const broken = [...Array.from({ length: 8 }, (_, i) => chat(`a${i}`)), mail('x'), ...Array.from({ length: 8 }, (_, i) => chat(`b${i}`))]
    expect(chatRuns(broken)).toEqual([])
  })
})

describe('fold state', () => {
  beforeEach(() => {
    folds.set({ all: null, items: {}, focused: null })
  })

  it('falls back to the default until set, then keeps the own value', () => {
    expect(isOpen(get(folds), 'm1', 'open', false)).toBe(false)
    setFold('m1', 'open', true)
    expect(isOpen(get(folds), 'm1', 'open', false)).toBe(true)
    toggleFold('m1', 'quote', false)
    expect(isOpen(get(folds), 'm1', 'quote', false)).toBe(true)
    expect(isOpen(get(folds), 'm1', 'full', false)).toBe(false)
  })

  it('expands and collapses all and clears per message overrides', () => {
    setFold('m1', 'open', false)
    foldAll('open')
    expect(isOpen(get(folds), 'm1', 'open', false)).toBe(true)
    expect(isOpen(get(folds), 'earlier:c1', 'open', false)).toBe(true)
    foldAll('closed')
    expect(isOpen(get(folds), 'm2', 'open', true)).toBe(false)
    toggleFold('m2', 'open', true)
    expect(isOpen(get(folds), 'm2', 'open', true)).toBe(true)
    resetFolds()
    expect(get(folds).all).toBeNull()
  })

  it('toggles the focused message from its registered default', () => {
    registerFold('mail-1', true)
    focusFold('mail-1')
    toggleFocused()
    expect(isOpen(get(folds), 'mail-1', 'open', true)).toBe(false)
    toggleFocused()
    expect(isOpen(get(folds), 'mail-1', 'open', true)).toBe(true)
    registerFold('run:m3', false)
    focusFold('run:m3')
    toggleFocused()
    expect(isOpen(get(folds), 'run:m3', 'open', false)).toBe(true)
  })
})

describe('chatRuns and sections', () => {
  const chat = (id: string, section: string): FoldGroup => ({ id, conversationId: 'c', email: false, mine: false, chat: true, size: 1, section })

  it('never folds across a day, so a day with few messages shows them all', () => {
    const groups = [...Array.from({ length: 20 }, (_, i) => chat(`a${i}`, 'monday')), chat('b1', 'tuesday'), chat('b2', 'wednesday')]
    const runs = chatRuns(groups)
    expect(runs.length).toBe(1)
    expect(runs[0].hidden.every((id) => id.startsWith('a'))).toBe(true)
    expect(foldPlan(groups).hidden.has('b1')).toBe(false)
    expect(foldPlan(groups).hidden.has('b2')).toBe(false)
  })
})
