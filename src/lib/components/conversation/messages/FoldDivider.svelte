<script lang="ts">
  import Icon from '@iconify/svelte'
  import { focusFold, markerLabel, registerFold, setFold, type FoldMarker } from '$lib/collapse'

  interface Props {
    marker: FoldMarker
  }

  let { marker }: Props = $props()

  $effect(() => registerFold(marker.key, false))
</script>

<button
  class="group/fold my-1.5 flex h-7 w-full items-center gap-3 text-[12px] text-ink-3 hover:text-ink"
  data-testid="fold-divider"
  data-kind={marker.kind}
  onclick={() => setFold(marker.key, 'open', true)}
  onmouseenter={() => focusFold(marker.key)}
  aria-expanded="false"
>
  <span class="h-0 flex-1 border-t border-dashed border-line-strong"></span>
  <span class="flex items-center gap-1.5 rounded-full bg-paper-3 px-2.5 py-0.5 font-mono text-[11px] group-hover/fold:bg-paper-2">
    <Icon icon={marker.kind === 'run' ? 'lucide:chevrons-up-down' : 'lucide:mails'} class="text-[13px]" />
    {markerLabel(marker)}
  </span>
  <span class="h-0 flex-1 border-t border-dashed border-line-strong"></span>
</button>
