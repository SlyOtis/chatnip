<script lang="ts">
  import { LIGHT, type Light } from './Connections.svelte'
  import { onMount } from 'svelte'
  import { anchored, boxOf } from '$lib/popover'
  import Icon from '@iconify/svelte'
  import { listen } from '@tauri-apps/api/event'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { api } from '$lib/tauri'
  import { SECRET_MARKER, type AiSetup, type IntelligenceConnection, type IntelligenceKind, type PullProgress } from '$lib/tauri/types'
  import { claudeConnectionInput, configFields, emptyConfig, formatBytes, kindIcon, kindLabel, kinds, localConnection, missingModel, ollamaConnectionInput, ollamaStep, pullPercent } from '$lib/intelligence'
  import { Button, Field, IconButton, MenuItem, Section, Select, Sheet, Switch } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    onchange?: (connections: IntelligenceConnection[]) => void
  }

  let { onchange }: Props = $props()

  let setup = $state<AiSetup | null>(null)
  let connections = $state<IntelligenceConnection[]>([])
  let checking = $state(false)
  let busy = $state<string | null>(null)
  let pulls = $state<Record<string, PullProgress>>({})
  let ollamaModel = $state('')
  let ollamaOpen = $state(false)
  let startFailure = $state<string | null>(null)
  let drafts = $state<Record<string, string>>({})
  let discovered = $state<Record<string, string[]>>({})
  let picking = $state(false)
  let addButton = $state<HTMLDivElement | null>(null)
  let editing = $state<{ id: string | null; kind: IntelligenceKind; name: string; config: Record<string, string>; is_default: boolean } | null>(null)
  let sealed = $state<Set<string>>(new Set())
  let saving = $state(false)
  let failure = $state<string | null>(null)
  let tests = $state<Record<string, string>>({})
  let testing = $state<Record<string, boolean>>({})

  const claude = $derived(localConnection(connections, 'claude_cli'))
  const ollama = $derived(localConnection(connections, 'ollama'))
  const others = $derived(connections.filter((c) => c.id !== claude?.id && c.id !== ollama?.id))
  const addable = $derived(kinds.filter((k) => k.kind !== 'claude_cli' && k.kind !== 'ollama'))
  const step = $derived(setup ? ollamaStep(setup.ollama) : null)
  const installed = $derived(setup?.ollama.models ?? [])
  const ollamaOptions = $derived(installed.map((m) => ({ value: m.name, label: `${m.name}${m.size_bytes ? ` · ${formatBytes(m.size_bytes)}` : ''}`, mono: true })))
  const ollamaHint = $derived(ollama?.config.model ? t('settings.ai.modelMissing', { model: ollama.config.model }) : t('settings.ai.modelUnset'))
  const ollamaNeeds = $derived(!!ollama && !!setup?.ollama.running && missingModel(ollama.config.model, installed.map((m) => m.name)))
  const claudeFallback = $derived(setup?.claude_model || (setup?.claude.model ? t('settings.ai.cliDefaultModel', { model: setup.claude.model }) : t('settings.ai.cliDefault')))

  async function refresh(rediscover = false) {
    checking = true
    failure = null
    try {
      setup = await api.getAiSetup(rediscover)
    } catch (error) {
      failure = String(error)
    }
    try {
      connections = await api.getIntelligenceConnections()
    } catch (error) {
      failure = String(error)
    }
    drafts = Object.fromEntries(connections.map((c) => [c.id, c.config.model ?? '']))
    discovered = {}
    ollamaModel = ollama?.config.model || ollamaModel || installed[0]?.name || ''
    onchange?.(connections)
    checking = false
  }

  onMount(() => {
    refresh()
    const stop = listen<PullProgress>('ollama-pull', (event) => {
      pulls = { ...pulls, [event.payload.model]: event.payload }
    })
    return () => {
      stop.then((unlisten) => unlisten())
    }
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

  function switchOnClaude() {
    const status = setup?.claude
    if (status) run('claude', () => api.postIntelligenceConnection(claudeConnectionInput(status, '')))
  }

  function switchOnOllama() {
    const status = setup?.ollama
    if (status) run('ollama', () => api.postIntelligenceConnection(ollamaConnectionInput(status, ollamaModel)))
  }

  function chooseModel(connection: IntelligenceConnection | undefined, model: string) {
    const next = model.trim()
    if (!connection || (connection.config.model ?? '') === next) return
    run(connection.kind, () => api.putIntelligenceConnection({ connection_id: connection.id, config: { ...connection.config, model: next } }))
  }

  function discover(connection: IntelligenceConnection) {
    if (discovered[connection.id] || connection.kind === 'claude_cli') return
    discovered = { ...discovered, [connection.id]: connection.models }
    api.getIntelligenceModels(connection.id).then((models) => (discovered = { ...discovered, [connection.id]: models })).catch(() => {})
  }

  function modelOptions(connection: IntelligenceConnection): string[] {
    const listed = connection.kind === 'claude_cli' ? [...connection.models, ...(setup?.claude.models ?? [])] : (discovered[connection.id] ?? connection.models)
    return [...new Set(listed.filter((m) => !!m))]
  }

  async function startOllama() {
    busy = 'start'
    startFailure = null
    try {
      await api.postOllamaStart()
      await refresh()
    } catch (error) {
      startFailure = String(error)
    } finally {
      busy = null
    }
  }

  function pull(model: string) {
    pulls = { ...pulls, [model]: { model, status: 'starting', completed: null, total: null, done: false, error: null } }
    run('pull', async () => {
      await api.postOllamaPull(model)
      if (!ollamaModel) ollamaModel = model
    })
  }

  function startAdd(kind: IntelligenceKind) {
    picking = false
    sealed = new Set()
    editing = { id: null, kind, name: kindLabel(kind), config: emptyConfig(kind), is_default: connections.length === 0 }
  }

  function startEdit(connection: IntelligenceConnection) {
    sealed = new Set(Object.entries(connection.config).filter(([, value]) => value === SECRET_MARKER).map(([key]) => key))
    const config = Object.fromEntries(Object.entries(connection.config).map(([key, value]) => [key, value === SECRET_MARKER ? '' : value]))
    editing = { id: connection.id, kind: connection.kind, name: connection.name, config: { ...emptyConfig(connection.kind), ...config }, is_default: connection.is_default }
  }

  async function save() {
    if (!editing || saving) return
    saving = true
    failure = null
    try {
      if (editing.id) {
        const config = Object.fromEntries(Object.entries(editing.config).map(([key, value]) => [key, !value && sealed.has(key) ? SECRET_MARKER : value]))
        await api.putIntelligenceConnection({ connection_id: editing.id, name: editing.name.trim(), config, is_default: editing.is_default })
      } else {
        await api.postIntelligenceConnection({ kind: editing.kind, name: editing.name.trim(), config: editing.config, is_default: editing.is_default })
      }
      await refresh()
      editing = null
    } catch (error) {
      failure = String(error)
    } finally {
      saving = false
    }
  }

  function remove(connection: IntelligenceConnection) {
    run(connection.kind, () => api.deleteIntelligenceConnection(connection.id))
  }

  function makeDefault(connection: IntelligenceConnection) {
    run(connection.kind, () => api.putIntelligenceConnection({ connection_id: connection.id, is_default: true }))
  }

  async function test(connection: IntelligenceConnection) {
    testing = { ...testing, [connection.id]: true }
    try {
      const result = await api.postIntelligenceConnectionTest(connection.id)
      tests = { ...tests, [connection.id]: result }
    } catch (error) {
      tests = { ...tests, [connection.id]: String(error) }
    } finally {
      testing = { ...testing, [connection.id]: false }
    }
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) save()
  }
</script>

{#snippet dot(tone: Light)}
  <span class="size-1.5 shrink-0 rounded-full {LIGHT[tone]}"></span>
{/snippet}

{#snippet tools(connection: IntelligenceConnection, local: boolean)}
  <div class="flex items-center gap-1 pl-3 opacity-0 group-hover:opacity-100 focus-within:opacity-100">
    <IconButton size="sm" icon="lucide:activity" label={t('common.test')} busy={testing[connection.id]} onclick={() => test(connection)} />
    <IconButton size="sm" icon="lucide:pencil" label={t('common.edit')} onclick={() => startEdit(connection)} />
    {#if !connection.is_default}
      <IconButton size="sm" icon="lucide:star" label={t('settings.origins.makeDefault')} onclick={() => makeDefault(connection)} />
    {/if}
    <IconButton size="sm" icon={local ? 'lucide:power' : 'lucide:x'} label={local ? t('settings.ai.switchOff') : t('common.remove')} tone="danger" busy={busy === connection.kind} onclick={() => remove(connection)} />
  </div>
{/snippet}

{#snippet marker(connection: IntelligenceConnection | undefined)}
  {#if connection?.is_default}
    <span class="shrink-0 font-mono text-[11px] text-ink-3">{t('settings.origins.default')}</span>
  {/if}
{/snippet}

{#snippet typed(connection: IntelligenceConnection, placeholder: string)}
  <div id={connection.is_default ? 'ai.default-model' : undefined} class="flex min-w-0 items-center gap-2">
    <Field
      size="sm"
      mono
      class="w-[min(18rem,40vw)]"
      list="provider-models-{connection.id}"
      aria-label={t('settings.models.model')}
      title={t('settings.ai.typeModel')}
      {placeholder}
      autocomplete="off"
      spellcheck="false"
      bind:value={() => drafts[connection.id] ?? '', (value) => (drafts[connection.id] = value)}
      onfocus={() => discover(connection)}
      onchange={() => chooseModel(connection, drafts[connection.id] ?? '')}
      onkeydown={(e) => e.key === 'Enter' && chooseModel(connection, drafts[connection.id] ?? '')}
    />
    <datalist id="provider-models-{connection.id}">
      {#each modelOptions(connection) as option (option)}
        <option value={option}></option>
      {/each}
    </datalist>
  </div>
{/snippet}

{#snippet outcome(connection: IntelligenceConnection | undefined)}
  {#if connection && tests[connection.id]}
    <p class="font-mono text-[12px] text-ink-2 selectable">{tests[connection.id]}</p>
  {/if}
{/snippet}

<Section id="ai.connection" title={t('settings.origins.title')} description={t('settings.origins.description')} total={others.length}>
  {#snippet actions()}
    <IconButton icon="lucide:refresh-cw" label={t('settings.ai.checkAgain')} size="sm" busy={checking} onclick={() => refresh(true)} />
    <div class="relative" bind:this={addButton}>
      <Button variant="primary" icon="lucide:plus" onclick={() => (picking = !picking)}>{t('settings.origins.add')}</Button>
      {#if picking}
        <div class="menu w-64" use:anchored={{ at: () => boxOf(addButton), onclose: () => (picking = false) }} role="menu">
          {#each addable as kind (kind.kind)}
            <MenuItem icon={kind.icon} onclick={() => startAdd(kind.kind)}>{kind.label}</MenuItem>
          {/each}
        </div>
      {/if}
    </div>
  {/snippet}

  {#snippet children(shown)}
  {#if failure}
    <p class="mb-2 text-[13px] text-error-500">{failure}</p>
  {/if}

  <ul class="mt-4 border-y border-line">
    <li id="ai.claude" data-testid="ai-provider" data-kind="claude_cli" class="group flex flex-col gap-2 border-b border-line py-3">
      <div class="grid grid-cols-[1rem_minmax(0,1fr)_auto] items-center gap-3">
        <Icon icon={kindIcon('claude_cli')} class="text-[14px] text-ink-3" />
        <div class="flex min-w-0 items-baseline gap-2">
          <span class="flex items-center gap-1.5 text-[14px] font-medium text-ink">
            {@render dot(!setup?.claude.installed ? 'red' : !setup.claude.signed_in ? 'amber' : claude ? 'green' : 'grey')}
            Claude Code
          </span>
          {@render marker(claude)}
          {#if setup?.claude.installed}
            <span class="truncate font-mono text-[12px] text-ink-3" title={setup.claude.binary ?? ''}>{t('settings.ai.found')}{setup.claude.version ? ` · ${setup.claude.version}` : ''}</span>
          {/if}
        </div>
        {#if claude}
          {@render tools(claude, true)}
        {:else if setup?.claude.installed}
          <Button size="sm" busy={busy === 'claude'} onclick={switchOnClaude}>{t('settings.ai.switchOn')}</Button>
        {/if}
      </div>
      <div class="flex min-w-0 flex-col gap-2 pl-7">
        {#if !setup}
          <p class="text-[13px] text-ink-3">{t('settings.ai.checking')}</p>
        {:else if !setup.claude.installed}
          <p class="text-[13px] leading-5 text-ink-3">{t('settings.ai.claudeMissing')}</p>
          <div><Button size="sm" icon="lucide:external-link" onclick={() => openUrl('https://claude.com/claude-code')}>{t('settings.ai.getClaude')}</Button></div>
        {:else}
          <p data-testid="claude-account" class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[13px] {setup.claude.signed_in ? 'text-ink-2' : 'text-warn-600'}">
            {setup.claude.signed_in && setup.claude.account ? t('settings.ai.signedIn', { account: `${setup.claude.account}${setup.claude.plan ? ` · ${setup.claude.plan}` : ''}` }) : t('settings.ai.notSignedIn')}
          </p>
          {#if claude}
            {@render typed(claude, claudeFallback)}
          {/if}
        {/if}
        {@render outcome(claude)}
      </div>
    </li>

    <li id="ai.ollama" data-testid="ai-provider" data-kind="ollama" class="group flex flex-col gap-2 border-b border-line py-3">
      <div class="grid grid-cols-[1rem_minmax(0,1fr)_auto] items-center gap-3">
        <Icon icon={kindIcon('ollama')} class="text-[14px] text-ink-3" />
        <div class="flex min-w-0 items-baseline gap-2">
          <span class="flex items-center gap-1.5 text-[14px] font-medium text-ink">
            {@render dot(!setup?.ollama.running ? (step === 'install' ? 'grey' : 'red') : ollamaNeeds ? 'amber' : ollama ? 'green' : 'grey')}
            Ollama
          </span>
          {@render marker(ollama)}
          {#if setup?.ollama.running}
            <span class="truncate font-mono text-[12px] text-ink-3">{t('settings.ai.running')} · {setup.ollama.version} · {setup.ollama.host}</span>
          {/if}
        </div>
        {#if ollama}
          {@render tools(ollama, true)}
        {:else if step === 'ready'}
          <Button size="sm" busy={busy === 'ollama'} onclick={switchOnOllama}>{t('settings.ai.switchOn')}</Button>
        {/if}
      </div>
      <div class="flex min-w-0 flex-col gap-2 pl-7">
        {#if !setup}
          <p class="text-[13px] text-ink-3">{t('settings.ai.checking')}</p>
        {:else if step === 'install'}
          <p class="text-[13px] leading-5 text-ink-3">{t('settings.ai.ollamaMissing')}</p>
          <div><Button size="sm" icon="lucide:download" onclick={() => openUrl(setup?.ollama.download_url ?? 'https://ollama.com/download')}>{t('settings.ai.downloadOllama')}</Button></div>
        {:else if step === 'start'}
          <p class="text-[13px] leading-5 text-ink-3">{t('settings.ai.ollamaStopped')}</p>
          <div><Button size="sm" icon="lucide:play" testid="ollama-start" busy={busy === 'start'} onclick={startOllama}>{t('settings.ai.startOllama')}</Button></div>
          {#if startFailure}
            <p data-testid="ollama-start-failure" class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[12px] text-error-500" title={startFailure}>{startFailure}</p>
          {/if}
        {:else}
          {#if installed.length === 0}
            <p class="text-[13px] text-ink-3">{t('settings.ai.noModels')}</p>
          {:else}
            <div id={ollama?.is_default ? 'ai.default-model' : undefined} class="flex min-w-0 items-center gap-2">
              <Select size="sm" class="w-[min(18rem,40vw)]" dialog="Ollama" testid="ollama-model" placeholder={ollamaHint} bind:open={ollamaOpen} value={ollama ? (ollama.config.model ?? '') : ollamaModel} options={ollamaOptions} onchange={(m) => ((ollamaModel = m), chooseModel(ollama, m))} />
              {#if ollamaNeeds}
                <IconButton size="sm" tone="warn" icon="lucide:triangle-alert" testid="ollama-model-needed" label={ollamaHint} onclick={() => (ollamaOpen = true)} />
              {/if}
            </div>
          {/if}
          {#if setup.suggestions.length}
            <p class="pt-1 text-[12px] font-medium text-ink-3">{t('settings.ai.notInstalledPull')}</p>
            <ul class="flex flex-col">
              {#each setup.suggestions as suggestion (suggestion.name)}
                {@const progress = pulls[suggestion.name]}
                {@const percent = pullPercent(progress)}
                <li class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 border-t border-line py-2 first:border-t-0">
                  <span class="flex min-w-0 flex-col">
                    <span class="flex items-baseline gap-2">
                      <span class="font-mono text-[12.5px] text-ink">{suggestion.name}</span>
                      <span class="font-mono text-[11px] text-ink-3">{suggestion.size}</span>
                    </span>
                    <span class="text-[12px] leading-4 text-ink-3">{suggestion.good_for}</span>
                    {#if progress && !progress.done}
                      <span class="mt-1 flex items-center gap-2">
                        <span class="h-1 flex-1 overflow-hidden rounded-full bg-paper-3"><span class="block h-full bg-accent" style:width="{percent ?? 0}%"></span></span>
                        <span class="font-mono text-[10.5px] text-ink-3">{percent != null ? `${percent}%` : progress.status}</span>
                      </span>
                    {:else if progress?.error}
                      <span class="text-[12px] text-error-500">{progress.error}</span>
                    {/if}
                  </span>
                  {#if progress?.done && !progress.error}
                    <span class="flex items-center gap-1 text-[12px] text-ink-3"><Icon icon="lucide:check" class="text-[12px]" />{t('settings.ai.pulled')}</span>
                  {:else}
                    <Button size="sm" icon="lucide:download" title={suggestion.command} disabled={!!progress && !progress.done} onclick={() => pull(suggestion.name)}>{t('settings.ai.pull')}</Button>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        {/if}
        {@render outcome(ollama)}
      </div>
    </li>

    {#each others.slice(0, shown) as connection (connection.id)}
      <li data-testid="ai-provider" data-kind={connection.kind} class="group flex flex-col gap-2 border-b border-line py-3 last:border-b-0">
        <div class="grid grid-cols-[1rem_minmax(0,1fr)_auto] items-center gap-3">
          <Icon icon={kindIcon(connection.kind)} class="text-[14px] text-ink-3" />
          <div class="flex min-w-0 items-baseline gap-2">
            <span class="flex items-center gap-1.5 text-[14px] font-medium text-ink">
              {@render dot(connection.available ? 'green' : 'red')}
              {connection.name}
            </span>
            {@render marker(connection)}
            <span class="truncate text-[13px] text-ink-3">{kindLabel(connection.kind)}{connection.detail ? ` · ${connection.detail}` : ''}</span>
          </div>
          {@render tools(connection, false)}
        </div>
        <div class="flex min-w-0 flex-col gap-2 pl-7">
          {@render typed(connection, t('settings.ai.typeModel'))}
          {@render outcome(connection)}
        </div>
      </li>
    {/each}
  </ul>
  {/snippet}
</Section>

{#if editing}
  <Sheet label={editing.id ? t('settings.origins.edit') : t('settings.origins.new', { kind: kindLabel(editing.kind) })} onclose={() => (editing = null)}>
    <form
      class="flex flex-col gap-4 px-5 py-4"
      onsubmit={(e) => {
        e.preventDefault()
        save()
      }}
      onkeydown={keydown}
    >
      <div class="flex items-center gap-2">
        <Icon icon={kindIcon(editing.kind)} class="text-[16px] text-ink-3" />
        <h4 class="text-[15px] font-semibold tracking-tight text-ink">{editing.id ? t('settings.origins.edit') : t('settings.origins.new', { kind: kindLabel(editing.kind) })}</h4>
      </div>
      <label class="flex flex-col gap-1">
        <span class="text-[13px] font-medium text-ink-2">{t('common.name')}</span>
        <Field bind:value={editing.name} />
      </label>
      {#each configFields(editing.kind) as field (field.key)}
        <label class="flex flex-col gap-1">
          <span class="text-[13px] font-medium text-ink-2">{field.label}</span>
          {#if field.options}
            <Select bind:value={editing.config[field.key]} options={field.options.map((o) => ({ value: o, label: o }))} />
          {:else}
            <Field mono type={field.secret ? 'password' : 'text'} placeholder={field.secret && sealed.has(field.key) ? t('settings.registrations.savedInKeychain') : (field.placeholder ?? '')} bind:value={editing.config[field.key]} autocomplete="off" spellcheck="false" />
          {/if}
          <span class="text-[12px] text-ink-3">{field.help}</span>
        </label>
      {/each}
      <div class="flex items-center gap-3 text-[13px] text-ink">
        <Switch label={t('settings.origins.useDefault')} bind:checked={editing.is_default} />
        {t('settings.origins.useDefault')}
      </div>
    </form>
    {#snippet footer()}
      <span class="mr-auto"></span>
      <Button onclick={() => (editing = null)}>{t('common.cancel')}</Button>
      <Button variant="primary" keys="⌘↵" busy={saving} disabled={!editing?.name.trim()} onclick={save}>{t('common.save')}</Button>
    {/snippet}
  </Sheet>
{/if}
