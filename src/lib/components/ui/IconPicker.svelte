<script module lang="ts">
  export interface Choice {
    value: string
    label: string
    icon?: string
    color?: string | null
    group?: string
    mono?: boolean
  }
</script>

<script lang="ts">
  import Icon from '@iconify/svelte'
  import { onMount } from 'svelte'
  import Sheet from './Sheet.svelte'
  import Field from './Field.svelte'
  import Button from './Button.svelte'
  import { loadIconBank, searchIcons } from '$lib/icons'
  import { t } from '$lib/i18n.svelte'
  import { anchored, type Box } from '$lib/popover'

  interface Props {
    label?: string
    value?: string | null
    fallback?: string
    resetLabel?: string
    options?: Choice[]
    at?: Box | null
    onpick: (value: string | null) => void
    onclose: () => void
  }

  let { label, value = null, fallback, resetLabel, options, at = null, onpick, onclose }: Props = $props()

  const LIMIT = 200

  let names = $state<string[]>([])
  let query = $state('')
  let failed = $state(false)
  let active = $state(0)
  let list = $state<HTMLDivElement | null>(null)

  const words = $derived(query.trim().toLowerCase().split(/\s+/).filter(Boolean))
  const matches = $derived(options ? options.filter((choice) => words.every((word) => `${choice.label} ${choice.group ?? ''}`.toLowerCase().includes(word))) : [])
  const choices = $derived(matches.slice(0, LIMIT))
  const icons = $derived(options ? [] : searchIcons(names, query))
  const values = $derived(options ? choices.map((choice) => choice.value) : icons)
  const groups = $derived(choices.reduce<{ name: string; items: { choice: Choice; index: number }[] }[]>((all, choice, index) => {
    const name = choice.group ?? ''
    const last = all[all.length - 1]
    if (last && last.name === name) last.items.push({ choice, index })
    else all.push({ name, items: [{ choice, index }] })
    return all
  }, []))
  const current = $derived(options?.find((choice) => choice.value === value) ?? null)

  onMount(() => {
    if (options) return
    loadIconBank()
      .then((bank) => (names = bank))
      .catch(() => (failed = true))
  })

  $effect(() => {
    const index = values.indexOf(value ?? '')
    active = query.trim() || index < 0 ? 0 : index
  })

  $effect(() => {
    list?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' })
  })

  function pick(next: string | null) {
    onpick(next)
    onclose()
  }

  function columns(): number {
    if (options || !list) return 1
    const grid = list.querySelector<HTMLElement>('[data-grid]')
    return grid ? getComputedStyle(grid).gridTemplateColumns.split(' ').length : 1
  }

  function keydown(e: KeyboardEvent) {
    const steps: Record<string, number> = { ArrowDown: columns(), ArrowUp: -columns(), ArrowRight: options ? 0 : 1, ArrowLeft: options ? 0 : -1 }
    if (e.key in steps && steps[e.key] !== 0) {
      e.preventDefault()
      active = Math.max(0, Math.min(values.length - 1, active + steps[e.key]))
      return
    }
    if (e.key === 'Enter' && values[active] !== undefined) {
      e.preventDefault()
      pick(values[active])
    }
  }
</script>

