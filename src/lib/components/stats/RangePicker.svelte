<script lang="ts">
  import { presetRange, type Range, type RangePreset } from './stats'
  import { Field, Segmented } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    range: Range
    onchange: (range: Range) => void
  }

  let { range, onchange }: Props = $props()

  const presets: { value: RangePreset; label: string }[] = $derived([
    { value: '7d', label: t('stats.range.7d') },
    { value: '30d', label: t('stats.range.30d') },
    { value: '90d', label: t('stats.range.90d') },
    { value: 'year', label: t('stats.range.year') }
  ])

  let startText = $state(range.start)
  let endText = $state(range.end)

  $effect(() => {
    startText = range.start
    endText = range.end
  })

  function valid(value: string): boolean {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false
    const [y, m, d] = value.split('-').map(Number)
    const date = new Date(y, m - 1, d)
    return date.getFullYear() === y && date.getMonth() === m - 1 && date.getDate() === d
  }

  function custom(start: string, end: string) {
    if (!valid(start) || !valid(end) || start > end) {
      startText = range.start
      endText = range.end
      return
    }
    onchange({ start, end, preset: 'custom' })
  }

  function commit(e: KeyboardEvent) {
    if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur()
  }
</script>

<div class="flex flex-wrap items-center gap-1.5" role="group" aria-label={t('stats.range.label')}>
  <Segmented testid="stats-range-preset" label={t('stats.range.label')} value={range.preset} options={presets} onchange={(v) => onchange(presetRange(v as RangePreset))} />
  <span class="mx-1 h-5 w-px bg-line"></span>
  <label class="flex items-center gap-1.5 text-[13px] text-ink-3">
    <span>{t('stats.range.from')}</span>
    <Field data-testid="stats-range-start" mono size="sm" class="w-[7.5rem]" inputmode="numeric" placeholder={t('stats.range.placeholder')} spellcheck="false" aria-label={t('stats.range.start')} bind:value={startText} onblur={() => custom(startText.trim(), range.end)} onkeydown={commit} />
  </label>
  <label class="flex items-center gap-1.5 text-[13px] text-ink-3">
    <span>{t('stats.range.to')}</span>
    <Field data-testid="stats-range-end" mono size="sm" class="w-[7.5rem]" inputmode="numeric" placeholder={t('stats.range.placeholder')} spellcheck="false" aria-label={t('stats.range.end')} bind:value={endText} onblur={() => custom(range.start, endText.trim())} onkeydown={commit} />
  </label>
</div>
