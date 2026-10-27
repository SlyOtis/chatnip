import type { Message, Provider } from '$lib/tauri/types'
import { imageSrc, type AvatarSource } from '$lib/avatar'

export type AccountHealth = 'ok' | 'reconsent' | 'disconnected'

export interface Author {
  name: string
  face: AvatarSource
  provider: Provider | null
}

type Named = Pick<Provider, 'id' | 'name' | 'account'>

export function accountName(provider: Named): string {
  return provider.account?.display_name?.trim() || provider.account?.email?.trim() || provider.name
}

export function accountAvatar(provider: Named): AvatarSource {
  const account = provider.account
  return { shape: 'person', id: account?.user_id ?? provider.id, name: accountName(provider), src: imageSrc(account?.avatar_data, account?.avatar_url), color: null, faces: [], icon: null }
}

export function accountOf(providers: Provider[], providerId: string | null | undefined): Provider | null {
  if (!providerId) return null
  return providers.find((provider) => provider.id === providerId) ?? null
}

export function ownAuthor(message: Pick<Message, 'provider_id' | 'sender_id'> | null | undefined, providers: Provider[], fallback: string): Author {
  const provider = accountOf(providers, message?.provider_id)
  if (provider) return { name: accountName(provider), face: accountAvatar(provider), provider }
  return { name: fallback, face: { shape: 'person', id: message?.sender_id || 'me', name: fallback, src: null, color: 'var(--accent)', faces: [], icon: null }, provider: null }
}

export function lastUsedAccount(providers: Provider[]): Provider | null {
  const connected = providers.filter((provider) => provider.is_connected)
  const candidates = connected.length ? connected : providers
  return candidates.reduce<Provider | null>((best, provider) => (!best || (provider.last_used_at ?? '') > (best.last_used_at ?? '') ? provider : best), null)
}

export function accountHealth(provider: Pick<Provider, 'is_connected' | 'missing_scopes'>): AccountHealth {
  if (!provider.is_connected) return 'disconnected'
  return provider.missing_scopes?.length ? 'reconsent' : 'ok'
}
