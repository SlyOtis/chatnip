<script lang="ts">
  import Icon from '@iconify/svelte'
  import { api, providersStore, connectionStore, connectionWaiting, connectionFailed } from '$lib/tauri'
  import { IconButton } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  let providers = $derived($providersStore?.data || [])
  let connectedCount = $derived(providers.filter((p) => p.provider_type === 'microsoft' && p.is_connected).length)
  let isConnecting = $derived($connectionStore.phase === 'waiting' && $connectionStore.providerType === 'microsoft')

  async function handleClick() {
    if (isConnecting) return
    connectionWaiting('microsoft')
    try {
      await api.postProviderAuth('microsoft')
    } catch (error) {
      connectionFailed(String(error))
    }
  }
</script>

<div class="grid h-12 grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-3 border-b border-line last:border-b-0">
  <Icon icon="logos:microsoft-icon" class="text-[15px] grayscale" />
  <div class="flex min-w-0 items-baseline gap-2">
    <span class="shrink-0 text-[14px] font-medium text-ink">Microsoft 365</span>
    <span class="truncate text-[13px] text-ink-3">
      {#if isConnecting}
        {t('setup.providers.waiting')}
      {:else if connectedCount > 0}
        {t('setup.providers.microsoft.connected', { count: connectedCount })}
      {:else}
        {t('setup.providers.microsoft.hint')}
      {/if}
    </span>
  </div>
  <IconButton size="sm" icon="lucide:plus" label={t('setup.providers.microsoft.connect')} busy={isConnecting} onclick={handleClick} />
</div>
