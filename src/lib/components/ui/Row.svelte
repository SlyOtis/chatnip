<script lang="ts">
  import type { Snippet } from 'svelte'

  interface Props {
    label: string
    hint?: string
    for?: string
    testid?: string
    id?: string
    children?: Snippet
  }

  let { label, hint, for: forId, id, testid, children }: Props = $props()
</script>

<div {id} data-testid={testid} class="grid min-h-12 grid-cols-[minmax(0,1fr)_auto] items-center gap-6 border-b border-line py-2.5 last:border-b-0">
  <div class="min-w-0">
    <label class="block text-[14px] font-medium text-ink" for={forId}>{label}</label>
    {#if hint}
      <p class="max-w-[68ch] text-[13px] leading-5 text-ink-3">{hint}</p>
    {/if}
  </div>
  <div class="flex min-w-0 items-center justify-end gap-2">{@render children?.()}</div>
</div>
