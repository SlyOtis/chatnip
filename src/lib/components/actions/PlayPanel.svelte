<script lang="ts">
  import { untrack } from 'svelte'
  import Icon from '@iconify/svelte'
  import { Button, Segmented, Sheet } from '$lib/components/ui'
  import CodeEditor from './CodeEditor.svelte'
  import SchemaControls from './SchemaControls.svelte'
  import RunOutcome from './RunOutcome.svelte'
  import type { ActionPort, ActionRun, ActionSample } from '$lib/tauri/types'
  import { mockEdit, mockProblems, mockWith, overridesOf, schemaOf, typeOf, type JsonSchema, type MockOverrides } from '$lib/actions'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    title: string
    nodeId: string | null
    inputs: ActionPort[]
    outputs: ActionPort[]
    sample: ActionSample | null
    overrides: MockOverrides
    run: ActionRun | null
    busy: boolean
    onoverrides: (next: MockOverrides) => void
    onrun: (inputs: Record<string, unknown>) => void
    onsteps: () => void
    onclose: () => void
    ondelete?: () => void
  }

  let { title, nodeId, inputs, outputs, sample, overrides, run, busy, onoverrides, onrun, onsteps, onclose, ondelete }: Props = $props()

  const ICONS: Record<string, string> = { message: 'lucide:message-square', conversation: 'lucide:messages-square', person: 'lucide:user', tag: 'lucide:tag', text: 'lucide:type', number: 'lucide:hash', bool: 'lucide:toggle-left', list: 'lucide:list', json: 'lucide:braces' }

  const inputSchema = $derived(schemaOf(inputs))
  const shape = $derived(JSON.stringify(inputSchema))
  let value = $state<Record<string, unknown>>({})
  let tab = $state('')
  let view = $state<'fields' | 'json'>('fields')
  let text = $state('{}')
  let failure = $state<string | null>(null)

  const generated = $derived(mockWith(inputSchema, sample, {}).value)
  const edits = $derived(overridesOf(inputSchema, value, generated))
  const edited = $derived(Object.keys(edits).length)
  const problems = $derived(view === 'json' ? mockEdit(inputSchema, sample, text).problems : mockProblems(inputSchema, value))
  const current = $derived(inputs.find((p) => p.name === tab) ?? inputs[0] ?? null)
  const field = $derived<JsonSchema>(current ? (inputSchema.properties?.[current.name] ?? {}) : {})
  const whole = $derived(typeOf(field) === 'object')
  const formSchema = $derived<JsonSchema>(whole || !current ? field : { type: 'object', properties: { [current.name]: field } })
  const formValue = $derived.by(() => {
    if (!current) return {}
    const own = value[current.name]
    return whole ? (own !== null && typeof own === 'object' && !Array.isArray(own) ? (own as Record<string, unknown>) : {}) : { [current.name]: own }
  })

  function pretty(next: Record<string, unknown>): string {
    return JSON.stringify(next, null, 2)
  }

  $effect(() => {
    void shape
    void sample
    untrack(() => {
      value = mockWith(inputSchema, sample, overrides).value
      text = pretty(value)
      if (!inputs.some((p) => p.name === tab)) tab = inputs[0]?.name ?? ''
    })
  })

  function changed(next: Record<string, unknown>) {
    if (!current) return
    value = whole ? { ...value, [current.name]: next } : { ...value, ...next }
  }

  function typed(next: string) {
    text = next
    const edit = mockEdit(inputSchema, sample, next)
    if (edit.overrides) value = mockWith(inputSchema, sample, edit.overrides).value
  }

  function switchView(next: string) {
    if (next === 'json') text = pretty(value)
    view = next as 'fields' | 'json'
  }

  function regenerate() {
    value = mockWith(inputSchema, sample, {}).value
    text = pretty(value)
  }

  function keep(): boolean {
    if (problems.length) {
      failure = t('actions.play.invalid')
      return false
    }
    failure = null
    onoverrides(edits)
    return true
  }

  function save() {
    if (keep()) onclose()
  }

  function play() {
    if (busy || !keep()) return
    onrun($state.snapshot(value))
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && !e.altKey) {
      e.preventDefault()
      e.stopPropagation()
      play()
    }
  }
</script>

