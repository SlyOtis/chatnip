<script lang="ts">
  import { api, providersStore } from '$lib/tauri'
  import type { ContentSignature, Provider, RemoteAlias, RenderedSignature } from '$lib/tauri/types'
  import { Button, IconButton, Select, Switch } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    contentId: string
    body: string
    onimported?: () => void
  }

  let { contentId, body, onimported }: Props = $props()

  const ANY = '*'

  let bindings = $state<ContentSignature[]>([])
  let aliases = $state<RemoteAlias[]>([])
  let account = $state('')
  let alias = $state(ANY)
  let preview = $state<RenderedSignature | null>(null)
  let busy = $state(false)
  let failure = $state<string | null>(null)

  const accounts = $derived(($providersStore.data ?? []).filter((provider) => provider.is_connected && provider.capabilities.supports_aliases))
  const accountOptions = $derived(accounts.map((provider) => ({ value: provider.id, label: provider.namespace ? `${provider.name} · ${provider.namespace}` : provider.name })))
  const aliasOptions = $derived([{ value: ANY, label: t('content.signature.anyAddress') }, ...aliases.map((entry) => ({ value: entry.email, label: entry.name ? `${entry.name} <${entry.email}>` : entry.email }))])
  const syncing = $derived(accounts.filter((provider) => provider.capabilities.signature_sync))

  function providerOf(id: string): Provider | undefined {
    return ($providersStore.data ?? []).find((provider) => provider.id === id)
  }

  async function load() {
    try {
      bindings = await api.getSignatures(contentId)
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  $effect(() => {
    contentId
    load()
  })

  $effect(() => {
    const id = account
    aliases = []
    alias = ANY
    if (!id) return
    let live = true
    api.getAliases(id).then((list) => live && (aliases = list)).catch(() => {})
    return () => {
      live = false
    }
  })

  $effect(() => {
    body
    const first = bindings[0]
    const provider = first?.provider_id ?? accounts[0]?.id
    if (!provider) {
      preview = null
      return
    }
    let live = true
    const timer = setTimeout(() => {
      api
        .getSignature({ provider_id: provider, alias: first?.alias || null, content_id: contentId })
        .then((rendered) => live && (preview = rendered))
        .catch((error) => live && (failure = String(error)))
    }, 250)
    return () => {
      live = false
      clearTimeout(timer)
    }
  })

  async function put(binding: ContentSignature) {
    busy = true
    try {
      bindings = (await api.putSignature(binding)).filter((entry) => entry.content_id === contentId)
      failure = null
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }

  async function add() {
    if (!account) return
    await put({ content_id: contentId, provider_id: account, alias: alias === ANY ? '' : alias, use_new: true, use_reply: true, use_forward: true })
    account = ''
  }

  async function remove(binding: ContentSignature) {
    busy = true
    try {
      await api.deleteSignature(binding.content_id, binding.provider_id, binding.alias)
      await load()
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }

  async function importFrom(provider: Provider) {
    busy = true
    try {
      await api.postSignatureImport(provider.id)
      onimported?.()
      await load()
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }
</script>

<section data-testid="signature-bindings" class="mt-4">
  <p class="text-[12px] text-ink-3">{t('content.signature.bindings')}</p>
  {#each bindings as binding (binding.provider_id + binding.alias)}
    {@const provider = providerOf(binding.provider_id)}
    <div data-testid="signature-binding" data-provider={binding.provider_id} data-alias={binding.alias} class="mt-1 border-b border-line py-1.5">
      <div class="flex min-w-0 items-center gap-2">
        <span class="min-w-0 flex-1 truncate text-[13px] text-ink">{provider?.name ?? binding.provider_id}</span>
        <span class="min-w-0 truncate font-mono text-[11.5px] text-ink-3">{binding.alias || t('content.signature.anyAddress')}</span>
        <IconButton size="sm" icon="lucide:x" tone="danger" testid="signature-binding-remove" label={t('content.signature.remove')} disabled={busy} onclick={() => remove(binding)} />
      </div>
      <div class="mt-1 flex flex-wrap items-center gap-x-4 gap-y-1 text-[12.5px] text-ink-2">
        <span class="inline-flex items-center gap-1.5"><Switch testid="signature-use-new" label={t('content.signature.useNew')} checked={binding.use_new} disabled={busy} onchange={(on) => put({ ...binding, use_new: on })} />{t('content.signature.useNew')}</span>
        <span class="inline-flex items-center gap-1.5"><Switch testid="signature-use-reply" label={t('content.signature.useReply')} checked={binding.use_reply} disabled={busy} onchange={(on) => put({ ...binding, use_reply: on })} />{t('content.signature.useReply')}</span>
        <span class="inline-flex items-center gap-1.5"><Switch testid="signature-use-forward" label={t('content.signature.useForward')} checked={binding.use_forward} disabled={busy} onchange={(on) => put({ ...binding, use_forward: on })} />{t('content.signature.useForward')}</span>
      </div>
      {#if provider?.capabilities.signature_sync}
        <p class="mt-1 text-[11.5px] text-ink-3">{t('content.signature.syncs', { name: provider.name })}</p>
      {/if}
    </div>
  {:else}
    <p class="mt-1 text-[12.5px] text-ink-3">{t('content.signature.unbound')}</p>
  {/each}

  {#if accounts.length}
    <div class="mt-2 grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] items-center gap-2">
      <Select size="sm" testid="signature-account" dialog={t('content.signature.account')} placeholder={t('content.signature.account')} bind:value={account} options={accountOptions} />
      <Select size="sm" testid="signature-alias" dialog={t('content.signature.alias')} bind:value={alias} options={aliasOptions} />
      <Button size="sm" testid="signature-bind" icon="lucide:plus" disabled={!account || busy} onclick={add}>{t('common.add')}</Button>
    </div>
  {:else}
    <p class="mt-1 text-[12.5px] text-ink-3">{t('content.signature.noAccounts')}</p>
  {/if}

  <p class="mt-3 text-[12px] text-ink-3">{t('content.signature.preview')}</p>
  <div data-testid="signature-preview" class="mt-1 rounded-[0.5rem] bg-paper-3 px-3 py-2 text-[13px] leading-5 whitespace-pre-wrap text-ink">-- {'\n'}{preview?.text || t('content.detail.previewEmpty')}</div>

  {#if syncing.length}
    <p class="mt-3 text-[12px] text-ink-3">{t('content.signature.import')}</p>
    <div class="mt-1 flex flex-wrap gap-1">
      {#each syncing as provider (provider.id)}
        <Button size="sm" variant="secondary" testid="signature-import" icon="lucide:download" disabled={busy} onclick={() => importFrom(provider)}>{t('content.signature.importFrom', { name: provider.name })}</Button>
      {/each}
    </div>
  {/if}

  {#if failure}<p data-testid="signature-failure" class="mt-2 text-[12.5px] text-error-600">{failure}</p>{/if}
</section>
