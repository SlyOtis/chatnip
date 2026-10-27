<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { HTMLInputAttributes } from 'svelte/elements'
  import Icon from '@iconify/svelte'
  import { showTooltip } from '$lib/tooltip'
  import { t } from '$lib/i18n.svelte'

  interface Props extends Omit<HTMLInputAttributes, 'size' | 'value' | 'class'> {
    value?: string
    mono?: boolean
    size?: 'sm' | 'md'
    class?: string
    icon?: string
    onicon?: () => void
    glyph?: Snippet
    inline?: boolean
  }

  let { value = $bindable(''), mono = false, size = 'md', class: className = '', icon, onicon, glyph, inline = false, ...rest }: Props = $props()

  const sizes = { sm: 'h-7 px-2 text-[13px]', md: 'h-8 px-2.5 text-[14px]' }
  const tiles = { sm: 'size-7 text-[14px]', md: 'size-8 text-[15px]' }
</script>

{#snippet input()}
  <input
    bind:value
    class="w-full min-w-0 rounded-none border-0 bg-transparent text-ink outline-none placeholder:text-ink-3 {inline ? 'rounded-[0.375rem] hover:bg-paper-2 focus:bg-paper-2' : 'border-b border-line focus:border-ink-3'} {sizes[size]} {mono ? 'font-mono tabular-nums' : ''} {className}"
    {...rest}
  />
{/snippet}

{#if icon || glyph}
  <div class="flex w-full min-w-0 items-center gap-2">
    <button
      type="button"
      class="flex shrink-0 items-center justify-center rounded-[0.5rem] text-ink-2 hover:text-ink disabled:cursor-default {inline ? 'hover:bg-paper-3' : 'bg-paper-3'} {tiles[size]}"
      data-testid="field-icon"
      aria-label={t('app.iconPicker.change')}
      disabled={!onicon || !!rest.disabled || !!rest.readonly}
      onmouseenter={(e) => showTooltip(e.currentTarget, t('app.iconPicker.change'))}
      onclick={onicon}
    >
      {#if glyph}{@render glyph()}{:else if icon}<Icon {icon} />{/if}
    </button>
    {@render input()}
  </div>
{:else}
  {@render input()}
{/if}
