<script lang="ts">
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { api, providersStore, presenceStore, updatePresence } from '$lib/tauri'
  import type { Provider } from '$lib/tauri/types'
  import { accountAvatar, accountHealth, accountName, lastUsedAccount, type AccountHealth } from '$lib/accounts'
  import { anchored, boxOf } from '$lib/popover'
  import { modalsShow } from '$lib/modals'
  import { providerLabel } from '$lib/providerIcons'
  import { settingHref } from '$lib/settings'
  import { myUserIds, peopleStore, personProfileHref } from '$lib/tauri/people'
  import { Avatar, IconButton } from '$lib/components/ui'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import PresenceDot from '$lib/components/PresenceDot.svelte'
  import EmojiGlyph from '$lib/components/shared/EmojiGlyph.svelte'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  interface Props {
    class?: string
  }

  let { class: className = '' }: Props = $props()

  const HEALTH: Record<AccountHealth, MessageKey> = { ok: 'app.rail.healthy', reconsent: 'settings.connections.reconsent', disconnected: 'settings.connections.disconnected' }
  const TONE: Record<AccountHealth, string> = { ok: 'text-ink-3', reconsent: 'text-warn-600', disconnected: 'text-error-500' }

  const providers = $derived($providersStore.data ?? [])
  const current = $derived(lastUsedAccount(providers))
  const ordered = $derived(current ? [current, ...providers.filter((provider) => provider.id !== current.id)] : providers)
  const name = $derived(current ? accountName(current) : t('app.rail.noAccount'))

  let open = $state(false)
  let button = $state<HTMLElement | null>(null)

  $effect(() => {
    if (!open) return
    for (const provider of providers) {
      const user = provider.account?.user_id
      if (!user) continue
      api.getUserStatus({ user_id: user }).then((status) => updatePresence(user, { status_text: status.status_text, status_emoji: status.status_emoji, status_expiration: status.status_expiration })).catch(() => {})
    }
  })

  function settable(provider: Provider): boolean {
    return provider.is_connected && provider.capabilities.supports_presence && !!provider.account
  }

  function setStatus(provider: Provider) {
    const presence = provider.account ? $presenceStore.get(provider.account.user_id) : undefined
    open = false
    modalsShow({ initial_text: presence?.status_text ?? '', initial_emoji: presence?.status_emoji ?? '', provider_id: provider.id })
  }

  function openProfile() {
    open = false
    const ids = new Set([...$myUserIds, ...providers.flatMap((provider) => (provider.account?.user_id ? [provider.account.user_id] : []))])
    const me = $peopleStore.find((person) => person.identities.some((identity) => !!identity.user_id && ids.has(identity.user_id)))
    goto(me ? personProfileHref(me.id) : settingHref('accounts.connections'))
  }

</script>

<div class={className}>
  <button type="button" class="col-span-2 grid h-full cursor-pointer grid-cols-[var(--icon-cell)_minmax(0,1fr)] items-center text-left" data-testid="rail-account-profile" onclick={openProfile} title={t('inbox.row.openProfile', { name })}>
    <span class="flex items-center justify-center">
      {#if current}
        <Avatar source={accountAvatar(current)} size="sm" />
      {:else}
        <Icon icon="lucide:user-round" class="text-[15px] text-ink-3" />
      {/if}
    </span>
    <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-medium text-ink" data-testid="rail-account-name">{name}</span>
  </button>
  <button bind:this={button} type="button" class="flex h-6 cursor-pointer items-center gap-0.5 rounded-[0.4rem] px-1 text-ink-3 hover:bg-paper-3 hover:text-ink" data-testid="rail-account" onclick={() => (open = !open)} aria-haspopup="dialog" aria-expanded={open} aria-label={t('app.rail.accountButton', { name })} title={current ? `${name} · ${current.name}` : name}>
    {#if current}
      <ProviderIcon providers={[current]} size="xs" max={1} />
    {/if}
    <Icon icon="lucide:chevron-up" class="text-[11px]" />
  </button>
</div>

{#if open}
  <div class="menu flex w-[min(21rem,90vw)] flex-col" use:anchored={{ at: () => boxOf(button), side: 'above', align: 'start', onclose: () => (open = false) }} role="dialog" aria-label={t('app.rail.accounts')} data-testid="rail-account-menu">
    <p class="px-2.5 pb-1 pt-1 text-[11.5px] text-ink-3">{t('app.rail.accounts')}</p>
    <ul class="flex flex-col gap-px">
      {#each ordered as provider (provider.id)}
        {@const health = accountHealth(provider)}
        {@const user = provider.account?.user_id}
        {@const presence = user ? $presenceStore.get(user) : undefined}
        <li class="grid grid-cols-[1.75rem_minmax(0,1fr)_auto] items-center gap-2 rounded-[0.5rem] px-1.5 py-1.5 hover:bg-paper-3" data-testid="rail-account-item" data-provider={provider.id} data-health={health}>
          <span class="relative flex">
            <Avatar source={accountAvatar(provider)} size="md" />
            {#if user && provider.capabilities.supports_presence}
              <span class="absolute -bottom-0.5 -right-0.5 rounded-full ring-2 ring-paper"><PresenceDot user_id={user} size="sm" /></span>
            {/if}
          </span>
          <span class="flex min-w-0 flex-col">
            <span class="flex min-w-0 items-center gap-1.5">
              <span class="truncate text-[13.5px] font-medium text-ink">{accountName(provider)}</span>
              {#if provider.id === current?.id}
                <span class="shrink-0 text-[11px] text-ink-3">{t('app.rail.lastUsed')}</span>
              {/if}
            </span>
            <span class="flex min-w-0 items-center gap-1.5 text-[12px] text-ink-3">
              <ProviderIcon providers={[provider]} size="xs" max={1} />
              <span class="truncate">{provider.name} · {providerLabel(provider)}</span>
            </span>
            <span class="flex min-w-0 items-center gap-1 text-[12px] {TONE[health]}">
              <span class="shrink-0">{t(HEALTH[health])}</span>
              {#if presence?.status_text || presence?.status_emoji}
                <span class="truncate text-ink-3">· {#if presence.status_emoji}<EmojiGlyph emoji={presence.status_emoji} providerId={provider.id} />{/if} {presence.status_text}</span>
              {/if}
            </span>
          </span>
          {#if settable(provider)}
            <IconButton size="sm" icon="lucide:smile-plus" testid="rail-account-status" label={t('app.profilePopover.setStatus')} onclick={() => setStatus(provider)} />
          {/if}
        </li>
      {:else}
        <li class="px-2.5 py-2 text-[13px] text-ink-3">{t('app.rail.noAccount')}</li>
      {/each}
    </ul>
  </div>
{/if}