<Sheet testid="play-panel" label={t('actions.play.title', { name: title })} width="48rem" emphasis="strong" movable {onclose}>
  <div class="flex min-h-0 flex-col" role="presentation" onkeydowncapture={key}>
    <header class="flex flex-col gap-1 border-b border-line px-4 py-3">
      <div class="flex items-center gap-2">
        <Icon icon="lucide:flask-conical" class="text-[15px] text-ink-3" />
        <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-[14px] font-semibold text-ink">{t('actions.play.title', { name: title })}</span>
        <Button size="sm" testid="play-regenerate" icon="lucide:refresh-cw" onclick={regenerate}>{t('actions.play.regenerate')}</Button>
      </div>
      <p class="text-[12px] leading-4 text-ink-3">{nodeId ? t('actions.play.introNode') : t('actions.play.introAction')}</p>
    </header>

    <div class="flex h-[min(56vh,34rem)] min-h-0 flex-col">
      <div class="flex items-center gap-2 border-b border-line px-4 py-2">
        {#if inputs.length}
          <Segmented size="sm" testid="play-tab" label={t('actions.play.mock')} value={current?.name ?? ''} options={inputs.map((p) => ({ value: p.name, label: p.name, icon: ICONS[p.type] ?? ICONS.json }))} onchange={(v) => (tab = v)} />
        {:else}
          <span class="text-[12px] text-ink-3">{t('actions.play.noInputs')}</span>
        {/if}
        <span class="flex-1"></span>
        {#if edited}<span class="text-[11.5px] text-ink-3">{t('actions.play.edited', { count: edited })}</span>{/if}
        <Segmented size="sm" testid="play-view" label={t('actions.play.mock')} value={view} options={[{ value: 'fields', label: t('actions.play.fields'), icon: 'lucide:list' }, { value: 'json', label: t('actions.play.json'), icon: 'lucide:braces' }]} onchange={switchView} />
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto {view === 'json' ? 'px-2 py-1' : 'px-4 py-3'}">
        {#if view === 'json'}
          <CodeEditor label={t('actions.play.mockLabel')} language="json" value={text} onchange={typed} />
        {:else if current}
          <div data-testid="play-fields" data-tab={current.name}>
            {#if current.description}<p class="pb-2 text-[12px] text-ink-3">{current.description}</p>{/if}
            <SchemaControls schema={formSchema} value={formValue} onchange={changed} />
          </div>
        {/if}
      </div>
      <div class="flex flex-wrap items-center gap-1.5 border-t border-line px-4 py-2" data-testid="play-outputs">
        <span class="text-[11.5px] font-medium uppercase tracking-wide text-ink-3">{t('actions.page.outputs')}</span>
        {#each outputs as port (port.name)}
          <span data-testid="play-output" data-name={port.name} class="rounded-[0.375rem] bg-paper-3 px-1.5 py-0.5 font-mono text-[11.5px] text-ink-2" title={port.description}>{port.name} <span class="text-ink-3">{port.type}</span></span>
        {:else}
          <span class="text-[12px] text-ink-3">{t('actions.runs.noOutputs')}</span>
        {/each}
      </div>
      <div class="max-h-[30%] overflow-y-auto border-t border-line px-4 py-2">
        {#if problems.length}
          <ul data-testid="play-problems" class="flex flex-col gap-1">
            {#each problems as problem, i (i)}
              <li class="flex gap-1.5 text-[12px] leading-4 text-error-600"><Icon icon="lucide:circle-alert" class="mt-0.5 shrink-0" /><span><span class="font-mono text-[11px]">{problem.path}</span> {problem.message}</span></li>
            {/each}
          </ul>
        {:else}
          <span data-testid="play-valid" class="flex items-center gap-1.5 text-[12px] text-ink-3"><Icon icon="lucide:circle-check" />{inputs.length ? t('actions.play.valid') : t('actions.play.noInputs')}</span>
        {/if}
      </div>
      {#if run}
        <div class="border-t border-line px-4 py-2">
          <RunOutcome {run} testid="play-result" {onsteps} />
        </div>
      {/if}
    </div>
  </div>

  {#snippet footer()}
    {#if ondelete}
      <Button size="sm" testid="play-delete" variant="danger" icon="lucide:trash-2" onclick={ondelete}>{t('common.delete')}</Button>
    {/if}
    {#if failure}
      <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-[12.5px] text-error-600" title={failure}>{failure}</span>
    {:else}
      <span class="flex-1"></span>
    {/if}
    <Button size="sm" testid="play-close" onclick={onclose}>{t('common.cancel')}</Button>
    <Button size="sm" testid="play-save" icon="lucide:save" onclick={save}>{t('common.save')}</Button>
    <Button size="sm" testid="play-run" variant="primary" icon="lucide:play" keys="⌘↵" {busy} onclick={play}>{t('actions.page.dryRun')}</Button>
  {/snippet}
</Sheet>
