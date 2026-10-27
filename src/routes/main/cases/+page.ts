import { redirect } from '@sveltejs/kit'
import type { PageLoad } from './$types'
import { featureShown } from '$lib/settings'
import { INBOX_ROUTE } from '$lib/startup'

export const load: PageLoad = ({ url }) => {
  if (!featureShown(url.pathname)) redirect(307, INBOX_ROUTE)
}
