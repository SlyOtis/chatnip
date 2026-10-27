<script lang="ts">
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import type { PersonStat } from '$lib/tauri/types'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { Avatar } from '$lib/components/ui'
  import { compact, hourLabel, minutes, sortPeople, weekdayLabel, type PeopleSortKey } from './stats'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  interface Props {
    people: PersonStat[]
    highlight?: string[]
  }

  let { people, highlight = [] }: Props = $props()

  let sortKey = $state<PeopleSortKey>('messages')
  let direction = $state<'asc' | 'desc'>('desc')
  let focused = $state<string | null>(null)

  const columns: { key: PeopleSortKey; label: MessageKey; numeric: boolean }[] = [
    { key: 'name', label: 'stats.people.person', numeric: false },
    { key: 'messages', label: 'stats.metric.messages', numeric: true },
    { key: 'characters', label: 'stats.people.chars', numeric: true },
    { key: 'avg_length', label: 'stats.people.avgLength', numeric: true },
    { key: 'median_response_minutes', label: 'stats.metric.theirReply', numeric: true },
    { key: 'my_median_response_minutes', label: 'stats.metric.myReply', numeric: true },
    { key: 'busiest_hour', label: 'stats.people.peak', numeric: true }
  ]

  const rows = $derived(sortPeople(people, sortKey, direction))
  const maxMessages = $derived(Math.max(1, ...people.map((p) => p.messages)))

  function sortBy(key: PeopleSortKey) {
    if (sortKey === key) direction = direction === 'asc' ? 'desc' : 'asc'
    else {
      sortKey = key
      direction = key === 'name' ? 'asc' : 'desc'
    }
  }

  function open(person: PersonStat) {
    if (person.person_id) goto('/main/person/' + person.person_id + '/profile')
  }
</script>

<div class="overflow-hidden">
  <table class="w-full table-fixed border-collapse text-[13px]" aria-label={t('stats.people.label')}>
    <colgroup>
      <col style="width: 30%" />
      <col style="width: 17%" />
      <col style="width: 11%" />
      <col style="width: 10%" />
      <col style="width: 11%" />
      <col style="width: 11%" />
      <col style="width: 10%" />
    </colgroup>
    <thead>
      <tr class="border-b border-line">
        {#each columns as column (column.key)}
          <th scope="col" class="h-9 px-2 align-bottom font-medium leading-tight text-ink-3 {column.numeric ? 'text-right' : 'text-left'}" aria-sort={sortKey === column.key ? (direction === 'asc' ? 'ascending' : 'descending') : 'none'}>
            <button data-testid="stats-sort" data-key={column.key} class="inline-flex items-center gap-1 hover:text-ink" onclick={() => sortBy(column.key)}>
              {t(column.label)}
              {#if sortKey === column.key}
                <Icon icon={direction === 'asc' ? 'lucide:arrow-up' : 'lucide:arrow-down'} class="text-[12px] text-accent" />
              {/if}
            </button>
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as person, index (person.person_id ?? `${person.name}:${index}`)}
        {@const on = highlight.length === 0 || (person.person_id !== null && highlight.includes(person.person_id))}
        <tr
          data-testid="stats-person"
          data-id={person.person_id}
          data-highlight={on}
          class="h-10 border-b border-line {on ? '' : 'opacity-40'} {focused === (person.person_id ?? person.name) ? 'bg-paper-2' : 'hover:bg-paper-2'}"
          onmouseenter={() => (focused = person.person_id ?? person.name)}
          onmouseleave={() => (focused = null)}
        >
          <td class="min-w-0 px-2 py-1.5">
            <button data-testid="stats-person-open" class="flex w-full min-w-0 items-center gap-2 text-left" onclick={() => open(person)} disabled={!person.person_id}>
              <Avatar source={{ shape: 'person', id: person.person_id ?? person.name, name: person.name, src: null, color: null, faces: [], icon: null }} size="sm" />
              <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap font-medium text-ink {person.person_id ? 'hover:underline' : ''}">{person.name}</span>
              <ProviderIcon providers={person.provider_types.map((provider_type) => ({ provider_type }))} size="xs" max={3} class="shrink-0 text-ink-3 grayscale" />
            </button>
          </td>
          <td class="px-2 py-1.5 text-right whitespace-nowrap">
            <div class="flex items-center justify-end gap-2">
              <span class="h-1.5 rounded-r-[2px] bg-[var(--series-1)]" style:width="{Math.max(2, (person.messages / maxMessages) * 64)}px"></span>
              <span class="font-mono tabular-nums text-ink">{compact(person.messages)}</span>
            </div>
          </td>
          <td class="px-2 py-1.5 text-right font-mono tabular-nums whitespace-nowrap text-ink-2">{compact(person.characters)}</td>
          <td class="px-2 py-1.5 text-right font-mono tabular-nums whitespace-nowrap text-ink-2">{compact(person.avg_length)}</td>
          <td class="px-2 py-1.5 text-right font-mono tabular-nums whitespace-nowrap text-ink-2">{minutes(person.median_response_minutes)}</td>
          <td class="px-2 py-1.5 text-right font-mono tabular-nums whitespace-nowrap text-ink-2">{minutes(person.my_median_response_minutes)}</td>
          <td class="px-2 py-1.5 text-right font-mono tabular-nums whitespace-nowrap text-ink-2" title={person.busiest_weekday !== null ? weekdayLabel(person.busiest_weekday, 'long') : ''}>{hourLabel(person.busiest_hour)}</td>
        </tr>
      {/each}
      {#if rows.length === 0}
        <tr data-testid="stats-empty"><td colspan="7" class="px-2 py-6 text-center text-ink-3">{t('stats.people.empty')}</td></tr>
      {/if}
    </tbody>
  </table>
</div>
