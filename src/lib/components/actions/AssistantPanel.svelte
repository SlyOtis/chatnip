<script lang="ts" module>
  import type { AssistTurn } from '$lib/actions'

  const session = $state<{ turns: AssistTurn[]; toast: { text: string; undo: (() => Promise<void>) | null } | null }>({ turns: [], toast: null })
  let toastTimer: ReturnType<typeof setTimeout> | undefined
</script>

<script lang="ts">
  import Icon from '@iconify/svelte'
  import { onMount, tick, untrack } from 'svelte'
  import { goto } from '$app/navigation'
  import { SvelteFlowProvider } from '@xyflow/svelte'
  import { Button, IconButton } from '$lib/components/ui'
  import { api } from '$lib/tauri'
  import type { Action, ActionCatalog, ActionDiagnostic, ActionDraft, ActionOperation, ActionOperationKind, ActionOperationPreview, ActionsOutcome, PromptOverride } from '$lib/tauri/types'
  import { ASSISTANT_PROMPT, ASSISTANT_PROMPT_KEY, REQUIREMENT, assistHistory, assistTurn, groupName, operationChange, operationTarget, requirementBadge, requirementsFor, resolvePrompt, type NodeRequirement } from '$lib/actions'
  import { t, type MessageKey } from '$lib/i18n.svelte'
  import ActionGraphView from './ActionGraph.svelte'
  import PromptEditor from './PromptEditor.svelte'

  interface Props {
    scope: Action | null
    draft: ActionDraft | null
    catalog: ActionCatalog | null
    dark: boolean
    initial?: string
    onchanged: (outcome: ActionsOutcome) => void | Promise<void>
    onopen: (actionId: string) => void
  }

  let { scope, draft, catalog, dark, initial = '', onchanged, onopen }: Props = $props()

  const OP_LABELS: Record<ActionOperationKind, MessageKey> = {
    create: 'actions.assistant.op.create',
    update: 'actions.assistant.op.update',
    copy: 'actions.assistant.op.copy',
    split: 'actions.assistant.op.split',
    merge: 'actions.assistant.op.merge',
    delete: 'actions.assistant.op.delete',
    move: 'actions.assistant.op.move',
    rename: 'actions.assistant.op.rename',
    restore: 'actions.assistant.op.restore'
  }
  const GRAPHED: ActionOperationKind[] = ['create', 'update', 'copy', 'split', 'merge']

  let text = $state(untrack(() => initial))
  let busy = $state(false)
  let applying = $state<number | null>(null)
  let runtime = $state.raw<NodeRequirement[]>([])
  let promptOpen = $state(false)
  let override = $state<PromptOverride | null>(null)
  let thread = $state<HTMLDivElement | null>(null)

  const needs = $derived(requirementsFor((catalog?.requirements ?? []).filter((r) => !r.met && r.key.startsWith('ai.')).map((r): ActionDiagnostic => ({ severity: REQUIREMENT, message: r.label, node: null, edge: null, line: null, column: null, setting: r.key }))))
  const badge = $derived(requirementBadge(runtime.length ? runtime : needs))
  const base = $derived(catalog?.prompts.find((p) => p.key === ASSISTANT_PROMPT_KEY) ?? null)
  const groups = $derived(new Map((catalog?.action_groups ?? []).map((g) => [g.id, g])))

  onMount(() => {
    api.getAppSettings().then((all) => (override = all[ASSISTANT_PROMPT] ? (JSON.parse(all[ASSISTANT_PROMPT]) as PromptOverride) : null)).catch(() => {})
    scrolled()
  })

  async function scrolled() {
    await tick()
    thread?.scrollTo({ top: thread.scrollHeight })
  }

  function notify(message: string, undo: (() => Promise<void>) | null) {
    session.toast = { text: message, undo }
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (session.toast = null), 6000)
  }

  async function ask() {
    const instruction = text.trim()
    if (!instruction || busy) return
    const scopeId = scope?.id ?? null
    const messages = assistHistory(session.turns)
    const at = session.turns.length
    session.turns.push(assistTurn('user', instruction, scopeId))
    text = ''
    busy = true
    scrolled()
    try {
      const outcome = await api.postActionAssist({ action_id: scopeId, draft: scope && draft ? ($state.snapshot(draft) as ActionDraft) : null, instruction, messages })
      runtime = requirementsFor([outcome.requirement])
      if (runtime.length) {
        session.turns.splice(at)
        text = instruction
        return
      }
      session.turns.push(assistTurn('assistant', outcome.reply, scopeId, { operations: outcome.operations, previews: outcome.previews, errors: outcome.errors }))
    } catch {
      session.turns.push(assistTurn('assistant', t('actions.requirement.failed'), scopeId, { failed: true }))
    } finally {
      busy = false
      scrolled()
    }
  }

  async function apply(index: number) {
    const turn = session.turns[index]
    if (!turn || applying !== null) return
    applying = index
    try {
      const outcome = await api.putActions({ operations: $state.snapshot(turn.operations) as ActionOperation[] })
      turn.status = 'applied'
      turn.ids = outcome.ids
      await onchanged(outcome)
      notify(t('actions.assistant.applied', { count: turn.operations.length }), async () => {
        const back = await api.putActions({ operations: outcome.undo })
        turn.status = 'undone'
        await onchanged(back)
      })
    } catch (e) {
      notify(String(e), null)
    } finally {
      applying = null
    }
  }

  function undo(run: () => Promise<void>) {
    session.toast = null
    run().catch((e) => notify(String(e), null))
  }

  function discard(index: number) {
    const turn = session.turns[index]
    if (turn) turn.status = 'discarded'
  }

  function clear() {
    session.turns = []
    runtime = []
  }

  async function savePrompt(next: PromptOverride | null) {
    try {
      await api.putAppSetting(ASSISTANT_PROMPT, next ? JSON.stringify(next) : '')
      override = next
      promptOpen = false
    } catch (e) {
      notify(String(e), null)
    }
  }

  function summary(preview: ActionOperationPreview): string {
    const change = operationChange(preview)
    const parts: string[] = []
    if (change.renamed) parts.push(t('actions.assistant.renamed', { from: change.renamed.from }))
    if (change.moved) {
      const group = preview.group_after ? groups.get(preview.group_after) : null
      parts.push(t('actions.assistant.moved', { group: group ? groupName(group) : t('actions.assistant.topLevel') }))
    }
    if (preview.op !== 'delete') {
      if (change.added.length) parts.push(t('actions.assistant.added', { count: change.added.length }))
      if (change.removed.length) parts.push(t('actions.assistant.removed', { count: change.removed.length }))
      if (change.changed.length) parts.push(t('actions.assistant.changed', { count: change.changed.length }))
      if (preview.before && preview.after && change.wires) parts.push(t('actions.assistant.wires', { count: change.wires }))
    }
    const problems = preview.diagnostics.filter((d) => d.severity === 'error').length
    if (problems) parts.push(t('actions.assistant.problems', { count: problems }))
    return parts.join(', ')
  }

  function configure() {
    const href = badge?.href
    runtime = []
    if (href) goto(href)
  }

  function send() {
    if (badge) configure()
    else ask()
  }
