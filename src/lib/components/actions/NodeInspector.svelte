<script lang="ts">
  import Icon from '@iconify/svelte'
  import { Button, IconButton, Segmented, Select, Switch } from '$lib/components/ui'
  import CodeEditor from './CodeEditor.svelte'
  import SchemaControls from './SchemaControls.svelte'
  import SignatureForm from './SignatureForm.svelte'
  import ReferencePanel from './ReferencePanel.svelte'
  import PromptEditor from './PromptEditor.svelte'
  import { t, type MessageKey } from '$lib/i18n.svelte'
  import type { ActionCatalog, ActionDiagnostic, ActionGraph, ActionNode, ActionPort, ConversationType, GroupQuery, PromptOverride, TriggerConfig, TriggerDirection, TriggerOn } from '$lib/tauri/types'
  import { ACTION, BUILTIN, chatOf, configSchemaOf, configValueOf, editable, iconOf, kindLabel, label, NODE_SCRIPT, NODE_TRIGGER, portsOf, presenterKind, problemsFor, promptsUsed, requirementsFor, resolvePrompt, triggerOf, withPrompt, withReturns, withValue, type Owner } from '$lib/actions'
  import { goto } from '$app/navigation'
  import { api, conversationsStore, peopleStore, providersStore } from '$lib/tauri'
  import { providerLabel } from '$lib/providerIcons'
  import { CHAT_MODES } from '$lib/tauri/chats'
  import PatternList from './PatternList.svelte'

  interface Props {
    node: ActionNode
    graph: ActionGraph
    catalog: ActionCatalog | null
    owner: Owner
    signature: ActionPort[]
    diagnostics: ActionDiagnostic[]
    onchange: (node: ActionNode) => void
    onremove: () => void
    onopen: (actionId: string) => void
    ontemplate: () => void
    actionId?: string | null
    part?: { name: string; onedit: () => void; onclone?: () => void } | null
  }

  let { node, actionId = null, graph, catalog, owner, signature, diagnostics, onchange, onremove, onopen, ontemplate, part = null }: Props = $props()

  let hook = $state<string | null>(null)
  $effect(() => {
    const id = actionId
    const nodeId = node.id
    hook = null
    if (id && node.config?.on === 'custom') api.getActionHook(id, nodeId).then((url) => (hook = url)).catch(() => {})
  })

  let editor = $state<CodeEditor | null>(null)
  let showReference = $state(false)
  let editing = $state<string | null>(null)

  const ports = $derived(portsOf(node, catalog, owner) ?? { inputs: [], outputs: [] })
  const finals = $derived(signature.filter((p) => p.node === node.id))
  const trigger = $derived(triggerOf(node))
  const chat = $derived(chatOf(node))
  const variant = $derived(chat ? (catalog?.chat_node?.variants[chat.mode] ?? null) : null)
  const patterned = $derived(node.type === `${BUILTIN}receipt_match`)
  const wired = $derived(new Set(graph.edges.filter((e) => e.target === node.id).map((e) => e.target_port)))
  const loose = $derived(ports.inputs.filter((p) => !wired.has(p.name) && !(patterned && p.name === 'patterns')))
  const problems = $derived(problemsFor(diagnostics, node.id, 'node'))
  const needs = $derived(requirementsFor(diagnostics, node.id))
  const scope = $derived((trigger?.scope ?? {}) as NonNullable<TriggerConfig['scope']>)
  const values = $derived(((node.config?.values ?? {}) as Record<string, unknown>))
  const labels = $derived((node.config?.labels ?? {}) as Record<string, string>)
  const bodyInputs = $derived(node.type === NODE_TRIGGER ? [...ports.outputs] : ports.inputs)
  const overrides = $derived((node.config?.prompts ?? {}) as Record<string, PromptOverride>)
  const code = $derived(node.type === NODE_SCRIPT ? node.body : node.type.startsWith(BUILTIN) ? (catalog?.builtins.find((b) => `${BUILTIN}${b.key}` === node.type)?.body ?? node.body) : '')
  const used = $derived(promptsUsed(code).map((key) => ({ key, base: catalog?.prompts.find((p) => p.key === key) ?? null, content: catalog?.content_prompts.find((p) => p.id === key || p.name.toLowerCase() === key.toLowerCase()) ?? null })))
  const editingBase = $derived(editing ? (catalog?.prompts.find((p) => p.key === editing) ?? null) : null)

  const ON_OPTIONS: { value: TriggerOn; label: string }[] = $derived([
    { value: 'arrival', label: t('actions.inspector.on.arrival') },
    { value: 'sent', label: t('actions.inspector.on.sent') },
    { value: 'edited', label: t('actions.inspector.on.edited') },
    { value: 'deleted', label: t('actions.inspector.on.deleted') },
    { value: 'reaction', label: t('actions.inspector.on.reaction') },
    { value: 'mention', label: t('actions.inspector.on.mention') },
    { value: 'filter', label: t('actions.inspector.on.filter') },
    { value: 'schedule', label: t('actions.inspector.on.schedule') },
    { value: 'event_soon', label: t('actions.inspector.on.eventSoon') },
    { value: 'manual', label: t('actions.inspector.on.manual') },
    { value: 'called', label: t('actions.inspector.on.called') },
    { value: 'custom', label: t('actions.inspector.on.custom') }
  ])
  const MESSAGE_ONS: TriggerOn[] = ['arrival', 'sent', 'edited', 'deleted', 'reaction', 'mention', 'filter']
  const DEFAULT_ANY: TriggerOn[] = ['sent', 'edited', 'deleted', 'reaction']
  const CHAT_KINDS: { value: ConversationType; label: MessageKey }[] = [
    { value: 'email', label: 'actions.inspector.mail' },
    { value: 'direct_message', label: 'actions.inspector.directChats' },
    { value: 'group_message', label: 'actions.inspector.groupChats' },
    { value: 'channel', label: 'actions.inspector.channels' }
  ]
  const MESSAGE_KINDS: { value: string; label: MessageKey }[] = [
    { value: 'email', label: 'actions.filter.kind.email' },
    { value: 'chat', label: 'actions.filter.kind.chat' },
    { value: 'channel', label: 'actions.filter.kind.channel' },
    { value: 'other', label: 'actions.filter.kind.other' }
  ]
  const providerKinds = $derived([...new Set($providersStore.data.map((p) => p.provider_type))])
  const controls = $derived(configSchemaOf(node))

  function flip(list: string[], value: string): string[] {
    return list.includes(value) ? list.filter((v) => v !== value) : [...list, value]
  }

  function setScope(patch: Partial<GroupQuery>) {
    setTrigger({ scope: { ...scope, ...patch } })
  }

  function config(patch: Record<string, unknown>) {
    onchange({ ...node, config: { ...(node.config ?? {}), ...patch } })
  }

  function setTrigger(patch: Partial<TriggerConfig>) {
    config({ ...(trigger ?? {}), ...patch })
  }

  function setValue(port: ActionPort, raw: string) {
    const parsed = port.type === 'number' ? (raw === '' ? null : Number(raw)) : port.type === 'bool' ? raw === 'true' : port.type === 'list' ? raw.split(',').map((s) => s.trim()).filter(Boolean) : raw === '' ? null : raw
    config({ values: { ...values, [port.name]: parsed } })
  }

  function shown(value: unknown): string {
    if (value === null || value === undefined) return ''
    return Array.isArray(value) ? value.join(', ') : String(value)
  }

  function choices(port: ActionPort): { value: string; label: string }[] | null {
    if (port.name === 'split') return [{ value: '', label: t('actions.inspector.chooseSplit') }, ...(catalog?.splits ?? []).map((s) => ({ value: s.name, label: s.name }))]
    if (port.type === 'tag') return [{ value: '', label: t('actions.inspector.chooseTag') }, ...(catalog?.tags ?? []).map((l) => ({ value: l.name, label: l.name }))]
    if (port.type === 'bool') return [{ value: 'false', label: t('common.no') }, { value: 'true', label: t('common.yes') }]
    return null
  }

  function setLabel(port: string, text: string) {
    const next = { ...labels }
    if (text.trim()) next[port] = text
    else delete next[port]
    config({ labels: next })
  }

  function setControls(next: Record<string, unknown>) {
    if (controls) onchange({ ...node, config: withValue(node.config ?? {}, controls.path, next) as Record<string, unknown> })
  }
