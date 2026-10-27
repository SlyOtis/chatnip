import { writable } from 'svelte/store'
import type { Message } from './types'

interface ThreadPanelState {
  open: boolean;
  thread_id: string | null;
  root_message: Message | null;
}

export const threadPanelStore = writable<ThreadPanelState>({
  open: false,
  thread_id: null,
  root_message: null,
});

export function openThread(thread_id: string, root_message: Message) {
  threadPanelStore.set({ open: true, thread_id, root_message });
}

export function closeThread() {
  threadPanelStore.set({ open: false, thread_id: null, root_message: null });
}
