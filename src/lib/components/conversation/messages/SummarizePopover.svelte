<script module lang="ts">
  import type { SummaryOptions } from '$lib/tauri/types'

  let last = $state<SummaryOptions>({ focus: '', include: '', exclude: '', without_files: false })
</script>

<script lang="ts">
  import type { Snippet } from 'svelte'
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { anchored, type Box } from '$lib/popover'
  import { settingHref } from '$lib/settings'
  import { Button, Field, Switch } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    at: Box | (() => Box | null)
    title: string
    onsubmit: (options: SummaryOptions) => void
    onclose: () => void
    children?: Snippet
  }

  let { at, title, onsubmit, onclose, children }: Props = $props()

  let filtering = $state(!!(last.include || last.exclude || last.without_files))
  const invalid = $derived([last.include, last.exclude].find((pattern) => {
    if (!pattern?.trim()) return false
    try {
      new RegExp(pattern)
      return false
    } catch {
      return true
    }
  }))

  function submit() {
    if (invalid) return
    onsubmit({ focus: last.focus?.trim() || undefined, include: last.include?.trim() || undefined, exclude: last.exclude?.trim() || undefined, without_files: last.without_files || undefined })
    onclose()
  }
</script>

<div class="menu w-[min(23rem,90vw)]" data-testid="summarize-popover" use:anchored={{ at, align: 'end', onclose }} role="dialog" aria-label={title}>
  <form class="flex flex-col gap-2.5 p-1.5" onsubmit={(e) => { e.preventDefault(); submit() }}>
    <p class="flex items-center gap-2 text-[13px] font-medium text-ink"><Icon icon="lucide:sparkles" class="text-[14px] text-accent" />{title}</p>
    {@render children?.()}
    <textarea
      data-testid="summarize-focus"
      rows="3"
      class="w-full resize-none rounded-[0.5rem] bg-paper-2 px-2.5 py-2 text-[13.5px] leading-5 text-ink outline-none placeholder:text-ink-3 focus:bg-paper-3"
      placeholder={t('conversation.summarize.focusPlaceholder')}
      aria-label={t('conversation.summarize.focus')}
      bind:value={last.focus}
      onkeydown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); submit() } }}
    ></textarea>
    <button type="button" data-testid="summarize-filter-toggle" class="flex items-center gap-1.5 self-start text-[12.5px] text-ink-3 hover:text-ink" aria-expanded={filtering} onclick={() => (filtering = !filtering)}>
      <Icon icon={filtering ? 'lucide:chevron-down' : 'lucide:chevron-right'} class="text-[12px]" />{t('conversation.summarize.filter')}
    </button>
    {#if filtering}
      <div class="flex flex-col gap-2 pl-1">
        <Field data-testid="summarize-include" size="sm" mono icon="lucide:filter" bind:value={last.include} placeholder={t('conversation.summarize.include')} aria-label={t('conversation.summarize.include')} />
        <Field data-testid="summarize-exclude" size="sm" mono icon="lucide:filter-x" bind:value={last.exclude} placeholder={t('conversation.summarize.exclude')} aria-label={t('conversation.summarize.exclude')} />
        <label class="flex items-center justify-between gap-3 text-[12.5px] text-ink-2">
          {t('conversation.summarize.files')}
          <Switch label={t('conversation.summarize.files')} checked={!last.without_files} onchange={(on) => (last.without_files = !on)} />
        </label>
        <p class="text-[11.5px] leading-4 text-ink-3">{t('conversation.summarize.filterHint')}</p>
      </div>
    {/if}
    {#if invalid}
      <p class="text-[12px] text-error-600">{t('conversation.summarize.invalid', { pattern: invalid })}</p>
    {/if}
    <div class="flex items-center gap-2">
      <button type="button" class="text-[12px] text-ink-3 hover:text-ink hover:underline" onclick={() => { onclose(); goto(settingHref('ai.summaries')) }}>{t('conversation.summarize.style')}</button>
      <span class="flex-1"></span>
      <Button size="sm" variant="primary" type="submit" icon="lucide:sparkles" keys="↵" testid="summarize-run" disabled={!!invalid}>{t('conversation.section.summarize')}</Button>
    </div>
  </form>
</div>
