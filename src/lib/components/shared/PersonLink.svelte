<script lang="ts">
  import { PERSON_LINK, PERSON_NAME, personHref } from '$lib/tauri/people'

  interface Props {
    name: string
    personId?: string | null
  }

  let { name, personId = null }: Props = $props()
</script>

{#if personId}<a href={personHref(personId)} class={PERSON_LINK} data-testid="person-link" data-person={personId}>{name}</a>{:else}<strong class={PERSON_NAME}>{name}</strong>{/if}
