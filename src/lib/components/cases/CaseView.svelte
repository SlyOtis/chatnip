<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import { layoutSettings } from '$lib/layoutSettings'
  import { collapsedSections, holdTimeline, putSectionCollapsed, splitSections } from '$lib/sections.svelte'
  import SectionHeader from '$lib/components/conversation/messages/SectionHeader.svelte'
  import SectionSummaryCard from '$lib/components/conversation/messages/SectionSummaryCard.svelte'
  import SectionActions from '$lib/components/conversation/SectionActions.svelte'
  import { setContext, tick } from 'svelte'
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { api, casesById, conversationsStore, profileStore } from '$lib/tauri'
  import { CASE_COLORS, CASE_STATUSES, dateValue, endOfDay, formatDue, timelineRuns } from '$lib/tauri/cases'
  import type { CaseExpectation, CaseFile, CaseItem, CaseRule, CaseStatus, Message, RuleCondition } from '$lib/tauri/types'
  import { collapsedEmails, type MessageKind } from '$lib/messageKind'
  import { Button, Field, IconButton, Segmented, Sheet } from '$lib/components/ui'
  import RuleConditions from '$lib/components/settings/RuleConditions.svelte'
  import { describeConditions, emptyCondition, regexError } from '$lib/components/settings/rules'
  import MessageGroup from '$lib/components/conversation/messages/MessageGroup.svelte'
  import EmptySpinner from '$lib/components/conversation/EmptySpinner.svelte'
  import CaseExpectations from './CaseExpectations.svelte'
  import { t } from '$lib/i18n.svelte'
  import { pulse } from '$lib/window'

  interface Props {
    caseId: string
  }

  let { caseId }: Props = $props()

  let loaded = $state<CaseFile | null>(null)
  let messages = $state<Message[]>([])
  let loading = $state(true)
  let failure = $state<string | null>(null)
  let name = $state('')
  let description = $state('')
  let status = $state<string>('open')
  let due = $state('')
  let scroller = $state<HTMLDivElement | null>(null)

  const caseFile = $derived($casesById.get(caseId) ?? loaded)
  const names = $derived(new Map($conversationsStore.data.map((c) => [c.id, c.name])))
  const timestampInterval = $derived($profileStore.data?.settings?.message_timestamp_interval || 5)
  const conversations = $derived(caseFile?.items.filter((item) => item.conversation_id) ?? [])
  const singles = $derived(caseFile?.items.filter((item) => !item.conversation_id && item.message_id) ?? [])

  async function load() {
    failure = null
    try {
      const [file, timeline] = await Promise.all([api.getCase(caseId), api.getCaseTimeline(caseId, 400)])
      loaded = file
      messages = timeline
    } catch (error) {
      failure = String(error)
    } finally {
      loading = false
    }
  }

  $effect(() => {
    caseId
    loading = true
    load()
  })

  let stamp = ''
  $effect(() => {
    const next = caseFile ? `${caseFile.updated_at}:${caseFile.items.length}:${caseFile.arrived_count}` : ''
    if (stamp && next && next !== stamp) api.getCaseTimeline(caseId, 400).then((timeline) => (messages = timeline)).catch(() => {})
    stamp = next
  })

  let synced = ''
  $effect(() => {
    if (!caseFile || synced === caseFile.id + caseFile.updated_at) return
    synced = caseFile.id + caseFile.updated_at
    name = caseFile.name
    description = caseFile.description
    status = caseFile.status
    due = dateValue(caseFile.due_at)
  })

  async function save(patch: { name?: string; description?: string; status?: CaseStatus; color?: string; due_at?: string | null }) {
    if (!caseFile) return
    try {
      loaded = await api.putCase({ case_id: caseFile.id, ...patch })
    } catch (error) {
      failure = String(error)
    }
  }

  function saveName() {
    const next = name.trim()
    if (!caseFile || !next || next === caseFile.name) {
      if (caseFile) name = caseFile.name
      return
    }
    save({ name: next })
  }

  function saveDescription() {
    if (caseFile && description !== caseFile.description) save({ description })
  }

  function cycleColor() {
    if (!caseFile) return
    const index = CASE_COLORS.indexOf(caseFile.color as (typeof CASE_COLORS)[number])
    save({ color: CASE_COLORS[(index + 1) % CASE_COLORS.length] })
  }

  async function removeItem(item: CaseItem) {
    await api.deleteCaseItems({ case_id: caseId, conversation_ids: item.conversation_id ? [item.conversation_id] : [], message_ids: !item.conversation_id && item.message_id ? [item.message_id] : [] }).catch(() => {})
  }

  async function removeCase() {
    if (!caseFile) return
    await api.deleteCase(caseFile.id).catch(() => {})
    goto('/main/conversation')
  }

  async function openArrival(expectation: CaseExpectation) {
    const id = expectation.arrived_message_id
    if (!id) return
    const owner = split.sections.find((s) => s.messages.some((m) => m.id === id))
    if (owner && collapsed.has(owner.id)) await putSectionCollapsed(scope, owner.id, false)
    await tick()
    const target = scroller?.querySelector<HTMLElement>(`[data-messages~="${CSS.escape(id)}"]`)
    if (target) {
      target.scrollIntoView({ block: 'center' })
      pulse(target)
    } else if (expectation.arrived_conversation_id) {
      goto(conversationHref(expectation.arrived_conversation_id))
    }
  }

  type Group = { messages: Message[] }

  function senderGroups(list: Message[]): Group[] {
    const out: Group[] = []
    for (const message of list) {
      const last = out[out.length - 1]
      const previous = last?.messages[last.messages.length - 1]
      const gap = previous ? (new Date(message.timestamp).getTime() - new Date(previous.timestamp).getTime()) / 1000 : Infinity
      if (last && previous && previous.sender_id === message.sender_id && gap <= timestampInterval * 60) last.messages.push(message)
      else out.push({ messages: [message] })
    }
    return out
  }

  const runs = $derived(timelineRuns(messages, names).map((run) => ({ ...run, groups: senderGroups(run.messages) })))
  const folded = $derived(collapsedEmails(runs.flatMap((run) => run.groups.map((g) => ({ id: g.messages[0].id, kind: (g.messages[0].type === 'email' ? 'email' : 'chat') as MessageKind, conversationId: run.conversationId })))))

  const scope = $derived(`case:${caseId}`)
  const groups = $derived(runs.flatMap((run) => run.groups))
  const split = $derived(splitSections($layoutSettings, groups, (group) => ({ key: group.messages[0].id, first: group.messages[0].timestamp, last: group.messages[group.messages.length - 1].timestamp, messages: group.messages })))
  const collapsed = $derived(split.headed ? collapsedSections($layoutSettings, scope) : new Set<string>())

  holdTimeline(() => (caseFile && messages.length ? { scope, source: { kind: 'case', id: caseFile.id }, conversationId: null, title: caseFile.name, sections: split.sections } : null))

  const repeatedSubjects = new Set<string>()
  setContext('subject-repeats', repeatedSubjects)

  const meta = $derived.by(() => {
    if (!caseFile) return [] as { text: string; overdue: boolean }[]
    const parts = [{ text: t('cases.meta.threads', { count: caseFile.conversation_count }), overdue: false }]
    if (caseFile.message_count) parts.push({ text: t('common.messages', { count: caseFile.message_count }), overdue: false })
    if (caseFile.expectations.length) parts.push({ text: t('cases.meta.arrived', { arrived: caseFile.arrived_count, total: caseFile.expectations.length }), overdue: false })
    const dueLabel = formatDue(caseFile)
    if (dueLabel) parts.push({ text: dueLabel, overdue: dateValue(caseFile.due_at) < dateValue(new Date().toISOString()) })
    return parts
  })
  let ruleDrafting = $state(false)
  let ruleMatch = $state<'all' | 'any'>('all')
  let ruleConditions = $state<RuleCondition[]>([emptyCondition()])
  let ruleBusy = $state(false)
  let ruleFailure = $state<string | null>(null)

  const ruleValid = $derived(ruleConditions.every((c) => c.value.trim().length > 0 && regexError(c) === null))

  function startRule() {
    ruleMatch = 'all'
    ruleConditions = [emptyCondition()]
    ruleFailure = null
    ruleDrafting = true
  }

  async function saveRule() {
    if (!caseFile || !ruleValid || ruleBusy) return
    ruleBusy = true
    ruleFailure = null
    try {
      loaded = await api.postCaseRule({ case_id: caseFile.id, match: ruleMatch, conditions: ruleConditions.map((c) => ({ ...c, value: c.value.trim() })) })
      ruleDrafting = false
    } catch (error) {
      ruleFailure = String(error)
    } finally {
      ruleBusy = false
    }
  }

  async function removeRule(rule: CaseRule) {
    await api.deleteCaseRule(rule.id).catch(() => {})
  }
