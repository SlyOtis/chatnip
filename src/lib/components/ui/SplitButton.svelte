<script lang="ts">
  import Icon from '@iconify/svelte'
  import type { Snippet } from 'svelte'
  import Kbd from './Kbd.svelte'
  import { showTooltip } from '$lib/tooltip'
  import { anchored, boxOf } from '$lib/popover'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    label: string
    keys?: string
    icon?: string
    menuLabel?: string
    menuKeys?: string
    menuIcon?: string
    menuActive?: boolean
    variant?: 'primary' | 'secondary' | 'ghost'
    size?: 'xs' | 'sm' | 'md'
    disabled?: boolean
    busy?: boolean
    class?: string
    onclick?: () => void
    testid?: string
    onmenu?: () => void
    menu?: Snippet<[() => void]>
  }

  let { label, keys, icon, menuLabel, menuKeys, menuIcon = 'lucide:chevron-down', menuActive = false, variant = 'primary', size = 'sm', disabled = false, busy = false, class: className = '', testid, onclick, onmenu, menu }: Props = $props()

  let open = $state(false)
  let root = $state<HTMLDivElement | null>(null)

  const sizes = { xs: 'h-6 text-[12px]', sm: 'h-7 text-[13px]', md: 'h-8 text-[13.5px]' }
  const squares = { xs: 'w-6 text-[12px]', sm: 'w-7 text-[14px]', md: 'w-8 text-[15px]' }
  const tones = {
    primary: 'bg-accent text-accent-ink hover:brightness-110',
    secondary: 'bg-accent-soft text-ink hover:bg-paper-3',
    ghost: 'text-ink-3 hover:text-ink hover:bg-paper-3'
  }
  const segmented = $derived(!!menu || !!onmenu)
  const segmentLabel = $derived(menuLabel ?? t('app.ui.moreOptions'))

  function close() {
    open = false
  }

  function segment(e: MouseEvent) {
    e.stopPropagation()
    if (onmenu) onmenu()
    else open = !open
  }
</script>

<div bind:this={root} class="relative inline-flex {className}" data-testid={testid}>
  <div class="inline-flex overflow-hidden rounded-[0.5rem] {sizes[size]} {disabled ? 'opacity-40' : ''}">
    {#if icon}
      <button type="button" data-testid={testid ? `${testid}-main` : undefined} class="inline-flex items-center justify-center {squares[size]} {tones[variant]} disabled:cursor-default" aria-label={label} disabled={disabled || busy} onmouseenter={(e) => showTooltip(e.currentTarget, keys ? `${label} · ${keys}` : label)} onclick={(e) => { e.stopPropagation(); onclick?.() }}>
        <Icon icon={busy ? 'lucide:loader-circle' : icon} class={busy ? 'animate-spin' : ''} />
      </button>
    {:else}
      <button type="button" data-testid={testid ? `${testid}-main` : undefined} class="inline-flex items-center gap-2 px-2.5 font-medium {tones[variant]} disabled:cursor-default" {disabled} onclick={onclick}>
        {#if busy}
          <Icon icon="lucide:loader-circle" class="animate-spin" />
        {/if}
        <span>{label}</span>
        {#if keys}
          <Kbd {keys} class={variant === 'primary' ? 'bg-white/20 text-accent-ink shadow-none' : ''} />
        {/if}
      </button>
    {/if}
    {#if segmented}
      <button
        type="button"
        data-testid={testid ? `${testid}-menu` : undefined}
        class="inline-flex items-center px-1 {variant === 'ghost' ? 'border-l border-line' : 'border-l border-[rgb(0_0_0/0.12)]'} {menuActive && variant === 'ghost' ? 'bg-accent-soft text-accent' : tones[variant]}"
        aria-label={segmentLabel}
        aria-haspopup={onmenu ? 'dialog' : 'menu'}
        aria-expanded={onmenu ? menuActive : open}
        {disabled}
        onmouseenter={(e) => showTooltip(e.currentTarget, menuKeys ? `${segmentLabel} · ${menuKeys}` : segmentLabel)}
        onclick={segment}
      >
        <Icon icon={menuIcon} class="text-[13px]" />
      </button>
    {/if}
  </div>
  {#if open && menu}
    <div class="menu min-w-[14rem]" use:anchored={{ at: () => boxOf(root), side: 'above', align: 'end', onclose: close }} role="menu" data-testid={testid ? `${testid}-list` : undefined}>
      {@render menu(close)}
    </div>
  {/if}
</div>
