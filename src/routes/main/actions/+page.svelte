<script lang="ts">
  import { isTyping } from '$lib/shortcuts'
  import { onMount, tick, untrack } from 'svelte'
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { page } from '$app/state'
  import { listen } from '@tauri-apps/api/event'
  import { SvelteFlowProvider } from '@xyflow/svelte'
  import { api, conversationsStore, chatsStore, loadChats } from '$lib/tauri'
  import { anchored, boxOf } from '$lib/popover'
  import type { Action, ActionCatalog, ActionDiagnostic, ActionDraft, ActionGraph, ActionGroup, ActionNode, ActionPort, ActionRun, ActionSample, ActionsOutcome } from '$lib/tauri/types'
  import { Button, IconButton, IconPicker, MenuItem, PaneGrid, Segmented, Switch, TagField } from '$lib/components/ui'
  import { themeChoice, resolvedTheme } from '$lib/theme-mode'
  import { BUILTIN_KEY, draftFor, presenterKind, inlineAction, mockKey, mockStored, mockWith, nestedOf, nodeFromAction, outputSources, partOf, playPorts, portsOf, prefillFrom, removeNodes, requirementsFor, schemaOf, triggerOf, usageBadges, withoutInvalidEdges, type MockOverrides, type Placeable, type TreeItem } from '$lib/actions'
  import { forgetTemplates, type TemplateKind } from '$lib/template'
  import { actionDefaults, actionSelection, type ModelSelection } from '$lib/intelligence'
  import { actionHref } from '$lib/settings'
  import { pulse } from '$lib/window'
  import { t } from '$lib/i18n.svelte'
  import ActionList from '$lib/components/actions/ActionList.svelte'
  import ActionGraphView from '$lib/components/actions/ActionGraph.svelte'
  import CodeView from '$lib/components/actions/CodeView.svelte'
  import NodeInspector from '$lib/components/actions/NodeInspector.svelte'
  import SignatureForm from '$lib/components/actions/SignatureForm.svelte'
  import PickerPanel from '$lib/components/actions/PickerPanel.svelte'
  import AssistantPanel from '$lib/components/actions/AssistantPanel.svelte'
  import RunPanel from '$lib/components/actions/RunPanel.svelte'
  import PlayPanel from '$lib/components/actions/PlayPanel.svelte'
  import RunOutcome from '$lib/components/actions/RunOutcome.svelte'
  import TemplateEditor from '$lib/components/actions/TemplateEditor.svelte'
  import GroupEditor from '$lib/components/settings/GroupEditor.svelte'
  import ModelChoice from '$lib/components/settings/ModelChoice.svelte'

  let actions = $state<Action[]>([])
  let groups = $state<ActionGroup[]>([])
  let catalog = $state<ActionCatalog | null>(null)
  let current = $state<Action | null>(null)
  let draft = $state<ActionDraft | null>(null)
  let dirty = $state(false)
  let diagnostics = $state<ActionDiagnostic[]>([])
  let outputs = $state<ActionPort[]>([])
  let selectedNode = $state<string | null>(null)
  let center = $state<'graph' | 'code' | 'runs'>('graph')
  let side = $state<'inspect' | 'picker' | 'assistant'>('picker')
  let playNode = $state<string | null>(null)
  let mockOpen = $state(false)
  let playBusy = $state<string | null>(null)
  let settings = $state<Record<string, string>>({})
  let sample = $state<ActionSample | null>(null)
  let mockTimers: Record<string, ReturnType<typeof setTimeout>> = {}
  let templateNode = $state<string | null>(null)
  let panelOpen = $state(false)
  const sideOpen = $derived(!!current && !!draft && panelOpen)
  let run = $state<ActionRun | null>(null)
  let failure = $state<string | null>(null)
  let notice = $state<string | null>(null)
  let saving = $state(false)
  let iconOpen = $state(false)
  let groupDialog = $state<{ group: ActionGroup | null; parentId: string | null } | null>(null)
  let assistInitial = $state('')
  let attachOpen = $state(false)
  let attachButton = $state<HTMLDivElement | null>(null)
  let attachQuery = $state('')
  let problemsOpen = $state(false)
  let problemsButton = $state<HTMLButtonElement | null>(null)
  let graphView = $state<ActionGraphView | null>(null)
  let checkTimer: ReturnType<typeof setTimeout> | null = null
  let noticeTimer: ReturnType<typeof setTimeout> | null = null

  const dark = $derived(resolvedTheme($themeChoice) === 'night')
  const owner = $derived({ inputs: draft?.inputs ?? [] })
  const sources = $derived(draft ? outputSources(outputs, draft.graph, catalog) : [])
  const node = $derived(draft?.graph.nodes.find((n) => n.id === selectedNode) ?? null)
  const part = $derived(draft ? partOf(draft.graph, catalog, selectedNode) : null)
  const playing = $derived(playNode ? (draft?.graph.nodes.find((n) => n.id === playNode) ?? null) : null)
  const templating = $derived(templateNode ? (draft?.graph.nodes.find((n) => n.id === templateNode) ?? null) : null)
  const errors = $derived(diagnostics.filter((d) => d.severity === 'error'))
  const needs = $derived(requirementsFor(diagnostics))
  const badges = $derived(usageBadges(current?.usage))
  const invalidEdges = $derived(errors.filter((d) => d.edge && draft?.graph.edges.some((e) => e.id === d.edge)).length)

  function removeInvalid() {
    if (!draft) return
    const count = invalidEdges
    setGraph(withoutInvalidEdges(draft.graph, diagnostics))
    problemsOpen = false
    say(t('actions.problems.fixed', { count }))
  }

  $effect(() => {
    if (selectedNode) untrack(() => {
      side = 'inspect'
    })
  })

  function say(message: string) {
    notice = message
    if (noticeTimer) clearTimeout(noticeTimer)
    noticeTimer = setTimeout(() => (notice = null), 4000)
  }

  function draftOf(action: Action): ActionDraft {
    return { name: action.name, description: action.description, icon: action.icon, max_sends_per_hour: action.max_sends_per_hour, inputs: structuredClone($state.snapshot(action.inputs)), graph: structuredClone($state.snapshot(action.graph)) }
  }

  async function load() {
    try {
      const [list, groupList, cat] = await Promise.all([api.getActions(), api.getActionGroups(), api.getActionCatalog()])
      actions = list
      groups = groupList
      catalog = cat
    } catch (e) {
      failure = String(e)
    }
  }

  async function open(id: string) {
    if (dirty && current && current.id !== id && !confirm(t('actions.page.leaveUnsaved', { name: current.name }))) return
    const action = actions.find((a) => a.id === id) ?? (await api.getAction(id))
    current = action
    draft = draftOf(action)
    outputs = action.outputs
    dirty = false
    selectedNode = null
    run = null
    playNode = null
    mockOpen = false
    templateNode = null
    side = 'inspect'
    if (page.url.searchParams.get('id') !== id) goto(actionHref(id), { replaceState: true, keepFocus: true, noScroll: true })
    await check()
  }

  async function check() {
    if (!draft) return
    try {
      const outcome = await api.postActionCheck({ draft: $state.snapshot(draft) as ActionDraft })
      diagnostics = outcome.diagnostics
      outputs = outcome.outputs
    } catch (e) {
      failure = String(e)
    }
  }

  function changed(next: ActionDraft) {
    if (current?.builtin) return
    draft = next
    dirty = true
    if (checkTimer) clearTimeout(checkTimer)
    checkTimer = setTimeout(check, 350)
  }

  function setGraph(graph: ActionGraph) {
    if (draft) changed({ ...draft, graph })
  }

  function setNode(next: ActionNode) {
    if (!draft) return
    setGraph({ ...draft.graph, nodes: draft.graph.nodes.map((n) => (n.id === next.id ? next : n)) })
  }

  function clone(nodeId: string) {
    if (!draft || current?.builtin) return
    const host = draft.graph.nodes.find((n) => n.id === nodeId)
    const nested = host ? nestedOf(host, catalog) : null
    if (!nested) return
    setGraph(inlineAction(draft.graph, nodeId, catalog, owner))
    selectedNode = null
    say(t('actions.frame.cloned', { name: nested.name }))
  }

  function playTarget(nodeId: string | null): string | null {
    const target = nodeId ? draft?.graph.nodes.find((n) => n.id === nodeId) : null
    return target && !triggerOf(target) ? target.id : null
  }

  function signatureOf(nodeId: string | null): { inputs: ActionPort[]; outputs: ActionPort[] } {
    const target = nodeId ? draft?.graph.nodes.find((n) => n.id === nodeId) : null
    if (target) return portsOf(target, catalog, owner) ?? { inputs: [], outputs: [] }
    return { inputs: draft ? playPorts(draft.graph, owner) : [], outputs }
  }

  function overridesOf(nodeId: string | null): MockOverrides {
    return current ? mockStored(settings[mockKey(current.id, nodeId)]) : {}
  }

  function setOverrides(nodeId: string | null, next: MockOverrides) {
    if (!current) return
    const key = mockKey(current.id, nodeId)
    const value = Object.keys(next).length ? JSON.stringify(next) : ''
    if ((settings[key] ?? '') === value) return
    const rest = { ...settings }
    delete rest[key]
    settings = value ? { ...rest, [key]: value } : rest
    clearTimeout(mockTimers[key])
    mockTimers[key] = setTimeout(() => api.putAppSetting(key, value).catch((e) => (failure = String(e))), 400)
  }

  async function runPlay(nodeId: string | null, inputs: Record<string, unknown>) {
    if (!current) return
    playBusy = nodeId ?? ''
    failure = null
    try {
      run = nodeId ? await api.postActionNodeRun(current.id, nodeId, inputs, true, null) : await api.postActionRun(current.id, inputs, true, null)
    } catch (e) {
      failure = String(e)
    } finally {
      playBusy = null
    }
  }

  function play(nodeId: string | null) {
    const target = playTarget(nodeId)
    const merged = mockWith(schemaOf(signatureOf(target).inputs), sample, overridesOf(target))
    setOverrides(target, merged.overrides)
    runPlay(target, merged.value)
  }

  function editMock(nodeId: string | null) {
    playNode = playTarget(nodeId)
    mockOpen = true
  }

  function switchCenter(next: 'graph' | 'code' | 'runs') {
    templateNode = null
    center = next
  }

  async function save() {
    if (!current || !draft) return
    saving = true
    failure = null
    try {
      const saved = await api.putAction({ action_id: current.id, name: draft.name, description: draft.description, icon: draft.icon, inputs: draft.inputs, graph: draft.graph, max_sends_per_hour: draft.max_sends_per_hour ?? undefined })
      current = saved
      draft = draftOf(saved)
      dirty = false
      actions = actions.map((a) => (a.id === saved.id ? saved : a))
      forgetTemplates()
      catalog = await api.getActionCatalog()
      await check()
      say(saved.enabled ? t('actions.page.saved') : errors.length ? t('actions.page.savedOff') : t('actions.page.saved'))
    } catch (e) {
      failure = String(e)
    } finally {
      saving = false
    }
  }

  async function toggle(action: Action) {
    try {
      const saved = await api.putAction({ action_id: action.id, enabled: !action.enabled })
      actions = actions.map((a) => (a.id === saved.id ? saved : a))
      if (current?.id === saved.id) current = saved
    } catch (e) {
      failure = String(e)
    }
  }

  async function putAi(selection: ModelSelection) {
    if (!current) return
    const saved = await api.putAction({ action_id: current.id, ai: { connection_id: selection.connection || null, model: selection.model || null } })
    current = saved
    actions = actions.map((a) => (a.id === saved.id ? saved : a))
  }

  async function focusAi() {
    panelOpen = true
    side = 'inspect'
    selectedNode = null
    await tick()
    const field = document.getElementById('action-ai')
    field?.scrollIntoView({ block: 'center', behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' })
    pulse(field)
  }

  async function duplicate() {
    if (!current || !draft) return
    const source = !!current.key?.startsWith(BUILTIN_KEY)
    try {
      const made = await api.postAction({ name: source ? t('actions.page.fromTrigger', { name: draft.name }) : t('actions.page.copyName', { name: draft.name }), description: draft.description, icon: draft.icon, group_id: current.builtin ? null : current.group_id, enabled: false, inputs: draft.inputs, graph: $state.snapshot(draft.graph) as ActionGraph, max_sends_per_hour: draft.max_sends_per_hour ?? undefined, ai: $state.snapshot(current.ai) })
      actions = [...actions, made]
      await open(made.id)
      say(source ? t('actions.page.madeFromTrigger') : t('actions.page.duplicated'))
    } catch (e) {
      failure = String(e)
    }
  }

  async function duplicateForChat() {
    if (!current) return
    try {
      const made = await api.postActionCopy(current.id, true, t('actions.attach.copyName', { name: current.name }))
      actions = [...actions, made]
      await open(made.id)
      say(t('actions.attach.copied'))
    } catch (e) {
      failure = String(e)
    }
  }

  async function toggleAttach(conversationId: string) {
    if (!current) return
    try {
      if (current.chats.includes(conversationId)) await api.deleteChatAction(conversationId, current.id)
      else await api.putChatAction(conversationId, current.id)
      const saved = await api.getAction(current.id)
      current = saved
      actions = actions.map((a) => (a.id === saved.id ? saved : a))
      await loadChats()
    } catch (e) {
      failure = String(e)
    }
  }

  const chatChoices = $derived.by(() => {
    const needle = attachQuery.trim().toLowerCase()
    const local = new Set($chatsStore.map((c) => c.conversation_id))
    return [...$conversationsStore.data]
      .filter((c) => !c.deleted_at && (!needle || c.name.toLowerCase().includes(needle)))
      .sort((a, b) => Number(local.has(b.id)) - Number(local.has(a.id)) || b.last_activity.localeCompare(a.last_activity))
      .slice(0, 40)
  })

  async function create(groupId: string | null) {
    try {
      const made = await api.postAction({ name: t('actions.draft.name'), group_id: groupId, enabled: false, graph: { nodes: [], edges: [] } })
      actions = [...actions, made]
      await open(made.id)
      side = 'picker'
    } catch (e) {
      failure = String(e)
    }
  }

  async function move(item: TreeItem, groupId: string | null, before: string | null) {
    try {
      if (item.kind === 'group' && item.group) {
        await api.putActionGroup({ group_id: item.group.id, parent_id: groupId })
      } else if (item.action) {
        const siblings = actions.filter((a) => (a.group_id ?? null) === groupId && a.id !== item.action!.id).sort((a, b) => a.position - b.position)
        const at = before ? Math.max(0, siblings.findIndex((a) => a.id === before)) : siblings.length
        const order = [...siblings.slice(0, at), item.action, ...siblings.slice(at)]
        for (const [position, action] of order.entries()) {
          if (action.position !== position || action.id === item.action.id) await api.putAction({ action_id: action.id, position, ...(action.id === item.action.id ? { group_id: groupId } : {}) })
        }
      }
      await load()
    } catch (e) {
      failure = String(e)
    }
  }

  async function remove() {
    if (!current || !confirm(t('actions.page.deleteConfirm', { name: current.name }))) return
    await api.deleteAction(current.id)
    current = null
    draft = null
    dirty = false
    await load()
    goto('/main/actions', { replaceState: true })
  }

  function drop(item: Placeable, x: number, y: number) {
    graphView?.dropAt(item, x, y)
    side = 'inspect'
  }

  function add(item: Placeable) {
    if (!draft) return
    const last = draft.graph.nodes.reduce((max, n) => Math.max(max, n.y), -160)
    const made = nodeFromAction(item, draft.graph, { x: 0, y: last + 180 })
    setGraph({ ...draft.graph, nodes: [...draft.graph.nodes, made] })
    selectedNode = made.id
    side = 'inspect'
  }

  async function refreshed(outcome: ActionsOutcome) {
    await load()
    forgetTemplates()
    if (!current) return
    const next = actions.find((a) => a.id === current!.id)
    if (!next) {
      current = null
      draft = null
      dirty = false
      goto('/main/actions', { replaceState: true })
      return
    }
    if (!outcome.actions.some((a) => a.id === next.id)) return
    current = next
    draft = draftOf(next)
    outputs = next.outputs
    dirty = false
    await check()
  }

  onMount(() => {
    const params = new URLSearchParams(page.url.searchParams)
    load().then(async () => {
      const prefill = prefillFrom(params)
      const assist = params.get('assist')
      if (prefill) {
        const made = await api.postAction(draftFor(prefill))
        actions = [...actions, made]
        await open(made.id)
        if (assist) {
          assistInitial = assist
          side = 'assistant'
        } else side = 'picker'
      } else if (params.get('id')) {
        await open(params.get('id')!)
        if (params.get('field') === 'ai') await focusAi()
      }
    })
    api.getAppSettings().then((all) => (settings = all)).catch((e) => (failure = String(e)))
    api.getActionSample().then((found) => (sample = found)).catch((e) => (failure = String(e)))
    const offs = [listen('actions-updated', load), listen('action-runs-updated', load)]
    const key = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || !current) return
      const target = e.target as HTMLElement | null
      const typing = isTyping(target)
      if (target?.closest('[role="dialog"]')) return
      if (e.key === 's' && !current.builtin) {
        e.preventDefault()
        save()
      } else if (e.key === 'd' && !e.shiftKey) {
        e.preventDefault()
        duplicate()
      } else if (e.key === 'Enter' && e.altKey) {
        e.preventDefault()
        editMock(null)
      } else if (e.key === 'Enter' && !typing) {
        e.preventDefault()
        play(null)
      } else if (e.key === 'Backspace' && !typing && !current.builtin) {
        e.preventDefault()
        remove()
      }
    }
    window.addEventListener('keydown', key)
    return () => {
      offs.forEach((off) => off.then((stop) => stop()))
      window.removeEventListener('keydown', key)
    }
  })
