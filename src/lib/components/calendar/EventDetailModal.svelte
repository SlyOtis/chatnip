<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import { goto } from '$app/navigation'
  import type { CalendarEvent } from './types'
  import { Button, Sheet, TagField } from '$lib/components/ui'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { joinUrl, sourceMarks } from '$lib/calendar'
  import { formatDate, formatNumber, t, type MessageKey } from '$lib/i18n.svelte'
  import { api, providersStore } from '$lib/tauri'
  import type { CalendarResponse, EventSource } from '$lib/tauri/types'

  interface Props {
    event: CalendarEvent | null
    visible: boolean
    onclose: () => void
    onstatus: (event: CalendarEvent, status: 'confirmed' | 'dismissed') => void
    ondelete: (event: CalendarEvent) => void
    onunmerge: (event: CalendarEvent) => void
  }

  let { event, visible, onclose, onstatus, ondelete, onunmerge }: Props = $props()

  const join = $derived(event ? joinUrl(event) : null)
  const local = $derived(!!event && event.sources.every((s) => !s.calendar_id))
  const RESPONSES: Record<string, MessageKey> = { accepted: 'calendar.response.accepted', tentativelyAccepted: 'calendar.response.maybe', tentative: 'calendar.response.maybe', declined: 'calendar.response.declined', organizer: 'calendar.response.organiser', needsAction: 'calendar.response.notAnswered', notResponded: 'calendar.response.notAnswered' }
  const ANSWERS: [CalendarResponse, MessageKey][] = [['accepted', 'calendar.answer.accept'], ['tentativelyAccepted', 'calendar.answer.maybe'], ['declined', 'calendar.answer.decline']]
  let answering = $state<string | null>(null)
  let answerFailure = $state<string | null>(null)
  let answered = $state<Record<string, string>>({})

  function answerable(source: EventSource): boolean {
    const provider = ($providersStore.data ?? []).find((p) => p.id === source.provider_id)
    return !!provider?.capabilities.supports_calendar_responses && !!source.response_status && source.response_status !== 'organizer'
  }

  async function answer(source: EventSource, response: CalendarResponse) {
    answering = source.event_id
    answerFailure = null
    try {
      await api.putEventResponse(source.event_id, response)
      answered[source.event_id] = response
    } catch (error) {
      answerFailure = String(error)
    } finally {
      answering = null
    }
  }
  const STATUSES: Record<CalendarEvent['status'], MessageKey> = { proposed: 'calendar.status.proposed', confirmed: 'calendar.status.confirmed', dismissed: 'calendar.status.dismissed' }

  function formatTimeRange(e: CalendarEvent): string {
    const startDate = new Date(e.starts_at)
    const endDate = new Date(e.ends_at)
    const dateStr = formatDate(startDate, { weekday: 'long', month: 'long', day: 'numeric', year: 'numeric' })
    if (e.all_day) return t('calendar.detail.allDay', { date: dateStr })
    const startStr = formatDate(startDate, { hour: '2-digit', minute: '2-digit' })
    const endStr = formatDate(endDate, { hour: '2-digit', minute: '2-digit' })
    return `${dateStr}, ${startStr} – ${endStr}`
  }

  function openConversation(id: string) {
    onclose()
    goto(conversationHref(id))
  }

  const line = 'flex items-start gap-3 border-b border-line py-2 last:border-b-0'
  const key = 'w-14 shrink-0 pt-0.5 font-mono text-[11px] text-ink-3'
</script>

