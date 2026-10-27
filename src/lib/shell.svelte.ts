import type { Component } from 'svelte'
import { get, writable, type Writable } from 'svelte/store'
import type { Message } from '$lib/tauri/types'
import { closeThread } from '$lib/tauri/threads'
import ThreadPanel from '$lib/components/conversation/ThreadPanel.svelte'
import DetailsPanel from '$lib/components/conversation/details/DetailsPanel.svelte'

export interface ShellSlot {
  key: string
  component: Component<any>
  props?: Record<string, unknown>
  open: boolean
  size?: string
  min?: string
  max?: string
}

export const shellPanel = writable<ShellSlot | null>(null)

export function holdSlot(store: Writable<ShellSlot | null>, read: () => ShellSlot | null) {
  let key: string | null = null
  $effect(() => {
    const slot = read()
    key = slot?.key ?? null
    store.set(slot)
  })
  $effect(() => () => {
    if (key && get(store)?.key === key) store.set(null)
  })
}

export function holdConversationPanel(read: () => { thread: { thread_id: string; root_message: Message } | null; threadOpen: boolean; detailsOpen: boolean }) {
  holdSlot(shellPanel, () => {
    const state = read()
    if (state.threadOpen && state.thread) {
      return { key: 'thread', component: ThreadPanel, props: { thread_id: state.thread.thread_id, root_message: state.thread.root_message, onclose: closeThread }, open: true, size: '28vw', min: '18rem', max: '40vw' }
    }
    return { key: 'details', component: DetailsPanel, open: state.detailsOpen, size: '26rem', min: '18rem', max: '45vw' }
  })
}
