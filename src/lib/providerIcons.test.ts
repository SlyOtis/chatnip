import { describe, expect, it } from 'vitest'
import { providerIcon, providerLabel } from './providerIcons'

describe('provider marks', () => {
  it('picks the product logo from the provider and the conversation kind', () => {
    expect(providerIcon('microsoft', 'email')).toBe('logos:microsoft-outlook')
    expect(providerIcon('microsoft', 'channel')).toBe('logos:microsoft-teams')
    expect(providerIcon({ provider_type: 'google' }, 'direct_message')).toBe('logos:google-chat')
    expect(providerLabel('slack')).toBe('Slack')
  })

  it('shows a test client as the product it stands in for', () => {
    expect(providerIcon({ provider_type: 'mock', external_id: 'mock-gmail-me' }, 'email')).toBe('logos:google-gmail')
    expect(providerIcon({ provider_type: 'mock', external_id: 'mock-teams-u5' })).toBe('logos:microsoft-teams')
    expect(providerLabel({ provider_type: 'mock', external_id: 'mock-google_chat-me' })).toBe('Mock Google Chat')
    expect(providerIcon('mock')).toBe('lucide:flask-conical')
    expect(providerLabel({ provider_type: 'mock', external_id: null })).toBe('Test client')
  })
})
