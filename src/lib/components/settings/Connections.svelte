<script module lang="ts">
  import type { MockClientKind } from '$lib/tauri/types';
  import type { MessageKey } from '$lib/i18n.svelte';

  export type Light = 'green' | 'amber' | 'red' | 'grey';
  export const LIGHT: Record<Light, string> = { green: 'bg-success-500', amber: 'bg-warn-500', red: 'bg-error-500', grey: 'bg-ink-3' };

  export const mockClients: { kind: MockClientKind; label: string; help: MessageKey }[] = [
    { kind: 'teams', label: 'Teams', help: 'settings.connections.mock.teams' },
    { kind: 'outlook', label: 'Outlook', help: 'settings.connections.mock.outlook' },
    { kind: 'slack', label: 'Slack', help: 'settings.connections.mock.slack' },
    { kind: 'gmail', label: 'Gmail', help: 'settings.connections.mock.gmail' },
    { kind: 'google_chat', label: 'Google Chat', help: 'settings.connections.mock.googleChat' },
    { kind: 'imap', label: 'IMAP', help: 'settings.connections.mock.imap' }
  ];
</script>

<script lang="ts">
  import Icon from '@iconify/svelte';
  import { onMount } from 'svelte';
  import { api, providersStore, connectionStore, connectionReset, connectionWaiting, connectionFailed } from '$lib/tauri';
  import type { MockMode, MockServerState, Provider } from '$lib/tauri/types';
  import { accountHealth } from '$lib/accounts';
  import { providerIcon, providerLabel } from '$lib/providerIcons';
  import AssetIcon from '$lib/components/shared/AssetIcon.svelte';
  import { Button, IconButton, Row, Section, Select, Switch } from '$lib/components/ui';
  import ConnectMicrosoft from '../../../routes/setup/providers/microsoft/ConnectMicrosoft.svelte';
  import ConnectGoogle from '../../../routes/setup/providers/google/ConnectGoogle.svelte';
  import ConnectSlack from '../../../routes/setup/providers/slack/ConnectSlack.svelte';
  import ConnectImap from '../../../routes/setup/providers/imap/ConnectImap.svelte';
  import AppRegistrations from './AppRegistrations.svelte';
  import AutoReply from './AutoReply.svelte';
  import { formatDate, t } from '$lib/i18n.svelte';
  import type { SettingField } from '$lib/settings';
  import SettingFields from './SettingFields.svelte';

  interface Props {
    showCredentials?: boolean;
  }

  let { showCredentials = true }: Props = $props();

  const providers = $derived($providersStore.data ?? []);
  const replying = $derived(providers.filter((p) => p.is_connected && p.capabilities.supports_auto_reply));
  const connection = $derived($connectionStore);

  let settings = $state<Record<string, string>>({});
  let busy = $state<Record<string, boolean>>({});
  let failure = $state<string | null>(null);
  let mockBusy = $state<string | null>(null);

  let mockMode = $state<MockMode | null>(null);
  const mockProviders = $derived(providers.filter((p) => p.provider_type === 'mock'));
  const mockStates: Record<MockServerState, MessageKey> = {
    ok: 'settings.connections.mockState.ok',
    expired: 'settings.connections.mockState.expired',
    rate_limited: 'settings.connections.mockState.rate_limited',
    offline: 'settings.connections.mockState.offline',
    denied: 'settings.connections.mockState.denied'
  };
  const mockOptions = $derived(Object.entries(mockStates).map(([value, label]) => ({ value, label: t(label as MessageKey) })));
  const mockFields: SettingField[] = [
    { key: 'mock.reply_seconds', anchor: 'accounts.mock-replies', label: 'settings.connections.mockReplies', hint: 'settings.connections.mockRepliesHint' },
    { key: 'mock.newsletter_minutes', anchor: 'accounts.mock-newsletters', label: 'settings.connections.mockNewsletters', hint: 'settings.connections.mockNewslettersHint' },
  ];

  async function putMockState(provider: Provider, state: string) {
    busy[provider.id] = true;
    try {
      mockMode = await api.putMockState(provider.id, state as MockServerState);
      failure = null;
    } catch (error) {
      failure = String(error);
    } finally {
      busy[provider.id] = false;
    }
  }
  const mockEnabled = $derived(mockMode?.forced ?? settings['mock.enabled'] === 'true');

  $effect(() => {
    settings['mock.enabled'];
    mockProviders.length;
    api.getMockMode().then((mode) => (mockMode = mode)).catch(() => {});
  });

  async function toggleMock() {
    try {
      settings = await api.putAppSetting('mock.enabled', mockEnabled ? 'false' : 'true');
      failure = null;
    } catch (error) {
      failure = String(error);
    }
  }

  async function addMock(kind: MockClientKind) {
    mockBusy = kind;
    try {
      await api.postMockConnection(kind);
      failure = null;
    } catch (error) {
      failure = String(error);
    } finally {
      mockBusy = null;
    }
  }

  const syncFields: SettingField[] = [
    { key: 'sync.interval_seconds', anchor: 'accounts.sync-interval', label: 'settings.connections.syncInterval', hint: 'settings.connections.syncIntervalHint' },
    { key: 'sync.live_interval_seconds', anchor: 'accounts.live-interval', label: 'settings.connections.liveInterval', hint: 'settings.connections.liveIntervalHint' },
    { key: 'sync.history_days', anchor: 'accounts.history-days', label: 'settings.connections.historyDays', hint: 'settings.connections.historyDaysHint' },
    { key: 'emoji.sync_interval_seconds', anchor: 'accounts.emoji-interval', label: 'settings.connections.emojiInterval', hint: 'settings.connections.emojiIntervalHint' },
    { key: 'directory.sync_interval_seconds', anchor: 'accounts.directory-interval', label: 'settings.connections.directoryInterval', hint: 'settings.connections.directoryIntervalHint' },
  ];

  onMount(async () => {
    try {
      settings = await api.getAppSettings();
    } catch (error) {
      failure = String(error);
    }
  });


  async function syncNow(provider: Provider) {
    busy[provider.id] = true;
    try {
      await api.postProviderSync(provider.id);
      failure = null;
    } catch (error) {
      failure = String(error);
    } finally {
      busy[provider.id] = false;
    }
  }

  const BROWSER_AUTH = new Set(['microsoft', 'google', 'slack']);

  function needsConsent(provider: Provider): boolean {
    return provider.is_connected && (provider.missing_scopes?.length ?? 0) > 0;
  }

  async function reconsent(provider: Provider) {
    connectionWaiting(provider.provider_type);
    try {
      await api.postProviderAuth(provider.provider_type);
    } catch (error) {
      connectionFailed(String(error));
    }
  }

  async function disconnect(provider: Provider) {
    if (!confirm(t('settings.connections.disconnectConfirm', { name: provider.name, service: providerLabel(provider) }))) return;
    busy[provider.id] = true;
    try {
      await api.deleteProvider(provider.id);
      failure = null;
    } catch (error) {
      failure = String(error);
    } finally {
      busy[provider.id] = false;
    }
  }

  function trafficLight(provider: Provider): 'green' | 'amber' | 'red' {
    const health = accountHealth(provider)
    if (health === 'disconnected') return 'red'
    if (health === 'reconsent') return 'amber'
    const synced = provider.last_sync_at ? Date.parse(provider.last_sync_at) : NaN
    return Number.isFinite(synced) && Date.now() - synced < 30 * 60 * 1000 ? 'green' : 'amber'
  }

  function lightTitle(provider: Provider, light: 'green' | 'amber' | 'red'): string {
    if (light === 'red') return t('settings.connections.disconnected')
    if (light === 'amber') return needsConsent(provider) ? t('settings.connections.reconsent') : t('settings.connections.stale')
    return t('app.rail.healthy')
  }

  function relative(timestamp: string | null): string {
    if (!timestamp) return t('settings.connections.syncedNever');
    const diff = Date.now() - new Date(timestamp).getTime();
    const minutes = Math.floor(diff / 60000);
    if (minutes < 1) return t('settings.connections.syncedJustNow');
    if (minutes < 60) return t('settings.connections.syncedMinutes', { count: minutes });
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return t('settings.connections.syncedHours', { count: hours });
    return t('settings.connections.syncedOn', { date: formatDate(timestamp, { day: 'numeric', month: 'short' }) });
  }
