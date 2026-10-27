<script lang="ts">
  import { goto } from '$app/navigation'
  import { onMount } from 'svelte'
  import { api } from '$lib/tauri/'
  import { putLaunchStep } from '$lib/window'
  import { MAIN_ROUTE, setupRoute } from '$lib/startup'

  onMount(() => {
    putLaunchStep('opening')
    api.postStartup()
      .then((profile) => {
        const path = setupRoute(profile)
        putLaunchStep(path === MAIN_ROUTE ? 'loading' : 'setup')
        return goto(path, { replaceState: true })
      })
      .catch((err: Error) => goto('/error/' + encodeURIComponent(err.message), { replaceState: true }))
  })
</script>