{#snippet body()}
  <div class="flex flex-col gap-3 p-4" role="presentation" onkeydown={keydown}>
    <div class="flex items-center gap-3">
      <span class="flex size-9 shrink-0 items-center justify-center rounded-[0.5rem] bg-paper-3 text-[18px] text-ink">
        {#if current?.color && !current.icon}
          <span class="size-2.5 rounded-full" style:background={current.color}></span>
        {:else}
          <Icon icon={(options ? current?.icon : value) ?? fallback ?? 'lucide:circle-dashed'} />
        {/if}
      </span>
      {#if options}
        <Field data-testid="select-search" bind:value={query} placeholder={t('app.select.search', { count: options.length })} aria-label={label ?? t('common.search')} autocomplete="off" />
      {:else}
        <Field data-testid="icon-picker-search" bind:value={query} placeholder={names.length ? t('app.iconPicker.searchCount', { count: names.length }) : t('app.iconPicker.search')} aria-label={t('app.iconPicker.search')} autocomplete="off" />
      {/if}
      {#if !options && value && fallback}
        <Button size="sm" icon="lucide:rotate-ccw" class="shrink-0" onclick={() => pick(null)}>{resetLabel ?? t('app.iconPicker.useDefault')}</Button>
      {/if}
    </div>
    <div bind:this={list} class="contents">
      {#if options}
        {#if !choices.length}
          <p class="text-[13px] text-ink-3">{t('app.select.noMatch', { query })}</p>
        {:else}
          <div class="flex flex-col gap-2" role="listbox" aria-label={label}>
            {#each groups as group, g (group.name + g)}
              <div class="flex flex-col gap-px" role="group" aria-label={group.name || undefined}>
                {#if group.name}
                  <p class="px-2 pt-1 text-[12.5px] font-medium text-ink-3">{group.name}</p>
                {/if}
                {#each group.items as { choice, index } (choice.value)}
                  {@const chosen = choice.value === value}
                  <button
                    type="button"
                    role="option"
                    data-testid="select-option"
                    data-value={choice.value}
                    data-index={index}
                    aria-selected={chosen}
                    class="flex h-8 w-full shrink-0 items-center gap-2.5 rounded-[0.5rem] px-2 text-left text-[13.5px] {index === active ? 'bg-paper-3 text-ink' : 'text-ink-2 hover:bg-paper-3 hover:text-ink'}"
                    onmouseenter={() => (active = index)}
                    onclick={() => pick(choice.value)}
                  >
                    {#if choice.color}
                      <span class="size-2 shrink-0 rounded-full" style:background={choice.color}></span>
                    {:else if choice.icon}
                      <Icon icon={choice.icon} class="shrink-0 text-[14px] {chosen ? 'text-accent' : 'text-ink-3'}" />
                    {/if}
                    <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap {choice.mono ? 'font-mono text-[12.5px]' : ''}">{choice.label}</span>
                    {#if chosen}<Icon icon="lucide:check" class="shrink-0 text-[14px] text-accent" />{/if}
                  </button>
                {/each}
              </div>
            {/each}
          </div>
        {/if}
      {:else if failed}
        <p class="text-[13px] text-ink-3">{t('app.iconPicker.failed')}</p>
      {:else if !names.length}
        <p class="text-[13px] text-ink-3">{t('app.iconPicker.loading')}</p>
      {:else if !icons.length}
        <p class="text-[13px] text-ink-3">{t('app.iconPicker.noMatch', { query })}</p>
      {:else}
        <div data-grid class="grid grid-cols-[repeat(auto-fill,minmax(2.5rem,1fr))] gap-1">
          {#each icons as icon, index (icon)}
            <button
              type="button"
              data-testid="icon-option"
              data-value={icon}
              data-index={index}
              class="flex aspect-square items-center justify-center rounded-[0.5rem] text-[16px] {icon === value ? 'bg-accent-soft text-accent' : index === active ? 'bg-paper-3 text-ink' : 'text-ink-2 hover:bg-paper-3 hover:text-ink'}"
              title={icon.slice(7)}
              aria-label={icon.slice(7)}
              aria-pressed={icon === value}
              onclick={() => pick(icon)}
            >
              <Icon {icon} />
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/snippet}

{#snippet footer()}
    <span class="text-[12px] text-ink-3">{options ? t('app.select.shown', { shown: choices.length, total: options.length }) : t('app.iconPicker.shown', { shown: icons.length, total: names.length })}</span>
    <span class="ml-auto"></span>
    <Button size="sm" keys="esc" onclick={onclose}>{t('common.close')}</Button>
{/snippet}

{#if at}
  <div use:anchored={{ at, side: 'below', align: 'start', onclose }}>
    <Sheet label={label ?? t('app.iconPicker.title')} width="min(30rem, 90vw)" anchored testid={options ? 'select-dialog' : 'icon-picker'} {onclose} {footer}>{@render body()}</Sheet>
  </div>
{:else}
  <Sheet label={label ?? t('app.iconPicker.title')} width="34rem" testid={options ? 'select-dialog' : 'icon-picker'} {onclose} {footer}>{@render body()}</Sheet>
{/if}
