<script lang="ts">
  import Icon from '@iconify/svelte'
  import type { CalendarEvent } from './types'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { eventColour, joinUrl, sourceMarks } from '$lib/calendar'
  import { formatDate, t } from '$lib/i18n.svelte'

  interface Props {
    event: CalendarEvent
    compact: boolean
    part?: 'single' | 'first' | 'middle' | 'last'
    onclick: () => void
  }

  let { event, compact, part = 'single', onclick }: Props = $props()

  const proposed = $derived(event.status === 'proposed')
  const colour = $derived(eventColour(event) ?? 'var(--accent)')
  const marks = $derived(sourceMarks(event.sources))
  const online = $derived(!!joinUrl(event))
  const fill = $derived(`linear-gradient(135deg, color-mix(in srgb, ${colour} 78%, var(--paper-3)), color-mix(in srgb, ${colour} 46%, var(--paper-2)))`)

  function formatTime(iso: string): string {
    return formatDate(iso, { hour: '2-digit', minute: '2-digit' })
  }

  function pick(e: MouseEvent) {
    e.stopPropagation()
    onclick()
  }
</script>

<button
  type="button"
  data-testid="calendar-event"
  data-id={event.id}
  data-status={event.status}
  data-compact={compact ? '1' : undefined}
  class="bar part-{part} {proposed ? 'is-proposed' : ''}"
  style:--bar-colour={colour}
  title={event.title}
  onclick={pick}
>
  {#if !proposed}<span class="bar-fill" style:background={fill} aria-hidden="true"></span>{/if}
  <span class="bar-content">
    {#if compact}
      <span class="bar-compact">{#if !event.all_day}<span class="bar-time">{formatTime(event.starts_at)}</span>{/if}{event.title}</span>
    {:else}
      <span class="bar-primary">
        <span class="bar-title">{event.title}</span>
        {#if online}<Icon icon="lucide:video" class="bar-glyph" />{/if}
        <ProviderIcon providers={marks} size="xs" max={3} />
      </span>
      <span class="bar-secondary">{event.all_day ? t('calendar.allDay') : `${formatTime(event.starts_at)} – ${formatTime(event.ends_at)}`}</span>
      {#if event.location}
        <span class="bar-secondary bar-place"><Icon icon="lucide:map-pin" class="bar-glyph" />{event.location}</span>
      {/if}
    {/if}
  </span>
</button>

<style>
  .bar {
    position: relative;
    display: block;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    padding: 0;
    border: 0;
    border-radius: 0.375rem;
    background: var(--paper-2);
    color: var(--ink);
    font: inherit;
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    user-select: none;
    transition:
      filter 0.1s ease,
      box-shadow 0.15s ease;
  }
  .bar:hover {
    filter: brightness(1.06);
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.18);
  }
  .bar:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .bar.part-first {
    border-bottom-left-radius: 0;
    border-bottom-right-radius: 0;
  }
  .bar.part-middle {
    border-radius: 0;
  }
  .bar.part-last {
    border-top-left-radius: 0;
    border-top-right-radius: 0;
  }
  .bar.is-proposed {
    background: transparent;
    outline: 1px dashed var(--line-strong);
    outline-offset: -1px;
    color: var(--ink-2);
  }
  .bar-fill {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .bar-content {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    gap: 0.05rem;
    padding: 0.25rem 0.5rem;
    overflow: hidden;
    pointer-events: none;
    text-shadow: 0 1px 2px rgb(0 0 0 / 0.35);
  }
  .bar[data-compact] .bar-content {
    flex-direction: row;
    align-items: center;
    gap: 0.35rem;
    padding: 0 0.4rem;
  }
  .bar-primary {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    min-width: 0;
    font-size: 0.8125rem;
    font-weight: 700;
    letter-spacing: 0.01em;
    line-height: 1.2;
  }
  .bar-title {
    min-width: 0;
    flex: 1 1 auto;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar-secondary {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    font-weight: 500;
    opacity: 0.88;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }
  .bar-place {
    font-family: inherit;
  }
  .bar-compact {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    min-width: 0;
    font-size: 0.6875rem;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar-time {
    font-family: var(--font-mono);
    opacity: 0.85;
  }
  .bar :global(.bar-glyph) {
    flex: 0 0 auto;
    font-size: 0.75rem;
  }
  @container (max-height: 2.6rem) {
    .bar-secondary {
      display: none;
    }
  }
  @container (max-height: 3.6rem) {
    .bar-place {
      display: none;
    }
  }
</style>
