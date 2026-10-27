import { describe, expect, it } from 'vitest'
import { cycleScope, highlightParts, inScope, matchScore, operatorAt, rank, snippetParts, uniqueTokens, visibleItems, withoutOperator } from './palette'

describe('palette operators', () => {
  it('finds the operator being typed at the end of the query', () => {
    expect(operatorAt('kontrakt @an')).toEqual({ op: '@', term: 'an', start: 9 })
    expect(operatorAt('#kun')).toEqual({ op: '#', term: 'kun', start: 0 })
    expect(operatorAt('#')).toEqual({ op: '#', term: '', start: 0 })
    expect(operatorAt('mail@client.example')).toBeNull()
    expect(operatorAt('@anna and more')).toBeNull()
  })

  it('drops the accepted operator from the free text', () => {
    const text = 'fast pris @ann'
    expect(withoutOperator(text, operatorAt(text)!)).toBe('fast pris')
  })

  it('keeps tokens unique', () => {
    const anna = { kind: 'person' as const, id: 'p1', label: 'Anna' }
    expect(uniqueTokens(uniqueTokens([], anna), anna)).toHaveLength(1)
  })
})

describe('palette ranking', () => {
  it('scores exact, prefix, word start and contains in that order', () => {
    expect(matchScore('Anna', 'anna')).toBe(100)
    expect(matchScore('Anna Berg', 'ann')).toBe(80)
    expect(matchScore('Kari Anna', 'ann')).toBe(60)
    expect(matchScore('Hannah', 'ann')).toBe(40)
    expect(matchScore('Oscar Strand', 'oscar')).toBe(80)
    expect(matchScore('Bob', 'ann')).toBe(0)
  })

  it('ranks by match then recency and drops misses', () => {
    const now = new Date('2026-10-14').getTime()
    const items = [
      { name: 'Hannah', at: '2026-10-13' },
      { name: 'Anna old', at: '2025-01-01' },
      { name: 'Anna new', at: '2026-10-13' },
      { name: 'Bob', at: '2026-10-14' }
    ]
    expect(rank(items, 'anna', (i) => [i.name], (i) => i.at, now).map((i) => i.name)).toEqual(['Anna new', 'Anna old', 'Hannah'])
  })
})

describe('palette snippets', () => {
  it('splits backend snippets on the hit markers', () => {
    expect(snippetParts('…går for \u0002fast\u0003 \u0002pris\u0003 fra…')).toEqual([
      { text: '…går for ', hit: false },
      { text: 'fast', hit: true },
      { text: ' ', hit: false },
      { text: 'pris', hit: true },
      { text: ' fra…', hit: false }
    ])
  })

  it('centres a local highlight on the first match', () => {
    const text = `${'a '.repeat(100)}viktig avtale ${'b '.repeat(100)}`
    const parts = highlightParts(text, 'avtale', 20)
    expect(parts.some((part) => part.hit && part.text === 'avtale')).toBe(true)
    expect(parts[0].text.startsWith('…')).toBe(true)
    expect(parts[parts.length - 1].text.endsWith('…')).toBe(true)
  })
})

describe('palette scopes', () => {
  it('maps scopes to groups and truncates only in All', () => {
    expect(inScope('mail', 'messages')).toBe(true)
    expect(inScope('places', 'tags')).toBe(true)
    expect(inScope('people', 'chats')).toBe(false)
    expect(visibleItems([1, 2, 3, 4, 5, 6], 'all')).toEqual({ shown: [1, 2, 3, 4], hidden: 2 })
    expect(visibleItems([1, 2, 3, 4, 5, 6], 'people')).toEqual({ shown: [1, 2, 3, 4, 5, 6], hidden: 0 })
    expect(cycleScope('all', -1)).toBe('commands')
    expect(cycleScope('commands', 1)).toBe('all')
  })
})
