<script lang="ts">
  import { tooltipStore } from './index'
  import { placePopover, portal } from '$lib/popover'

  let tooltipElement: HTMLDivElement | null = $state(null)
  let left = $state(0)
  let top = $state(0)

  function place(target: HTMLElement) {
    if (!tooltipElement) return
    const size = { width: tooltipElement.offsetWidth, height: tooltipElement.offsetHeight }
    const rect = target.getBoundingClientRect()
    const anchor = { left: rect.left + rect.width / 2 - size.width / 2, top: rect.top, width: size.width, height: rect.height }
    const spot = placePopover(anchor, size, { width: window.innerWidth, height: window.innerHeight }, { side: 'above', align: 'start', gap: 8 })
    left = spot.left
    top = spot.top
  }

  $effect(() => {
    const tooltip = $tooltipStore
    if (!tooltip.show || !tooltip.element) return
    const target = tooltip.element
    const frame = requestAnimationFrame(() => place(target))
    return () => cancelAnimationFrame(frame)
  })
</script>

{#if $tooltipStore.show && $tooltipStore.text}
  <div
    bind:this={tooltipElement}
    use:portal
    class="pointer-events-none fixed z-[50000] max-w-[min(300px,calc(100vw-16px))] select-none break-words rounded-[6px] bg-ink px-2 py-1 text-[12px] font-medium text-paper shadow-[0_6px_16px_-8px_rgb(0_0_0/0.4)]"
    style:left="{left}px"
    style:top="{top}px"
  >
    {$tooltipStore.text}
  </div>
{/if}