</script>

<div class="flex min-h-0 flex-1 flex-col gap-2" data-testid="assistant-chat">
  <div class="flex shrink-0 items-center gap-1">
    <span data-testid="assistant-scope" data-scope={scope?.id ?? ''} class="flex min-w-0 flex-1 items-center gap-1.5 font-mono text-[11.5px] text-ink-3">
      <Icon icon={scope ? (scope.icon ?? 'lucide:zap') : 'lucide:layers'} class="shrink-0" />
      <span class="overflow-hidden text-ellipsis whitespace-nowrap">{scope ? t('actions.assistant.scopeOne', { name: scope.name }) : t('actions.assistant.scopeAll')}</span>
    </span>
    {#if base}
      <IconButton size="sm" testid="assistant-prompt" icon="lucide:file-cog" label={t('actions.assistant.prompt')} onclick={() => (promptOpen = true)} />
    {/if}
    <IconButton size="sm" testid="assistant-clear" icon="lucide:message-square-dashed" label={t('actions.assistant.clear')} disabled={!session.turns.length || busy} onclick={clear} />
  </div>
  <div bind:this={thread} class="no-scrollbar flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto py-2">
    {#if !session.turns.length && !busy}
      <p class="px-2 py-6 text-center font-mono text-[12px] leading-5 text-ink-3">{t('actions.assistant.intro')}</p>
    {/if}
    {#each session.turns as turn, i (i)}
      {#if turn.role === 'user'}
        <p data-testid="assistant-message" data-role="user" class="whitespace-pre-wrap font-mono text-[12px] leading-5 text-accent">{turn.text}</p>
      {:else}
        <div data-testid="assistant-message" data-role="assistant" data-status={turn.status ?? ''} class="flex flex-col gap-1.5">
          {#if turn.text}
            <p class="whitespace-pre-wrap font-mono text-[12px] leading-relaxed {turn.failed ? 'text-ink-3' : 'text-ink-2'}">{turn.text}</p>
          {/if}
          {#if turn.errors.length}
            <div data-testid="assistant-errors" class="flex flex-col gap-1 text-[12px] leading-4 text-error-600">
              <p>{t('actions.assistant.refused')}</p>
              <ul class="flex flex-col gap-0.5">
                {#each turn.errors as error, j (j)}
                  <li class="flex gap-1.5"><Icon icon="lucide:circle-alert" class="mt-0.5 shrink-0" />{error}</li>
                {/each}
              </ul>
            </div>
          {/if}
          {#if turn.previews.length}
            <div data-testid="assistant-proposal" class="flex flex-col gap-2 rounded-[0.75rem] border border-line bg-paper-2 p-2 {turn.status && turn.status !== 'applied' ? 'opacity-60' : ''}">
              {#each turn.previews as preview, j (j)}
                {@const shown = preview.after ?? preview.before}
                {@const target = operationTarget(preview, turn.ids)}
                <div data-testid="assistant-operation" data-op={preview.op} data-action={preview.action_id} class="flex flex-col gap-1.5">
                  <div class="flex min-w-0 items-center gap-1.5 text-[12.5px]">
                    <span class="shrink-0 rounded-[0.25rem] bg-paper-3 px-1.5 font-mono text-[10.5px] uppercase tracking-wide {preview.op === 'delete' ? 'text-error-600' : 'text-ink-2'}">{t(OP_LABELS[preview.op])}</span>
                    <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap font-medium text-ink" title={preview.name}>{preview.name}</span>
                    {#if target && turn.status === 'applied'}
                      <IconButton size="sm" testid="assistant-operation-open" icon="lucide:arrow-up-right" label={t('common.open')} onclick={() => onopen(target)} />
                    {/if}
                  </div>
                  {#if summary(preview)}
                    <p class="text-[12px] leading-4 text-ink-3">{summary(preview)}</p>
                  {/if}
                  {#if shown && GRAPHED.includes(preview.op) && shown.graph.nodes.length}
                    <div class="relative h-[clamp(8rem,30vh,15rem)] overflow-hidden rounded-[0.5rem] border border-line bg-paper">
                      <SvelteFlowProvider>
                        <ActionGraphView graph={shown.graph} {catalog} owner={{ inputs: shown.inputs }} diagnostics={preview.diagnostics} run={null} selected={null} {dark} onchange={() => {}} onreject={() => {}} preview />
                      </SvelteFlowProvider>
                    </div>
                  {/if}
                </div>
              {/each}
              {#if turn.status}
                <p data-testid="assistant-proposal-status" class="text-[12px] text-ink-3">{turn.status === 'applied' ? t('actions.assistant.applied', { count: turn.operations.length }) : turn.status === 'undone' ? t('actions.assistant.undone') : t('actions.assistant.discarded')}</p>
              {:else}
                <div class="flex justify-end gap-1.5">
                  <Button size="sm" testid="assistant-discard" disabled={applying !== null} onclick={() => discard(i)}>{t('actions.page.discard')}</Button>
                  <Button size="sm" variant="primary" testid="assistant-apply" icon="lucide:check" busy={applying === i} disabled={applying !== null && applying !== i} onclick={() => apply(i)}>{t('common.apply')}</Button>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/if}
    {/each}
    {#if busy}
      <p class="flex items-center gap-2 font-mono text-[12px] text-ink-3"><Icon icon="lucide:loader-circle" class="animate-spin" /><span class="animate-pulse">{t('actions.assistant.writing')}</span></p>
    {/if}
  </div>
  <label data-testid="assistant-input" class="ai-input relative flex shrink-0 cursor-text flex-col gap-1.5 rounded-[0.75rem] border p-2 {badge ? 'is-blocked' : ''}" data-busy={busy || undefined}>
    {#if badge}
      <button
        type="button"
        data-testid="assistant-requirement"
        data-href={badge.href}
        class="requirement-badge absolute -top-2.5 right-3 z-10 flex h-5 items-center gap-1 rounded-[0.25rem] px-1.5 font-mono text-[10.5px] font-medium"
        title={badge.title}
        aria-label={badge.title}
        onclick={configure}
      ><Icon icon="lucide:triangle-alert" class="text-[11px]" />{badge.label}</button>
    {/if}
    <textarea
      rows="1"
      class="max-h-44 min-h-9 w-full resize-none overflow-y-auto bg-transparent px-2 py-1.5 text-[13px] leading-5 text-ink outline-none [field-sizing:content] placeholder:text-ink-3"
      placeholder={t('actions.assistant.placeholder')}
      aria-label={t('actions.assistant.instruction')}
      bind:value={text}
      onkeydown={(e) => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); send() } }}
    ></textarea>
    <span class="flex justify-end">
      <IconButton testid="assistant-send" tone="primary" icon="lucide:send" label={badge ? badge.title : t('actions.assistant.write')} keys="⌘↵" {busy} disabled={!badge && !text.trim()} class="rounded-full!" onclick={send} />
    </span>
  </label>
</div>

{#if session.toast}
  {@const current = session.toast}
  <div class="glass rise fixed bottom-6 left-1/2 z-[1000] flex -translate-x-1/2 items-center gap-3 rounded-[0.75rem] px-4 py-2 text-[13px] text-ink shadow-[0_12px_32px_rgb(0_0_0/0.28)]" data-testid="assistant-toast" role="status">
    <span>{current.text}</span>
    {#if current.undo}
      <Button testid="assistant-toast-undo" size="sm" icon="lucide:undo-2" onclick={() => current.undo && undo(current.undo)}>{t('actions.assistant.undo')}</Button>
    {/if}
  </div>
{/if}

{#if promptOpen && base}
  <PromptEditor prompt={resolvePrompt(base, override ?? undefined)} {base} node="" note={t('actions.assistant.promptNote')} onsave={savePrompt} onclose={() => (promptOpen = false)} />
{/if}

<style>
  .ai-input {
    border-color: var(--line-strong);
    transition: border-color 0.15s ease;
  }
  .ai-input::after {
    content: '';
    position: absolute;
    inset: -1px;
    padding: 2px;
    border-radius: calc(0.75rem + 1px);
    background: linear-gradient(to right, var(--accent), var(--signal));
    mask:
      linear-gradient(#fff 0 0) content-box,
      linear-gradient(#fff 0 0);
    mask-composite: exclude;
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.15s ease;
  }
  .ai-input:not(.is-blocked):not([data-busy]):hover,
  .ai-input:not(.is-blocked):not([data-busy]):focus-within {
    border-color: transparent;
  }
  .ai-input:not(.is-blocked):not([data-busy]):hover::after,
  .ai-input:not(.is-blocked):not([data-busy]):focus-within::after {
    opacity: 1;
  }
  .ai-input[data-busy] {
    cursor: default;
  }
  .ai-input.is-blocked {
    border-color: var(--color-warn-500);
    box-shadow: inset 0 0 0 0.5px var(--color-warn-500);
  }
  .requirement-badge {
    background: var(--color-warn-500);
    color: var(--paper);
  }
  .requirement-badge:hover {
    background: var(--color-warn-600);
  }
</style>
