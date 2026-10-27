<script lang="ts">
  import Icon from '@iconify/svelte'
  import { showTooltip } from '$lib/tooltip'
  import { elementSize } from '$lib/window'

  interface Option {
    value: string
    label: string
    icon?: string
    mono?: boolean
  }

  interface Props {
    value: string
    options: Option[]
    size?: 'sm' | 'md'
    label: string
    testid?: string
    stretch?: boolean
    onchange?: (value: string) => void
  }

  let { value = $bindable(), options, size = 'md', label, stretch = false, testid, onchange }: Props = $props()
  const sizes = { sm: 'h-6 px-2 text-[12.5px]', md: 'h-7 px-2.5 text-[13px]' }
  const iconOnly = { sm: 'h-6 w-7', md: 'h-7 w-8' }
  const rows = { sm: '1.75rem', md: '2rem' }

  let labelled = $state<HTMLDivElement | null>(null)
  let full = $state(0)
  let width = $state(0)
  const compact = $derived(full > 0 && width + 1 < full)

  const canCompact = $derived(options.every((option) => !!option.icon))
  const group = $derived(`${stretch ? 'flex w-full' : 'inline-flex max-w-full shrink-0'} items-center gap-0.5 whitespace-nowrap rounded-[0.625rem] bg-paper-3 p-0.5`)

  function measure() {
    if (labelled) full = labelled.scrollWidth
  }

  $effect(() => {
    void options
    void size
    if (!labelled || !canCompact) return
    measure()
    document.fonts?.ready.then(measure).catch(() => {})
  })

  function pick(option: Option) {
    value = option.value
    onchange?.(option.value)
  }
</script>

{#snippet buttons(compact: boolean)}
  {#each options as option (option.value)}
    {@const on = value === option.value}
    <button
      type="button"
      role="radio"
      data-value={option.value}
      aria-checked={on}
      aria-label={option.label}
      class="inline-flex {stretch ? 'min-w-0 flex-1' : 'shrink-0'} items-center justify-center gap-1 whitespace-nowrap rounded-[0.5rem] font-medium {compact ? iconOnly[size] : sizes[size]} {on ? 'bg-paper text-ink shadow-[0_1px_2px_rgb(0_0_0/0.08)]' : 'text-ink-3 hover:text-ink'} {option.mono ? 'font-mono' : ''}"
      onmouseenter={(e) => compact && showTooltip(e.currentTarget, option.label)}
      onclick={() => pick(option)}
    >
      {#if option.icon}
        <Icon icon={option.icon} class="shrink-0 text-[14px]" />
      {/if}
      {#if !compact}
        {option.label}
      {/if}
    </button>
  {/each}
{/snippet}

{#if canCompact}
  <div class="segmented-fit {stretch ? 'flex w-full' : 'inline-flex max-w-full shrink-0'}" use:elementSize={(size) => (width = size.width)} style:--segmented-full="{full}px" style:--segmented-row={rows[size]} data-testid={testid} role="radiogroup" aria-label={label}>
    <div bind:this={labelled} class={group} inert={compact} data-segmented="labels">{@render buttons(false)}</div>
    <div class={group} inert={!compact} data-segmented="icons">{@render buttons(true)}</div>
  </div>
{:else}
  <div class={group} data-testid={testid} role="radiogroup" aria-label={label}>{@render buttons(false)}</div>
{/if}
