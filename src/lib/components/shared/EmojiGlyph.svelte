<script lang="ts">
  import { customSrc, emojiDisplay, workspaceEmoji } from '$lib/emoji'

  interface Props {
    emoji: string;
    providerId?: string | null;
    class?: string;
  }

  let { emoji, providerId = null, class: className = '' }: Props = $props()

  const shown = $derived.by(() => {
    $workspaceEmoji
    return emojiDisplay(emoji, providerId)
  })
</script>

{#if shown.custom}
  <img src={customSrc(shown.custom)} alt={shown.code} title={shown.code} class="inline-block h-[1.25em] w-auto align-[-0.25em] {className}" />
{:else}
  <span class={className} title={shown.code.startsWith(':') ? shown.code : undefined}>{shown.unicode ?? shown.code}</span>
{/if}
