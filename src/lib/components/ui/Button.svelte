<script lang="ts">
  import type { Snippet } from 'svelte'
  import Icon from '@iconify/svelte'
  import Kbd from './Kbd.svelte'
  import { showTooltip } from '$lib/tooltip'

  interface Props {
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger' | 'warn'
    size?: 'sm' | 'md'
    icon?: string
    keys?: string
    type?: 'button' | 'submit'
    disabled?: boolean
    busy?: boolean
    title?: string
    class?: string
    testid?: string
    onclick?: (e: MouseEvent) => void
    children?: Snippet
  }

  let { variant = 'ghost', size = 'md', icon, keys, type = 'button', disabled = false, busy = false, title, class: className = '', testid, onclick, children }: Props = $props()

  const base = 'inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-[0.5rem] font-medium leading-none select-none disabled:cursor-default disabled:opacity-40'
  const sizes = { sm: 'h-7 px-2.5 text-[13px]', md: 'h-8 px-3 text-[13.5px]' }
  const variants = {
    primary: 'bg-accent text-accent-ink hover:brightness-110 active:brightness-95',
    secondary: 'bg-accent-soft text-accent hover:bg-accent hover:text-accent-ink',
    ghost: 'text-ink-2 hover:bg-paper-3 hover:text-ink',
    danger: 'text-ink-2 hover:bg-error-50 hover:text-error-600',
    warn: 'bg-warn-500 text-paper hover:bg-warn-600'
  }
</script>

<button {type} data-testid={testid} aria-label={children ? undefined : title} onmouseenter={(e) => title && showTooltip(e.currentTarget, keys ? `${title} · ${keys}` : title)} class="{base} {sizes[size]} {variants[variant]} {className}" disabled={disabled || busy} aria-busy={busy} {onclick}>
  {#if icon}
    <Icon icon={busy ? 'lucide:loader-circle' : icon} class="text-[15px] {busy ? 'animate-spin' : ''}" />
  {/if}
  {@render children?.()}
  {#if keys}
    <Kbd {keys} class={variant === 'primary' ? 'ml-0.5 bg-white/20 text-accent-ink shadow-none' : 'ml-0.5'} />
  {/if}
</button>
