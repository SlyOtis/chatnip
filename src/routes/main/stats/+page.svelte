<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import { onMount } from 'svelte'
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { api } from '$lib/tauri'
  import type { Stats } from '$lib/tauri/types'
  import { providerLabel } from '$lib/providerIcons'
  import RangePicker from '$lib/components/stats/RangePicker.svelte'
  import ComparePicker, { type CompareMode } from '$lib/components/stats/ComparePicker.svelte'
  import PeopleTable from '$lib/components/stats/PeopleTable.svelte'
  import Heatmap from '$lib/components/stats/Heatmap.svelte'
  import Timeline from '$lib/components/stats/Timeline.svelte'
  import Bars from '$lib/components/stats/Bars.svelte'
  import Distribution from '$lib/components/stats/Distribution.svelte'
  import Habits from '$lib/components/stats/Habits.svelte'
  import Mix from '$lib/components/stats/Mix.svelte'
  import Panel from '$lib/components/stats/Panel.svelte'
  import Words from '$lib/components/stats/Words.svelte'
  import Globe from '$lib/components/stats/Globe.svelte'
  import Rankings from '$lib/components/stats/Rankings.svelte'
  import GroupStats from '$lib/components/stats/GroupStats.svelte'
  import SenderSheet from '$lib/components/stats/SenderSheet.svelte'
  import {
    BACKLOG_AGES,
    backlog,
    bytes,
    compact,
    conversationTypeLabel,
    deltaLabel,
    demoStats,
    diff,
    fastestWeekday,
    fileLabel,
    fileRows,
    habits,
    hourLabel,
    kindLabel,
    kindRows,
    leadSentence,
    minutes,
    pairRows,
    percent,
    presetRange,
    previousRange,
    rangeDays,
    rangeInput,
    rankingPositions,
    segments,
    volumePeak,
    volumeSeries,
    weekdayLabel,
    weekdayReplyRows,
    type Range,
    type Side
  } from '$lib/components/stats/stats'
  import { formatDate, t } from '$lib/i18n.svelte'
  import { settingHref } from '$lib/settings'

  let range = $state<Range>(presetRange('30d'))
  let stats = $state<Stats | null>(null)
  let previous = $state<Stats | null>(null)
  let synthetic = $state(false)
  let loading = $state(true)
  let mode = $state<CompareMode>('off')
  let left = $state<string | null>(null)
  let right = $state<string | null>(null)
  let leftStats = $state<Stats | null>(null)
  let rightStats = $state<Stats | null>(null)
  let sender = $state<string | null>(null)

  async function fetch(r: Range, filters: Parameters<typeof rangeInput>[1] = {}): Promise<{ stats: Stats; synthetic: boolean }> {
    try {
      return { stats: await api.getStats(rangeInput(r, filters)), synthetic: false }
    } catch {
      return { stats: demoStats(r), synthetic: true }
    }
  }

  async function load() {
    loading = true
    const main = await fetch(range)
    stats = main.stats
    synthetic = main.synthetic
    const prev = await fetch(previousRange(range))
    previous = prev.stats
    await loadCompare()
    loading = false
  }

  async function loadCompare() {
    if (mode === 'people' && left && right) {
      leftStats = (await fetch(range, { person_ids: [left] })).stats
      rightStats = (await fetch(range, { person_ids: [right] })).stats
    } else if (mode === 'periods') {
      leftStats = stats
      rightStats = previous
    } else {
      leftStats = null
      rightStats = null
    }
  }

  function setRange(next: Range) {
    range = next
    load()
  }

  function setCompare(nextMode: CompareMode, nextLeft: string | null, nextRight: string | null) {
    mode = nextMode
    left = nextLeft
    right = nextRight
    loadCompare()
  }

  onMount(load)

  const lead = $derived(stats ? leadSentence(stats) : null)
  const days = $derived(rangeDays(range))
  const messageDiff = $derived(stats && previous && previous.totals.messages > 0 ? diff(stats.totals.messages, previous.totals.messages) : null)
  const personName = (id: string | null) => stats?.by_person.find((p) => p.person_id === id)?.name ?? '—'
  const personStat = (id: string | null) => stats?.by_person.find((p) => p.person_id === id) ?? null

  const pair = $derived<[Side, Side] | null>(
    mode !== 'off' && leftStats && rightStats
      ? [
          { label: mode === 'people' ? personName(left) : t('stats.compare.thisPeriod'), stats: leftStats, range },
          { label: mode === 'people' ? personName(right) : t('stats.compare.periodBefore'), stats: rightStats, range: mode === 'periods' ? previousRange(range) : range }
        ]
      : null
  )
  const sides = $derived<Side[]>(pair ?? (stats ? [{ label: t('stats.compare.thisPeriod'), stats, range }] : []))
  const a = $derived<Side | null>(sides[0] ?? null)
  const b = $derived<Side | null>(pair?.[1] ?? null)
  const names = $derived<[string, string] | undefined>(pair ? [pair[0].label, pair[1].label] : undefined)

  const rhythm = $derived(a ? habits(a.stats, a.range) : null)
  const volume = $derived(a ? volumeSeries({ points: a.stats.timeline, range: a.range }, b ? { points: b.stats.timeline, range: b.range } : null) : [])
  const volumeTakeaway = $derived.by(() => {
    if (a && b) {
      const total = (s: Stats) => s.totals.messages
      return t('stats.timeline.takeawayCompare', { a: compact(total(a.stats)), b: compact(total(b.stats)), delta: deltaLabel(diff(total(a.stats), total(b.stats))) })
    }
    const peak = volumePeak(volume)
    return peak ? t('stats.timeline.takeaway', { date: formatDate(new Date(`${peak.date}T00:00:00`), { weekday: 'long', day: 'numeric', month: 'short' }), count: peak.values[0] + peak.values[1] }) : t('stats.timeline.quiet')
  })

  const weekdayRows = $derived(a ? weekdayReplyRows(a.stats.reply_by_weekday ?? [], b ? (b.stats.reply_by_weekday ?? []) : null).map((r) => ({ key: r.key, label: weekdayLabel(r.weekday), value: r.value, other: r.other })) : [])
  const fastest = $derived(a ? fastestWeekday(a.stats.reply_by_weekday ?? []) : null)

  const ages = $derived(a ? backlog(a.stats.timeline) : [])
  const otherAges = $derived(b ? backlog(b.stats.timeline) : null)
  const backlogRows = $derived(BACKLOG_AGES.map((age, i) => ({ key: age.key, label: t(`stats.backlog.age.${age.key}`), value: ages[i]?.value ?? 0, ...(otherAges ? { other: otherAges[i].value } : {}) })))
  const unread = $derived(ages.reduce((s, r) => s + r.value, 0))
  const stale = $derived(ages.filter((r) => r.key === 'month' || r.key === 'older').reduce((s, r) => s + r.value, 0))

  const kinds = $derived(a ? kindRows(a.stats.by_kind ?? [], b ? (b.stats.by_kind ?? []) : null).map((r) => ({ ...r, label: kindLabel(r.key) })) : [])
  const kindTotal = $derived(kinds.reduce((s, r) => s + r.value, 0))

  const files = $derived(a ? fileRows(a.stats.by_file_type ?? [], b ? (b.stats.by_file_type ?? []) : null).map((r) => ({ ...r, label: fileLabel(r.key) })) : [])
  const fileCount = $derived(files.reduce((s, r) => s + r.value, 0))
  const fileBytes = $derived((a?.stats.by_file_type ?? []).reduce((s, f) => s + f.bytes, 0))

  const providerRows = $derived(
    a
      ? pairRows(a.stats.by_provider.map((p) => ({ key: p.provider_id, value: p.messages })), b ? b.stats.by_provider.map((p) => ({ key: p.provider_id, value: p.messages })) : null).map((r) => {
          const p = a.stats.by_provider.find((q) => q.provider_id === r.key) ?? b?.stats.by_provider.find((q) => q.provider_id === r.key)
          return { ...r, label: p?.name ?? r.key, detail: p && !b ? providerLabel(p) : undefined }
        })
      : []
  )
  const tagRows = $derived(
    a
      ? pairRows(a.stats.by_label.map((l) => ({ key: l.tag_id, value: l.messages })), b ? b.stats.by_label.map((l) => ({ key: l.tag_id, value: l.messages })) : null).map((r) => {
          const l = a.stats.by_label.find((q) => q.tag_id === r.key)
          return { ...r, label: l?.name ?? r.key, color: b ? undefined : l?.color, detail: l && !b ? t('stats.tag.conversations', { count: l.conversations }) : undefined }
        })
      : []
  )
  const shareOf = (rows: { label: string; value: number }[]) => {
    const total = rows.reduce((s, r) => s + r.value, 0)
    return rows[0] && total ? { name: rows[0].label, share: percent(rows[0].value, total) } : null
  }
  const topProvider = $derived(shareOf(providerRows))
  const topTag = $derived(shareOf(tagRows))
  const topKind = $derived(shareOf(kinds))
  const topThree = $derived(stats && stats.totals.messages ? percent(stats.by_conversation.slice(0, 3).reduce((s, c) => s + c.messages, 0), stats.totals.messages) : null)
