import type { MergeGroup } from '$lib/tauri/types'

export interface MergeTree {
  clusters: Map<string, string>
  parents: Map<string, string>
  names: Map<string, string>
  ids: Map<string, string>
}

export function mergeTree(groups: MergeGroup[]): MergeTree {
  const leads = new Map(groups.map((group) => [group.id, group.lead_conversation_id]))
  const clusters = new Map<string, string>()
  const parents = new Map<string, string>()
  const names = new Map<string, string>()
  const ids = new Map<string, string>()
  for (const group of groups) {
    const lead = group.lead_conversation_id
    clusters.set(lead, lead)
    ids.set(lead, group.id)
    for (const id of group.conversation_ids) clusters.set(id, lead)
    const above = group.parent_id ? leads.get(group.parent_id) : undefined
    if (above) parents.set(lead, above)
    if (group.name) names.set(lead, group.name)
  }
  return { clusters, parents, names, ids }
}
