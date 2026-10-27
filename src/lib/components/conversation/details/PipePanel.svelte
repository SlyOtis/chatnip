<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import Icon from '@iconify/svelte'
  import { api } from '$lib/tauri'
  import type { ChatPipe, ChatRoute, ChatView, PipeStep } from '$lib/tauri/types'
  import { changeRoute, extraInputs, fittingInputs, newRoute, stepState } from '$lib/pipe'
  import { Button, IconButton, Select } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    conversationId: string
    view: ChatView
    revision?: number
    ondetach?: (actionId: string, name: string) => void
  }

  let { conversationId, view, revision = 0, ondetach }: Props = $props()

  let pipe = $state<ChatPipe | null>(null)
  let edits = $state<Record<string, ChatRoute[]>>({})
  let saving = $state<string | null>(null)
  let failure = $state<string | null>(null)

  const steps = $derived(pipe ? pipe.steps.map((step) => stepState(step, pipe!.landings, edits[step.action_id])) : [])
  const lands = $derived([...new Map(steps.flatMap((s) => s.lands).map((l) => [l.node, l])).values()])

  async function load() {
    try {
      pipe = await api.getChatPipe(conversationId)
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  $effect(() => {
    conversationId
    revision
    edits = {}
    load()
  })

  onMount(() => {
    const off = listen('actions-updated', load)
    return () => off.then((stop) => stop())
  })

  function edit(step: PipeStep, routes: ChatRoute[]) {
    edits = { ...edits, [step.action_id]: routes }
  }

  async function save(step: PipeStep, routes: ChatRoute[]) {
    saving = step.action_id
    failure = null
    try {
      pipe = await api.putChatRoutes(conversationId, step.action_id, routes)
      const { [step.action_id]: _, ...rest } = edits
      edits = rest
    } catch (error) {
      failure = String(error)
    } finally {
      saving = null
    }
  }
</script>

{#if pipe}
  <div class="flex flex-col gap-2" data-testid="pipe-panel">
    <p class="flex items-center gap-1.5 text-[12px] text-ink-3">
      <Icon icon="lucide:folder-input" class="text-[13px]" />{t('chats.pipe.sources', { count: pipe.sources.length })}
      <Icon icon="lucide:arrow-right" class="text-[12px]" />{t('chats.pipe.actions', { count: pipe.steps.length })}
      <Icon icon="lucide:arrow-right" class="text-[12px]" />{lands.length ? lands.map((l) => l.label).join(', ') : t('chats.pipe.nowhere')}
    </p>
    {#each steps as state (state.step.action_id)}
      {@const step = state.step}
      <section data-testid="pipe-step" data-id={step.action_id} class="flex flex-col gap-1.5 border-t border-line pt-2">
        <header class="flex min-w-0 items-center gap-1.5 text-[13px] text-ink">
          <Icon icon={step.icon ?? 'lucide:zap'} class="shrink-0 text-[13px] text-ink-3" />
          <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap {step.enabled ? '' : 'text-ink-3 line-through decoration-ink-3'}">{step.name}</span>
          {#if !step.stored && !state.dirty}<span class="shrink-0 text-[11.5px] text-ink-3">{t('chats.pipe.defaults')}</span>{/if}
          {#if ondetach}<IconButton testid="pipe-detach" size="sm" icon="lucide:unlink" label={t('chats.actions.detach', { name: step.name })} onclick={() => ondetach?.(step.action_id, step.name)} />{/if}
        </header>
        <dl class="grid grid-cols-[minmax(0,max-content)_minmax(0,1fr)] gap-x-2 gap-y-0.5 font-mono text-[11px]">
          <dt class="text-ink-3">{t('chats.pipe.in')}</dt>
          <dd class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-ink-2">{step.inputs.length ? step.inputs.map((p) => `${p.name}: ${p.type}`).join(', ') : t('chats.pipe.fromChat')}</dd>
          <dt class="text-ink-3">{t('chats.pipe.out')}</dt>
          <dd class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-ink-2">{step.outputs.length ? step.outputs.map((p) => `${p.name}: ${p.type}`).join(', ') : t('chats.pipe.noOutputs')}</dd>
        </dl>
        <ul class="flex flex-col gap-1">
          {#each state.routes as route, i (i)}
            {@const landing = pipe.landings.find((l) => l.node === route.node)}
            {@const problem = state.problems.find((p) => p.index === i)}
            <li data-testid="pipe-route" class="flex flex-col gap-1 rounded-[0.5rem] px-1.5 py-1 {problem ? 'bg-error-500/10' : 'bg-paper-3/60'}">
              <div class="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)_auto] items-center gap-1">
                <Select size="sm" value={route.output} options={step.outputs.map((p) => ({ value: p.name, label: `${p.name}: ${p.type}` }))} onchange={(output) => edit(step, state.routes.map((r, j) => (j === i ? changeRoute(r, { output }, step.outputs, pipe!.landings) : r)))} />
                <Icon icon="lucide:arrow-right" class="text-[12px] text-ink-3" />
                <Select size="sm" value={route.node} options={pipe.landings.map((l) => ({ value: l.node, label: l.label }))} onchange={(node) => edit(step, state.routes.map((r, j) => (j === i ? changeRoute(r, { node }, step.outputs, pipe!.landings) : r)))} />
                <IconButton testid="pipe-route-remove" size="sm" icon="lucide:x" label={t('chats.pipe.removeRoute')} onclick={() => edit(step, state.routes.filter((_, j) => j !== i))} />
              </div>
              <div class="grid grid-cols-[minmax(0,max-content)_minmax(0,1fr)] items-center gap-x-2 gap-y-1 text-[12px]">
                <span class="text-ink-3">{t('chats.pipe.into')}</span>
                <Select size="sm" value={route.input} options={(landing?.inputs ?? []).map((p) => ({ value: p.name, label: `${p.name}: ${p.type}${fittingInputs(landing, step.outputs.find((o) => o.name === route.output)).some((f) => f.name === p.name) ? '' : ' ✕'}` }))} onchange={(input) => edit(step, state.routes.map((r, j) => (j === i ? { ...r, input } : r)))} />
                {#each extraInputs(landing, route) as port (port.name)}
                  <span class="font-mono text-ink-3">{port.name}</span>
                  <input class="h-7 min-w-0 border-b border-line bg-transparent text-ink outline-none placeholder:text-ink-3 focus:border-ink-3" placeholder={port.type} value={route.values[port.name] == null ? '' : String(route.values[port.name])} oninput={(e) => edit(step, state.routes.map((r, j) => (j === i ? { ...r, values: { ...r.values, [port.name]: e.currentTarget.value || null } } : r)))} />
                {/each}
              </div>
              {#if problem}<p class="flex gap-1 text-[11.5px] leading-4 text-error-600"><Icon icon="lucide:circle-alert" class="mt-0.5 shrink-0" />{problem.message}</p>{/if}
            </li>
          {/each}
        </ul>
        <div class="flex flex-wrap items-center gap-1">
          <Button testid="pipe-add-route" size="sm" icon="lucide:plus" disabled={!step.outputs.length} onclick={() => edit(step, [...state.routes, newRoute(step.outputs, pipe!.landings)])}>{t('chats.pipe.addRoute')}</Button>
          {#if step.stored}<Button testid="pipe-defaults" size="sm" onclick={() => save(step, [])}>{t('chats.pipe.useDefaults')}</Button>{/if}
          {#if state.dirty}
            <span class="flex-1"></span>
            <Button testid="pipe-cancel" size="sm" onclick={() => { const { [step.action_id]: _, ...rest } = edits; edits = rest }}>{t('common.cancel')}</Button>
            <Button testid="pipe-save" size="sm" variant="primary" busy={saving === step.action_id} disabled={state.problems.length > 0} onclick={() => save(step, state.routes)}>{t('common.save')}</Button>
          {/if}
        </div>
      </section>
    {/each}
    {#if lands.some((l) => l.node === 'chat')}
      <p class="text-[11.5px] leading-4 text-ink-3">{view === 'grid' ? t('chats.pipe.groupedGrid') : t('chats.pipe.groupedChat')}</p>
    {/if}
    {#if failure}<p class="text-[12px] text-error-600">{failure}</p>{/if}
  </div>
{/if}
