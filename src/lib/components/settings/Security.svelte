<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '$lib/tauri'
  import type { LockState } from '$lib/tauri/types'
  import { Button, Row, Section, Select, Switch } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  let lock = $state<LockState | null>(null)
  let busy = $state(false)
  let failure = $state<string | null>(null)

  const idleOptions = $derived([
    { value: '0', label: t('common.never') },
    { value: '5', label: t('common.minutes', { count: 5 }) },
    { value: '10', label: t('common.minutes', { count: 10 }) },
    { value: '30', label: t('common.minutes', { count: 30 }) },
    { value: '60', label: t('settings.messages.hoursShort', { count: 1 }) },
  ])

  onMount(async () => {
    try {
      lock = await api.getLockState()
    } catch (error) {
      failure = String(error)
    }
  })

  async function put(enabled: boolean, idle?: number) {
    busy = true
    try {
      lock = await api.putLock(enabled, idle)
      failure = null
    } catch (error) {
      failure = String(error)
      lock = await api.getLockState().catch(() => lock)
    } finally {
      busy = false
    }
  }
</script>

<Section title={t('settings.security.title')} description={t('settings.security.description')}>
  <Row id="privacy.touch-id" label={t('settings.security.touchId')} hint={t('settings.security.touchIdHint')}>
    <Switch label={t('settings.security.touchId')} checked={lock?.enabled ?? false} disabled={busy || !lock} onchange={(on) => put(on)} />
  </Row>
  <Row id="privacy.idle" label={t('settings.security.idle')} hint={t('settings.security.idleHint')}>
    <Select size="sm" class="w-[clamp(9rem,30%,13rem)]" testid="privacy-idle" dialog={t('settings.security.idle')} value={String(lock?.idle_minutes ?? 10)} options={idleOptions} onchange={(value) => lock && put(lock.enabled, Number(value))} />
  </Row>
  {#if lock?.enabled}
    <Row id="privacy.lock-now" label={t('settings.security.lockNow')} hint={t('settings.security.lockNowHint', { keys: '⌃⌘L' })}>
      <Button size="sm" disabled={busy} onclick={() => api.postLock()}>{t('settings.security.lock')}</Button>
    </Row>
  {/if}
  {#if failure}
    <p class="py-2 text-[13px] text-error-600">{failure}</p>
  {/if}
</Section>
