import { describe, expect, it } from 'vitest'
import { describeConditions, emptyCondition, regexError } from './rules'

describe('describeConditions', () => {
  it('joins with and for match all', () => {
    expect(
      describeConditions(
        [
          { field: 'sender_domain', operator: 'ends_with', value: 'acme.com' },
          { field: 'subject', operator: 'contains', value: 'invoice' }
        ],
        'all'
      )
    ).toBe('sender domain ends with acme.com and subject contains invoice')
  })
  it('joins with or for match any', () => {
    expect(
      describeConditions(
        [
          { field: 'provider_type', operator: 'is', value: 'slack' },
          { field: 'member_count', operator: 'greater_than', value: '5' }
        ],
        'any'
      )
    ).toBe('provider is slack or member count is greater than 5')
  })
  it('says everything with no conditions', () => {
    expect(describeConditions([], 'all')).toBe('everything')
  })
})

describe('condition helpers', () => {
  it('starts a condition on the sender domain', () => {
    expect(emptyCondition()).toEqual({ field: 'sender_domain', operator: 'ends_with', value: '' })
  })
  it('reports a broken pattern only for matches', () => {
    expect(regexError({ field: 'subject', operator: 'matches', value: '(' })).not.toBeNull()
    expect(regexError({ field: 'subject', operator: 'contains', value: '(' })).toBeNull()
  })
})
