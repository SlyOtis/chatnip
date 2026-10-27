<script lang="ts">
  import Icon from '@iconify/svelte'
  import { onMount } from 'svelte'
  import type { Snippet } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { api } from '$lib/tauri'
  import type { LockState } from '$lib/tauri/types'
  import { Button } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'
  import { putLaunchStep } from '$lib/window'

  interface Props {
    children?: Snippet
  }

  let { children }: Props = $props()

  const IDLE_CHECK_MS = 15_000
  const READY_RETRIES = 20

  let lockState = $state<LockState | null>(null)
  let busy = $state(false)
  let failure = $state<string | null>(null)
  let lastActivity = Date.now()
  let reloading = false

  function reload() {
    if (reloading) return
    reloading = true
    location.reload()
  }

  async function readState(): Promise<LockState> {
    for (let attempt = 0; attempt < READY_RETRIES; attempt++) {
      try {
        return await api.getLockState()
      } catch {
        await new Promise((resolve) => setTimeout(resolve, 250))
      }
    }
    return { enabled: false, locked: false, idle_minutes: 0 }
  }

  async function unlock() {
    busy = true
    failure = null
    try {
      const next = await api.deleteLock()
      if (!next.locked) reload()
      lockState = next
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }

  onMount(() => {
    let stop: (() => void) | undefined
    putLaunchStep('unlocking')
    readState().then((initial) => {
      lockState = initial
      if (initial.locked) {
        putLaunchStep('locked')
        unlock()
      }
    })
    listen<LockState>('lock-changed', (event) => {
      const wasLocked = lockState?.locked
      lockState = event.payload
      if (wasLocked && !event.payload.locked) reload()
    }).then((unlisten) => (stop = unlisten))
    const touch = () => (lastActivity = Date.now())
    const shortcut = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.metaKey && event.key.toLowerCase() === 'l') {
        event.preventDefault()
        api.postLock().catch(() => {})
      }
    }
    const timer = setInterval(() => {
      const current = lockState
      if (!current?.enabled || current.locked || current.idle_minutes <= 0) return
      if (Date.now() - lastActivity >= current.idle_minutes * 60_000) api.postLock().catch(() => {})
    }, IDLE_CHECK_MS)
    window.addEventListener('pointermove', touch, { passive: true })
    window.addEventListener('keydown', touch)
    window.addEventListener('keydown', shortcut)
    return () => {
      stop?.()
      clearInterval(timer)
      window.removeEventListener('pointermove', touch)
      window.removeEventListener('keydown', touch)
      window.removeEventListener('keydown', shortcut)
    }
  })
</script>

{#if lockState && !lockState.locked}
  {@render children?.()}
{:else}
  <div class="fixed inset-0 z-[1000] flex flex-col items-center justify-center gap-5 bg-paper text-center">
    {#if lockState?.locked}
      <span class="flex size-12 items-center justify-center rounded-[0.875rem] bg-paper-3 text-ink-2">
        <Icon icon="lucide:lock" class="text-[22px]" />
      </span>
      <div class="flex flex-col gap-1">
        <p class="text-[15px] font-semibold text-ink">{t('app.lock.title')}</p>
        <p class="text-[13px] text-ink-3">{t('app.lock.hint')}</p>
      </div>
      <Button variant="primary" icon="lucide:fingerprint" busy={busy} onclick={unlock}>{t('app.lock.unlock')}</Button>
      {#if failure}
        <p class="max-w-[40ch] text-[13px] text-ink-3">{failure}</p>
      {/if}
    {/if}
  </div>
{/if}
