<script lang="ts">
  import type { GroupStat } from '$lib/tauri/types'
  import Bars from './Bars.svelte'
  import { groupName, groupSentence, hourLabel, orderGroups, rankingValue, weekdayLabel } from './stats'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  interface Props {
    groups: GroupStat[]
    onpick?: (personId: string) => void
  }

  let { groups, onpick }: Props = $props()

  const ordered = $derived(orderGroups(groups))
  let selected = $state<string | null>(null)
  const current = $derived(ordered.find((g) => g.key === selected) ?? ordered[0] ?? null)

  const kindLabel: Record<GroupStat['kind'], MessageKey> = { bucket: 'stats.groups.kind.bucket', split: 'stats.groups.kind.split', tag: 'stats.groups.kind.tag', group: 'stats.groups.kind.group' }

  const hourRows = $derived(
    current ? current.by_hour.map((value, hour) => ({ key: String(hour), label: hourLabel(hour), value })).filter((r) => r.value > 0 || (Number(r.key) >= 6 && Number(r.key) <= 20)) : []
  )
  const weekdayRows = $derived(current ? current.by_weekday.map((value, day) => ({ key: String(day), label: weekdayLabel(day), value })) : [])
</script>

<section class="@container min-w-0">
  <div class="mb-3 flex items-baseline justify-between">
    <h2 class="text-[15px] font-semibold tracking-tight text-ink">{t('stats.groups.title')}</h2>
    <span class="font-mono text-[11px] text-ink-3">{t('stats.count.groups', { count: ordered.length })}</span>
  </div>
  <div class="flex flex-wrap gap-x-5 gap-y-1 border-b border-line" role="tablist" aria-label={t('stats.groups.label')}>
    {#each ordered as group (group.key)}
      {@const active = current?.key === group.key}
      <button
        role="tab"
        data-testid="stats-group-tab"
        data-key={group.key}
        aria-selected={active}
        class="-mb-px flex h-9 items-center gap-1.5 border-b-2 text-[14px] {active ? 'border-accent font-medium text-ink' : 'border-transparent text-ink-3 hover:text-ink'}"
        onclick={() => (selected = group.key)}
      >
        {#if group.color}
          <span class="size-1.5 rounded-full" style:background={group.color}></span>
        {/if}
        {groupName(group)}
        <span class="font-mono text-[11px] text-ink-3">{group.messages}</span>
      </button>
    {/each}
  </div>
  {#if current}
    <p class="mt-4 text-[14px] leading-6 text-ink">
      <span class="font-semibold">{groupName(current)}</span>
      <span class="font-mono text-[11px] text-ink-3"> {t(kindLabel[current.kind])} </span>
      {groupSentence(current)}.
    </p>
    <div class="mt-4 grid gap-8 @3xl:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1fr)]">
      <figure class="min-w-0">
        <figcaption class="mb-2 flex items-baseline justify-between">
          <span class="text-[13px] font-medium text-ink">{t('stats.groups.topSenders')}</span>
          <span class="font-mono text-[11px] text-ink-3">{t('stats.section.byMessages')}</span>
        </figcaption>
        <ol class="divide-y divide-line">
          {#each current.top as entry, index (entry.person_id ?? `${entry.name}:${index}`)}
            <li class="grid grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-2 py-1.5">
              <span class="font-mono text-[11px] tabular-nums {index === 0 ? 'text-accent' : 'text-ink-3'}">{index + 1}</span>
              <button class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-left text-[13px] font-medium text-ink {entry.person_id ? 'hover:underline' : ''}" onclick={() => entry.person_id && onpick?.(entry.person_id)} disabled={!entry.person_id}>{entry.name}</button>
              <span class="font-mono text-[12px] tabular-nums text-ink">{rankingValue(entry)}</span>
            </li>
          {/each}
          {#if current.top.length === 0}
            <li class="py-2 text-[13px] text-ink-3">{t('stats.groups.noSenders')}</li>
          {/if}
        </ol>
      </figure>
      <Bars title={t('stats.groups.byWeekday')} rows={weekdayRows} />
      <Bars title={t('stats.groups.byHour')} rows={hourRows} />
    </div>
  {:else}
    <p data-testid="stats-empty" class="mt-4 text-[13px] text-ink-3">{t('stats.groups.empty')}</p>
  {/if}
</section>
