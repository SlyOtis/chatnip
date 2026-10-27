<script lang="ts">
  import Icon from '@iconify/svelte'
  import { anchored, boxOf } from '$lib/popover'
  import { showTooltip } from '$lib/tooltip'
  import { TAG_COLOURS, recentColours, rememberColour, tagTone } from '$lib/tauri/tags'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    value: string | null
    onpick: (color: string | null) => void
    clearable?: boolean
    size?: 'sm' | 'md'
    label?: string
  }

  let { value, onpick, clearable = false, size = 'md', label }: Props = $props()

  let root = $state<HTMLButtonElement | null>(null)
  let open = $state(false)

  const name = $derived(label ?? t('app.colour.title'))
  const custom = $derived(!!value && !TAG_COLOURS.includes(value.toLowerCase()))
  const recent = $derived($recentColours.filter((color) => !TAG_COLOURS.includes(color)).slice(0, 5))
  const tiles = { sm: 'size-7', md: 'size-8' }

  function pick(color: string | null) {
    rememberColour(color)
    onpick(color)
    open = false
  }
</script>

<button
  bind:this={root}
  type="button"
  data-testid="colour-swatch"
  class="flex shrink-0 items-center justify-center rounded-[0.5rem] hover:bg-paper-3 {tiles[size]} {open ? 'bg-paper-3' : ''}"
  aria-label={name}
  aria-haspopup="dialog"
  aria-expanded={open}
  onmouseenter={(e) => showTooltip(e.currentTarget, name)}
  onclick={() => (open = !open)}
>
  <span class="tag-mark size-3.5 rounded-full border border-line-strong bg-current" style:--tag-day={value ? tagTone(value, 'day') : 'transparent'} style:--tag-night={value ? tagTone(value, 'night') : 'transparent'}></span>
</button>

{#if open}
  <div class="menu w-[min(13.5rem,80vw)]" use:anchored={{ at: () => boxOf(root), align: 'start', onclose: () => (open = false) }} role="dialog" aria-label={name}>
    <div class="grid grid-cols-5 gap-1" role="radiogroup" aria-label={name}>
      {#each TAG_COLOURS as color (color)}
        {@const on = value?.toLowerCase() === color}
        <button
          type="button"
          role="radio"
          data-testid="colour-option"
          data-value={color}
          aria-checked={on}
          aria-label={color}
          class="flex aspect-square items-center justify-center rounded-[0.4rem] hover:bg-paper-3 {on ? 'bg-paper-3' : ''}"
          onclick={() => pick(color)}
        >
          <span class="tag-mark flex size-4 items-center justify-center rounded-full bg-current" style:--tag-day={tagTone(color, 'day')} style:--tag-night={tagTone(color, 'night')}>
            {#if on}<Icon icon="lucide:check" class="text-[10px] text-paper" />{/if}
          </span>
        </button>
      {/each}
    </div>
    <div class="mt-2 grid grid-cols-5 gap-1 border-t border-line pt-2" role="group" aria-label={t('app.colour.recent')}>
      {#each recent as color (color)}
        {@const on = value?.toLowerCase() === color}
        <button type="button" data-testid="colour-recent" data-value={color} aria-pressed={on} aria-label={color} class="flex aspect-square items-center justify-center rounded-[0.4rem] hover:bg-paper-3 {on ? 'bg-paper-3' : ''}" onmouseenter={(e) => showTooltip(e.currentTarget, color)} onclick={() => pick(color)}>
          <span class="flex size-4 items-center justify-center rounded-full" style:background={color}>
            {#if on}<Icon icon="lucide:check" class="text-[10px] text-paper" />{/if}
          </span>
        </button>
      {/each}
      <label class="relative flex aspect-square cursor-pointer items-center justify-center rounded-[0.4rem] text-ink-3 hover:bg-paper-3 hover:text-ink {custom && !recent.includes(value?.toLowerCase() ?? '') ? 'bg-paper-3 text-ink' : ''}" onmouseenter={(e) => showTooltip(e.currentTarget, custom && value ? `${t('app.colour.custom')} ${value}` : t('app.colour.custom'))}>
        <Icon icon="lucide:pipette" class="text-[13px]" />
        <input type="color" class="absolute inset-0 cursor-pointer opacity-0" aria-label={t('app.colour.custom')} value={value ?? TAG_COLOURS[0]} onchange={(e) => pick(e.currentTarget.value)} />
      </label>
      {#if clearable && value}
        <button type="button" data-testid="colour-clear" aria-label={t('app.colour.none')} class="flex aspect-square items-center justify-center rounded-[0.4rem] text-ink-3 hover:bg-paper-3 hover:text-ink" onmouseenter={(e) => showTooltip(e.currentTarget, t('app.colour.none'))} onclick={() => pick(null)}>
          <Icon icon="lucide:circle-off" class="text-[13px]" />
        </button>
      {/if}
    </div>
  </div>
{/if}
