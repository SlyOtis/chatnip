<script lang="ts">
  import type { Snippet } from 'svelte'
  import { dismissable, navigable, portal } from '$lib/popover'

  interface Props {
    label: string
    width?: string
    anchored?: boolean
    emphasis?: 'normal' | 'strong'
    within?: string
    movable?: boolean
    testid?: string
    onclose: () => void
    children?: Snippet
    footer?: Snippet
  }

  let { label, width = '32rem', anchored = false, emphasis = 'normal', within, movable = true, testid, onclose, children, footer }: Props = $props()

  let area = $state<{ top: number; left: number; width: number; height: number } | null>(null)
  let panel = $state<HTMLDivElement | null>(null)
  let shift = $state({ x: 0, y: 0 })

  const INTERACTIVE = 'button, a, input, textarea, select, label, iframe, [contenteditable], [role="menuitem"], [role="option"], [role="switch"], [role="tab"], [role="slider"], [role="checkbox"], [role="radio"], .cm-editor'

  function grab(e: PointerEvent) {
    if (!movable || !panel || e.button !== 0) return
    const target = e.target as HTMLElement
    if (target.closest(INTERACTIVE)) return
    const scroller = target.closest<HTMLElement>('.overflow-y-auto, .overflow-auto')
    if (scroller && scroller.scrollHeight > scroller.clientHeight && e.clientX > scroller.getBoundingClientRect().right - 14) return
    e.preventDefault()
    const box = panel.getBoundingClientRect()
    const origin = { x: e.clientX - shift.x, y: e.clientY - shift.y }
    const base = { left: box.left - shift.x, top: box.top - shift.y, right: box.right - shift.x, bottom: box.bottom - shift.y }
    const move = (ev: PointerEvent) => {
      shift = {
        x: Math.min(Math.max(ev.clientX - origin.x, -base.left), window.innerWidth - base.right),
        y: Math.min(Math.max(ev.clientY - origin.y, -base.top), window.innerHeight - base.bottom)
      }
    }
    const release = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', release)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', release)
  }

  $effect(() => {
    if (!within) return
    const measure = () => {
      const box = document.querySelector(within)?.getBoundingClientRect()
      area = box ? { top: box.top, left: box.left, width: box.width, height: box.height } : null
    }
    measure()
    window.addEventListener('resize', measure)
    return () => window.removeEventListener('resize', measure)
  })
</script>

{#if anchored}
  <div class="rise z-40 flex max-h-[70vh] flex-col overflow-hidden rounded-[var(--radius-sheet)] glass" role="dialog" data-testid={testid} aria-label={label} tabindex="-1" style:width={width} use:navigable onkeydown={(e) => e.key === 'Escape' && onclose()}>
    <div class="min-h-0 flex-1 overflow-y-auto">{@render children?.()}</div>
    {#if footer}
      <div class="flex items-center gap-2 border-t border-line px-3 py-2">{@render footer()}</div>
    {/if}
  </div>
{:else}
  <div use:portal class="fixed z-[1000] flex justify-center {area ? 'items-center p-6' : 'inset-0 items-start pt-[14vh]'} {emphasis === 'strong' ? 'bg-black/40 backdrop-blur-[3px]' : ''}" style:top={area ? `${area.top}px` : undefined} style:left={area ? `${area.left}px` : undefined} style:width={area ? `${area.width}px` : undefined} style:height={area ? `${area.height}px` : undefined} role="presentation">
    <div class="rise flex max-h-[72vh] w-full flex-col overflow-hidden rounded-[var(--radius-sheet)] glass {emphasis === 'strong' ? 'shadow-[0_32px_80px_rgb(0_0_0/0.45),0_0_0_1px_rgb(255_255_255/0.06)]' : ''}" role="dialog" data-testid={testid} aria-label={label} tabindex="-1" style:max-width={width} style:translate={movable ? `${shift.x}px ${shift.y}px` : undefined} bind:this={panel} use:navigable use:dismissable={{ onclose }} onpointerdown={grab}>
      <div class="min-h-0 flex-1 overflow-y-auto">{@render children?.()}</div>
      {#if footer}
        <div class="flex items-center gap-2 border-t border-line px-4 py-3">{@render footer()}</div>
      {/if}
    </div>
  </div>
{/if}
