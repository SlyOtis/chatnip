<script lang="ts">
  import { goto } from '$app/navigation'
  import { api, casesStore, casePickerStore, closeCasePicker } from '$lib/tauri'
  import { caseSignal, filterCases, memberships, waitingCount } from '$lib/tauri/cases'
  import type { CaseFile } from '$lib/tauri/types'
  import { Button, Field, Kbd, MenuItem, Sheet } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  let query = $state('')
  let index = $state(0)
  let busy = $state(false)
  let failure = $state<string | null>(null)

  const target = $derived($casePickerStore)
  const hasTarget = $derived(!!target && (target.conversationIds.length > 0 || target.messageIds.length > 0))
  const inside = $derived(target ? memberships($casesStore, target.conversationIds, target.messageIds) : new Set<string>())
  const matches = $derived(filterCases($casesStore, query))
  const canCreate = $derived(query.trim().length > 0 && !$casesStore.some((c) => c.name.toLowerCase() === query.trim().toLowerCase()))
  const total = $derived(matches.length + (canCreate ? 1 : 0))

  $effect(() => {
    if (target) {
      query = ''
      index = 0
      failure = null
    }
  })

  $effect(() => {
    query
    index = 0
  })

  function close() {
    closeCasePicker()
  }

  async function toggle(c: CaseFile) {
    if (!target) return
    if (!hasTarget) {
      close()
      goto('/main/case/' + c.id)
      return
    }
    busy = true
    failure = null
    const input = { case_id: c.id, conversation_ids: target.conversationIds, message_ids: target.messageIds }
    try {
      await (inside.has(c.id) ? api.deleteCaseItems(input) : api.putCaseItems(input))
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }

  async function create() {
    if (!target || !query.trim()) return
    busy = true
    failure = null
    try {
      const created = await api.postCase({ name: query.trim(), conversation_ids: target.conversationIds, message_ids: target.messageIds })
      query = ''
      if (target.create || !hasTarget) {
        close()
        goto('/main/case/' + created.id)
      }
    } catch (error) {
      failure = String(error)
    } finally {
      busy = false
    }
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      index = Math.min(index + 1, Math.max(total - 1, 0))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      index = Math.max(index - 1, 0)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (index < matches.length) toggle(matches[index])
      else if (canCreate) create()
    } else if (e.key === 'Escape') {
      e.preventDefault()
      close()
    }
  }
</script>

{#if target}
  <Sheet label={hasTarget ? t('inbox.addToCase') : t('cases.title')} width="28rem" onclose={close}>
    <div class="p-3 pb-1">
      <Field bind:value={query} placeholder={target.create ? t('cases.picker.nameNew') : t('cases.picker.find')} onkeydown={keydown} aria-label={t('cases.picker.caseName')} />
    </div>
    <div class="p-1.5" role="menu" aria-label={t('cases.title')}>
      {#each matches as c, i (c.id)}
        <MenuItem selected={i === index} checked={inside.has(c.id)} detail={waitingCount(c) ? t('cases.waitingCount', { count: waitingCount(c) }) : undefined} onclick={() => toggle(c)}>
          <span class="mr-2 inline-block size-2 rounded-full align-middle" style:background={c.color}></span><span class={caseSignal(c) ? 'text-ink' : c.status === 'done' ? 'text-ink-3' : ''}>{c.name}</span>
        </MenuItem>
      {/each}
      {#if canCreate}
        <MenuItem icon="lucide:plus" selected={index === matches.length} onclick={create}>{t('cases.picker.newCase', { name: query.trim() })}</MenuItem>
      {:else if matches.length === 0}
        <p class="px-2.5 py-2 text-[13px] text-ink-3">{t('cases.picker.empty')}</p>
      {/if}
      {#if failure}
        <p class="selectable px-2.5 py-1 text-[13px] text-error-600">{failure}</p>
      {/if}
    </div>
    {#snippet footer()}
      <span class="flex-1 text-[12.5px] text-ink-3">
        {#if hasTarget}
          {inside.size ? t('cases.picker.inCases', { count: inside.size }) : t('cases.picker.notInCase')}
        {:else}
          <Kbd keys="↵" /> {t('cases.picker.toOpen')}
        {/if}
      </span>
      <Button variant="primary" size="sm" busy={busy} onclick={close}>{t('common.done')}</Button>
    {/snippet}
  </Sheet>
{/if}
