<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from '@iconify/svelte'
  import { api } from '$lib/tauri'
  import type { CliSetup, IntelligenceConnection, SettingRegistry } from '$lib/tauri/types'
  import { PLUGIN_HOME, PLUGIN_HOME_PATH, PLUGIN_MOCK, pluginAction, pluginCommand, pluginHome, pluginOptions } from '$lib/intelligence'
  import { settingValue } from '$lib/settings'
  import { Button, Field, IconButton, Select, Switch } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'
  import IntelligenceConnections from './IntelligenceConnections.svelte'

  interface Props {
    part?: 'developer'
    onconnections?: (connections: IntelligenceConnection[]) => void
  }

  let { part, onconnections }: Props = $props()

  let cli = $state<CliSetup | null>(null)
  let steps = $state<string[]>([])
  let copied = $state<string | null>(null)
  let busy = $state<string | null>(null)
  let failure = $state<string | null>(null)
  let registry = $state<SettingRegistry | null>(null)
  let homePath = $state('')

  const homeChoice = $derived(pluginHome(registry))
  const chosenPlugin = $derived(pluginOptions(registry))
  const homeOptions = $derived([
    { value: 'app', label: t('settings.ai.pluginHomeApp') },
    { value: 'mock', label: t('settings.ai.pluginHomeMock') },
    { value: 'custom', label: t('settings.ai.pluginHomeCustom') }
  ])

  async function refresh() {
    failure = null
    api.getCliSetup().then((value) => (cli = value)).catch(() => (cli = null))
    api.getSettingRegistry().then(loadRegistry).catch((error) => (failure = String(error)))
  }

  onMount(() => {
    refresh()
  })

  async function run(key: string, action: () => Promise<unknown>) {
    busy = key
    failure = null
    try {
      await action()
      await refresh()
    } catch (error) {
      failure = String(error)
    } finally {
      busy = null
    }
  }

  async function copy(text: string) {
    await navigator.clipboard.writeText(text)
    copied = text
    setTimeout(() => (copied = copied === text ? null : copied), 1600)
  }

  function putPluginSetting(key: string, value: string) {
    if (registry) registry = { ...registry, values: { ...registry.values, [key]: value } }
    api.putAppSetting(key, value).catch((error) => (failure = String(error)))
  }

  function loadRegistry(value: SettingRegistry) {
    registry = value
    homePath = settingValue(value, PLUGIN_HOME_PATH)
  }

  function installPlugin() {
    run('plugin', async () => {
      if (homeChoice === 'custom' && homePath.trim() !== settingValue(registry, PLUGIN_HOME_PATH)) await api.putAppSetting(PLUGIN_HOME_PATH, homePath)
      loadRegistry(await api.getSettingRegistry())
      steps = await api.postClaudePlugin(pluginOptions(registry))
    })
  }

  function linkCli() {
    run('path', async () => (steps = [t('settings.ai.linked', { path: await api.postCliPath() })]))
  }
</script>

