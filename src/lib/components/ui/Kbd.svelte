<script lang="ts">
  interface Props {
    keys: string
    class?: string
  }

  let { keys, class: className = '' }: Props = $props()
  const parts = $derived(keys.split(' ').filter(Boolean))
</script>

<span class="inline-flex items-center gap-0.5 {className}">
  {#each parts as part, i (i)}
    <kbd class="min-w-[1.25rem] text-center">{part}</kbd>
  {/each}
</span>
