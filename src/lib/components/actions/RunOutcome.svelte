<script module lang="ts">
  export const STATUS_TONE: Record<string, string> = { done: 'text-success-600', failed: 'text-error-600', would: 'text-signal', skipped: 'text-ink-3', waiting: 'text-accent', running: 'text-accent', requirement: 'text-warn-600' }
  export const STATUS_DOT: Record<string, string> = { done: 'bg-success-500', failed: 'bg-error-500', would: 'bg-signal', skipped: 'bg-ink-3', waiting: 'bg-accent', running: 'bg-accent', requirement: 'bg-warn-500' }
  export const STEP_ICON: Record<string, string> = { failed: 'lucide:circle-x', requirement: 'lucide:triangle-alert', skipped: 'lucide:circle-dashed', would: 'lucide:circle-dot' }
</script>

<script lang="ts">
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import type { ActionRun } from '$lib/tauri/types'
  import { runReport, statusLabel } from '$lib/actions'
  import { Button, IconButton } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    run: ActionRun
    testid: string
    prefix?: string
    onsteps: () => void
    ondismiss?: () => void
  }

  let { run, testid, prefix = testid, onsteps, ondismiss }: Props = $props()

  const report = $derived(runReport(run))
  const glyph = $derived(report.failed ? 'lucide:circle-x' : report.needs.length ? 'lucide:triangle-alert' : run.status === 'running' ? 'lucide:loader-circle' : report.dry ? 'lucide:flask-conical' : 'lucide:circle-check')
</script>

<div class="flex min-w-0 items-center gap-3 text-[12.5px]" role="status" data-testid={testid} data-status={run.status}>
  <Icon icon={glyph} class="shrink-0 text-[15px] {STATUS_TONE[report.status] ?? 'text-ink-3'} {run.status === 'running' ? 'animate-spin' : ''}" />
  <span class="shrink-0 font-medium {STATUS_TONE[report.status] ?? 'text-ink'}">{t('actions.play.result', { status: report.label })}</span>
  {#if report.counts.length}
    <span class="flex shrink-0 items-center gap-1" data-testid="{prefix}-counts">
      {#each report.counts as entry (entry.status)}
        <span class="flex items-center gap-1 rounded-full bg-paper-3 px-1.5 text-[10.5px] leading-4 text-ink-2" title={statusLabel(entry.status)}><span class="size-1.5 rounded-full {STATUS_DOT[entry.status] ?? 'bg-ink-3'}"></span>{entry.count}</span>
      {/each}
    </span>
  {/if}
  {#if report.needs.length}
    <button type="button" data-testid="{prefix}-needs" data-href={report.needs[0].href} class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-left text-warn-600 hover:underline" title={report.message} onclick={() => goto(report.needs[0].href)}>{report.message}</button>
  {:else}
    <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap {report.failed ? 'text-error-600' : 'text-ink-2'}" title={report.effects.join('\n') || report.message}>{report.message}</span>
  {/if}
  {#if report.duration}<span class="shrink-0 font-mono text-[11px] tabular-nums text-ink-3">{report.duration}</span>{/if}
  <span class="shrink-0 text-[11px] text-ink-3">{t('actions.runs.logged')}</span>
  <Button size="sm" testid="{prefix}-steps" icon="lucide:list-tree" onclick={onsteps}>{t('actions.play.steps')}</Button>
  {#if ondismiss}
    <IconButton size="sm" testid="{prefix}-dismiss" icon="lucide:x" label={t('common.dismiss')} onclick={ondismiss} />
  {/if}
</div>
