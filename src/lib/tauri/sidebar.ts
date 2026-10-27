import { writable } from 'svelte/store'
import type { SidebarSection } from './types'

export type SortPreference = 'recent' | 'unread' | 'alphabetical'

export const sortPreferenceStore = writable<SortPreference>('recent')
export const unreadOnlyStore = writable<boolean>(false)
export const sidebarSectionsStore = writable<SidebarSection[]>([])
