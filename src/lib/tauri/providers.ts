import { writable } from 'svelte/store'
import type { ProvidersResponse } from './types'

export const providersStore = writable<ProvidersResponse>({ is_loading: false, data: [] })

export type ConnectionPhase = 'idle' | 'waiting' | 'connected' | 'failed'

export interface ConnectionState {
  phase: ConnectionPhase
  providerType: string | null
  message: string | null
}

export const connectionStore = writable<ConnectionState>({ phase: 'idle', providerType: null, message: null })

export function connectionWaiting(providerType: string) {
  connectionStore.set({ phase: 'waiting', providerType, message: null })
}

export function connectionConnected(name: string) {
  connectionStore.set({ phase: 'connected', providerType: null, message: name })
}

export function connectionFailed(message: string) {
  connectionStore.set({ phase: 'failed', providerType: null, message })
}

export function connectionReset() {
  connectionStore.set({ phase: 'idle', providerType: null, message: null })
}

export function putAccountUsed(providerId: string | null | undefined, at = new Date().toISOString()) {
  if (!providerId) return
  providersStore.update((state) => ({ ...state, data: state.data.map((provider) => (provider.id === providerId ? { ...provider, last_used_at: at } : provider)) }))
}
