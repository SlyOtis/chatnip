<script lang="ts">
  import Icon from '@iconify/svelte'
  import { page } from '$app/state'
  import { goto } from '$app/navigation'
  import { api } from '$lib/tauri'
  import type { CaseFile } from '$lib/tauri/types'
  import { casesStore, loadCases, caseSignal, waitingCount, CASE_COLORS } from '$lib/tauri/cases'
  import { Button, Field, PaneGrid } from '$lib/components/ui'
  import CaseView from '$lib/components/cases/CaseView.svelte'
  import { t } from '$lib/i18n.svelte'

  let creating = $state(false)
  let name = $state('')
  let query = $state('')

  $effect(() => {
    loadCases().catch(() => {})
  })

  const selected = $derived(page.url.searchParams.get('id'))
  const matching = $derived($casesStore.filter((c) => !query.trim() || c.name.toLowerCase().includes(query.trim().toLowerCase())))
  const open = $derived(matching.filter((c) => c.status !== 'done').sort((a, b) => (b.last_activity ?? b.updated_at).localeCompare(a.last_activity ?? a.updated_at)))
  const closed = $derived(matching.filter((c) => c.status === 'done').sort((a, b) => b.updated_at.localeCompare(a.updated_at)))

  function pick(id: string) {
    goto(`/main/cases?id=${id}`, { replaceState: true, keepFocus: true, noScroll: true })
  }

  async function create() {
    const trimmed = name.trim()
    if (!trimmed) return
    const made = await api.postCase({ name: trimmed, color: CASE_COLORS[$casesStore.length % CASE_COLORS.length] })
    name = ''
    creating = false
    await loadCases()
    pick(made.id)
  }
</script>

{#snippet caseRow(c: CaseFile)}
  {@const active = selected === c.id}
  {@const waiting = waitingCount(c)}
  <button
    type="button"
    class="grid h-12 w-full grid-cols-[1.25rem_minmax(0,1fr)_auto] items-center gap-2 rounded-[0.5rem] px-2 text-left {active ? 'bg-accent-soft text-ink' : 'text-ink-2 hover:bg-paper-2 hover:text-ink'}"
    onclick={() => pick(c.id)}
    aria-current={active ? 'page' : undefined}
  >
    <span class="flex items-center justify-center"><span class="size-2 rounded-full" style:background={c.color}></span></span>
    <span class="flex min-w-0 flex-col">
      <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[14px] {c.status === 'done' ? 'text-ink-3 line-through' : ''}">{c.name}</span>
      <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[12px] text-ink-3">{t('cases.list.conversations', { count: c.conversation_count })} · {t('common.messages', { count: c.message_count })}</span>
    </span>
    <span class="font-mono text-[11px] {caseSignal(c) ? 'text-signal' : 'text-ink-3'}">{waiting > 0 ? waiting : ''}</span>
  </button>
{/snippet}

<PaneGrid name="cases" panes={[{ key: 'list', size: '20rem', min: '14rem', max: '36vw', edge: 'end', class: 'bg-paper' }, { key: 'case' }]}>
  {#snippet pane(key)}
    {#if key === 'list'}
      <header class="flex h-14 shrink-0 items-center gap-2 px-3">
        <Icon icon="lucide:briefcase" class="text-[15px] text-ink-2" />
        <h1 class="min-w-0 flex-1 text-[15px] font-medium tracking-tight text-ink">{t('cases.title')}</h1>
        <Button size="sm" icon="lucide:plus" onclick={() => (creating = !creating)}>{t('cases.list.new')}</Button>
      </header>
      <div class="flex shrink-0 flex-col gap-2 px-3 pb-2">
        {#if creating}
          <form class="flex gap-2" onsubmit={(e) => { e.preventDefault(); create().catch(() => {}) }}>
            <Field bind:value={name} size="sm" placeholder={t('cases.view.namePlaceholder')} aria-label={t('cases.picker.caseName')} />
            <Button size="sm" variant="primary" type="submit" keys="↵">{t('common.create')}</Button>
          </form>
        {/if}
        <Field bind:value={query} size="sm" placeholder={t('cases.list.find')} aria-label={t('cases.list.find')} />
      </div>
      <div class="no-scrollbar flex min-h-0 flex-1 flex-col gap-px overflow-y-auto px-1.5 pb-3">
        {#each open as c (c.id)}
          {@render caseRow(c)}
        {/each}
        {#if closed.length}
          <p class="mt-4 px-2 pb-1 text-[12.5px] text-ink-3">{t('cases.list.closed')}</p>
          {#each closed as c (c.id)}
            {@render caseRow(c)}
          {/each}
        {/if}
        {#if !$casesStore.length}
          <p class="px-2 py-6 text-[13px] text-ink-3">{t('cases.list.empty', { key: '⇧C' })}</p>
        {/if}
      </div>
    {:else if selected}
      {#key selected}
        <div class="h-full min-h-0 bg-paper"><CaseView caseId={selected} /></div>
      {/key}
    {:else}
      <div class="empty-host relative flex h-full flex-col items-center justify-center px-6 text-center">
        <div class="empty-frost" aria-hidden="true"></div>
        <p class="text-[15px] font-medium tracking-tight text-ink">{t('cases.list.pick')}</p>
        <p class="mt-1 text-[13px] text-ink-3">{t('cases.list.pickHint')}</p>
      </div>
    {/if}
  {/snippet}
</PaneGrid>
