<script lang="ts">
  import { goto } from '$app/navigation'
  import { api, peopleStore } from '$lib/tauri'
  import { personAvatar, type AvatarSource } from '$lib/avatar'
  import type { PersonStat, Ranking, Stats } from '$lib/tauri/types'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { Avatar, Button, IconButton, Sheet } from '$lib/components/ui'
  import Heatmap from './Heatmap.svelte'
  import Timeline from './Timeline.svelte'
  import Words from './Words.svelte'
  import { compact, hourLabel, minutes, rankingPositions, rangeInput, segments, weekdayLabel, type Range } from './stats'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    personId: string
    person: PersonStat | null
    rankings: Ranking[]
    range: Range
    onclose: () => void
  }

  let { personId, person, rankings, range, onclose }: Props = $props()

  let own = $state<Stats | null>(null)
  let loading = $state(true)
  let failure = $state<string | null>(null)

  const positions = $derived(rankingPositions(rankings, personId))
  const known = $derived($peopleStore.find((p) => p.id === personId) ?? null)
  const face = $derived<AvatarSource>(known ? personAvatar(known) : { shape: 'person', id: personId, name: person?.name ?? '?', src: null, color: null, faces: [], icon: null })

  $effect(() => {
    const id = personId
    const r = range
    loading = true
    failure = null
    own = null
    api
      .getStats(rangeInput(r, { person_ids: [id] }))
      .then((stats) => {
        own = stats
      })
      .catch((error) => {
        failure = String(error)
      })
      .finally(() => {
        loading = false
      })
  })

</script>

{#snippet rich(text: string, values: Record<string, string>)}{#each segments(text) as part, i (i)}{#if part.slot && values[part.slot] !== undefined}<span class="font-mono text-ink">{values[part.slot]}</span>{:else}{part.text}{/if}{/each}{/snippet}

<Sheet testid="stats-sender" label={person?.name ?? t('stats.sender.fallback')} width="52rem" {onclose}>
  <div class="stats-tokens p-6">
    <div class="flex items-start gap-4">
      <Avatar source={face} size="xl" label={t('stats.sender.openProfile')} onclick={() => goto('/main/person/' + personId + '/profile')} />
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2">
          <h2 class="truncate text-[15px] font-semibold tracking-tight text-ink">{person?.name ?? t('stats.sender.unknown')}</h2>
          <ProviderIcon providers={(person?.provider_types ?? []).map((provider_type) => ({ provider_type }))} size="md" class="text-ink-3 grayscale" />
        </div>
        {#if person}
          <p class="mt-1 text-[14px] leading-6 text-ink-2">
            {@render rich(t('stats.sender.summary'), { messages: compact(person.messages), characters: compact(person.characters), average: compact(person.avg_length), theirs: minutes(person.median_response_minutes), mine: minutes(person.my_median_response_minutes) })}{#if person.busiest_hour !== null}{@render rich(person.busiest_weekday !== null ? t('stats.sender.busiestDay', { weekday: weekdayLabel(person.busiest_weekday, 'long') }) : t('stats.sender.busiest'), { hour: hourLabel(person.busiest_hour) })}{/if}.
          </p>
        {/if}
      </div>
      <IconButton icon="lucide:x" label={t('common.close')} keys="esc" onclick={onclose} />
    </div>

    <div class="mt-5 grid gap-6 md:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)]">
      <figure class="min-w-0">
        <figcaption class="mb-2 flex items-baseline justify-between">
          <span class="text-[13px] font-medium text-ink">{t('stats.rankings.title')}</span>
          <span class="font-mono text-[11px] text-ink-3">{t('stats.sender.rankedOf', { count: positions.length, total: rankings.length })}</span>
        </figcaption>
        <ol class="divide-y divide-line">
          {#each positions as p (p.key)}
            <li class="flex items-center gap-2 py-1.5 text-[13px]">
              <span class="font-mono tabular-nums {p.position === 1 ? 'text-accent' : 'text-ink-3'}">#{p.position}</span>
              <span class="font-mono text-[11px] text-ink-3">{t('stats.sender.positionOf', { total: p.of })}</span>
              <span class="min-w-0 flex-1 truncate text-ink">{p.title}</span>
              <span class="font-mono text-[12px] tabular-nums text-ink-2">{p.value}</span>
            </li>
          {/each}
          {#if positions.length === 0}
            <li class="py-2 text-[13px] text-ink-3">{t('stats.sender.notRanked')}</li>
          {/if}
        </ol>
      </figure>
      <div class="min-w-0">
        {#if loading}
          <p class="font-mono text-[11px] text-ink-3">{t('stats.counting')}</p>
        {:else if failure}
          <p class="text-[13px] text-ink-3">{failure}</p>
        {:else if own}
          <Heatmap grid={own.heatmap} title={t('stats.sender.rhythm')} />
          <div class="mt-4"><Timeline points={own.timeline} {range} /></div>
          <div class="mt-4"><Words words={own.words.slice(0, 20)} /></div>
        {/if}
      </div>
    </div>
  </div>
  {#snippet footer()}
    <span class="font-mono text-[11px] text-ink-3">{range.start} → {range.end}</span>
    <Button class="ml-auto" icon="lucide:id-card" onclick={() => goto('/main/person/' + personId + '/profile')}>{t('stats.sender.openProfile')}</Button>
    <Button variant="primary" icon="lucide:arrow-right" onclick={() => goto('/main/person/' + personId)}>{t('stats.sender.openThread')}</Button>
  {/snippet}
</Sheet>
