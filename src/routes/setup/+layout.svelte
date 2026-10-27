<script lang="ts">
  import { onMount } from 'svelte'
  import type { Snippet } from 'svelte'
  import { putLaunchStep } from '$lib/window'
  import WindowManager from '$lib/components/window/WindowManager.svelte'
  import Modals from '$lib/components/modals/Modals.svelte'

  interface Props {
    children?: Snippet
  }

  let { children }: Props = $props()

  onMount(() => putLaunchStep('ready'))
</script>

<WindowManager />
<Modals />
<div class="relative z-[1] grid h-full bg-paper grid-rows-[2.5rem_minmax(0,1fr)]">
  <div></div>
  <div class="overflow-y-auto">
    {@render children?.()}
  </div>
</div>
