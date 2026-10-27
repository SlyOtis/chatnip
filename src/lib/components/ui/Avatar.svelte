<script lang="ts">
  import Icon from '@iconify/svelte'
  import { avatarColor, avatarInitials, type AvatarFace, type AvatarSource } from '$lib/avatar'
  import { showTooltip } from '$lib/tooltip'
  import { globalAppearance } from '$lib/appearance'

  interface Props {
    source: AvatarSource
    size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl' | 'xxl'
    picked?: boolean
    label?: string
    hint?: string
    ring?: boolean
    testid?: string
    class?: string
    onclick?: (event: MouseEvent) => void
  }

  let { source, size = 'md', picked = false, label, hint, ring: ringed = false, class: className = '', testid, onclick }: Props = $props()

  const sizes = {
    xs: 'size-5 text-[0.5625rem]',
    sm: 'size-6 text-[0.625rem]',
    md: 'size-7 text-[0.6875rem]',
    lg: 'size-9 text-[0.75rem]',
    xl: 'size-12 text-[0.9375rem]',
    xxl: 'size-20 text-[1.5rem]'
  }

  let broken = $state<Set<string>>(new Set())

  const shape = $derived($globalAppearance.avatarForm === 'round' ? 'rounded-full' : 'rounded-[28%]')
  const ring = $derived($globalAppearance.avatarBorder ? 'before:pointer-events-none before:absolute before:inset-0 before:z-10 before:rounded-[inherit] before:shadow-[inset_0_0_0_0.125em_var(--avatar-colour)]' : '')
  const faces = $derived(source.src || source.faces.length < 2 ? [] : source.faces)
  const layout = $derived(faces.length >= 4 ? 'grid-cols-2 grid-rows-2' : faces.length === 3 ? 'grid-cols-2 grid-rows-2 [&>*:first-child]:row-span-2' : 'grid-cols-2 grid-rows-1')

  function shown(src: string | null): string | null {
    return src && !broken.has(src) ? src : null
  }

  function fail(src: string) {
    broken = new Set([...broken, src])
  }
</script>

{#snippet face(item: AvatarFace, icon: string | null)}
  {@const src = shown(item.src)}
  {#if src}
    <img {src} alt="" class="size-full object-cover" draggable="false" onerror={() => fail(src)} />
  {:else}
    <span class="avatar-tone flex size-full items-center justify-center font-semibold tracking-wide" style:--avatar-colour={avatarColor(item)}>
      {#if icon}<Icon {icon} class="text-[1.15em]" />{:else}{avatarInitials(item.name)}{/if}
    </span>
  {/if}
{/snippet}

<svelte:element
  this={onclick ? 'button' : 'span'}
  type={onclick ? 'button' : undefined}
  data-testid={testid}
  class="relative flex shrink-0 items-center justify-center overflow-hidden select-none {shape} {sizes[size]} {ring} {ringed ? 'ring-1 ring-paper' : ''} {onclick ? 'cursor-pointer after:pointer-events-none after:absolute after:inset-0 after:rounded-[inherit] after:transition-colors hover:after:bg-black/20 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent' : ''} {className}"
  aria-label={onclick ? (label ?? source.name) : undefined}
  aria-hidden={onclick ? undefined : 'true'}
  role={onclick ? undefined : 'presentation'}
  onclick={onclick ? (event: MouseEvent) => { event.stopPropagation(); onclick(event) } : undefined}
  onmouseenter={hint ? (event: MouseEvent) => showTooltip(event.currentTarget as HTMLElement, hint) : undefined}
  style:--avatar-colour={avatarColor(source)}
>
  {#if faces.length}
    <span class="grid size-full gap-px bg-paper {layout}">
      {#each faces as item (item.id)}
        <span class="min-h-0 min-w-0 overflow-hidden">{@render face(item, null)}</span>
      {/each}
    </span>
  {:else}
    {@render face(source, source.src ? null : source.icon)}
  {/if}
  {#if picked}
    <span class="absolute inset-0 flex items-center justify-center bg-accent text-accent-ink"><Icon icon="lucide:check" class="text-[1.4em]" /></span>
  {/if}
</svelte:element>
