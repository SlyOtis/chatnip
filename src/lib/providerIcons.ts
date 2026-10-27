import type { ConversationType } from '$lib/tauri/types'
import { t } from '$lib/i18n.svelte'

export interface ProviderSource {
  provider_type: string
  external_id?: string | null
}

const MOCK_KINDS: Record<string, { icon: string; label: string }> = {
  teams: { icon: 'logos:microsoft-teams', label: 'Mock Teams' },
  outlook: { icon: 'logos:microsoft-outlook', label: 'Mock Outlook' },
  slack: { icon: 'logos:slack-icon', label: 'Mock Slack' },
  gmail: { icon: 'logos:google-gmail', label: 'Mock Gmail' },
  google_chat: { icon: 'logos:google-chat', label: 'Mock Google Chat' },
  imap: { icon: 'lucide:at-sign', label: 'Mock IMAP' }
}

export function mockKind(source: string | ProviderSource): string | null {
  if (typeof source === 'string' || source.provider_type !== 'mock') return null
  return source.external_id?.match(/^mock-([a-z_]+)-/)?.[1] ?? null
}

function mockOf(source: string | ProviderSource): { icon: string; label: string } | null {
  const kind = mockKind(source)
  return kind ? (MOCK_KINDS[kind] ?? null) : null
}

export function providerIcon(source: string | ProviderSource, conversationType?: ConversationType | string): string {
  const mock = mockOf(source)
  if (mock) return mock.icon
  const providerType = typeof source === 'string' ? source : source.provider_type
  const chat = conversationType === 'direct_message' || conversationType === 'channel' || conversationType === 'group_message'
  switch (providerType.toLowerCase()) {
    case 'microsoft':
      return conversationType === 'email' ? 'logos:microsoft-outlook' : chat ? 'logos:microsoft-teams' : 'logos:microsoft-icon'
    case 'google':
      return conversationType === 'email' ? 'logos:google-gmail' : chat ? 'logos:google-chat' : 'logos:google-icon'
    case 'slack':
      return 'logos:slack-icon'
    case 'imap':
      return 'lucide:at-sign'
    case 'mock':
      return 'lucide:flask-conical'
    default:
      return conversationType === 'email' ? 'lucide:mail' : 'lucide:message-circle'
  }
}

export function providerLabel(source: string | ProviderSource): string {
  const mock = mockOf(source)
  if (mock) return mock.label
  const providerType = typeof source === 'string' ? source : source.provider_type
  switch (providerType.toLowerCase()) {
    case 'microsoft':
      return 'Microsoft 365'
    case 'google':
      return 'Google'
    case 'slack':
      return 'Slack'
    case 'imap':
      return 'IMAP'
    case 'mock':
      return t('people.provider.testClient')
    default:
      return providerType
  }
}

export function providerApp(source: string | ProviderSource, messageType?: string | null): string {
  const providerType = typeof source === 'string' ? source : source.provider_type
  const mail = messageType === 'email'
  switch (providerType.toLowerCase()) {
    case 'microsoft':
      return mail ? 'Outlook' : 'Teams'
    case 'google':
      return mail ? 'Gmail' : 'Google Chat'
    default:
      return providerLabel(source)
  }
}

