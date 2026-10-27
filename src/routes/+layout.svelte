<script lang="ts">
  import '../app.css'
  import { onDestroy, onMount } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import type { Snippet } from 'svelte'
  import { api, initStores } from '$lib/tauri'
  import { initLogger } from '$lib/tauri/log'
  import { installIcons } from '$lib/icons'
  import { watchTheme } from '$lib/theme-mode'
  import { loadLocale, t } from '$lib/i18n.svelte'
  import { getLaunchDuration, getLaunchReady, getLaunchText, getReducedMotion, launchStep, putLaunchFrame, watchLaunch, watchWindowActivity, watchWindowDrag } from '$lib/window'
  import { loadAppearance, shell } from '$lib/appearance'
  import LockScreen from '$lib/components/LockScreen.svelte'

  interface Props {
    children?: Snippet
  }

  initLogger()
  installIcons()

  let { children }: Props = $props()
  let unsubscribeStores: (() => void) | undefined
  const unwatchTheme = watchTheme()
  let unwatchActivity: (() => void) | undefined
  let unwatchDrag: (() => void) | undefined
  const launch = getLaunchDuration(getReducedMotion())
  let grown = $state(false)
  const ready = $derived(grown && getLaunchReady($launchStep))
  const progress = $derived(getLaunchText($launchStep))
  let faded = $state(false)
  const stopLaunch = watchLaunch(() => {
    grown = true
    putLaunchFrame(() => api.putWindowFrame(launch), launch)
  })

  onMount(async () => {
    unwatchActivity = watchWindowActivity(window, document)
    unwatchDrag = watchWindowDrag(document, getCurrentWindow)
    loadAppearance().catch(() => {})
    loadLocale()
    unsubscribeStores = await initStores()
  })

  onDestroy(() => {
    unsubscribeStores?.()
    unwatchTheme()
    unwatchActivity?.()
    unwatchDrag?.()
    stopLaunch()
  })
</script>

<div class="relative h-full" style={$shell.style} style:--launch-duration="{launch}ms">
  {#if $shell.glass}
    <div class="bg-glass {$shell.backdropClass ?? ''}" style:background={$shell.backdrop}>
      {#each Array.from({ length: $shell.spots }, (_, index) => index) as spot (spot)}<span class="bg-glass-spot"></span>{/each}
    </div>
  {/if}
  <div class="relative h-full {ready ? 'launch' : 'pointer-events-none opacity-0'}" aria-hidden={!ready} data-testid="app-root" data-ready={ready}>
    <LockScreen>
      {@render children?.()}
    </LockScreen>
  </div>
  {#if !ready || (launch > 0 && !faded)}
    <div data-testid="launch-splash" class="launch-splash {ready ? 'launch-out' : ''}" role="status" onanimationend={(event) => event.animationName === 'launch-out' && (faded = true)}>
      <img src="/favicon.svg" alt="Chatnip" class="launch-logo pointer-events-none aspect-square w-[clamp(3rem,22%,5.5rem)] shrink-0" />
      <div class="pointer-events-none flex min-w-0 flex-col gap-1">
        <p class="text-[15px] font-semibold tracking-tight text-ink">{t('app.launch.title')}</p>
        {#if progress}<p class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[12.5px] text-ink-3">{t(progress)}</p>{/if}
      </div>
    </div>
  {/if}
</div>
