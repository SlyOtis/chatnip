<script lang="ts">
  import { untrack } from 'svelte'
  import { goto } from '$app/navigation'
  import Icon from '@iconify/svelte'
  import { Button, IconButton, Segmented, Select } from '$lib/components/ui'
  import { api, messagesStore } from '$lib/tauri'
  import type { ActionDiagnostic, ActionRun } from '$lib/tauri/types'
  import { presented, inviteOf } from '$lib/messageKind'
  import { LOG_STEP, requirementBadge, requirementsFor, statusLabel, type NodeRequirement } from '$lib/actions'
  import { t } from '$lib/i18n.svelte'
  import { compileTemplate, defaultTemplate, forgetTemplates, SAMPLE_PROPS, templateProps, type TemplateKind, type TemplateProps } from '$lib/template'
  import CodeEditor from './CodeEditor.svelte'
  import TemplateFrame from '../conversation/messages/TemplateFrame.svelte'

  interface Props {
    title: string
    kind: TemplateKind
    value: string
    owner?: { action_id: string; node_id: string } | null
    onchange: (source: string) => void
    onclose?: () => void
  }

  let { title, kind, value, owner = null, onchange, onclose }: Props = $props()

  let source = $state(untrack(() => value.trim() || defaultTemplate(kind)))
  let mode = $state<'code' | 'preview'>('code')
  let theme = $state<'day' | 'night'>('day')
  let picked = $state('')
  let diagnostics = $state<ActionDiagnostic[]>([])
  let runtimeError = $state<string | null>(null)
  let checked = $state<'ok' | 'failed' | null>(null)
  let dry = $state<ActionRun | null>(null)
  let busy = $state<'check' | 'dry' | 'assist' | null>(null)
  let instruction = $state('')
  let turns = $state<{ role: 'you' | 'assistant'; text: string }[]>([])
  let proposal = $state<{ source: string; original: string } | null>(null)
  let needs = $state.raw<NodeRequirement[]>([])
  const badge = $derived(requirementBadge(needs))

  const messages = $derived(($messagesStore.data ?? []).slice(-40).reverse())
  const pickable = $derived([{ value: '', label: t('actions.template.sample') }, ...messages.map((m) => ({ value: m.id, label: `${m.sender_name ?? m.sender_email ?? t('actions.template.someone')}: ${(m.subject || m.content || '').replace(/<[^>]+>/g, ' ').slice(0, 60)}` }))])
  const preview = $derived.by<TemplateProps>(() => {
    const message = messages.find((m) => m.id === picked)
    if (!message) return { ...SAMPLE_PROPS[kind], theme }
    const presentation = { ...presented(message), kind }
    return templateProps(message, presentation, inviteOf(message), theme)
  })

  function edit(next: string) {
    source = next
    checked = null
    onchange(next)
  }

  async function check(): Promise<boolean> {
    busy = 'check'
    const compiled = await compileTemplate(source)
    busy = null
    diagnostics = compiled.ok ? [] : compiled.diagnostics.map((d) => ({ severity: 'error', message: d.message, node: null, edge: null, line: d.line, column: d.column }))
    checked = compiled.ok ? 'ok' : 'failed'
    return compiled.ok
  }

  async function dryRun() {
    if (!(await check())) return
    runtimeError = null
    mode = 'preview'
    if (!picked || !owner) {
      dry = null
      return
    }
    busy = 'dry'
    try {
      dry = await api.postActionNodeRun(owner.action_id, owner.node_id, { message: { id: picked } }, true, picked)
    } catch {
      runtimeError = t('actions.requirement.failed')
    } finally {
      busy = null
    }
  }

  function configure() {
    const href = badge?.href
    needs = []
    if (href) goto(href)
  }

  async function assist() {
    if (badge) return configure()
    const ask = instruction.trim()
    if (!ask) return
    turns = [...turns, { role: 'you', text: ask }]
    instruction = ''
    busy = 'assist'
    try {
      const outcome = await api.postTemplateAssist({ action_id: owner?.action_id ?? null, node_id: owner?.node_id ?? null, kind, instruction: ask, source })
      needs = requirementsFor([outcome.requirement])
      if (needs.length) {
        turns = turns.slice(0, -1)
        instruction = ask
        return
      }
      turns = [...turns, { role: 'assistant', text: outcome.explanation || t('actions.template.newVersion') }]
      if (outcome.source.trim() && outcome.source !== source) proposal = { source: outcome.source, original: source }
      mode = 'code'
    } catch {
      turns = [...turns, { role: 'assistant', text: t('actions.requirement.failed') }]
    } finally {
      busy = null
    }
  }

  function apply() {
    if (!proposal) return
    edit(proposal.source)
    proposal = null
    check()
  }

  function reset() {
    source = defaultTemplate(kind)
    checked = null
    onchange('')
    check()
  }

  $effect(() => () => forgetTemplates())
</script>

