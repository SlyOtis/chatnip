<script lang="ts">
  import Icon from '@iconify/svelte'
  import IconPicker, { type Choice } from './IconPicker.svelte'

  interface Props {
    value: string
    options: Choice[]
    size?: 'sm' | 'md'
    class?: string
    testid?: string
    dialog?: string
    placeholder?: string
    muted?: boolean
    open?: boolean
    onchange?: (value: string) => void
  }

  let { value = $bindable(), options, size = 'md', class: className = '', testid, dialog, placeholder = '', muted = false, open = $bindable(false), onchange }: Props = $props()
  const sizes = { sm: 'h-7 pl-2 pr-7 text-[13px]', md: 'h-8 pl-2.5 pr-8 text-[14px]' }

  const current = $derived(options.find((option) => option.value === value) ?? null)

  function pick(next: string | null) {
    if (next === null || next === value) return
    value = next
    onchange?.(next)
  }
</script>

{#if dialog}
  <span class="relative inline-flex min-w-0 {className}">
    <button
      type="button"
      data-testid={testid}
      data-value={value}
      aria-haspopup="dialog"
      aria-expanded={open}
      class="flex w-full min-w-0 items-center gap-2 border-b border-line bg-transparent text-left text-ink outline-none hover:border-ink-3 focus-visible:border-ink-3 {sizes[size]}"
      onclick={() => (open = true)}
    >
      {#if current?.color}
        <span class="size-2 shrink-0 rounded-full" style:background={current.color}></span>
      {:else if current?.icon}
        <Icon icon={current.icon} class="shrink-0 text-[14px] text-ink-3" />
      {/if}
      <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap {current && !muted ? '' : 'text-ink-3'} {current?.mono ? 'font-mono' : ''}">{current?.label ?? placeholder}</span>
    </button>
    <Icon icon="lucide:chevrons-up-down" class="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-[14px] text-ink-3" />
  </span>
  {#if open}
    <IconPicker label={dialog} {value} {options} onpick={pick} onclose={() => (open = false)} />
  {/if}
{:else}
  <span class="relative inline-flex min-w-0 {className}">
    <select
      data-testid={testid}
      bind:value
      class="w-full min-w-0 appearance-none rounded-none border-0 border-b border-line bg-transparent text-ink outline-none hover:border-ink-3 focus:border-ink-3 {sizes[size]}"
      onchange={(e) => onchange?.(e.currentTarget.value)}
    >
      {#each options as option (option.value)}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
    <Icon icon="lucide:chevron-down" class="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-[14px] text-ink-3" />
  </span>
{/if}