</script>

{#snippet rich(text: string, mono: Record<string, string>, strong: Record<string, string>)}{#each segments(text) as part, i (i)}{#if part.slot && mono[part.slot] !== undefined}<span class="font-mono text-[15px] font-medium">{mono[part.slot]}</span>{:else if part.slot && strong[part.slot] !== undefined}<span class="font-medium">{strong[part.slot]}</span>{:else}{part.text}{/if}{/each}{/snippet}

<div class="stats flex h-full min-w-0 flex-col overflow-hidden bg-paper">
  <header class="flex h-12 shrink-0 items-center gap-3 border-b border-line px-5 pt-1">
    <h1 class="text-[15px] font-semibold tracking-tight text-ink">{t('stats.title')}</h1>
    <span data-testid="stats-range" data-start={range.start} data-end={range.end} class="font-mono text-[11px] text-ink-3">{t('stats.header.range', { start: range.start, end: range.end, period: t('common.days', { count: days }) })}</span>
    {#if synthetic}
      <span data-testid="stats-synthetic" class="font-mono text-[11px] text-signal">{t('stats.synthetic')}</span>
    {/if}
    <div class="ml-auto">
      <RangePicker {range} onchange={setRange} />
    </div>
  </header>

  <div class="board min-h-0 flex-1 overflow-y-auto">
    {#if !stats || !lead || !a}
      <div data-testid="stats-counting" class="flex h-full items-center justify-center font-mono text-[11px] text-ink-3">{t('stats.counting')}</div>
    {:else}
      <div data-testid="stats-sheet" data-loading={loading} data-mode={mode} class="sheet {loading ? 'opacity-60' : ''} transition-opacity">
        <section class="full flex flex-wrap items-end justify-between gap-x-10 gap-y-4">
          <p data-testid="stats-lead" class="max-w-[68ch] text-[15px] leading-7 text-ink">
            {@render rich(t('stats.lead.totals', { count: stats.totals.messages, period: t('common.days', { count: days }) }), { messages: compact(stats.totals.messages), received: compact(stats.totals.received), sent: compact(stats.totals.sent) }, {})}{#if messageDiff}&nbsp;<span class="font-mono text-[12px] {messageDiff.delta >= 0 ? 'text-ink-2' : 'text-ink-3'}">{t('stats.lead.delta', { delta: deltaLabel(messageDiff), period: t('common.days', { count: days }) })}</span>{/if}{@render rich(t('stats.lead.reach'), { people: String(stats.totals.people), services: String(stats.totals.services), conversations: String(stats.totals.conversations) }, {})}
            {#if lead.top}
              {@render rich(t('stats.lead.top', { messages: compact(lead.top.messages) }), {}, { name: lead.top.name })}
            {/if}
            {#if lead.longest}
              {@render rich(t('stats.lead.longest', { characters: compact(lead.longest.avg_length) }), {}, { name: lead.longest.name })}
            {/if}
            {#if lead.fastest}
              {@render rich(t('stats.lead.fastest', { time: minutes(lead.fastest.median_response_minutes) }), {}, { name: lead.fastest.name })}
            {/if}
            {@render rich(t('stats.lead.peak', { weekday: weekdayLabel(lead.peakDay, 'long'), theirs: minutes(stats.totals.median_response_minutes), mine: minutes(stats.totals.my_median_response_minutes) }), { hour: hourLabel(lead.peakHour) }, {})}
          </p>
          <ComparePicker {mode} people={stats.by_person} {left} {right} onchange={setCompare} />
        </section>

        {#if pair}
          <section data-testid="stats-sides" class="full sides">
            {#each pair as side, i (i)}
              <div data-testid="stats-side" data-index={i} class="min-w-0">
                <h2 class="flex items-center gap-2 text-[15px] font-semibold tracking-tight text-ink"><span class="size-2 rounded-[2px]" style:background="var(--series-{i + 1})"></span>{side.label}</h2>
                <dl class="mt-2 grid grid-cols-3 gap-x-4 gap-y-1 text-[13px]">
                  <dt class="text-ink-3">{t('stats.metric.messages')}</dt><dd class="col-span-2 font-mono tabular-nums text-ink">{compact(side.stats.totals.messages)}</dd>
                  <dt class="text-ink-3">{t('stats.metric.characters')}</dt><dd class="col-span-2 font-mono tabular-nums text-ink">{compact(side.stats.totals.characters)}</dd>
                  <dt class="text-ink-3">{t('stats.metric.theirReply')}</dt><dd class="col-span-2 font-mono tabular-nums text-ink">{minutes(side.stats.totals.median_response_minutes)}</dd>
                  <dt class="text-ink-3">{t('stats.metric.myReply')}</dt><dd class="col-span-2 font-mono tabular-nums text-ink">{minutes(side.stats.totals.my_median_response_minutes)}</dd>
                  <dt class="text-ink-3">{t('stats.metric.sentShare')}</dt><dd class="col-span-2 font-mono tabular-nums text-ink">{percent(side.stats.totals.sent, side.stats.totals.messages)}</dd>
                </dl>
                {#if mode === 'people'}
                  {@const positions = rankingPositions(stats.rankings, i === 0 ? (left ?? '') : (right ?? ''))}
                  <ol class="mt-3 divide-y divide-line">
                    {#each positions as p (p.key)}
                      <li class="flex items-center gap-2 py-1 text-[13px]">
                        <span class="font-mono tabular-nums {p.position === 1 ? 'text-accent' : 'text-ink-3'}">#{p.position}</span>
                        <span class="min-w-0 flex-1 truncate text-ink-2">{p.title}</span>
                        <span class="font-mono text-[12px] tabular-nums text-ink">{p.value}</span>
                      </li>
                    {/each}
                  </ol>
                {/if}
                <div class="mt-4"><Heatmap grid={side.stats.heatmap} title={t('stats.section.rhythm')} /></div>
              </div>
            {/each}
          </section>
        {/if}

        <section data-testid="stats-panel" data-key="people" class="wide tall min-w-0">
          <h2 class="mb-3 text-[15px] font-semibold tracking-tight text-ink">{t('stats.section.people')}</h2>
          <PeopleTable people={stats.by_person} highlight={mode === 'people' ? [left, right].filter((v): v is string => !!v) : []} />
        </section>

        <div data-testid="stats-panel" data-key="heatmap" class="contents"><Heatmap grid={stats.heatmap} takeaway={rhythm ? t('stats.heatmap.takeaway', { hour: hourLabel(lead.peakHour), weekday: weekdayLabel(lead.peakDay, 'long'), share: percent(rhythm.afterHours, 1) }) : null} /></div>

        <div data-testid="stats-panel" data-key="distribution" class="contents"><Distribution buckets={a.stats.response_times} other={b?.stats.response_times ?? null} labels={names} median={a.stats.totals.median_response_minutes} mine={a.stats.totals.my_median_response_minutes} /></div>

        <div data-testid="stats-panel" data-key="timeline" class="wide min-w-0">
          <Timeline points={a.stats.timeline} range={a.range} other={b ? { points: b.stats.timeline, range: b.range } : null} labels={names} takeaway={volumeTakeaway} onbrush={(start, end) => setRange({ start, end, preset: 'custom' })} />
        </div>

        <div data-testid="stats-panel" data-key="weekday-replies" class="contents"><Bars
          title={t('stats.weekdayReplies.title')}
          rows={weekdayRows}
          legend={names ?? [t('stats.legend.theirs'), t('stats.legend.mine')]}
          aside={t('stats.weekdayReplies.median')}
          format={(v) => (v > 0 ? minutes(v) : '—')}
          empty={t('stats.weekdayReplies.empty')}
          takeaway={fastest && fastest.mine !== null ? t('stats.weekdayReplies.takeaway', { weekday: weekdayLabel(fastest.weekday, 'long'), time: minutes(fastest.mine) }) : null}
        /></div>

        <div data-testid="stats-panel" data-key="habits" class="contents"><Habits {sides} /></div>

        <div data-testid="stats-panel" data-key="mix" class="contents"><Mix {sides} /></div>

        <div data-testid="stats-panel" data-key="backlog" class="contents"><Bars
          title={t('stats.backlog.title')}
          rows={backlogRows}
          legend={names ?? null}
          aside={compact(unread)}
          takeaway={unread ? t('stats.backlog.takeaway', { count: unread, old: compact(stale) }) : t('stats.backlog.clear')}
        /></div>

        <div data-testid="stats-panel" data-key="kinds" class="contents"><Bars title={t('stats.kinds.title')} rows={kinds} legend={names ?? null} takeaway={topKind ? t('stats.kinds.takeaway', { kind: topKind.name.toLowerCase(), share: topKind.share }) : null} aside={kindTotal ? t('stats.count.messages', { count: kindTotal, value: compact(kindTotal) }) : null} /></div>

        <div data-testid="stats-panel" data-key="files" class="contents"><Bars
          title={t('stats.files.title')}
          rows={files}
          legend={names ?? null}
          aside={fileCount ? bytes(fileBytes) : null}
          empty={t('stats.files.empty')}
          unit={(count) => t('stats.count.times', { count })}
          takeaway={files[0] ? t('stats.files.takeaway', { count: fileCount, size: bytes(fileBytes), kind: files[0].label.toLowerCase() }) : null}
        /></div>

        <div data-testid="stats-panel" data-key="channels" class="contents"><Bars title={t('stats.section.channels')} rows={providerRows} legend={names ?? null} takeaway={topProvider ? t('stats.channels.takeaway', topProvider) : null} onpick={() => goto(settingHref('accounts.connections'))} /></div>

        <div data-testid="stats-panel" data-key="tags" class="contents"><Bars title={t('stats.section.splitsAndTags')} rows={tagRows} legend={names ?? null} takeaway={topTag ? t('stats.tags.takeaway', topTag) : null} /></div>

        <div data-testid="stats-panel" data-key="conversations" class="pair min-w-0">
          <Panel title={t('stats.section.busiestConversations')} aside={t('stats.section.byMessages')} takeaway={topThree ? t('stats.conversations.takeaway', { share: topThree }) : null}>
            <ol class="divide-y divide-line">
              {#each stats.by_conversation.slice(0, 8) as c (c.conversation_id)}
                <li>
                  <button data-testid="stats-conversation" data-id={c.conversation_id} class="grid w-full grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-3 py-1.5 text-left hover:bg-paper-2" onclick={() => goto(conversationHref(c.conversation_id))}>
                    <span class="min-w-0">
                      <span class="block overflow-hidden text-ellipsis whitespace-nowrap text-[13px] font-medium text-ink">{c.name}</span>
                      <span class="block font-mono text-[10px] text-ink-3">{t('stats.conversation.meta', { type: conversationTypeLabel(c.type), count: c.people })}</span>
                    </span>
                    <span class="font-mono text-[12px] tabular-nums text-ink">{compact(c.messages)}</span>
                    <span class="font-mono text-[10px] tabular-nums text-ink-3">{t('stats.conversation.characters', { value: compact(c.characters) })}</span>
                  </button>
                </li>
              {/each}
            </ol>
          </Panel>
        </div>

        <div data-testid="stats-panel" data-key="words" class="pair min-w-0">
          <Words words={stats.words} />
        </div>

        <div data-testid="stats-panel" data-key="rankings" class="full min-w-0">
          <Rankings rankings={stats.rankings ?? []} onpick={(id) => (sender = id)} />
        </div>

        <div data-testid="stats-panel" data-key="groups" class="full min-w-0">
          <GroupStats groups={stats.groups ?? []} onpick={(id) => (sender = id)} />
        </div>

        <div data-testid="stats-panel" data-key="globe" class="full min-w-0">
          <Globe origins={stats.origins} />
        </div>

        <p class="full flex items-center gap-2 font-mono text-[11px] text-ink-3">
          <Icon icon="lucide:info" class="text-[13px]" />
          {t('stats.localOnly')}
        </p>
      </div>
    {/if}
  </div>
  {#if sender && stats}
    <SenderSheet personId={sender} person={personStat(sender)} rankings={stats.rankings ?? []} {range} onclose={() => (sender = null)} />
  {/if}
</div>

<style>
  .board {
    container: board / inline-size;
  }
  .sheet {
    --cols: 1;
    --wide: 1;
    --tall: 1;
    --pair: 1;
    display: grid;
    grid-template-columns: repeat(var(--cols), minmax(0, 1fr));
    grid-auto-flow: row dense;
    align-items: start;
    gap: clamp(2rem, 3.5cqi, 3rem) clamp(1.5rem, 3cqi, 3rem);
    padding: clamp(1rem, 2.5cqi, 2rem) clamp(1rem, 3cqi, 2.5rem) 4rem;
  }
  .full {
    grid-column: 1 / -1;
  }
  .wide {
    grid-column: span var(--wide);
  }
  .tall {
    grid-row: span var(--tall);
  }
  .pair {
    grid-column: span var(--pair);
  }
  .sides {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 22rem), 1fr));
    gap: 2rem clamp(1.5rem, 3cqi, 3rem);
  }
  @container board (min-width: 44rem) {
    .sheet {
      --cols: 2;
      --wide: 2;
    }
  }
  @container board (min-width: 72rem) {
    .sheet {
      --cols: 3;
      --wide: 2;
      --tall: 2;
    }
  }
  @container board (min-width: 108rem) {
    .sheet {
      --cols: 4;
      --wide: 2;
      --tall: 2;
      --pair: 2;
    }
  }
</style>
