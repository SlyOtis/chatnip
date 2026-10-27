<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import Icon from '@iconify/svelte'
  import { listen } from '@tauri-apps/api/event'
  import { api } from '$lib/tauri'
  import type { Action, ActionRun } from '$lib/tauri/types'
  import { durationLabel, foldToggle, runRow, runSteps, statusLabel } from '$lib/actions'
  import { formatDate, t } from '$lib/i18n.svelte'
  import CodeEditor from './CodeEditor.svelte'
  import { STATUS_DOT, STATUS_TONE, STEP_ICON } from './RunOutcome.svelte'

  interface Props {
    action: Action
    selected: ActionRun | null
    onrun: (run: ActionRun | null) => void
  }

  let { action, selected, onrun }: Props = $props()

  let runs = $state<ActionRun[]>([])
  let expanded = $state<Set<string>>(new Set())
  let failure = $state<string | null>(null)

  const rows = $derived(runs.map((run) => ({ run, row: runRow(run) })))
  const live = $derived(rows.some(({ row }) => row.live))

  async function load() {
    try {
      runs = await api.getActionRuns(action.id, 50)
      failure = null
    } catch (e) {
      failure = String(e)
    }
  }

  $effect(() => {
    void action.id
    expanded = untrack(() => (selected?.action_id === action.id ? new Set([selected.id]) : new Set()))
    load()
  })

  $effect(() => {
    if (!live) return
    const timer = setInterval(load, 1500)
    return () => clearInterval(timer)
  })

  onMount(() => {
    const off = listen<string>('action-runs-updated', (event) => {
      if (!event.payload || event.payload === action.id) load()
    })
    return () => {
      off.then((stop) => stop())
    }
  })

  function when(iso: string): string {
    return formatDate(iso, { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit', second: '2-digit' }).replace(' at ', ' ')
  }

  function shown(value: unknown): string {
    return JSON.stringify(value ?? null, null, 2)
  }

  function toggle(run: ActionRun) {
    expanded = foldToggle(expanded, run.id)
    onrun(expanded.has(run.id) ? run : null)
  }
</script>

<div class="flex h-full min-h-0 flex-col">
  <div class="grid shrink-0 grid-cols-[minmax(0,11rem)_minmax(0,8rem)_minmax(0,12rem)_minmax(0,5rem)_minmax(0,1fr)] gap-3 border-b border-line px-4 py-1.5 text-[11.5px] font-medium uppercase tracking-wide text-ink-3">
    <span>{t('actions.runs.time')}</span>
    <span>{t('actions.runs.status')}</span>
    <span>{t('actions.runs.source')}</span>
    <span>{t('actions.runs.duration')}</span>
    <span>{t('actions.runs.summary')}</span>
  </div>
  {#if failure}<p class="px-4 py-2 text-[12.5px] text-error-600">{failure}</p>{/if}
  <ol class="no-scrollbar min-h-0 flex-1 overflow-y-auto">
    {#each rows as { run, row } (run.id)}
      {@const open = expanded.has(run.id)}
      <li class="border-b border-line" data-testid="run-row" data-id={run.id} data-status={run.status} data-dry={run.dry_run}>
        <button type="button" data-testid="run-toggle" class="grid w-full grid-cols-[minmax(0,11rem)_minmax(0,8rem)_minmax(0,12rem)_minmax(0,5rem)_minmax(0,1fr)] items-baseline gap-3 px-4 py-1.5 text-left text-[12.5px] {open ? 'bg-paper-2' : 'hover:bg-paper-2'}" aria-expanded={open} aria-label={t('actions.runs.expand', { when: when(row.at) })} onclick={() => toggle(run)}>
          <span class="flex items-center gap-1.5 whitespace-nowrap font-mono text-[11.5px] tabular-nums text-ink-2"><Icon icon={open ? 'lucide:chevron-down' : 'lucide:chevron-right'} class="shrink-0 text-ink-3" />{when(row.at)}</span>
          <span class="flex min-w-0 items-center gap-1.5">
            {#if row.live}<Icon icon="lucide:loader-circle" class="shrink-0 animate-spin text-accent" />{:else}<span class="size-1.5 shrink-0 rounded-full {STATUS_DOT[row.status] ?? 'bg-ink-3'}"></span>{/if}
            <span class="{STATUS_TONE[row.status] ?? 'text-ink-3'} font-medium">{row.label}</span>
            {#if row.dry}<span class="shrink-0 rounded-full bg-paper-3 px-1.5 text-[10.5px] leading-4 text-ink-2">{t('actions.runs.dryRun')}</span>{/if}
          </span>
          <span class="overflow-hidden text-ellipsis whitespace-nowrap text-ink-2" title={row.source}>{row.source}</span>
          <span class="font-mono text-[11.5px] tabular-nums text-ink-3">{row.duration ?? ''}</span>
          <span class="overflow-hidden text-ellipsis whitespace-nowrap {row.failed ? 'text-error-600' : row.needs.length ? 'text-warn-600' : 'font-mono text-[11.5px] text-ink-3'}" title={row.summary}>{row.summary}</span>
        </button>
        {#if open}
          <div data-testid="run-detail" class="flex flex-col gap-2 bg-paper-2 px-4 pb-3 pl-9">
            {#if run.resume_at}<span class="text-[12px] text-ink-3">{t('actions.runs.continues', { when: when(run.resume_at) })}</span>{/if}
            {#each runSteps(run) as step (step.node)}
              <section data-testid="run-step" data-node={step.node} data-status={step.status} class="flex flex-col gap-1 border-t border-line pt-2">
                <div class="flex min-w-0 items-baseline gap-2 text-[12.5px]">
                  <Icon icon={STEP_ICON[step.status] ?? 'lucide:circle-check'} class="shrink-0 self-center {STATUS_TONE[step.status] ?? 'text-ink-3'}" />
                  <span class="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11.5px] text-ink">{step.node}</span>
                  <span class="{STATUS_TONE[step.status] ?? 'text-ink-3'}">{statusLabel(step.status)}</span>
                  {#if step.ms !== null}<span class="ml-auto font-mono text-[11px] tabular-nums text-ink-3">{durationLabel(step.ms)}</span>{/if}
                </div>
                {#each step.details as detail, i (i)}
                  {#if /^[a-z_][a-z0-9_]*\(.*\)$/s.test(detail)}
                    <p class="overflow-hidden text-ellipsis whitespace-nowrap font-mono text-[11.5px] leading-5 text-ink-3" title={detail}>{detail}</p>
                  {:else}
                    <p class="whitespace-pre-wrap text-[12px] leading-5 text-ink-2">{detail}</p>
                  {/if}
                {/each}
                {#if step.error}<p class="whitespace-pre-wrap text-[12px] leading-5 text-error-600">{step.error}</p>{/if}
                {#if step.inputs !== undefined || step.outputs !== undefined}
                  <div class="grid grid-cols-2 gap-2">
                    <div class="flex min-w-0 flex-col gap-0.5">
                      <span class="text-[11px] font-medium uppercase tracking-wide text-ink-3">{t('actions.runs.step.inputs')}</span>
                      <div class="max-h-[16rem] min-h-0"><CodeEditor label={t('actions.runs.step.inputs')} language="json" value={shown(step.inputs)} readonly /></div>
                    </div>
                    <div class="flex min-w-0 flex-col gap-0.5">
                      <span class="text-[11px] font-medium uppercase tracking-wide text-ink-3">{t('actions.runs.step.outputs')}</span>
                      <div class="max-h-[16rem] min-h-0"><CodeEditor label={t('actions.runs.step.outputs')} language="json" value={shown(step.outputs)} readonly /></div>
                    </div>
                  </div>
                {/if}
              </section>
            {/each}
            {#if Object.keys(run.outputs ?? {}).length}
              <div class="flex flex-col gap-0.5 border-t border-line pt-2">
                <span class="text-[11px] font-medium uppercase tracking-wide text-ink-3">{t('actions.page.outputs')}</span>
                <div class="max-h-[16rem] min-h-0"><CodeEditor label={t('actions.page.outputs')} language="json" value={shown(run.outputs)} readonly /></div>
              </div>
            {/if}
          </div>
        {/if}
      </li>
    {:else}
      <li class="px-4 py-6 text-[13px] text-ink-3">{t('actions.runs.none')}</li>
    {/each}
  </ol>
</div>
