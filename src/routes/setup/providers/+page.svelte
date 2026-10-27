<script lang="ts">
  import { goto } from '$app/navigation'
  import Icon from '@iconify/svelte'
  import { api, providersStore, connectionStore, connectionReset } from '$lib/tauri'
  import type { MockClientKind, MockMode } from '$lib/tauri/types'
  import ConnectMicrosoft from './microsoft/ConnectMicrosoft.svelte'
  import ConnectSlack from './slack/ConnectSlack.svelte'
  import ConnectGoogle from './google/ConnectGoogle.svelte'
  import ConnectImap from './imap/ConnectImap.svelte'
  import { mockClients } from '$lib/components/settings/Connections.svelte'
  import { Button, IconButton } from '$lib/components/ui'
  import { setupReady } from '$lib/startup'
  import { t } from '$lib/i18n.svelte'

  let connected = $derived(($providersStore?.data || []).filter((p) => p.is_connected).length)
  let ready = $derived(setupReady($providersStore?.data || []))
  let mockMode = $state<MockMode | null>(null)
  let mockBusy = $state<MockClientKind | null>(null)
  let failure = $state<string | null>(null)

  $effect(() => {
    api.getMockMode().then((mode) => (mockMode = mode)).catch(() => {})
  })

  async function addMock(kind: MockClientKind) {
    mockBusy = kind
    try {
      await api.postMockConnection(kind)
      failure = null
    } catch (error) {
      failure = String(error)
    } finally {
      mockBusy = null
    }
  }

  async function continueToMain() {
    if (ready) await goto('/setup/intelligence')
  }
</script>

<div class="mx-auto flex h-full max-w-[34rem] flex-col justify-center px-10 py-8">
  <h1 class="text-[28px] font-semibold leading-tight tracking-tight text-ink">{t('setup.providers.title')}</h1>
  <p class="mt-2 text-[14px] text-ink-2">
    {t('setup.providers.intro')}
  </p>

  <div class="mt-8 border-t border-line">
    <ConnectMicrosoft />
    <ConnectGoogle />
    <ConnectSlack />
    <ConnectImap />
  </div>

  {#if mockMode?.enabled}
    <div class="mt-6" data-testid="setup-test-clients">
      <p class="text-[13px] font-medium text-ink-2">{t('settings.connections.testClients')}</p>
      <p class="text-[12px] text-ink-3">{t('setup.providers.testClientsHint')}</p>
      <ul class="mt-2 border-t border-line">
        {#each mockClients as client (client.kind)}
          <li class="grid h-10 grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-3 border-b border-line">
            <Icon icon="lucide:flask-conical" class="text-[15px] text-ink-3" />
            <span class="truncate text-[14px] text-ink">{client.label}</span>
            <IconButton size="sm" icon="lucide:plus" label={t('settings.connections.connectClient', { name: client.label })} testid="setup-mock-{client.kind}" busy={mockBusy === client.kind} disabled={mockBusy !== null} onclick={() => addMock(client.kind)} />
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if failure}
    <p class="mt-4 flex items-center gap-2 text-[13px] text-error-500">
      <span class="min-w-0 flex-1 break-words">{failure}</span>
      <Button size="sm" onclick={() => (failure = null)}>{t('common.dismiss')}</Button>
    </p>
  {/if}
  {#if $connectionStore.phase === 'failed'}
    <p class="mt-4 flex items-center gap-2 text-[13px] text-error-500">
      <span class="min-w-0 flex-1 break-words">{$connectionStore.message}</span>
      <Button size="sm" onclick={connectionReset}>{t('common.dismiss')}</Button>
    </p>
  {:else if $connectionStore.phase === 'connected'}
    <p class="mt-4 text-[13px] text-success-500">{t('setup.providers.connected', { name: $connectionStore.message ?? '' })}</p>
  {/if}

  <div class="mt-8 flex items-center gap-3">
    <Button variant="primary" keys="↵" testid="setup-providers-continue" disabled={!ready} onclick={continueToMain}>{t('common.continue')}</Button>
    <span class="font-mono text-[11px] text-ink-3">{t('setup.step', { step: 2, total: 3 })} · {ready ? t('setup.providers.connectedCount', { count: connected }) : t('setup.providers.needsAccount')}</span>
  </div>
</div>
