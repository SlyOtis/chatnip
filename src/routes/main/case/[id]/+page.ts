import { redirect } from '@sveltejs/kit'
import type { PageLoad } from './$types'
import { featureShown } from '$lib/settings'
import { INBOX_ROUTE } from '$lib/startup'

export const load: PageLoad = async ({ parent, params }) => {
  if (!featureShown(`/main/case/${params.id}`)) redirect(307, INBOX_ROUTE)
  await parent()
  return { caseId: params.id }
}