</script>

{#if loading && !caseFile}
  <EmptySpinner />
{:else if !caseFile}
  <div class="flex h-full flex-col items-center justify-center gap-2 px-8 text-center">
    <p class="text-[15px] font-semibold tracking-tight text-ink">{t('cases.view.notFound')}</p>
    <p class="selectable max-w-sm text-[13px] text-ink-3">{failure ?? t('cases.view.maybeDeleted')}</p>
    <Button variant="primary" onclick={() => goto('/main/conversation')}>{t('cases.view.backToInbox')}</Button>
  </div>
{:else}
  <div class="rise grid h-full grid-cols-[minmax(0,1fr)_clamp(16rem,30%,22rem)] grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-paper">
    <header class="group/bar col-span-2 flex items-start gap-4 border-b border-line px-8 pb-3 pt-7">
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2.5">
          <button class="size-2.5 shrink-0 rounded-full" style:background={caseFile.color} onclick={cycleColor} aria-label={t('cases.view.changeColour')} title={t('cases.view.changeColour')}></button>
          <input
            bind:value={name}
            class="min-w-0 flex-1 bg-transparent text-[18px] font-semibold leading-7 tracking-tight text-ink outline-none placeholder:text-ink-3"
            aria-label={t('cases.picker.caseName')}
            placeholder={t('cases.view.namePlaceholder')}
            onblur={saveName}
            onkeydown={(e) => {
              if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur()
              if (e.key === 'Escape') {
                name = caseFile?.name ?? name
                ;(e.currentTarget as HTMLInputElement).blur()
              }
            }}
          />
        </div>
        <p class="mt-0.5 text-[13px] leading-5 text-ink-3">
          {#each meta as part, i (i)}
            {#if i > 0}<span class="mx-1.5">·</span>{/if}<span class={part.overdue ? 'text-signal' : ''}>{part.text}</span>
          {/each}
        </p>
        <textarea
          bind:value={description}
          rows="1"
          class="mt-2 w-full max-w-[72ch] resize-none bg-transparent text-[13.5px] leading-5 text-ink-2 outline-none [field-sizing:content] placeholder:text-ink-3"
          placeholder={t('cases.view.descriptionPlaceholder')}
          aria-label={t('cases.view.description')}
          onblur={saveDescription}
        ></textarea>
      </div>
      <div class="flex shrink-0 items-center gap-2 pt-1">
        <span class="flex items-center gap-0.5"><SectionActions /></span>
        <Segmented bind:value={status} options={CASE_STATUSES.map((s) => ({ value: s.value, label: t(s.label) }))} size="sm" label={t('cases.view.status')} onchange={(value) => save({ status: value as CaseStatus })} />
        <Field bind:value={due} type="date" size="sm" mono class="w-36" aria-label={t('cases.view.dueDate')} onchange={() => save({ due_at: endOfDay(due) })} />
        <IconButton icon="lucide:trash-2" label={t('cases.view.delete')} tone="danger" onclick={removeCase} />
      </div>
    </header>

    <div class="relative min-h-0 overflow-hidden">
      {#if messages.length === 0}
        <div class="flex h-full flex-col items-center justify-center px-8 text-center">
          <p class="text-[15px] font-semibold tracking-tight text-ink">{t('cases.view.emptyTitle')}</p>
          <p class="mt-1 max-w-sm text-[13px] leading-5 text-ink-3">{t('cases.view.emptyBody')}</p>
        </div>
      {:else}
        <div class="absolute inset-0 overflow-y-auto overflow-x-hidden" bind:this={scroller}>
          <div class="w-full max-w-[min(100%,88ch)] px-8 pb-8 pt-4">
            <SectionSummaryCard {scope} kind="all" />
            <SectionSummaryCard {scope} kind="selection" />
            {#each runs as run, r (run.messages[0].id)}
              {@const previousRun = r > 0 ? runs[r - 1] : null}
              {@const head = split.owner.get(run.groups[0].messages[0].id)}
              {@const headOpens = !!head && head !== split.owner.get(previousRun ? previousRun.groups[previousRun.groups.length - 1].messages[0].id : '')}
              {#if head && headOpens}
                <SectionHeader section={head} {scope} headed={split.headed} />
              {/if}
              {#if !head || !collapsed.has(head.id)}
                <div class="flex items-center gap-2 text-[13px] text-ink-3 {r === 0 || headOpens ? 'pb-1' : 'pb-1 pt-4'}">
                  <Icon icon="lucide:corner-down-right" class="text-[13px]" />
                  <button class="truncate hover:text-ink" onclick={() => goto(conversationHref(run.conversationId))}>{t('cases.view.fromConversation', { name: run.conversationName })}</button>
                  <span class="h-px flex-1 bg-line"></span>
                </div>
              {/if}
              {#each run.groups as group, g (group.messages[0].id)}
                {@const section = split.owner.get(group.messages[0].id)}
                {#if g > 0 && section && section !== split.owner.get(run.groups[g - 1].messages[0].id)}
                  <SectionHeader {section} {scope} headed={split.headed} />
                {/if}
                {#if !section || !collapsed.has(section.id)}
                  <div data-messages={group.messages.map((m) => m.id).join(' ')} class="rounded-[0.5rem]">
                    <MessageGroup messages={group.messages} collapsed={folded.has(group.messages[0].id)} />
                  </div>
                {/if}
              {/each}
            {/each}
          </div>
        </div>
      {/if}
    </div>

    <aside class="min-h-0 overflow-y-auto border-l border-line px-5 pb-6 pt-4" aria-label={t('cases.view.details')}>
      <CaseExpectations {caseFile} onopen={openArrival} />

      <div class="flex items-center gap-2 pb-2 pt-6">
        <h2 class="flex-1 text-[13px] font-medium text-ink-2">{t('cases.view.items')}</h2>
      </div>
      {#if conversations.length + singles.length === 0}
        <p class="text-[13px] leading-5 text-ink-3">{t('cases.view.noItems')}</p>
      {:else}
        <ul class="flex flex-col" role="list" aria-label={t('cases.view.items')}>
          {#each [...conversations, ...singles] as item (item.id)}
            <li class="group flex items-center gap-2 border-b border-line py-1.5 last:border-b-0">
              <Icon icon={item.conversation_id ? (item.conversation_type === 'email' ? 'lucide:mail' : 'lucide:message-circle') : 'lucide:text-quote'} class="shrink-0 text-[13px] text-ink-3" />
              <button class="min-w-0 flex-1 truncate text-left text-[13.5px] text-ink-2 hover:text-ink" onclick={() => item.conversation_id && goto(conversationHref(item.conversation_id))}>{item.title || names.get(item.conversation_id ?? '') || t('cases.view.message')}</button>
              <IconButton icon="lucide:x" label={t('cases.view.removeItem')} size="sm" tone="danger" class="opacity-0 group-hover:opacity-100 focus:opacity-100" onclick={() => removeItem(item)} />
            </li>
          {/each}
        </ul>
      {/if}
      <div class="flex items-center gap-2 pb-2 pt-6">
        <h2 class="flex-1 text-[13px] font-medium text-ink-2">{t('cases.view.autoAdd')}</h2>
        <Button size="sm" icon="lucide:plus" onclick={startRule}>{t('cases.view.addRuleButton')}</Button>
      </div>
      {#if caseFile.rules.length === 0}
        <p class="text-[13px] leading-5 text-ink-3">{t('cases.view.rulesEmpty')}</p>
      {:else}
        <ul class="flex flex-col" role="list" aria-label={t('cases.view.rules')}>
          {#each caseFile.rules as rule (rule.id)}
            <li class="group flex items-start gap-2 border-b border-line py-1.5 last:border-b-0">
              <Icon icon="lucide:filter" class="mt-1 shrink-0 text-[13px] text-ink-3" />
              <p class="min-w-0 flex-1 text-[13px] leading-5 {rule.enabled ? 'text-ink-2' : 'text-ink-3 line-through'}">{describeConditions(rule.conditions, rule.match)}</p>
              <IconButton icon="lucide:x" label={t('cases.view.removeRule')} size="sm" tone="danger" class="opacity-0 group-hover:opacity-100 focus:opacity-100" onclick={() => removeRule(rule)} />
            </li>
          {/each}
        </ul>
      {/if}
    </aside>
  </div>

  {#if ruleDrafting}
    <Sheet label={t('cases.view.ruleSheet')} width="40rem" onclose={() => (ruleDrafting = false)}>
      <form
        class="flex flex-col gap-3 px-5 py-4"
        onsubmit={(e) => {
          e.preventDefault()
          saveRule()
        }}
        onkeydown={(e) => {
          if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) saveRule()
        }}
      >
        <div class="flex items-center gap-3">
          <p class="flex-1 text-[14px] font-medium text-ink">{t('cases.view.ruleIntro', { name: caseFile.name })}</p>
          <Segmented label={t('cases.view.match')} value={ruleMatch} options={[{ value: 'all', label: t('cases.view.matchAll') }, { value: 'any', label: t('cases.view.matchAny') }]} onchange={(v) => (ruleMatch = v as 'all' | 'any')} />
        </div>
        <RuleConditions bind:conditions={ruleConditions} />
        {#if ruleFailure}
          <p class="selectable text-[13px] text-error-600">{ruleFailure}</p>
        {/if}
        <button type="submit" class="hidden" aria-hidden="true" tabindex="-1"></button>
      </form>
      {#snippet footer()}
        <span class="flex-1 text-[12.5px] text-ink-3">{t('cases.view.ruleFooter')}</span>
        <Button size="sm" onclick={() => (ruleDrafting = false)}>{t('common.cancel')}</Button>
        <Button variant="primary" size="sm" keys="⌘↵" busy={ruleBusy} disabled={!ruleValid} onclick={saveRule}>{t('cases.view.addRule')}</Button>
      {/snippet}
    </Sheet>
  {/if}
{/if}