</script>

{#snippet chips(title: string, options: { value: string; label: string }[], chosen: string[], onpick: (next: string[]) => void)}
  {#if options.length}
    <div class="flex flex-wrap items-center gap-1" role="group" aria-label={title}>
      <span class="w-full text-[12px] text-ink-3">{title}{chosen.length ? '' : ` · ${t('actions.filter.any')}`}</span>
      {#each options as option (option.value)}
        {@const on = chosen.includes(option.value)}
        <button type="button" aria-pressed={on} class="h-6 max-w-full overflow-hidden text-ellipsis whitespace-nowrap rounded-[0.375rem] px-1.5 text-[12px] {on ? 'bg-accent-soft font-medium text-accent' : 'text-ink-2 hover:bg-paper-3 hover:text-ink'}" onclick={() => onpick(flip(chosen, option.value))}>{option.label}</button>
      {/each}
    </div>
  {/if}
{/snippet}

{#snippet picked(title: string, placeholder: string, options: { value: string; label: string }[], chosen: string[], onpick: (next: string[]) => void)}
  <div class="flex flex-col gap-1" role="group" aria-label={title}>
    <span class="text-[12px] text-ink-3">{title}{chosen.length ? '' : ` · ${t('actions.filter.any')}`}</span>
    {#if chosen.length}
      <div class="flex flex-wrap gap-1">
        {#each chosen as id (id)}
          <button type="button" class="flex h-6 max-w-full items-center gap-1 rounded-[0.375rem] bg-accent-soft px-1.5 text-[12px] text-accent" aria-label={t('actions.filter.remove', { name: options.find((o) => o.value === id)?.label ?? id })} onclick={() => onpick(chosen.filter((c) => c !== id))}>
            <span class="overflow-hidden text-ellipsis whitespace-nowrap">{options.find((o) => o.value === id)?.label ?? id}</span><Icon icon="lucide:x" class="shrink-0 text-[11px]" />
          </button>
        {/each}
      </div>
    {/if}
    <Select size="sm" value="" options={[{ value: '', label: placeholder }, ...options.filter((o) => !chosen.includes(o.value))]} onchange={(id) => id && onpick([...chosen, id])} />
  </div>
{/snippet}

{#snippet prompts()}
  {#if used.length}
    <div class="flex flex-col gap-1">
      <span class="text-[12px] font-medium uppercase tracking-wide text-ink-3">{t('actions.inspector.prompts')}</span>
      {#each used as { key, base, content } (key)}
        {@const custom = !!overrides[key]}
        {@const linked = overrides[key]?.content_id ? catalog?.content_prompts.find((p) => p.id === overrides[key]?.content_id) : undefined}
        {#if base}
          <button type="button" class="grid h-8 grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center rounded-[0.375rem] px-1 text-left text-[13px] text-ink-2 hover:bg-paper-3 hover:text-ink" onclick={() => (editing = key)}>
            <Icon icon={linked ? 'lucide:library' : 'lucide:message-square-text'} class="text-[14px] text-ink-3" />
            <span class="overflow-hidden text-ellipsis whitespace-nowrap">{linked ? linked.name : base.name}</span>
            <span class="font-mono text-[11px] {custom ? 'text-accent' : 'text-ink-3'}">{linked ? t('content.title') : custom ? t('actions.inspector.custom') : t('actions.inspector.default')}</span>
          </button>
        {:else if content}
          <a href="/main/content" class="grid h-8 grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center rounded-[0.375rem] px-1 text-left text-[13px] text-ink-2 hover:bg-paper-3 hover:text-ink">
            <Icon icon="lucide:library" class="text-[14px] text-ink-3" />
            <span class="overflow-hidden text-ellipsis whitespace-nowrap">{content.name}</span>
            <span class="font-mono text-[11px] text-accent">{t('content.title')}</span>
          </a>
        {:else}
          <span class="grid h-8 grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center px-1 text-[13px] text-ink-3">
            <Icon icon="lucide:message-square-text" class="text-[14px]" />
            <span class="overflow-hidden text-ellipsis whitespace-nowrap">{key}</span>
            <span class="font-mono text-[11px]">{t('actions.inspector.unknown')}</span>
          </span>
        {/if}
      {/each}
    </div>
  {/if}
{/snippet}

<div class="flex min-h-0 flex-1 flex-col gap-4">
  <header class="flex items-start gap-2">
    <span class="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-[0.375rem] bg-paper-3 text-[14px] text-ink-2"><Icon icon={iconOf(node, catalog)} /></span>
    <div class="flex min-w-0 flex-1 flex-col">
      <input
        class="h-7 w-full border-b border-transparent bg-transparent text-[14px] font-semibold text-ink outline-none focus:border-line"
        value={node.label}
        placeholder={label(node, catalog)}
        aria-label={t('actions.inspector.nodeName')}
        readonly={!!part}
        oninput={(e) => onchange({ ...node, label: e.currentTarget.value })}
      />
      <span class="font-mono text-[11px] text-ink-3">{kindLabel(node)} · {node.id}</span>
    </div>
    {#if node.type.startsWith(ACTION)}
      <IconButton size="sm" icon="lucide:external-link" label={t('actions.inspector.openAction')} onclick={() => onopen(node.type.slice(ACTION.length))} />
    {/if}
    {#if !part}
      {#if trigger}
        <span class="mt-1"><Switch testid="node-enabled" label={trigger.disabled ? t('actions.inspector.triggerOn') : t('actions.inspector.triggerOff')} checked={!trigger.disabled} onchange={(on) => setTrigger({ disabled: !on })} /></span>
      {/if}
      <IconButton size="sm" testid="node-remove" icon="lucide:trash-2" tone="danger" label={t('actions.inspector.removeNode')} onclick={onremove} />
    {/if}
  </header>

  {#if part}
    <div class="flex flex-col gap-2 border-y border-line py-2 text-[12.5px] leading-5 text-ink-2" role="note">
      <span class="flex items-center gap-1.5"><Icon icon="lucide:lock" class="shrink-0 text-ink-3" />{t('actions.frame.partOf', { name: part.name })}</span>
      <span class="flex gap-1.5">
        <Button size="sm" icon="lucide:square-pen" title={t('actions.frame.editNamed', { name: part.name })} onclick={part.onedit}>{t('actions.frame.edit')}</Button>
        {#if part.onclone}<Button size="sm" icon="lucide:copy-plus" title={t('actions.frame.cloneNamed', { name: part.name })} onclick={part.onclone}>{t('actions.frame.clone')}</Button>{/if}
      </span>
    </div>
  {/if}

  <fieldset class="contents" disabled={!!part}>

  {#if problems.length}
    <ul class="flex flex-col gap-1">
      {#each problems as problem, i (i)}
        <li class="flex gap-1.5 text-[12.5px] leading-4 text-error-600"><Icon icon="lucide:circle-alert" class="mt-0.5 shrink-0" />{problem.line ? t('actions.inspector.line', { line: problem.line }) : ''}{problem.message}</li>
      {/each}
    </ul>
  {/if}

  {#if needs.length}
    <div class="flex flex-col gap-1">
      <span class="flex items-center gap-1.5 text-[12px] font-medium uppercase tracking-wide text-warn-600"><Icon icon="lucide:triangle-alert" class="text-[13px]" />{t('actions.requirement.heading')}</span>
      {#each needs as need (need.setting)}
        <a href={need.href} data-testid="node-need" data-setting={need.setting} class="grid h-8 grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center rounded-[0.375rem] px-1 text-left text-[13px] text-ink-2 hover:bg-paper-3 hover:text-ink" aria-label={t('actions.requirement.open', { name: need.label })} onclick={(e) => { e.preventDefault(); goto(need.href) }}>
          <span class="h-3.5 w-1 justify-self-center rounded-[0.125rem] bg-warn-500"></span>
          <span class="overflow-hidden text-ellipsis whitespace-nowrap">{need.label}</span>
          <span class="flex items-center gap-1 font-mono text-[11px] text-ink-3">{need.setting}<Icon icon="lucide:arrow-up-right" class="text-[12px]" /></span>
        </a>
      {/each}
    </div>
  {/if}

  {#if trigger}
    <div class="flex flex-col gap-3">
      <label class="flex flex-col gap-1 text-[12px] text-ink-3">{t('actions.inspector.runsOn')}
        <Select size="sm" testid="node-trigger-on" value={trigger.on} options={ON_OPTIONS} onchange={(on) => setTrigger({ on: on as TriggerOn })} />
        <div class="flex flex-col gap-1.5" data-testid="node-trigger-labels">
          <span class="text-[12px] font-medium uppercase tracking-wide text-ink-3">{t('actions.inspector.portLabels')}</span>
          {#each ports.outputs as port (port.name)}
            <label class="grid grid-cols-[minmax(0,0.6fr)_minmax(0,1fr)] items-center gap-2 text-[12.5px]">
              <span class="flex min-w-0 items-center gap-1.5 font-mono text-ink-2"><span class="overflow-hidden text-ellipsis whitespace-nowrap">{port.name}</span><span class="text-[10.5px] text-ink-3">{port.type}</span></span>
              <input class="h-8 min-w-0 border-b border-line bg-transparent text-ink outline-none placeholder:text-ink-3 focus:border-ink-3" placeholder={t('actions.inspector.portLabelPlaceholder')} value={labels[port.name] ?? ''} oninput={(e) => setLabel(port.name, e.currentTarget.value)} />
            </label>
          {/each}
        </div>
      </label>
      {#if MESSAGE_ONS.includes(trigger.on)}
        <div class="flex flex-col gap-2">
          <span class="text-[12px] font-medium uppercase tracking-wide text-ink-3">{t('actions.inspector.where')}</span>
          {@render chips(t('actions.filter.kinds'), providerKinds.map((kind) => ({ value: kind, label: providerLabel(kind) })), scope.providers ?? [], (providers) => setScope({ providers }))}
          {@render chips(t('actions.filter.connections'), $providersStore.data.map((p) => ({ value: p.id, label: p.name })), scope.connection_ids ?? [], (connection_ids) => setScope({ connection_ids }))}
          {@render chips(t('actions.filter.chatKinds'), CHAT_KINDS.map((k) => ({ value: k.value, label: t(k.label) })), scope.types ?? [], (types) => setScope({ types: types as ConversationType[] }))}
          {@render chips(t('actions.filter.messageKinds'), MESSAGE_KINDS.map((k) => ({ value: k.value, label: t(k.label) })), trigger.message_kinds ?? [], (message_kinds) => setTrigger({ message_kinds }))}
          {@render chips(t('actions.filter.groups'), (catalog?.groups ?? []).map((g) => ({ value: g.id, label: g.name })), trigger.group_ids ?? [], (group_ids) => setTrigger({ group_ids }))}
          {@render chips(t('actions.filter.tags'), (catalog?.tags ?? []).map((l) => ({ value: l.id, label: l.name })), scope.tag_ids ?? [], (tag_ids) => setScope({ tag_ids }))}
          {@render picked(t('actions.filter.chats'), t('actions.filter.addChat'), $conversationsStore.data.filter((c) => !c.deleted_at).map((c) => ({ value: c.id, label: c.name })), scope.conversation_ids ?? [], (conversation_ids) => setScope({ conversation_ids }))}
          {@render picked(t('actions.filter.people'), t('actions.filter.addPerson'), $peopleStore.map((p) => ({ value: p.id, label: p.display_name })), scope.person_ids ?? [], (person_ids) => setScope({ person_ids }))}
          <div class="grid grid-cols-[minmax(0,0.45fr)_minmax(0,1fr)] items-center gap-2 text-[12px] text-ink-3">
            <span>{t('actions.filter.direction')}</span>
            <Segmented size="sm" label={t('actions.filter.direction')} value={trigger.direction ?? (DEFAULT_ANY.includes(trigger.on) ? 'any' : 'incoming')} options={[{ value: 'incoming', label: t('actions.filter.incoming') }, { value: 'outgoing', label: t('actions.filter.outgoing') }, { value: 'any', label: t('actions.filter.anyDirection') }]} onchange={(direction) => setTrigger({ direction: direction as TriggerDirection })} />
            <span>{t('actions.filter.attachments')}</span>
            <Segmented size="sm" label={t('actions.filter.attachments')} value={scope.has_attachments === true ? 'with' : scope.has_attachments === false ? 'without' : 'any'} options={[{ value: 'any', label: t('actions.filter.anyAttachments') }, { value: 'with', label: t('actions.filter.withAttachments') }, { value: 'without', label: t('actions.filter.withoutAttachments') }]} onchange={(v) => setScope({ has_attachments: v === 'with' ? true : v === 'without' ? false : undefined })} />
          </div>
          <label class="flex items-center justify-between text-[12.5px] text-ink-2">{t('actions.filter.humansOnly')}
            <Switch label={t('actions.filter.humansOnly')} checked={!!scope.humans_only} onchange={(on) => setScope({ humans_only: on || undefined })} />
          </label>
        </div>
      {/if}
      {#if trigger.on === 'event_soon' && controls}
        <SchemaControls testid="inspector-controls" schema={controls.schema} value={configValueOf(node, controls)} onchange={setControls} />
        {@render picked(t('actions.filter.chats'), t('actions.filter.addChat'), $conversationsStore.data.filter((c) => !c.deleted_at).map((c) => ({ value: c.id, label: c.name })), scope.conversation_ids ?? [], (conversation_ids) => setScope({ conversation_ids }))}
      {/if}
      {#if trigger.on === 'schedule' && controls}
        <SchemaControls testid="inspector-controls" schema={controls.schema} value={configValueOf(node, controls)} onchange={setControls} />
      {/if}
      {#if trigger.on === 'custom' && controls}
        <SchemaControls testid="inspector-controls" schema={controls.schema} value={configValueOf(node, controls)} onchange={setControls} />
        {#if trigger.watch?.mode !== 'poll' && hook}
          <div class="flex flex-col gap-1 text-[12px] text-ink-3" data-testid="inspector-hook">
            <span>{t('actions.inspector.watchAddress')}</span>
            <div class="flex min-w-0 items-center gap-1.5 rounded-[0.5rem] bg-paper-2 px-2 py-1.5">
              <code class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11.5px] text-ink-2">{hook}{trigger.watch?.token ? `?token=${trigger.watch.token}` : ''}</code>
              <IconButton icon="lucide:copy" size="sm" label={t('common.copy')} onclick={() => navigator.clipboard.writeText(hook + (trigger.watch?.token ? `?token=${trigger.watch.token}` : ''))} />
            </div>
            <span class="leading-5">{t('actions.inspector.watchHint')}</span>
          </div>
        {/if}
      {/if}
      <div class="flex flex-col gap-1">
        <span class="text-[12px] font-medium uppercase tracking-wide text-ink-3">{t('actions.inspector.onlyWhen')}</span>
        <div class="h-[9rem] border-b border-line">
          <CodeEditor bind:this={editor} label={t('actions.inspector.condition')} readonly={!!part} value={node.body} functions={catalog?.functions ?? []} inputs={bodyInputs} diagnostics={problems.filter((p) => p.line)} onchange={(body) => onchange({ ...node, body })} />
        </div>
      </div>
    </div>
  {:else if chat}
    <div class="flex flex-col gap-3">
      <Segmented size="sm" label={t('actions.chat.mode')} value={chat.mode} options={CHAT_MODES.map((m) => ({ value: m.value, label: t(m.label), icon: m.icon }))} onchange={(mode) => config({ mode })} />
      <label class="flex flex-col gap-1 text-[12px] text-ink-3">{t('actions.chat.which')}
        <Select size="sm" value={chat.conversation_id ?? ''} options={[{ value: '', label: t('actions.chat.attached') }, ...$conversationsStore.data.filter((c) => !c.deleted_at).map((c) => ({ value: c.id, label: c.name }))]} onchange={(id) => config({ conversation_id: id || null })} />
      </label>
      {#if variant}
        <p class="text-[12.5px] leading-5 text-ink-2">{variant.description}</p>
        <dl class="grid grid-cols-[minmax(0,max-content)_minmax(0,1fr)] gap-x-3 gap-y-1 text-[12.5px]">
          <dt class="text-ink-3">{t('actions.chat.start')}</dt>
          <dd class="text-ink-2">{variant.start}</dd>
          <dt class="text-ink-3">{t('actions.chat.end')}</dt>
          <dd class="text-ink-2">{variant.end}</dd>
        </dl>
      {/if}
      <p class="text-[12px] leading-5 text-ink-3">{wired.size ? t('actions.chat.asEnd') : t('actions.chat.asStart')}</p>
      <div class="grid grid-cols-2 gap-3">
        <SignatureForm title={t('actions.page.inputs')} ports={ports.inputs} types={catalog?.types ?? []} readonly onchange={() => {}} />
        <SignatureForm title={t('actions.page.outputs')} ports={ports.outputs} types={catalog?.types ?? []} readonly onchange={() => {}} />
      </div>
    </div>
  {:else if node.type === NODE_SCRIPT}
    <SignatureForm title={t('actions.page.inputs')} ports={ports.inputs} types={catalog?.types ?? []} readonly={!!part} onchange={(inputs) => config({ inputs })} />
    <SignatureForm title={t('actions.page.outputs')} ports={ports.outputs} types={catalog?.types ?? []} readonly={!!part} onchange={(outputs) => config({ outputs })} />
    {#if finals.length}
      <p class="text-[12px] leading-5 text-ink-3">{t('actions.inspector.final')}</p>
    {/if}
    {@render prompts()}
  {:else}
    {#if node.type.startsWith(BUILTIN)}
      <p class="text-[12.5px] leading-5 text-ink-2">{catalog?.builtins.find((b) => `${BUILTIN}${b.key}` === node.type)?.description}</p>
    {/if}
    {@render prompts()}
    {#if patterned}
      <PatternList patterns={Array.isArray(values.patterns) ? values.patterns : []} onchange={(patterns) => config({ values: { ...values, patterns } })} />
    {/if}
    {#if presenterKind(node)}
      {@const custom = typeof node.config?.template === 'string' && node.config.template.trim() !== ''}
      <div class="flex flex-col gap-1.5">
        <span class="text-[12px] font-medium uppercase tracking-wide text-ink-3">{t('actions.inspector.template')}</span>
        <p class="text-[12.5px] leading-5 text-ink-2">{custom ? t('actions.inspector.templateCustom') : t('actions.inspector.templateBuiltin')}</p>
        <div class="flex gap-2">
          <Button size="sm" icon="lucide:code" onclick={ontemplate}>{custom ? t('actions.inspector.editTemplate') : t('actions.inspector.customise')}</Button>
          {#if custom}<Button size="sm" onclick={() => config({ template: '' })}>{t('actions.inspector.useBuiltin')}</Button>{/if}
        </div>
      </div>
    {/if}
    {#if finals.length}
      <SignatureForm title={t('actions.page.inputs')} ports={ports.inputs} types={catalog?.types ?? []} readonly onchange={() => {}} />
      <div class="flex flex-col gap-1">
        <SignatureForm title={t('actions.page.outputs')} ports={finals} types={catalog?.types ?? []} fixed readonly={!!part} onchange={(next) => onchange(withReturns(node, finals, next))} />
        <p class="text-[12px] leading-5 text-ink-3">{t('actions.inspector.final')}</p>
      </div>
    {:else}
      <div class="grid grid-cols-2 gap-3">
        <SignatureForm title={t('actions.page.inputs')} ports={ports.inputs} types={catalog?.types ?? []} readonly onchange={() => {}} />
        <SignatureForm title={t('actions.page.outputs')} ports={ports.outputs} types={catalog?.types ?? []} readonly onchange={() => {}} />
      </div>
    {/if}
  {/if}

  {#if !trigger && !chat && loose.length}
    <div class="flex flex-col gap-1.5">
      <span class="text-[12px] font-medium uppercase tracking-wide text-ink-3">{t('actions.inspector.values')}</span>
      {#each loose as port (port.name)}
        {@const options = choices(port)}
        <label class="grid grid-cols-[minmax(0,0.6fr)_minmax(0,1fr)] items-center gap-2 text-[12.5px]">
          <span class="font-mono text-ink-2">{port.name}</span>
          {#if options}
            <Select size="sm" value={shown(values[port.name])} {options} onchange={(v) => setValue(port, v)} />
          {:else}
            <input class="h-8 min-w-0 border-b border-line bg-transparent text-ink outline-none placeholder:text-ink-3 focus:border-ink-3" placeholder={t('actions.inspector.valuePlaceholder')} value={shown(values[port.name])} oninput={(e) => setValue(port, e.currentTarget.value)} />
          {/if}
        </label>
      {/each}
    </div>
  {/if}

  {#if !trigger && !chat}
    <div class="flex min-h-0 flex-1 flex-col gap-1">
      <div class="flex h-7 items-center justify-between">
        <span class="text-[12px] font-medium uppercase tracking-wide text-ink-3">{editable(node) && !part ? t('actions.inspector.code') : t('actions.inspector.codeReadOnly')}</span>
        {#if editable(node) && !part}
          <IconButton size="sm" icon="lucide:book-open" label={t('actions.inspector.reference')} active={showReference} onclick={() => (showReference = !showReference)} />
        {/if}
      </div>
      {#if node.type.startsWith(ACTION)}
        <p class="text-[12.5px] text-ink-3">{t('actions.inspector.runsAction')}</p>
      {:else}
        <div class="min-h-[10rem] flex-1 border-b border-line">
          <CodeEditor bind:this={editor} label={t('actions.inspector.nodeCode')} readonly={!!part || !editable(node)} value={node.body} functions={catalog?.functions ?? []} inputs={bodyInputs} diagnostics={problems.filter((p) => p.line)} onchange={(body) => onchange({ ...node, body })} />
        </div>
      {/if}
      {#if showReference && editable(node)}
        <div class="max-h-[16rem] min-h-0">
          <ReferencePanel functions={catalog?.functions ?? []} inputs={bodyInputs} oninsert={(text) => editor?.insert(text)} />
        </div>
      {/if}
    </div>
  {/if}
  </fieldset>
</div>

{#if editing && editingBase}
  {@const key = editing}
  <PromptEditor prompt={resolvePrompt(editingBase, overrides[key], catalog?.content_prompts ?? [])} base={editingBase} node={node.label || node.id} contentId={overrides[key]?.content_id ?? null} contentPrompts={catalog?.content_prompts ?? []} onsave={(override) => { onchange(withPrompt(node, key, override)); editing = null }} onclose={() => (editing = null)} />
{/if}
