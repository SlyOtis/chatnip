import type {LayoutLoad} from './$types';
import {api, profileStore} from "$lib/tauri";
import {redirect} from "@sveltejs/kit";
import { t } from '$lib/i18n.svelte';

export const load: LayoutLoad = async ({parent}) => {
  await parent();

  try {
    await api.getProfile().then(profile => profileStore.set(profile))
  } catch (err) {
    const message = typeof err === 'string' ? err : err instanceof Error ? err.message : t('app.error.missingProfile')
    console.error(message)
    throw redirect(303, '/error/' + message);
  }
}