<div class="flex flex-col gap-4">
  {#if part !== 'developer'}
    <IntelligenceConnections onchange={onconnections} />
  {/if}

  {#if failure}
    <p class="text-[13px] text-error-500">{failure}</p>
  {/if}

  {#snippet command(text: string)}
    <span class="flex min-w-0 items-center gap-1.5">
      <code class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap rounded-[0.375rem] bg-paper-3 px-2 py-1 font-mono text-[12px] text-ink-2">{text}</code>
      <IconButton icon={copied === text ? 'lucide:check' : 'lucide:copy'} label={t('settings.ai.copyCommand')} size="sm" onclick={() => copy(text)} />
    </span>
  {/snippet}

  <div class="grid grid-cols-[repeat(auto-fit,minmax(min(100%,17rem),1fr))] border-y border-line">
    <section id="developer.plugin" class="flex min-w-0 flex-col gap-3 py-4 pr-5">
      <header class="flex items-center gap-2.5">
        <Icon icon="lucide:puzzle" class="text-[15px] text-ink-2" />
        <h3 class="min-w-0 flex-1 text-[14px] font-semibold text-ink">{t('settings.ai.plugin')}</h3>
        {#if cli?.plugin_installed && cli.plugin_version === cli.plugin_current}
          <span class="flex items-center gap-1 text-[12px] text-ink-3"><Icon icon="lucide:check" class="text-[12px]" />{t('settings.ai.installed')}</span>
        {/if}
      </header>
      <p class="text-[13px] leading-5 text-ink-3">{t('settings.ai.pluginDescription')}</p>
      {#if cli}
        <dl class="grid grid-cols-[4.5rem_minmax(0,1fr)] gap-x-3 gap-y-1 text-[12.5px]">
          <dt class="text-ink-3">{t('settings.ai.version')}</dt>
          <dd class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-ink-2">{cli.plugin_version ?? t('settings.ai.notInstalled')}</dd>
          <dt class="text-ink-3">{t('settings.ai.folder')}</dt>
          <dd class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-ink-2" title={cli.plugin_dir}>{cli.plugin_dir}</dd>
          {#if cli.plugin_options}
            <dt class="text-ink-3">{t('settings.ai.pluginInstalledHome')}</dt>
            <dd data-testid="plugin-installed-home" class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-ink-2" title={cli.plugin_options.home ?? ''}>{cli.plugin_options.home ?? t('settings.ai.pluginHomeApp')}</dd>
            <dt class="text-ink-3">{t('settings.ai.pluginInstalledMode')}</dt>
            <dd data-testid="plugin-installed-mode" class="text-ink-2">{cli.plugin_options.mock ? t('settings.ai.pluginMockOn') : t('settings.ai.pluginMockSettings')}</dd>
          {/if}
        </dl>
        <div id="developer.plugin-home" class="flex flex-col gap-1.5">
          <div class="flex items-center gap-3">
            <span class="min-w-0 flex-1 text-[13px] text-ink-2" title={t('settings.ai.pluginHomeHint')}>{t('settings.ai.pluginHome')}</span>
            <Select size="sm" testid="plugin-home" value={homeChoice} options={homeOptions} onchange={(value) => putPluginSetting(PLUGIN_HOME, value)} />
          </div>
          {#if homeChoice === 'mock'}
            <p class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[12px] text-ink-3" title={chosenPlugin.home ?? ''}>{chosenPlugin.home}</p>
          {:else if homeChoice === 'custom'}
            <Field size="sm" mono type="text" data-testid="plugin-home-path" aria-label={t('settings.ai.pluginHomePath')} placeholder={t('settings.ai.pluginHomePath')} autocomplete="off" bind:value={homePath} onchange={() => putPluginSetting(PLUGIN_HOME_PATH, homePath)} />
          {/if}
        </div>
        <div id="developer.plugin-mock" class="flex items-center gap-3">
          <span class="min-w-0 flex-1 text-[13px] text-ink-2" title={t('settings.ai.pluginMockHint')}>{t('settings.ai.pluginMock')}</span>
          <Switch testid="plugin-mock" label={t('settings.ai.pluginMock')} checked={chosenPlugin.mock} onchange={(on) => putPluginSetting(PLUGIN_MOCK, on ? 'true' : 'false')} />
        </div>
        {#if cli.claude}
          <div class="flex items-center gap-2">
            <Button size="sm" variant="secondary" icon="lucide:download" busy={busy === 'plugin'} testid="plugin-install" onclick={installPlugin}>{t(({ install: 'settings.ai.installPlugin', update: 'settings.ai.update', reinstall: 'settings.ai.reinstall' } as const)[pluginAction(cli)])}</Button>
          </div>
        {:else}
          <p class="text-[13px] text-ink-3">{t('settings.ai.installClaudeFirst')}</p>
        {/if}
        {@render command(pluginCommand(chosenPlugin))}
      {/if}
    </section>

    <section id="developer.cli" class="flex min-w-0 flex-col gap-3 border-l border-line py-4 pl-5">
      <header class="flex items-center gap-2.5">
        <Icon icon="lucide:square-terminal" class="text-[15px] text-ink-2" />
        <h3 class="min-w-0 flex-1 text-[14px] font-semibold text-ink">{t('settings.ai.cli')}</h3>
        {#if cli?.linked}
          <span class="flex items-center gap-1 text-[12px] text-ink-3"><Icon icon="lucide:check" class="text-[12px]" />{t('settings.ai.onPath')}</span>
        {/if}
      </header>
      <p class="text-[13px] leading-5 text-ink-3">{t('settings.ai.cliDescription')}</p>
      {#if cli}
        <dl class="grid grid-cols-[4.5rem_minmax(0,1fr)] gap-x-3 gap-y-1 text-[12.5px]">
          <dt class="text-ink-3">{t('settings.ai.link')}</dt>
          <dd class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-ink-2" title={cli.link}>{cli.linked ? cli.link : t('settings.ai.notLinked')}</dd>
          {#if cli.linked && !cli.on_path}
            <dt class="text-ink-3">PATH</dt>
            <dd class="text-ink-2">{t('settings.ai.addToPathHint', { path: '~/.local/bin' })}</dd>
          {/if}
        </dl>
        <div class="flex items-center gap-2">
          <Button size="sm" variant="secondary" icon="lucide:link" busy={busy === 'path'} onclick={linkCli}>{cli.linked ? t('settings.ai.linkAgain') : t('settings.ai.addToPath')}</Button>
        </div>
        {@render command('chatnip conversations get')}
        {@render command('chatnip commands')}
      {/if}
    </section>
  </div>

  {#if steps.length}
    <ul class="flex flex-col gap-0.5">
      {#each steps as line, i (i)}
        <li class="flex items-center gap-1.5 font-mono text-[12px] text-ink-3"><Icon icon="lucide:check" class="text-[12px]" />{line}</li>
      {/each}
    </ul>
  {/if}

</div>
