<script lang="ts">
  import { onMount } from 'svelte'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { api } from '$lib/tauri'
  import { SECRET_MARKER, type ProviderSetup } from '$lib/tauri/types'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { Button, Field, IconButton, Row, Switch } from '$lib/components/ui'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  const registrations: Record<'microsoft' | 'google' | 'slack', { name: string; covers: MessageKey; id: string; secret: string; secretHint: MessageKey; guide: MessageKey }> = {
    microsoft: {
      name: 'Microsoft 365',
      covers: 'settings.registrations.microsoft.covers',
      id: 'microsoft.client_id',
      secret: 'microsoft.client_secret',
      secretHint: 'settings.registrations.microsoft.secretHint',
      guide: 'settings.registrations.microsoft.guide'
    },
    google: {
      name: 'Google Workspace',
      covers: 'settings.registrations.google.covers',
      id: 'google.client_id',
      secret: 'google.client_secret',
      secretHint: 'settings.registrations.google.secretHint',
      guide: 'settings.registrations.google.guide'
    },
    slack: {
      name: 'Slack',
      covers: 'settings.registrations.slack.covers',
      id: 'slack.client_id',
      secret: 'slack.client_secret',
      secretHint: 'settings.registrations.slack.secretHint',
      guide: 'settings.registrations.slack.guide'
    }
  }

  let setups = $state<ProviderSetup[]>([])
  let settings = $state<Record<string, string>>({})
  let drafts = $state<Record<string, string>>({})
  let saving = $state<Record<string, boolean>>({})
  let copied = $state<string | null>(null)
  let failure = $state<string | null>(null)

  async function load() {
    try {
      const [setup, values] = await Promise.all([api.getConnectionSetup(), api.getAppSettings()])
      const pending = Object.fromEntries(Object.keys(drafts).filter((key) => !saving[key] && changed(key)).map((key) => [key, drafts[key]]))
      setups = setup
      settings = values
      drafts = { ...Object.fromEntries(Object.entries(values).map(([key, value]) => [key, value === SECRET_MARKER ? '' : value])), ...pending }
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  onMount(load)

  async function save(key: string, value = drafts[key] ?? '') {
    saving[key] = true
    try {
      await api.putAppSetting(key, value)
      await load()
    } catch (error) {
      failure = String(error)
    } finally {
      saving[key] = false
    }
  }

  async function copy(key: string, text: string) {
    await navigator.clipboard.writeText(text)
    copied = key
    setTimeout(() => (copied = copied === key ? null : copied), 1600)
  }

  async function postAdminConsent() {
    try {
      await api.postAdminConsent()
    } catch (error) {
      failure = String(error)
    }
  }

  const changed = (key: string) => (settings[key] === SECRET_MARKER ? !!drafts[key] : (drafts[key] ?? '') !== (settings[key] ?? ''))
</script>

{#snippet field(key: string, label: string, hint: string, secret = false, placeholder = '')}
  <Row {label} {hint} for={key}>
    <Field id={key} mono type={secret ? 'password' : 'text'} class="w-[clamp(12rem,40%,20rem)]" bind:value={() => drafts[key] ?? '', (value) => (drafts[key] = value)} placeholder={secret && settings[key] === SECRET_MARKER ? t('settings.registrations.savedInKeychain') : placeholder} autocomplete="off" spellcheck="false" onkeydown={(e) => e.key === 'Enter' && save(key)} />
    <Button size="sm" variant="secondary" busy={saving[key]} disabled={!changed(key)} onclick={() => save(key)}>{t('common.save')}</Button>
  </Row>
{/snippet}

<div>
  <p class="max-w-[64ch] py-3 text-[13px] leading-5 text-ink-2">
    {t('settings.registrations.intro')}
  </p>

  {#if failure}
    <p class="border-b border-line py-2 text-[13px] text-error-500">{failure}</p>
  {/if}

  {#each setups as setup (setup.provider_type)}
    {@const info = registrations[setup.provider_type]}
    <section class="border-t border-line py-3">
      <header class="flex min-w-0 items-center gap-2.5 pb-1">
        <ProviderIcon providers={[{ provider_type: setup.provider_type, name: info.name }]} conversationType={setup.provider_type === 'slack' ? 'channel' : undefined} size="md" />
        <span class="text-[14px] font-medium text-ink">{info.name}</span>
        <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[13px] text-ink-3">{t(info.covers)}</span>
        <span class="ml-auto shrink-0 font-mono text-[11px] {setup.ready ? 'text-success-500' : 'text-ink-3'}">{setup.ready ? t('settings.registrations.registered') : t('settings.registrations.needsClientId')}</span>
        <IconButton size="sm" icon="lucide:external-link" label={t('settings.registrations.openConsole', { name: info.name })} onclick={() => openUrl(setup.console_url)} />
      </header>
      <p class="pb-1 text-[12.5px] text-ink-3">{t(info.guide)}</p>

      {@render field(info.id, t('settings.registrations.clientId'), t('settings.registrations.clientIdHint'), false, t('settings.registrations.empty'))}
      {@render field(info.secret, t('settings.registrations.clientSecret'), t(info.secretHint), true)}

      {#if setup.provider_type === 'slack'}
        {@render field('slack.app_token', t('settings.registrations.slackAppToken'), t('settings.registrations.slackAppTokenHint'), true, 'xapp-')}
      {/if}

      {#if setup.provider_type === 'microsoft'}
        {@render field('microsoft.tenant', t('settings.registrations.tenant'), t('settings.registrations.tenantHint'), false, 'organizations')}
        <Row label={t('settings.registrations.teamsChannels')} hint={t('settings.registrations.teamsChannelsHint', { scopes: setup.admin_scopes.join(', ') })}>
          <Switch label={t('settings.registrations.readTeamsChannels')} checked={settings['microsoft.admin_scopes'] === 'true'} onchange={(on) => save('microsoft.admin_scopes', on ? 'true' : 'false')} />
        </Row>
      {/if}

      <Row label={t('settings.registrations.redirect')} hint={t('settings.registrations.redirectHint')}>
        <span class="font-mono text-[12px] text-ink-2 selectable">{setup.redirect_uri}</span>
        <IconButton size="sm" icon={copied === `${setup.provider_type}-redirect` ? 'lucide:check' : 'lucide:copy'} label={t('settings.registrations.copyRedirect')} onclick={() => copy(`${setup.provider_type}-redirect`, setup.redirect_uri)} />
      </Row>

      <Row label={t('settings.registrations.permissions')} hint={setup.provider_type === 'slack' ? t('settings.registrations.slackScopes') : t('settings.registrations.delegated')}>
        <p class="max-w-[48ch] font-mono text-[11.5px] leading-5 text-ink-2 selectable">
          {setup.scopes.map((s) => s.replace('https://www.googleapis.com/auth/', '')).join(' · ')}{#if setup.admin_scopes.length} · <span class="text-signal">{t('settings.registrations.administratorScopes', { scopes: setup.admin_scopes.join(' · ') })}</span>{/if}
        </p>
      </Row>

      {#if setup.provider_type === 'microsoft'}
        <Row label={t('settings.registrations.approval')} hint={t('settings.registrations.approvalHint')}>
          <Button size="sm" icon={copied === 'admin' ? 'lucide:check' : 'lucide:copy'} disabled={!setup.admin_consent_url} onclick={() => setup.admin_consent_url && copy('admin', setup.admin_consent_url)}>{t('settings.registrations.copyLink')}</Button>
          <Button size="sm" variant="secondary" icon="lucide:external-link" disabled={!setup.admin_consent_url} onclick={postAdminConsent}>{t('common.open')}</Button>
        </Row>
      {/if}
    </section>
  {/each}
</div>
