<script lang="ts">
  import Panel from './Panel.svelte'
  import { compact, percent, senderMix, type MixKey, type Side } from './stats'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  interface Props {
    sides: Side[]
  }

  let { sides }: Props = $props()

  const colors: Record<MixKey, string> = { mine: 'var(--series-1)', people: 'var(--series-2)', services: 'color-mix(in oklab, var(--ink) 38%, var(--paper-2))' }
  const names: Record<MixKey, MessageKey> = { mine: 'stats.mix.mine', people: 'stats.mix.people', services: 'stats.mix.services' }
  const mixes = $derived(sides.map((side) => ({ label: side.label, parts: senderMix(side.stats) })))
  const share = (key: MixKey) => percent(mixes[0]?.parts.find((p) => p.key === key)?.share ?? 0, 1)
  let hover = $state<string | null>(null)
  const readout = $derived.by(() => {
    if (!hover) return null
    const [side, key] = hover.split(':')
    const part = mixes[Number(side)]?.parts.find((p) => p.key === key)
    return part ? t('stats.mix.segment', { label: t(names[part.key]), value: compact(part.value), count: part.value, share: percent(part.share, 1) }) : null
  })
</script>

<Panel
  title={t('stats.mix.title')}
  takeaway={mixes[0] && mixes[0].parts.some((p) => p.value > 0) ? t('stats.mix.takeaway', { people: share('people'), services: share('services'), mine: share('mine') }) : null}
  aside={readout}
  legend={(['mine', 'people', 'services'] as MixKey[]).map((key) => ({ label: t(names[key]), color: colors[key] }))}
>
  <div class="flex flex-col gap-3">
    {#each mixes as mix, s (s)}
      <div class="flex flex-col gap-1">
        {#if mixes.length > 1}
          <span class="truncate font-mono text-[11px] text-ink-3">{mix.label}</span>
        {/if}
        <div class="flex h-6 gap-[2px]" role="img" aria-label={mix.parts.map((p) => t('stats.mix.segment', { label: t(names[p.key]), value: compact(p.value), count: p.value, share: percent(p.share, 1) })).join('; ')}>
          {#each mix.parts.filter((p) => p.value > 0) as part, i (part.key)}
            <button
              class="flex min-w-0 items-center justify-start overflow-hidden px-1.5 {i === 0 ? 'rounded-l-[4px]' : ''} {i === mix.parts.filter((p) => p.value > 0).length - 1 ? 'rounded-r-[4px]' : ''} {hover === `${s}:${part.key}` ? 'brightness-110' : ''}"
              style:flex-grow={part.value}
              style:flex-basis="0"
              style:background={colors[part.key]}
              aria-label={t('stats.mix.segment', { label: t(names[part.key]), value: compact(part.value), count: part.value, share: percent(part.share, 1) })}
              onmouseenter={() => (hover = `${s}:${part.key}`)}
              onmouseleave={() => (hover = null)}
              onfocus={() => (hover = `${s}:${part.key}`)}
              onblur={() => (hover = null)}
            >
              {#if part.share >= 0.14 && part.key !== 'services'}
                <span class="font-mono text-[11px] tabular-nums {part.key === 'mine' ? 'text-[#1d1a17]' : 'text-white'}">{percent(part.share, 1)}</span>
              {/if}
            </button>
          {/each}
        </div>
      </div>
    {/each}
  </div>
</Panel>