</script>

<PaneGrid name="actions" panes={[{ key: 'list', size: '26vw', min: '20rem', max: '40vw', edge: 'end', class: 'bg-paper' }, { key: 'main' }, { key: 'side', size: '24rem', min: '18rem', max: '40vw', edge: 'start', open: sideOpen, class: 'bg-paper' }]}>
  {#snippet pane(key)}
  {#if key === 'list'}
  <aside class="flex min-h-0 flex-1 flex-col">
    <ActionList
      {groups}
      {actions}
      {catalog}
      selected={current?.id ?? null}
      onselect={open}
      oncreate={create}
      ongroup={(group) => (groupDialog = { group, parentId: group?.parent_id ?? null })}
      onmove={move}
      ongraph={(action, x, y) => { if (current && !current.builtin && action.id !== current.id) drop(action, x, y) }}
    />
  </aside>
  {:else if key === 'main'}
  {#if current && draft}
    <main class="flex h-full min-h-0 min-w-0 flex-col bg-paper">
      <header class="flex flex-col gap-1.5 border-b border-line bg-[linear-gradient(to_bottom,var(--paper-2),transparent)] px-5 pb-2 pt-4">
        <div class="flex items-center gap-2">
          <button type="button" class="flex size-8 shrink-0 items-center justify-center rounded-[0.5rem] bg-paper-3 text-[15px] text-ink-2 hover:text-ink disabled:cursor-default" title={t('actions.page.changeIcon')} disabled={current.builtin} onclick={() => (iconOpen = true)}><Icon icon={draft.icon ?? 'lucide:zap'} /></button>
          <input
            data-testid="action-name"
            class="h-9 min-w-0 flex-1 border-b border-transparent bg-transparent text-[18px] font-semibold tracking-tight text-ink outline-none focus:border-line"
            value={draft.name}
            readonly={current.builtin}
            aria-label={t('actions.page.name')}
            oninput={(e) => changed({ ...draft!, name: e.currentTarget.value })}
          />
          {#if current.key}<span data-testid="action-key" class="font-mono text-[11px] text-ink-3" title={current.builtin ? t('actions.page.builtinReadOnly') : t('actions.page.callByKey')}>{current.key}</span>{/if}
          {#each badges as badge (badge.key)}<span data-testid="action-badge" data-key={badge.key} class="flex shrink-0 items-center gap-1 rounded-full bg-paper-3 px-2 text-[11.5px] leading-5 text-ink-2" title={badge.hint}><Icon icon={badge.icon} class="text-[12px]" />{badge.key === 'chat' && current.chats.length ? t('actions.attach.attachedTo', { count: current.chats.length }) : badge.label}</span>{/each}
          <IconButton testid="action-panel" icon={panelOpen ? 'lucide:panel-right-close' : 'lucide:panel-right-open'} label={panelOpen ? t('actions.page.hidePanel') : t('actions.page.showPanel')} active={panelOpen} onclick={() => (panelOpen = !panelOpen)} />
        </div>
        <input
          data-testid="action-description"
          class="h-7 w-full border-b border-transparent bg-transparent text-[13px] text-ink-2 outline-none placeholder:text-ink-3 focus:border-line"
          value={draft.description}
          readonly={current.builtin}
          placeholder={t('actions.page.descriptionPlaceholder')}
          aria-label={t('actions.page.description')}
          oninput={(e) => changed({ ...draft!, description: e.currentTarget.value })}
        />
        <TagField objectType="action" objectId={current.id} />
        <div class="flex min-w-0 items-center gap-3" role="toolbar" aria-label={t('actions.page.controls')}>
          <span class="shrink-0"><Segmented size="sm" testid="action-view" label={t('actions.page.view')} value={center} options={[{ value: 'graph', label: t('actions.page.graph'), icon: 'lucide:workflow' }, { value: 'code', label: t('actions.page.codeView'), icon: 'lucide:code' }, { value: 'runs', label: t('actions.page.runs'), icon: 'lucide:history' }]} onchange={(v) => switchCenter(v as 'graph' | 'code' | 'runs')} /></span>
          <span class="flex min-w-0 items-center gap-3 overflow-hidden whitespace-nowrap">
            {#if current.builtin}
              <span data-testid="action-builtin" class="flex items-center gap-1 text-[12.5px] text-ink-3"><Icon icon="lucide:lock" />{t('actions.page.builtinHint')}</span>
            {:else if errors.length}
              <button type="button" data-testid="action-problems" data-count={errors.length} class="flex items-center gap-1 text-[12.5px] text-error-600 hover:underline" aria-expanded={problemsOpen} bind:this={problemsButton} onclick={() => (problemsOpen = !problemsOpen)}><Icon icon="lucide:circle-alert" />{t('actions.page.problems', { count: errors.length })}</button>
              {#if problemsOpen}
                <div class="menu flex w-96 max-w-[90vw] flex-col gap-1" use:anchored={{ at: () => boxOf(problemsButton), align: 'start', onclose: () => (problemsOpen = false) }} role="dialog" data-testid="action-problems-list" aria-label={t('actions.page.problems', { count: errors.length })}>
                  {#if invalidEdges}
                    <Button size="sm" testid="action-problems-fix" icon="lucide:unplug" onclick={removeInvalid}>{t('actions.problems.fix')}</Button>
                  {/if}
                  <ul class="flex max-h-[18rem] flex-col gap-1 overflow-y-auto">
                    {#each errors as d, i (i)}
                      <li data-testid="action-problem" data-edge={d.edge} data-node={d.node} class="flex gap-1.5 text-[12.5px] leading-4 text-error-600">
                        <Icon icon={d.edge ? 'lucide:unplug' : 'lucide:circle-alert'} class="mt-0.5 shrink-0" />
                        <button type="button" class="text-left hover:underline" onclick={() => { problemsOpen = false; if (d.node) selectedNode = d.node }}>{d.node ? `${d.node}: ` : ''}{d.line ? t('actions.page.line', { line: d.line }) : ''}{d.message}</button>
                      </li>
                    {/each}
                  </ul>
                </div>
              {/if}
            {:else}
              <span data-testid="action-ready" class="flex items-center gap-1 text-[12.5px] text-ink-3"><Icon icon="lucide:circle-check" />{t('actions.page.ready')}</span>
            {/if}
            {#if needs.length}
              <button type="button" data-testid="action-needs" data-count={needs.length} data-href={needs[0].href} class="flex items-center gap-1 text-[12.5px] text-warn-600 hover:underline" title={needs.map((n) => n.label).join(', ')} onclick={() => goto(needs[0].href)}><Icon icon="lucide:triangle-alert" />{t('actions.page.needs', { count: needs.length })}</button>
            {/if}
            {#if notice}<span data-testid="action-notice" class="text-[12.5px] text-ink-2">{notice}</span>{/if}
            {#if failure}<span data-testid="action-failure" class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-[12.5px] text-error-600" title={failure}>{failure}</span>{/if}
          </span>
          <span class="ml-auto flex shrink-0 items-center gap-1.5">
            {#if !current.builtin || current.builtin_kind === 'action'}
              <span class="flex items-center"><Switch testid="action-enabled" label={current.enabled ? t('actions.page.switchOff') : t('actions.page.switchOn')} checked={current.enabled} onchange={() => toggle(current!)} /></span>
            {/if}
            {#if !current.builtin}
              <IconButton testid="action-save" icon="lucide:save" label={t('common.save')} keys="⌘S" tone={dirty ? 'primary' : 'default'} busy={saving} disabled={!dirty} onclick={save} />
            {/if}
            <IconButton testid="action-duplicate" icon="lucide:copy" label={current.key?.startsWith(BUILTIN_KEY) ? t('actions.page.makeFromTrigger') : t('actions.page.duplicate')} keys="⌘D" onclick={duplicate} />
            {#if current.usage.chat}
              <div class="relative" bind:this={attachButton}>
                <IconButton testid="action-attach" icon="lucide:link-2" label={t('actions.attach.toChat')} active={attachOpen || current.chats.length > 0} onclick={() => { attachQuery = ''; attachOpen = !attachOpen }} />
                {#if attachOpen}
                  <div class="menu flex w-72 flex-col" use:anchored={{ at: () => boxOf(attachButton), align: 'end', onclose: () => (attachOpen = false) }} role="menu" data-testid="action-attach-menu">
                    <input data-testid="action-attach-search" class="mx-2 mb-1 h-8 border-b border-line bg-transparent text-[13px] text-ink outline-none placeholder:text-ink-3 focus:border-ink-3" placeholder={t('actions.attach.search')} aria-label={t('actions.attach.search')} bind:value={attachQuery} />
                    <div class="max-h-[18rem] overflow-y-auto">
                      {#each chatChoices as chat (chat.id)}
                        <MenuItem testid={`action-attach-chat-${chat.id}`} icon={chat.mode === 'read' ? 'lucide:book-open' : 'lucide:message-square'} checked={current.chats.includes(chat.id)} onclick={() => toggleAttach(chat.id)}>{chat.name}</MenuItem>
                      {:else}
                        <p class="px-3 py-2 text-[12.5px] text-ink-3">{t('actions.attach.none')}</p>
                      {/each}
                    </div>
                  </div>
                {/if}
              </div>
            {:else if draft.graph.nodes.some((n) => n.type === 'trigger')}
              <IconButton testid="action-duplicate-chat" icon="lucide:unplug" label={t('actions.attach.duplicateWithout')} onclick={duplicateForChat} />
            {/if}
            <IconButton testid="action-play" icon="lucide:play" label={t('actions.page.dryRun')} keys="⌘↵" busy={playBusy === ''} onclick={() => play(null)} />
            <IconButton testid="action-more" icon="lucide:ellipsis" label={t('common.more')} keys="⌥⌘↵" active={mockOpen && !playNode} onclick={() => editMock(null)} />
          </span>
        </div>
      </header>

      <div class="relative min-h-0 flex-1">
        {#if templating}
          {#key templating.id}
            <TemplateEditor title={templating.label || templating.id} kind={(presenterKind(templating) ?? 'mail') as TemplateKind} value={typeof templating.config?.template === 'string' ? templating.config.template : ''} owner={{ action_id: current.id, node_id: templating.id }} onchange={(template) => setNode({ ...templating, config: { ...(templating.config ?? {}), template } })} onclose={() => (templateNode = null)} />
          {/key}
        {:else if center === 'graph'}
          <SvelteFlowProvider>
            <ActionGraphView bind:this={graphView} graph={draft.graph} {catalog} {owner} {diagnostics} {run} bind:selected={selectedNode} {dark} onchange={setGraph} onreject={(reason) => say(reason)} onopen={open} onclone={current.builtin ? undefined : clone} />
          </SvelteFlowProvider>
          {#if run?.dry_run}
            <div class="absolute inset-x-0 bottom-0 border-t border-line bg-paper px-4 py-2">
              <RunOutcome {run} testid="action-run-status" prefix="action-run" onsteps={() => switchCenter('runs')} ondismiss={() => (run = null)} />
            </div>
          {/if}
          {#if !draft.graph.nodes.length}
            <div class="pointer-events-none absolute inset-0 flex items-center justify-center">
              <p class="max-w-[22rem] text-center text-[13px] leading-5 text-ink-3">{t('actions.page.emptyGraph')}</p>
            </div>
          {/if}
        {:else if center === 'code'}
          <div class="flex h-full min-h-0 flex-col">
            <div class="min-h-0 flex-1">
              <CodeView graph={draft.graph} {catalog} {owner} {diagnostics} selected={selectedNode} locked={current.builtin} onchange={setNode} onselect={(id) => { selectedNode = id; switchCenter('graph') }} />
            </div>
          </div>
        {:else}
          <RunPanel action={current} selected={run} onrun={(r) => (run = r)} />
        {/if}
      </div>
    </main>
  {:else}
    <main class="relative flex h-full min-h-0 flex-col items-center gap-4 bg-paper p-6">
      <div class="flex max-w-[26rem] shrink-0 flex-col items-center gap-3 pt-[6vh] text-center">
        <Icon icon="lucide:zap" class="text-[28px] text-ink-3" />
        <h2 class="text-[16px] font-semibold text-ink">{t('actions.page.title')}</h2>
        <p class="text-[13px] leading-5 text-ink-3">{t('actions.page.intro')}</p>
        <Button variant="primary" testid="actions-empty-new" icon="lucide:plus" onclick={() => create(null)}>{t('actions.draft.name')}</Button>
        {#if failure}<p class="text-[12.5px] text-error-600">{failure}</p>{/if}
      </div>
      <div class="flex min-h-0 w-full max-w-[44rem] flex-1 flex-col">
        <AssistantPanel scope={null} draft={null} {catalog} {dark} onchanged={refreshed} onopen={open} />
      </div>
    </main>
  {/if}
  {:else if current && draft}
    <aside class="flex min-h-0 flex-1 flex-col gap-3 p-3">
      <Segmented size="sm" testid="action-side" label={t('actions.page.panel')} value={side} options={[{ value: 'inspect', label: node || part ? t('actions.page.node') : t('actions.page.action') }, { value: 'picker', label: t('actions.page.picker') }, { value: 'assistant', label: t('actions.page.assistant') }]} onchange={(v) => (side = v as 'inspect' | 'picker' | 'assistant')} />
      {#if side === 'inspect'}
        <div class="no-scrollbar flex min-h-0 flex-1 flex-col overflow-y-auto">
          {#if node}
            <NodeInspector {node} actionId={current?.id ?? null} graph={draft.graph} {catalog} {owner} signature={outputs} {diagnostics} onchange={setNode} onremove={() => { setGraph(removeNodes(draft!.graph, [node.id])); selectedNode = null }} onopen={open} ontemplate={() => (templateNode = node.id)} />
          {:else if part}
            {@const holder = part.owner}
            <NodeInspector node={part.node} graph={holder.graph} {catalog} owner={{ inputs: holder.inputs }} signature={holder.outputs} diagnostics={holder.diagnostics} onchange={() => {}} onremove={() => {}} onopen={open} ontemplate={() => {}} part={{ name: holder.name, onedit: () => open(holder.id), onclone: current.builtin ? undefined : () => clone(part.root) }} />
          {:else}
            <div class="flex flex-col gap-4">
              <p class="text-[12.5px] leading-5 text-ink-3">{t('actions.page.signatureHint')}</p>
              {#each badges as badge (badge.key)}
                <p class="flex gap-1.5 text-[12.5px] leading-5 text-ink-2" title={badge.hint}><Icon icon={badge.icon} class="mt-1 shrink-0 text-ink-3" />{badge.label}</p>
              {/each}
              <SignatureForm title={t('actions.page.inputs')} ports={draft.inputs} types={catalog?.types ?? []} onchange={(inputs) => changed({ ...draft!, inputs })} />
              <div class="flex flex-col gap-1">
                <SignatureForm title={t('actions.page.outputs')} ports={outputs} types={catalog?.types ?? []} readonly origins={sources} onorigin={(id) => (selectedNode = id)} onchange={() => {}} />
                <p class="text-[12px] leading-5 text-ink-3">{t('actions.page.outputsHint')}</p>
              </div>
              {#if !current.builtin || current.builtin_kind === 'action'}
                <div id="action-ai" data-testid="action-ai" class="flex flex-col gap-1 rounded-[0.5rem]">
                  <span class="text-[12px] text-ink-3">{t('actions.page.ai')}</span>
                  {#key current.id}
                    <ModelChoice label={t('actions.page.aiFor', { name: current.name })} testid="action" class="min-w-0 flex-1" selection={actionSelection(current)} defaults={actionDefaults(current.graph)} onchange={putAi} />
                  {/key}
                  <p class="text-[12px] leading-5 text-ink-3">{t('actions.page.aiHint')}</p>
                </div>
              {/if}
              <label class="flex flex-col gap-1 text-[12px] text-ink-3">{t('actions.page.maxSends')}
                <input type="number" min="0" class="h-8 border-b border-line bg-transparent font-mono text-[13px] text-ink outline-none focus:border-ink-3" value={draft.max_sends_per_hour ?? 10} oninput={(e) => changed({ ...draft!, max_sends_per_hour: Number(e.currentTarget.value) })} />
              </label>
              {#if diagnostics.length}
                <ul class="flex flex-col gap-1">
                  {#each diagnostics as d, i (i)}
                    <li data-testid="action-diagnostic" data-severity={d.severity} class="flex gap-1.5 text-[12.5px] leading-4 {d.severity === 'error' ? 'text-error-600' : d.severity === 'requirement' ? 'text-warn-600' : 'text-signal'}">
                      <Icon icon={d.severity === 'requirement' ? 'lucide:triangle-alert' : 'lucide:circle-alert'} class="mt-0.5 shrink-0" />
                      <button type="button" class="text-left hover:underline" onclick={() => d.node && (selectedNode = d.node)}>{d.node ? `${d.node}: ` : ''}{d.line ? t('actions.page.line', { line: d.line }) : ''}{d.message}</button>
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
          {/if}
        </div>
      {:else if side === 'picker'}
        <PickerPanel actions={catalog?.actions ?? []} groups={catalog?.action_groups ?? []} exclude={current.id} ondrop={drop} onadd={add} />
      {:else}
        {#key current.id}
          <AssistantPanel scope={current} {draft} {catalog} {dark} initial={assistInitial} onchanged={refreshed} onopen={open} />
        {/key}
      {/if}
    </aside>
  {/if}
  {/snippet}
</PaneGrid>

{#if mockOpen && current && draft}
  {@const target = playing?.id ?? null}
  {@const signature = signatureOf(target)}
  <PlayPanel
    title={playing ? playing.label || playing.id : draft.name}
    nodeId={target}
    inputs={signature.inputs}
    outputs={signature.outputs}
    {sample}
    overrides={overridesOf(target)}
    run={run?.dry_run ? run : null}
    busy={playBusy === (target ?? '')}
    onoverrides={(next) => setOverrides(target, next)}
    onrun={(inputs) => runPlay(target, inputs)}
    onsteps={() => { mockOpen = false; switchCenter('runs') }}
    onclose={() => (mockOpen = false)}
    ondelete={!playing && !current.builtin ? () => { mockOpen = false; remove() } : undefined}
  />
{/if}

{#if iconOpen && draft}
  <IconPicker label={t('actions.page.iconFor', { name: draft.name })} value={draft.icon} fallback="lucide:zap" onpick={(icon) => changed({ ...draft!, icon })} onclose={() => (iconOpen = false)} />
{/if}


{#if groupDialog}
  {@const context = groupDialog}
  {#key context}
    <GroupEditor
      subject={{ scope: 'action', group: context.group, groups }}
      parentId={context.parentId}
      onsaved={() => {
        groupDialog = null
        load()
      }}
      oncancel={() => (groupDialog = null)}
      onnewgroup={(parentId) => (groupDialog = { group: null, parentId })}
      onnewaction={(groupId) => {
        groupDialog = null
        create(groupId)
      }}
      ondeleted={() => {
        groupDialog = null
        load()
      }}
    />
  {/key}
{/if}
