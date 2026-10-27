<script lang="ts">
  import Icon from '@iconify/svelte'
  import { onMount } from 'svelte'
  import { api, providersStore, connectionStore, connectionConnected, connectionFailed, connectionWaiting } from '$lib/tauri'
  import { Button, Field, IconButton } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  let providers = $derived($providersStore?.data || [])
  let connectedCount = $derived(providers.filter((p) => p.provider_type === 'slack' && p.is_connected).length)
  let open = $state(false)
  let token = $state('')
  let submitting = $state(false)
  let registered = $state(false)
  let isConnecting = $derived($connectionStore.phase === 'waiting' && $connectionStore.providerType === 'slack')

  onMount(async () => {
    registered = (await api.getConnectionSetup().catch(() => [])).some((s) => s.provider_type === 'slack' && s.ready)
  })

  async function signIn() {
    if (isConnecting) return
    connectionWaiting('slack')
    try {
      await api.postProviderAuth('slack')
    } catch (error) {
      connectionFailed(String(error))
    }
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    if (submitting) return
    submitting = true
    try {
      const connected = await api.postSlackConnection(token)
      connectionConnected(connected.name)
      token = ''
      open = false
    } catch (error) {
      connectionFailed(String(error))
    } finally {
      submitting = false
    }
  }
</script>

<div class="border-b border-line last:border-b-0">
  <div class="grid h-12 grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-3">
    <Icon icon="logos:slack-icon" class="text-[15px] grayscale" />
    <div class="flex min-w-0 items-baseline gap-2">
      <span class="shrink-0 text-[14px] font-medium text-ink">Slack</span>
      <span class="truncate text-[13px] text-ink-3">
        {#if isConnecting}
          {t('setup.providers.waiting')}
        {:else if connectedCount > 0}
          {t('setup.providers.slack.connected', { count: connectedCount })}
        {:else}
          {t('setup.providers.slack.hint')}
        {/if}
      </span>
    </div>
    {#if registered}
      <span class="flex items-center gap-0.5">
        <IconButton size="sm" icon="lucide:key-round" label={t('setup.providers.slack.pasteToken')} active={open} onclick={() => (open = !open)} />
        <IconButton size="sm" icon="lucide:plus" label={t('setup.providers.slack.connectBrowser')} busy={isConnecting} onclick={signIn} />
      </span>
    {:else}
      <IconButton size="sm" icon={open ? 'lucide:chevron-up' : 'lucide:plus'} label={t('setup.providers.slack.connectToken')} active={open} onclick={() => (open = !open)} />
    {/if}
  </div>
  {#if open}
    <form class="flex flex-col gap-3 pb-4 pl-9" onsubmit={submit}>
      <p class="max-w-[60ch] text-[13px] leading-5 text-ink-3">
        {registered ? t('setup.providers.slack.orPaste') : t('setup.providers.slack.noApp')} {t('setup.providers.slack.tokenStays')}
      </p>
      <div class="flex items-center gap-2">
        <Field type="password" mono class="max-w-sm" placeholder="xoxp-…" bind:value={token} autocomplete="off" spellcheck="false" />
        <Button type="submit" variant="primary" busy={submitting} disabled={token.trim().length === 0}>{t('common.connect')}</Button>
        <Button onclick={() => (open = false)}>{t('common.cancel')}</Button>
      </div>
    </form>
  {/if}
</div>
