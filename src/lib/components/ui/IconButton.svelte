<script lang="ts">
  import Icon from '@iconify/svelte'
  import { showTooltip } from '$lib/tooltip'

  interface Props {
    icon: string
    label: string
    keys?: string
    size?: 'sm' | 'md'
    active?: boolean
    tone?: 'default' | 'signal' | 'danger' | 'primary' | 'warn'
    disabled?: boolean
    busy?: boolean
    testid?: string
    class?: string
    onclick?: (e: MouseEvent) => void
  }

  let { icon, label, keys, size = 'md', active = false, tone = 'default', disabled = false, busy = false, class: className = '', testid, onclick }: Props = $props()

  const sizes = { sm: 'size-7 text-[14px]', md: 'size-8 text-[15px]' }
  const tones = {
    default: active ? 'text-accent bg-accent-soft' : 'text-ink-3 hover:text-ink hover:bg-paper-3',
    signal: active ? 'text-signal' : 'text-ink-3 hover:text-signal hover:bg-paper-3',
    danger: 'text-ink-3 hover:text-error-600 hover:bg-error-50',
    primary: 'bg-accent text-accent-ink hover:brightness-110 active:brightness-95',
    warn: 'bg-warn-500 text-paper hover:bg-warn-600'
  }
  const hint = $derived(keys ? `${label} · ${keys}` : label)
</script>

<button
  type="button"
  data-testid={testid}
  class="inline-flex shrink-0 items-center justify-center rounded-[0.5rem] disabled:cursor-default disabled:opacity-40 {sizes[size]} {tones[tone]} {className}"
  aria-label={label}
  aria-pressed={active || undefined}
  disabled={disabled || busy}
  onmouseenter={(e) => showTooltip(e.currentTarget, hint)}
  {onclick}
>
  <Icon icon={busy ? 'lucide:loader-circle' : icon} class="{busy ? 'animate-spin' : ''} {active && tone === 'signal' ? 'fill-current' : ''}" />
</button>
