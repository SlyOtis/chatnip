<script lang="ts">
  import type { Snippet } from 'svelte'
  import Icon from '@iconify/svelte'
  import Kbd from './Kbd.svelte'

  interface Props {
    icon?: string
    keys?: string
    selected?: boolean
    checked?: boolean
    danger?: boolean
    warn?: boolean
    detail?: string
    testid?: string
    value?: string
    onclick?: () => void
    children?: Snippet
  }

  let { icon, keys, selected = false, checked = false, danger = false, warn = false, detail, testid, value, onclick, children }: Props = $props()
</script>

<button
  type="button"
  role="menuitem"
  data-testid={testid}
  data-checked={checked}
  data-value={value}
  class="flex h-8 w-full items-center gap-2.5 rounded-[0.5rem] px-2.5 text-left text-[14px] {selected ? 'bg-accent-soft text-ink' : warn ? 'text-warn-600 hover:bg-paper-3' : danger ? 'text-ink-2 hover:bg-error-50 hover:text-error-600' : 'text-ink hover:bg-paper-3'}"
  aria-selected={selected || undefined}
  {onclick}
>
  {#if icon}
    <Icon icon={icon} class="shrink-0 text-[15px] {warn ? 'text-warn-600' : 'text-ink-3'}" />
  {/if}
  <span class="min-w-0 flex-1 truncate">{@render children?.()}</span>
  {#if detail}
    <span class="shrink-0 font-mono text-[11px] text-ink-3">{detail}</span>
  {/if}
  {#if checked}
    <Icon icon="lucide:check" class="shrink-0 text-[15px] text-accent" />
  {/if}
  {#if keys}
    <Kbd {keys} />
  {/if}
</button>