</script>

<div>
  {#if connection.phase === 'waiting'}
    <div class="flex items-center gap-3 border-b border-line py-3">
      <Icon icon="lucide:loader-circle" class="animate-spin text-[15px] text-accent" />
      <div class="flex-1">
        <p class="text-[14px] font-medium text-ink">{t('settings.connections.waiting')}</p>
        <p class="text-[13px] text-ink-3">{t('settings.connections.waitingHint')}</p>
      </div>
      <Button size="sm" onclick={connectionReset}>{t('common.cancel')}</Button>
    </div>
  {:else if connection.phase === 'connected'}
    <div class="flex items-center gap-3 border-b border-line py-3">
      <Icon icon="lucide:circle-check" class="text-[15px] text-success-500" />
      <p class="flex-1 text-[14px] text-ink">{t('settings.connections.connected', { name: connection.message ?? '' })}</p>
      <Button size="sm" onclick={connectionReset}>{t('settings.connections.ok')}</Button>
    </div>
  {:else if connection.phase === 'failed'}
    <div class="flex items-center gap-3 border-b border-line py-3">
      <Icon icon="lucide:circle-alert" class="text-[15px] text-error-500" />
      <p class="flex-1 text-[14px] text-ink">{connection.message}</p>
      <Button size="sm" onclick={connectionReset}>{t('common.dismiss')}</Button>
    </div>
  {/if}

  {#if failure}
    <div data-testid="connections-failure" class="flex items-center gap-3 border-b border-line py-3">
      <Icon icon="lucide:circle-alert" class="text-[15px] text-error-500" />
      <p class="flex-1 text-[13px] text-ink">{failure}</p>
      <Button size="sm" onclick={() => (failure = null)}>{t('common.dismiss')}</Button>
    </div>
  {/if}

  <Section id="accounts.connections" title={t('settings.tab.connections')} description={t('settings.connections.description')} total={providers.length}>
    {#snippet children(shown)}
    {#if providers.length === 0}
      <p class="py-6 text-[13px] text-ink-3">{t('settings.connections.empty')}</p>
    {:else}
      <ul>
        {#each providers.slice(0, shown) as provider (provider.id)}
          {@const light = trafficLight(provider)}
          <li data-testid="provider-row" data-provider={provider.id} data-connected={provider.is_connected} class="group grid h-12 grid-cols-[0.5rem_1.5rem_minmax(0,1fr)_auto] items-center gap-3 border-b border-line last:border-b-0">
            <span data-testid="provider-light" data-light={light} class="size-2 rounded-full {LIGHT[light]}" title={lightTitle(provider, light)}></span>
            <span class="relative flex size-6 items-center justify-center overflow-hidden rounded-full text-ink-3">
              {#if provider.logo}
                <AssetIcon asset={provider.logo} size={24} class="size-6 object-cover grayscale" fallbackIcon={providerIcon(provider)} />
              {:else}
                <Icon icon={providerIcon(provider)} class="text-[15px] grayscale" />
              {/if}
            </span>
            <div class="flex min-w-0 items-baseline gap-2">
              <span class="truncate text-[14px] font-medium text-ink">{provider.name}</span>
              {#if providerLabel(provider) !== provider.name || provider.namespace}
                <span class="truncate text-[13px] text-ink-3">{providerLabel(provider) !== provider.name ? providerLabel(provider) : ''}{#if provider.namespace}{providerLabel(provider) !== provider.name ? ' · ' : ''}{provider.namespace}{/if}</span>
              {/if}
              <span class="shrink-0 font-mono text-[11px] text-ink-3">{provider.is_connected ? relative(provider.last_sync_at) : t('settings.connections.disconnected')}</span>
              {#if needsConsent(provider)}
                <span class="shrink-0 text-[12px] font-medium text-warn-600" title={provider.missing_scopes.join(' ')} data-reconsent={provider.id}>{t('settings.connections.reconsent')}</span>
              {/if}
            </div>
            <div class="flex items-center gap-0.5 {needsConsent(provider) ? '' : 'opacity-0'} group-hover:opacity-100 focus-within:opacity-100">
              {#if needsConsent(provider) && BROWSER_AUTH.has(provider.provider_type)}
                <IconButton size="sm" icon="lucide:key-round" label={t('settings.connections.reconsentAction')} disabled={busy[provider.id] || connection.phase === 'waiting'} onclick={() => reconsent(provider)} />
              {/if}
              <IconButton size="sm" testid="provider-sync-{provider.id}" icon="lucide:refresh-cw" label={t('settings.connections.syncNow')} busy={busy[provider.id]} onclick={() => syncNow(provider)} />
              <IconButton size="sm" icon="lucide:unplug" label={t('common.disconnect')} tone="danger" disabled={busy[provider.id]} onclick={() => disconnect(provider)} />
            </div>
          </li>
          {#if needsConsent(provider)}
            <li class="border-b border-line pb-2 pl-[3.75rem] text-[12px] text-ink-3 last:border-b-0">{t('settings.connections.reconsentHint', { scopes: provider.missing_scopes.join(', ') })}</li>
          {/if}
        {/each}
      </ul>
    {/if}
    <div class="mt-4 border-t border-line pt-4">
      <p class="pb-1 text-[12.5px] text-ink-3">{t('settings.connections.addDescription')}</p>
      <ConnectMicrosoft />
      <ConnectGoogle />
      <ConnectSlack />
      <ConnectImap />
    </div>
    {/snippet}
  </Section>

  {#if replying.length}
    <Section id="accounts.auto-replies" title={t('settings.autoReply.title')} description={t('settings.autoReply.description')}>
      {#each replying as provider (provider.id)}
        <AutoReply {provider} />
      {/each}
    </Section>
  {/if}

  <Section id="accounts.test-clients" title={t('settings.connections.testClients')} description={t('settings.connections.testClientsDescription')}>
    {#snippet actions()}
      {#if mockMode?.forced != null}
        <span class="inline-flex items-center gap-1.5 font-mono text-[11.5px] text-ink-3"><Icon icon="lucide:lock" class="text-[12px]" />{t('settings.connections.forced', { value: `CHATNIP_USE_DEBUG=${mockMode.forced ? 'true' : 'false'}` })}</span>
      {:else}
        <Switch label={t('settings.connections.enableTestClients')} checked={mockEnabled} onchange={toggleMock} />
      {/if}
    {/snippet}
    {#if mockEnabled}
      <ul>
        {#each mockClients as client (client.kind)}
          <li class="grid h-12 grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-3 border-b border-line last:border-b-0">
            <Icon icon="lucide:flask-conical" class="text-[15px] text-ink-3" />
            <div class="flex min-w-0 items-baseline gap-2">
              <span class="shrink-0 text-[14px] font-medium text-ink">{client.label}</span>
              <span class="truncate text-[13px] text-ink-3">{t(client.help)}</span>
            </div>
            <IconButton size="sm" icon="lucide:plus" label={t('settings.connections.connectClient', { name: client.label })} busy={mockBusy === client.kind} disabled={mockBusy !== null} onclick={() => addMock(client.kind)} />
          </li>
        {/each}
      </ul>
      {#if mockProviders.length}
        <div id="accounts.mock-state">
          {#each mockProviders as provider (provider.id)}
            <Row label={`${t('settings.connections.mockState')} · ${provider.name}`} hint={t('settings.connections.mockStateHint')}>
              <Select size="sm" class="w-[clamp(9rem,30%,13rem)]" testid="mock-state-{provider.id}" dialog={`${t('settings.connections.mockState')} · ${provider.name}`} value={mockMode?.states[provider.id] ?? 'ok'} options={mockOptions} onchange={(state) => putMockState(provider, state)} />
            </Row>
          {/each}
        </div>
      {/if}
      <SettingFields fields={mockFields} />
    {/if}
  </Section>

  {#if showCredentials}
    <Section id="accounts.registrations" title={t('settings.connections.registrations')} description={t('settings.connections.registrationsDescription')}>
      <AppRegistrations />
    </Section>

    <Section title={t('settings.connections.sync')}>
      <SettingFields fields={syncFields} />
    </Section>
  {/if}
</div>
