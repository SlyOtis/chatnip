<script lang="ts">
  import { Panel, useSvelteFlow } from '@xyflow/svelte'
  import { IconButton } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    direction: 'TB' | 'LR'
    locked: boolean
    onlayout: (direction: 'TB' | 'LR') => void
    onlock: (locked: boolean) => void
    onsearch: (query: string) => string[]
  }

  let { direction, locked, onlayout, onlock, onsearch }: Props = $props()
  const flow = useSvelteFlow()
  let query = $state('')

  function search(value: string) {
    query = value
    const hits = onsearch(value)
    if (hits.length) flow.fitView({ nodes: hits.map((id) => ({ id })), duration: 300, padding: 0.6 })
  }
</script>

<Panel position="bottom-left">
  <div class="flex items-center gap-1 rounded-[0.625rem] bg-paper px-1.5 py-1 shadow-[0_4px_16px_rgb(0_0_0/0.14)] hairline">
    <IconButton size="sm" icon="lucide:zoom-in" label={t('actions.graph.zoomIn')} onclick={() => flow.zoomIn({ duration: 150 })} />
    <IconButton size="sm" icon="lucide:zoom-out" label={t('actions.graph.zoomOut')} onclick={() => flow.zoomOut({ duration: 150 })} />
    <IconButton size="sm" testid="graph-fit" icon="lucide:maximize" label={t('actions.graph.fit')} onclick={() => flow.fitView({ padding: 0.3, duration: 300 })} />
    <IconButton size="sm" icon={direction === 'LR' ? 'lucide:arrow-down-wide-narrow' : 'lucide:arrow-right-left'} label={direction === 'LR' ? t('actions.graph.topToBottom') : t('actions.graph.leftToRight')} onclick={() => onlayout(direction === 'LR' ? 'TB' : 'LR')} />
    <IconButton size="sm" icon="lucide:network" label={t('actions.graph.tidy')} onclick={() => onlayout(direction)} />
    <IconButton size="sm" icon={locked ? 'lucide:lock' : 'lucide:lock-open'} label={locked ? t('actions.graph.unlock') : t('actions.graph.lock')} active={locked} onclick={() => onlock(!locked)} />
    <input
      class="ml-1 h-7 w-[9rem] border-b border-line bg-transparent px-1 text-[12.5px] text-ink outline-none placeholder:text-ink-3 focus:border-ink-3"
      placeholder={t('actions.graph.find')}
      aria-label={t('actions.graph.find')}
      value={query}
      oninput={(e) => search(e.currentTarget.value)}
    />
  </div>
</Panel>
