<script lang="ts">
  import { Segmented, Select } from '$lib/components/ui'
  import type { PersonStat } from '$lib/tauri/types'
  import { t } from '$lib/i18n.svelte'

  export type CompareMode = 'off' | 'people' | 'periods'

  interface Props {
    mode: CompareMode
    people: PersonStat[]
    left: string | null
    right: string | null
    onchange: (mode: CompareMode, left: string | null, right: string | null) => void
  }

  let { mode, people, left, right, onchange }: Props = $props()

  const modes: { value: CompareMode; label: string }[] = $derived([
    { value: 'off', label: t('stats.compare.off') },
    { value: 'people', label: t('stats.compare.people') },
    { value: 'periods', label: t('stats.compare.periods') }
  ])

  const candidates = $derived(people.filter((p) => p.person_id))
  const options = $derived([{ value: '', label: t('stats.compare.pickPerson') }, ...candidates.map((p) => ({ value: p.person_id as string, label: p.name }))])
</script>

<div class="flex flex-wrap items-center gap-2" role="group" aria-label={t('stats.compare.label')}>
  <Segmented testid="stats-compare" label={t('stats.compare.label')} value={mode} options={modes} onchange={(v) => onchange(v as CompareMode, left, right)} />
  {#if mode === 'people'}
    <Select testid="stats-compare-left" size="sm" class="w-[11rem]" value={left ?? ''} {options} onchange={(v) => onchange(mode, v || null, right)} />
    <span class="text-[13px] text-ink-3">{t('stats.compare.versus')}</span>
    <Select testid="stats-compare-right" size="sm" class="w-[11rem]" value={right ?? ''} {options} onchange={(v) => onchange(mode, left, v || null)} />
  {/if}
</div>
