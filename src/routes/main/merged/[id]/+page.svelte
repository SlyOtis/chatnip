<script lang="ts">
  import PersonThread from '$lib/components/conversation/PersonThread.svelte'
  import { appearanceSettings, globalAppearance, conversationKey, scoped, scopePresentation } from '$lib/appearance'
  import { themeChoice, resolvedTheme } from '$lib/theme-mode'
  import type { PageProps } from './$types'

  let { data }: PageProps = $props()

  const keys = $derived([conversationKey(data.mergedId)])
  const scope = $derived(scopePresentation($globalAppearance, scoped($appearanceSettings, keys), resolvedTheme($themeChoice)))
</script>

<div class="relative isolate h-full overflow-hidden {scope.tinted ? `frost-${scope.tint}` : ''}" style={scope.style} style:--frost-alpha={scope.solid ? '1' : undefined}>
  {#if scope.backdrop}<div class="scope-backdrop" style:background={scope.backdrop}></div>{/if}
  {#if scope.tinted}<div class="scope-frost"></div>{/if}
  {#key data.mergedId}
    <PersonThread mergedId={data.mergedId} />
  {/key}
</div>
