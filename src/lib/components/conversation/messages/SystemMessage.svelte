<script lang="ts">
  import type { Message } from '$lib/tauri';
  import { peopleStore, personOf } from '$lib/tauri/people';
  import { plainText } from '$lib/messageKind';
  import { systemEventOf, systemParts } from '$lib/systemEvent';
  import PersonLink from '$lib/components/shared/PersonLink.svelte';

  interface Props {
    message: Message;
  }

  let { message }: Props = $props();

  const event = $derived(systemEventOf(message.metadata));
  const parts = $derived(event ? systemParts(event) : []);
</script>

<p class="py-1 text-center text-[13px] text-ink-3">{#if event}{#each parts as part, index (index)}{#if 'person' in part}<PersonLink name={part.name} personId={personOf($peopleStore, part.person)?.id} />{:else}{part.text}{/if}{/each}{:else}{plainText(message.content)}{/if}</p>
