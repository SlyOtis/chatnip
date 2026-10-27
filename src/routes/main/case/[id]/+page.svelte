<script lang="ts">
  import CaseView from '$lib/components/cases/CaseView.svelte'
  import type { PageProps } from './$types'

  let { data }: PageProps = $props()
</script>

{#key data.caseId}
  <CaseView caseId={data.caseId} />
{/key}
