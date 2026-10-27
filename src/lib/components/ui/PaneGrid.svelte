<script lang="ts" module>
  export interface PaneSpec {
    key: string
    size?: string
    min?: string
    max?: string
    open?: boolean
    edge?: 'start' | 'end'
    class?: string
  }
</script>

<script lang="ts">
  import type { Snippet } from 'svelte'
  import { DraggableEdge } from 'sly-svelte-pane'
  import { holdPane, paneSize } from '$lib/inbox'

  interface Props {
    name: string
    panes: PaneSpec[]
    pane: Snippet<[string]>
    class?: string
  }

  let { name, panes, pane, class: className = '' }: Props = $props()

  const gridId = $derived(`pane-grid-${name}`)
  let sizes = $state<Record<string, string>>({})

  const storeKey = (key: string): `chatnip.${string}` => `chatnip.pane.${name}.${key}`
  const sizeOf = (spec: PaneSpec) => sizes[spec.key] ?? paneSize(storeKey(spec.key), spec.size ?? '20vw')
  const track = (spec: PaneSpec) => (!spec.size ? 'minmax(0, 1fr)' : spec.open === false ? '0px' : `clamp(var(--${spec.key}-size-min), var(--${spec.key}-size), var(--${spec.key}-size-max))`)

  const style = $derived(
    panes
      .filter((spec) => spec.size)
      .map((spec) => `--${spec.key}-size: ${sizeOf(spec)}; --${spec.key}-size-min: ${spec.min ?? '12rem'}; --${spec.key}-size-max: ${spec.max ?? '50vw'}`)
      .concat(`grid-template-columns: ${panes.map(track).join(' ')}`)
      .join('; ')
  )

  function saved(key: string) {
    return (value: string) => (sizes = { ...sizes, [key]: value })
  }
</script>

<div id={gridId} data-pane-grid class="grid-slide relative grid h-full min-h-0 min-w-0 grid-rows-1 overflow-hidden {className}" {style}>
  {#each panes as spec (spec.key)}
    <div class="relative flex min-h-0 min-w-0 flex-col overflow-hidden {spec.size && spec.open !== false ? (spec.edge === 'start' ? 'border-l border-line' : 'border-r border-line') : ''} {spec.class ?? ''}">
      {#if spec.size}
        {@const fixed = `clamp(var(--${spec.key}-size-min), var(--${spec.key}-size), var(--${spec.key}-size-max))`}
        <div class="flex h-full min-h-0 flex-col" style:width={fixed} style:min-width={fixed} inert={spec.open === false}>
          {@render pane(spec.key)}
        </div>
      {:else}
        {@render pane(spec.key)}
      {/if}
      {#if spec.open !== false}
        {#if spec.size}
          <div class="pane-edge {spec.edge === 'start' ? 'left' : 'right'}" role="presentation" onpointerdowncapture={(e) => holdPane(e, storeKey(spec.key), `--${spec.key}-size`, saved(spec.key))}>
            <DraggableEdge options={{ property: `--${spec.key}-size`, anchor: spec.edge === 'start' ? 'left' : 'right', gridSelector: `#${gridId}` }} />
          </div>
        {/if}
      {/if}
    </div>
  {/each}
</div>
