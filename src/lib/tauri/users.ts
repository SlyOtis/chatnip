import { writable } from 'svelte/store'
import type { User } from './types'

export const usersStore = writable<{
  is_loading: boolean;
  data: User[];
}>({ is_loading: false, data: [] });