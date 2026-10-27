<script lang="ts">
  import Icon from '@iconify/svelte'
  import { api } from '$lib/tauri'
  import { describeExpectation, endOfDay, formatArrival, formatDue, sortExpectations } from '$lib/tauri/cases'
  import type { CaseExpectation, CaseFile, ExpectationKind } from '$lib/tauri/types'
  import { Button, Field, IconButton, Segmented, Sheet } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    caseFile: CaseFile
    onopen: (expectation: CaseExpectation) => void
  }

  let { caseFile, onopen }: Props = $props()

  let drafting = $state(false)
  let title = $state('')
  let kind = $state<ExpectationKind>('file')
  let from = $state('')
  let subject = $state('')
  let filename = $state('')
  let due = $state('')
  let busy = $state(false)
  let failure = $state<string | null>(null)

  const expectations = $derived(sortExpectations(caseFile.expectations))

  const kinds = $derived<{ value: ExpectationKind; label: string }[]>([
    { value: 'file', label: t('cases.expect.kindFile') },
    { value: 'message', label: t('cases.expect.kindMessage') },
    { value: 'reply', label: t('cases.expect.kindReply') }
  ])

  function start() {
    title = ''
    kind = 'file'
    from = ''
    subject = ''
    filename = ''
    due = ''
    failure = null
    drafting = true
  }

  const blank = (value: string) => (value.trim() ? value.trim() : null)

  async function save() {
    if (!title.trim()) {
      failure = t('cases.expect.needTitle')
      return
    }
    busy = true
    failure = null
    try {
      await api.postExpectation({
        case_id: caseFile.id,
        title: title.trim(),
        kind,
        from_match: blank(from),
        subject_match: blank(subject),
        filename_match: kind === 'file' ? blank(filename) : null,
        due_at: endOfDay(due)
      })
      drafting = false
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }

  async function toggle(expectation: CaseExpectation) {
    await api.putExpectation({ expectation_id: expectation.id, status: expectation.status === 'arrived' ? 'pending' : 'arrived' }).catch(() => {})
  }

  async function drop(expectation: CaseExpectation) {
    await api.deleteExpectation(expectation.id).catch(() => {})
  }
</script>

<div class="flex items-center gap-2 pb-2">
  <h2 class="flex-1 text-[13px] font-medium text-ink-2">{t('cases.expect.heading')}</h2>
  <Button size="sm" icon="lucide:plus" onclick={start}>{t('cases.expect.start')}</Button>
</div>

{#if expectations.length === 0}
  <p class="pb-2 text-[13px] leading-5 text-ink-3">{t('cases.expect.empty')}</p>
{:else}
  <ul class="flex flex-col" role="list" aria-label={t('cases.expect.heading')}>
    {#each expectations as expectation (expectation.id)}
      {@const arrived = expectation.status === 'arrived'}
      {@const overdue = expectation.status === 'overdue'}
      <li class="group flex items-start gap-2.5 border-b border-line py-2 last:border-b-0">
        <button class="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full {arrived ? 'bg-accent text-accent-ink' : overdue ? 'ring-1 ring-signal' : 'ring-1 ring-line-strong hover:ring-ink-3'}" onclick={() => toggle(expectation)} aria-label={arrived ? t('cases.expect.markExpected') : t('cases.expect.markArrived')} aria-pressed={arrived}>
          {#if arrived}<Icon icon="lucide:check" class="text-[11px]" />{/if}
        </button>
        <div class="min-w-0 flex-1">
          <p class="truncate text-[14px] leading-5 {arrived ? 'text-ink-2' : 'text-ink'}">{expectation.title}</p>
          {#if arrived}
            <button class="block max-w-full truncate text-left text-[12.5px] leading-5 text-accent hover:underline" onclick={() => onopen(expectation)}>
              {#if expectation.arrived_attachment_name}<span class="font-mono">{expectation.arrived_attachment_name}</span> · {/if}{formatArrival(expectation)}
            </button>
          {:else}
            <p class="truncate text-[12.5px] leading-5 {overdue ? 'text-signal' : 'text-ink-3'}">
              {[formatDue(expectation), describeExpectation(expectation)].filter(Boolean).join(' · ') || t('cases.expect.waiting')}
            </p>
          {/if}
        </div>
        <IconButton icon="lucide:x" label={t('cases.expect.stop')} size="sm" tone="danger" class="opacity-0 group-hover:opacity-100 focus:opacity-100" onclick={() => drop(expectation)} />
      </li>
    {/each}
  </ul>
{/if}

{#if drafting}
  <Sheet label={t('cases.expect.sheet')} width="30rem" onclose={() => (drafting = false)}>
    <form
      class="flex flex-col gap-3 p-4"
      onsubmit={(e) => {
        e.preventDefault()
        save()
      }}
    >
      <p class="text-[15px] font-semibold tracking-tight text-ink">{t('cases.expect.title')}</p>
      <Segmented bind:value={kind} options={kinds} size="sm" label={t('cases.expect.what')} />
      <label class="flex flex-col gap-1">
        <span class="text-[13px] text-ink-2">{t('cases.expect.fieldTitle')}</span>
        <Field bind:value={title} placeholder={t('cases.expect.titlePlaceholder')} />
      </label>
      <label class="flex flex-col gap-1">
        <span class="text-[13px] text-ink-2">{t('cases.expect.from')}</span>
        <Field bind:value={from} placeholder={t('cases.expect.fromPlaceholder')} />
      </label>
      <label class="flex flex-col gap-1">
        <span class="text-[13px] text-ink-2">{t('cases.expect.subject')}</span>
        <Field bind:value={subject} placeholder={t('cases.expect.subjectPlaceholder')} />
      </label>
      {#if kind === 'file'}
        <label class="flex flex-col gap-1">
          <span class="text-[13px] text-ink-2">{t('cases.expect.filename')}</span>
          <Field bind:value={filename} mono placeholder={t('cases.expect.filenamePlaceholder')} />
        </label>
      {/if}
      <label class="flex flex-col gap-1">
        <span class="text-[13px] text-ink-2">{t('cases.expect.due')}</span>
        <Field bind:value={due} type="date" size="sm" mono class="w-40" />
      </label>
      {#if failure}
        <p class="selectable text-[13px] text-error-600">{failure}</p>
      {/if}
      <button type="submit" class="hidden" aria-hidden="true" tabindex="-1"></button>
    </form>
    {#snippet footer()}
      <span class="flex-1 text-[12.5px] text-ink-3">{t('cases.expect.footer')}</span>
      <Button size="sm" onclick={() => (drafting = false)}>{t('common.cancel')}</Button>
      <Button variant="primary" size="sm" busy={busy} onclick={save}>{t('cases.expect.save')}</Button>
    {/snippet}
  </Sheet>
{/if}
