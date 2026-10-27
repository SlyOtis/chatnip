import {writable, derived, get} from 'svelte/store'
import type {ProfileResponse, ProfileSettings} from '$lib/tauri/types'
import {putProfileSettings} from './api'

export const profileStore = writable<ProfileResponse>();

export const profileSettingsStore = derived(
  profileStore,
  $profileStore => $profileStore.data?.settings
);

export const profileThemeStore = derived(
  profileStore,
  $profileStore => $profileStore.data?.theme
);
export async function putProfilePatch(patch: Partial<ProfileSettings>): Promise<void> {
  const settings = get(profileStore)?.data?.settings
  if (!settings) return
  const next = { ...settings, ...patch }
  if (Object.entries(patch).every(([key, value]) => settings[key as keyof ProfileSettings] === value)) return
  profileStore.update((state) => (state.data ? { ...state, data: { ...state.data, settings: next } } : state))
  await putProfileSettings({ settings: next })
}

export function putDetailsOpen(open: boolean | null) {
  const settings = get(profileStore)?.data?.settings
  if (!settings) return
  putProfilePatch({ details_panel_open: open ?? !settings.details_panel_open }).catch(() => {})
}
