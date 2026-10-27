<script lang="ts">
  import Panel from './Panel.svelte'
  import { compact, habits, percent, type Habits as Rhythm, type Side } from './stats'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    sides: Side[]
  }

  let { sides }: Props = $props()

  const values = $derived(sides.map((side) => habits(side.stats, side.range)))
  const lead = $derived(values[0])
  const days = (count: number) => t('common.days', { count })
  const rows = $derived([
    { key: 'active', label: t('stats.habits.active'), show: (h: Rhythm) => t('stats.habits.activeValue', { active: h.active, days: h.days }) },
    { key: 'streak', label: t('stats.habits.streak'), show: (h: Rhythm) => days(h.streak) },
    { key: 'quiet', label: t('stats.habits.quiet'), show: (h: Rhythm) => days(h.quiet) },
    { key: 'perDay', label: t('stats.habits.perDay'), show: (h: Rhythm) => compact(h.perActiveDay) },
    { key: 'afterHours', label: t('stats.habits.afterHours'), show: (h: Rhythm) => percent(h.afterHours, 1) },
    { key: 'weekend', label: t('stats.habits.weekend'), show: (h: Rhythm) => percent(h.weekend, 1) }
  ])
</script>

<Panel
  title={t('stats.habits.title')}
  takeaway={lead ? t('stats.habits.takeaway', { active: lead.active, days: days(lead.days), streak: days(lead.streak) }) : null}
  legend={sides.length > 1 ? sides.map((s, i) => ({ label: s.label, color: `var(--series-${i + 1})` })) : []}
>
  <table class="w-full table-fixed border-collapse text-[13px]">
    {#if sides.length > 1}
      <thead>
        <tr class="border-b border-line">
          <th class="w-1/2"></th>
          {#each sides as side, i (i)}
            <th scope="col" class="truncate pb-1 text-right font-mono text-[11px] font-normal text-ink-3">{side.label}</th>
          {/each}
        </tr>
      </thead>
    {/if}
    <tbody>
      {#each rows as row (row.key)}
        <tr class="border-b border-line last:border-b-0">
          <th scope="row" class="w-1/2 py-1.5 text-left font-normal text-ink-2">{row.label}</th>
          {#each values as h, i (i)}
            <td class="truncate py-1.5 text-right font-mono tabular-nums text-ink">{row.show(h)}</td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</Panel>
