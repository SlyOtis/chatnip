<script lang="ts">
  import Icon from '@iconify/svelte'
  import { api, providersStore, connectionConnected, connectionFailed } from '$lib/tauri'
  import { Button, Field, IconButton } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  let providers = $derived($providersStore?.data || [])
  let connectedCount = $derived(providers.filter((p) => p.provider_type === 'imap' && p.is_connected).length)
  let open = $state(false)
  let submitting = $state(false)
  let email = $state('')
  let password = $state('')
  let displayName = $state('')
  let username = $state('')
  let imapHost = $state('')
  let imapPort = $state('993')
  let smtpHost = $state('')
  let smtpPort = $state('587')
  let aliases = $state('')

  function guessHosts() {
    const domain = email.split('@')[1]?.toLowerCase()
    if (!domain) return
    if (!imapHost) imapHost = `imap.${domain}`
    if (!smtpHost) smtpHost = `smtp.${domain}`
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    if (submitting) return
    submitting = true
    try {
      const input: Parameters<typeof api.postImapConnection>[0] = {
        email,
        password,
        display_name: displayName || undefined,
        username: username || undefined,
        imap_host: imapHost,
        imap_port: Number(imapPort) || undefined,
        smtp_host: smtpHost,
        smtp_port: Number(smtpPort) || undefined,
        aliases: aliases.split(/[\s,;]+/).map((alias) => alias.trim()).filter((alias) => alias.includes('@'))
      }
      const connected = await api.postImapConnection(input)
      connectionConnected(connected.name)
      password = ''
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
    <Icon icon="lucide:at-sign" class="text-[15px] text-ink-3" />
    <div class="flex min-w-0 items-baseline gap-2">
      <span class="shrink-0 text-[14px] font-medium text-ink">{t('setup.providers.imap.title')}</span>
      <span class="truncate text-[13px] text-ink-3">
        {#if connectedCount > 0}
          {t('setup.providers.imap.connected', { count: connectedCount })}
        {:else}
          {t('setup.providers.imap.hint')}
        {/if}
      </span>
    </div>
    <IconButton size="sm" icon={open ? 'lucide:chevron-up' : 'lucide:plus'} label={t('setup.providers.imap.connect')} active={open} onclick={() => (open = !open)} />
  </div>
  {#if open}
    <form class="grid grid-cols-2 gap-x-4 gap-y-3 pb-4 pl-9" onsubmit={submit}>
      <label class="col-span-2 flex flex-col gap-1 text-[13px] text-ink-2">
        {t('setup.providers.imap.address')}
        <Field type="email" mono bind:value={email} onblur={guessHosts} autocomplete="off" required />
      </label>
      <label class="col-span-2 flex flex-col gap-1 text-[13px] text-ink-2">
        {t('setup.providers.imap.password')}
        <Field type="password" mono bind:value={password} autocomplete="off" required />
      </label>
      <label class="flex flex-col gap-1 text-[13px] text-ink-2">
        {t('setup.providers.imap.displayName')}
        <Field bind:value={displayName} autocomplete="off" />
      </label>
      <label class="flex flex-col gap-1 text-[13px] text-ink-2">
        {t('setup.providers.imap.username')}
        <Field mono bind:value={username} autocomplete="off" />
      </label>
      <label class="flex flex-col gap-1 text-[13px] text-ink-2">
        {t('setup.providers.imap.imapHost')}
        <Field mono bind:value={imapHost} autocomplete="off" required />
      </label>
      <label class="flex flex-col gap-1 text-[13px] text-ink-2">
        {t('setup.providers.imap.imapPort')}
        <Field type="number" mono bind:value={imapPort} />
      </label>
      <label class="flex flex-col gap-1 text-[13px] text-ink-2">
        {t('setup.providers.imap.smtpHost')}
        <Field mono bind:value={smtpHost} autocomplete="off" required />
      </label>
      <label class="flex flex-col gap-1 text-[13px] text-ink-2">
        {t('setup.providers.imap.smtpPort')}
        <Field type="number" mono bind:value={smtpPort} />
      </label>
      <label class="col-span-2 flex flex-col gap-1 text-[13px] text-ink-2">
        {t('setup.providers.imap.aliases')}
        <textarea data-testid="imap-aliases" bind:value={aliases} rows="2" spellcheck="false" placeholder={t('setup.providers.imap.aliasesPlaceholder')} class="w-full resize-y border-b border-line bg-transparent px-2.5 py-1.5 font-mono text-[13px] text-ink outline-none placeholder:text-ink-3 focus:border-ink-3"></textarea>
        <span class="text-[12px] text-ink-3">{t('setup.providers.imap.aliasesHint')}</span>
      </label>
      <p class="col-span-2 text-[12px] text-ink-3">{t('setup.providers.imap.security')}</p>
      <div class="col-span-2 flex items-center gap-2">
        <Button type="submit" variant="primary" busy={submitting}>{t('common.connect')}</Button>
        <Button onclick={() => (open = false)}>{t('common.cancel')}</Button>
      </div>
    </form>
  {/if}
</div>