<div class="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_minmax(16rem,0.4fr)]">
  <section class="flex min-h-0 flex-col border-r border-line">
    <div class="flex flex-wrap items-center gap-2 border-b border-line px-4 py-2">
      <Icon icon="lucide:layout-template" class="text-[15px] text-ink-3" />
      <span class="text-[13px] font-semibold text-ink">{title}</span>
      <span class="font-mono text-[11px] text-ink-3">{kind}</span>
      <Segmented size="sm" label={t('actions.template.editor')} value={mode} options={[{ value: 'code', label: t('actions.inspector.code'), icon: 'lucide:code' }, { value: 'preview', label: t('actions.template.preview'), icon: 'lucide:eye' }]} onchange={(v) => (mode = v as 'code' | 'preview')} />
      {#if mode === 'preview'}
        <Segmented size="sm" label={t('actions.template.theme')} value={theme} options={[{ value: 'day', label: t('actions.template.day'), icon: 'lucide:sun' }, { value: 'night', label: t('actions.template.night'), icon: 'lucide:moon' }]} onchange={(v) => (theme = v as 'day' | 'night')} />
      {/if}
      <span class="flex-1"></span>
      <Button size="sm" icon="lucide:circle-check" busy={busy === 'check'} onclick={check}>{t('actions.template.check')}</Button>
      <Button size="sm" icon="lucide:flask-conical" busy={busy === 'dry'} onclick={dryRun}>{t('actions.page.dryRun')}</Button>
      <Button size="sm" onclick={reset}>{t('actions.picker.builtin')}</Button>
      {#if onclose}<IconButton size="sm" icon="lucide:x" label={t('actions.template.close')} onclick={onclose} />{/if}
    </div>
    {#if proposal}
      <div class="flex items-center gap-2 border-b border-line bg-accent-soft px-4 py-2 text-[13px] text-ink">
        <Icon icon="lucide:sparkles" />
        <span class="flex-1">{t('actions.template.proposal')}</span>
        <Button size="sm" onclick={() => (proposal = null)}>{t('actions.page.discard')}</Button>
        <Button size="sm" variant="primary" onclick={apply}>{t('common.apply')}</Button>
      </div>
    {/if}
    {#if checked === 'ok' && !runtimeError}
      <p class="flex items-center gap-1.5 border-b border-line px-4 py-1.5 text-[12.5px] text-success-600"><Icon icon="lucide:circle-check" />{t('actions.template.compiles')}</p>
    {:else if diagnostics.length || runtimeError}
      <ul class="flex flex-col gap-1 border-b border-line px-4 py-1.5">
        {#each diagnostics as d, i (i)}
          <li class="flex gap-1.5 text-[12.5px] leading-4 text-error-600"><Icon icon="lucide:circle-alert" class="mt-0.5 shrink-0" />{d.line ? t('actions.page.line', { line: d.line }) : ''}{d.message}</li>
        {/each}
        {#if runtimeError}<li class="flex gap-1.5 text-[12.5px] leading-4 text-error-600"><Icon icon="lucide:circle-alert" class="mt-0.5 shrink-0" />{runtimeError}</li>{/if}
      </ul>
    {/if}
    <div class="min-h-0 flex-1">
      {#if mode === 'code'}
        <div class="h-full px-2">
          <CodeEditor label={t('actions.template.source')} language="svelte" value={proposal ? proposal.source : source} original={proposal?.original ?? null} readonly={!!proposal} {diagnostics} onchange={edit} />
        </div>
      {:else}
        <div class="no-scrollbar flex h-full min-h-0 flex-col gap-3 overflow-y-auto p-4">
          <Select size="sm" bind:value={picked} options={pickable} />
          <div class="rounded-[0.75rem] p-4 {theme === 'night' ? 'frost-dark bg-[#211e1b]' : 'frost-light bg-[#faf8f4]'}">
            <TemplateFrame {source} props={preview} onerror={(message) => (runtimeError = message)} oncompiled={(found) => { if (found.length) checked = 'failed' }} />
          </div>
          {#if dry}
            <div class="flex flex-col gap-1">
              <span class="text-[12px] font-medium uppercase tracking-wide text-ink-3">{t('actions.template.would')}</span>
              {#each dry.log.filter((entry) => entry.kind !== LOG_STEP) as entry, i (i)}
                <p class="text-[12.5px] text-ink-2"><span class="font-mono text-[11px] text-ink-3">{statusLabel(entry.status)}</span> {entry.detail}</p>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </section>
  <aside class="flex min-h-0 flex-col">
    <div class="border-b border-line px-3 py-2 text-[12px] font-medium uppercase tracking-wide text-ink-3">{t('actions.template.assistant')}</div>
    <ol class="no-scrollbar flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto p-3">
      {#each turns as turn, i (i)}
        <li class="rounded-[0.5rem] px-2.5 py-1.5 text-[12.5px] leading-5 {turn.role === 'you' ? 'self-end bg-accent-soft text-ink' : 'bg-paper-2 text-ink-2'}">{turn.text}</li>
      {/each}
      {#if !turns.length}
        <li class="text-[12.5px] leading-5 text-ink-3">{t('actions.template.intro')}</li>
      {/if}
    </ol>
    <form class="flex flex-col gap-2 border-t border-line p-3" onsubmit={(e) => { e.preventDefault(); assist() }}>
      <textarea rows="3" class="resize-none bg-transparent text-[13px] text-ink outline-none placeholder:text-ink-3" placeholder={t('actions.template.placeholder')} bind:value={instruction} onkeydown={(e) => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); assist() } }}></textarea>
      {#if badge}
        <Button size="sm" variant="warn" testid="template-assist-needs" icon="lucide:triangle-alert" title={badge.title} onclick={configure}>{badge.label}</Button>
      {:else}
        <Button size="sm" variant="primary" type="submit" icon="lucide:sparkles" busy={busy === 'assist'}>{t('actions.template.ask')}</Button>
      {/if}
    </form>
  </aside>
</div>