{#if visible && event}
  <Sheet testid="calendar-detail" label={event.title} width="30rem" onclose={onclose}>
    <div class="px-5 pb-3 pt-5">
      <h2 data-testid="calendar-detail-title" data-id={event.id} data-status={event.status} class="break-words text-[15px] font-semibold tracking-tight text-ink">{event.title}</h2>
      <p class="mt-0.5 text-[13px] text-ink-3">{event.origin === 'synced' ? (event.sources.length > 1 ? t('calendar.detail.inCalendars', { count: event.sources.length }) : t('calendar.detail.fromCalendar')) : t(STATUSES[event.status])}{event.origin === 'parsed' ? t('calendar.detail.parsedAt', { percent: formatNumber(event.confidence, { style: 'percent' }) }) : ''}</p>
      <div class="mt-2"><TagField objectType="event" objectId={event.id} /></div>
      <div class="mt-4">
        <div class={line}>
          <span class={key}>{t('calendar.detail.when')}</span>
          <span class="text-[14px] text-ink">{formatTimeRange(event)}</span>
        </div>
        {#if event.sources.length > 0}
          <div class={line}>
            <span class={key}>{t('calendar.detail.in')}</span>
            <div class="flex min-w-0 flex-1 flex-col gap-1">
              {#each event.sources as source (source.event_id)}
                <div class="flex min-w-0 items-center gap-2 text-[14px] text-ink">
                  <span class="size-2 shrink-0 rounded-full" style:background={source.colour ?? 'var(--line-strong)'}></span>
                  <ProviderIcon providers={sourceMarks([source])} size="sm" />
                  <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{source.calendar_name}</span>
                  {#if source.response_status}
                    {@const status = answered[source.event_id] ?? source.response_status}
                    <span data-testid="calendar-detail-response" data-source={source.event_id} data-status={status} class="font-mono text-[11px] text-ink-3">{RESPONSES[status] ? t(RESPONSES[status]) : status}</span>
                  {/if}
                </div>
                {#if answerable(source)}
                  <div class="flex items-center gap-1 pl-4">
                    {#each ANSWERS as [response, label] (response)}
                      <Button testid="calendar-answer-{response}" size="sm" variant={(answered[source.event_id] ?? source.response_status) === response ? 'primary' : 'secondary'} busy={answering === source.event_id} disabled={answering !== null} onclick={() => answer(source, response)}>{t(label)}</Button>
                    {/each}
                  </div>
                {/if}
              {/each}
              {#if answerFailure}
                <p data-testid="calendar-answer-failure" class="text-[13px] text-error-500">{answerFailure}</p>
              {/if}
            </div>
          </div>
        {/if}
        {#if join}
          <div class={line}>
            <span class={key}>{t('calendar.detail.join')}</span>
            <Button testid="calendar-join" size="sm" variant="secondary" icon="lucide:video" onclick={() => openUrl(join as string).catch(() => {})}>{join.includes('meet.google') ? t('calendar.detail.joinMeet') : join.includes('teams.microsoft') ? t('calendar.detail.joinTeams') : t('calendar.detail.joinPlain')}</Button>
          </div>
        {/if}
        {#if event.location}
          <div class={line}>
            <span class={key}>{t('calendar.detail.where')}</span>
            <span class="text-[14px] text-ink">{event.location}</span>
          </div>
        {/if}
        {#if event.attendees.length > 0}
          <div class={line}>
            <span class={key}>{t('calendar.detail.with')}</span>
            <span class="font-mono text-[12px] leading-5 text-ink">{event.attendees.join(', ')}</span>
          </div>
        {/if}
        {#if event.notes}
          <div class={line}>
            <span class={key}>{t('calendar.detail.notes')}</span>
            <span class="whitespace-pre-wrap text-[14px] leading-6 text-ink-2">{event.notes}</span>
          </div>
        {/if}
        {#if event.conversation_id}
          <div class={line}>
            <span class={key}>{t('calendar.detail.from')}</span>
            <button data-testid="calendar-detail-conversation" class="text-[14px] text-accent hover:underline" onclick={() => openConversation(event.conversation_id as string)}>{t('calendar.detail.openConversation')}</button>
          </div>
        {/if}
      </div>
    </div>
    {#snippet footer()}
      {#if local}
        <Button testid="calendar-detail-delete" variant="danger" icon="lucide:trash-2" onclick={() => ondelete(event as CalendarEvent)}>{t('common.delete')}</Button>
      {/if}
      {#if event.sources.length > 1}
        <Button testid="calendar-detail-unmerge" icon="lucide:split" onclick={() => onunmerge(event as CalendarEvent)}>{t('calendar.detail.showSeparately')}</Button>
      {/if}
      <span class="ml-auto"></span>
      {#if event.status !== 'dismissed'}
        <Button testid="calendar-detail-dismiss" onclick={() => onstatus(event as CalendarEvent, 'dismissed')}>{t('common.dismiss')}</Button>
      {/if}
      {#if event.status !== 'confirmed'}
        <Button testid="calendar-detail-confirm" variant="primary" onclick={() => onstatus(event as CalendarEvent, 'confirmed')}>{t('common.confirm')}</Button>
      {:else}
        <Button testid="calendar-detail-close" onclick={onclose}>{t('common.close')}</Button>
      {/if}
    {/snippet}
  </Sheet>
{/if}
