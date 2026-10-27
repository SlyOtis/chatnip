<script lang="ts">
  import Icon from '@iconify/svelte'
  import type { PostEventInput, ProviderCalendar } from '$lib/tauri/types'
  import { Button, Field, Kbd, Sheet, Switch } from '$lib/components/ui'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { connections, defaultTargets, LOCAL_TARGET, meetingKinds, meetingLabel } from '$lib/calendar'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    initial_date: Date
    visible: boolean
    calendars: ProviderCalendar[]
    onclose: () => void
    oncreate: (input: PostEventInput) => void
  }

  let { initial_date, visible, calendars, onclose, oncreate }: Props = $props()

  let targets = $state<string[]>([])
  let online_meeting = $state(false)
  const writable = $derived(calendars.filter((c) => c.can_write))
  const groups = $derived(connections(writable))
  const kinds = $derived(meetingKinds(writable, targets))
  const localOn = $derived(targets.includes(LOCAL_TARGET))

  function toggleTarget(id: string) {
    targets = targets.includes(id) ? targets.filter((target) => target !== id) : [...targets, id]
  }

  let title = $state('')
  let notes = $state('')
  let location = $state('')
  let all_day = $state(false)
  let start_date = $state('')
  let start_time = $state('')
  let end_date = $state('')
  let end_time = $state('')
  let attendee_input = $state('')
  let attendees = $state<string[]>([])

  function localDate(date: Date): string {
    const y = date.getFullYear()
    const m = (date.getMonth() + 1).toString().padStart(2, '0')
    const d = date.getDate().toString().padStart(2, '0')
    return `${y}-${m}-${d}`
  }

  $effect(() => {
    if (visible && initial_date) {
      const hours = initial_date.getHours().toString().padStart(2, '0')
      const minutes = initial_date.getMinutes().toString().padStart(2, '0')
      start_date = localDate(initial_date)
      start_time = `${hours}:${minutes}`
      end_date = start_date
      end_time = `${((initial_date.getHours() + 1) % 24).toString().padStart(2, '0')}:${minutes}`
      targets = defaultTargets(writable)
      online_meeting = false
    }
  })

  function addAttendee() {
    const email = attendee_input.trim()
    if (email && email.includes('@') && !attendees.includes(email)) {
      attendees = [...attendees, email]
      attendee_input = ''
    }
  }

  function removeAttendee(email: string) {
    attendees = attendees.filter((a) => a !== email)
  }

  function handleAttendeeKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault()
      addAttendee()
    }
  }

  function handleSubmit() {
    if (!title.trim() || targets.length === 0) return
    const starts = all_day ? new Date(`${start_date}T00:00:00`) : new Date(`${start_date}T${start_time}:00`)
    const ends = all_day ? new Date(`${end_date}T23:59:00`) : new Date(`${end_date}T${end_time}:00`)
    oncreate({ draft: { title: title.trim(), notes: notes.trim(), location: location.trim(), starts_at: starts.toISOString(), ends_at: ends.toISOString(), all_day, attendees }, targets, online_meeting: online_meeting && kinds.length > 0 })
    resetForm()
    onclose()
  }

  function resetForm() {
    title = ''
    notes = ''
    location = ''
    all_day = false
    attendee_input = ''
    attendees = []
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) handleSubmit()
  }

  const line = 'flex items-center gap-3 border-b border-line py-2'
  const key = 'w-14 shrink-0 font-mono text-[11px] text-ink-3'
</script>

{#if visible}
  <Sheet testid="calendar-create-sheet" label={t('calendar.create.title')} width="30rem" onclose={onclose}>
    <div class="px-5 pb-3 pt-5" onkeydown={handleKeydown} role="presentation">
      <input data-testid="calendar-create-title" bind:value={title} type="text" placeholder={t('calendar.create.eventTitle')} class="w-full border-b border-line bg-transparent pb-2 text-[15px] font-medium text-ink outline-none placeholder:text-ink-3 focus:border-accent" />
      <div class={line}>
        <span class={key}>{t('calendar.detail.when')}</span>
        <Field data-testid="calendar-create-start-date" bind:value={start_date} type="date" size="sm" mono class="w-36" />
        {#if !all_day}
          <Field data-testid="calendar-create-start-time" bind:value={start_time} type="time" size="sm" mono class="w-24" />
        {/if}
        <span class="text-ink-3">–</span>
        <Field data-testid="calendar-create-end-date" bind:value={end_date} type="date" size="sm" mono class="w-36" />
        {#if !all_day}
          <Field data-testid="calendar-create-end-time" bind:value={end_time} type="time" size="sm" mono class="w-24" />
        {/if}
      </div>
      <div class={line}>
        <span class={key}>{t('calendar.create.allDayKey')}</span>
        <Switch testid="calendar-create-all-day" bind:checked={all_day} label={t('calendar.allDay')} />
      </div>
      <div class={line}>
        <span class={key}>{t('calendar.detail.where')}</span>
        <input data-testid="calendar-create-location" bind:value={location} type="text" placeholder={t('calendar.create.location')} class="flex-1 bg-transparent text-[14px] text-ink outline-none placeholder:text-ink-3" />
      </div>
      <div class={line}>
        <span class={key}>{t('calendar.detail.with')}</span>
        <div class="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">
          {#each attendees as email (email)}
            <button type="button" data-testid="calendar-create-attendee" data-email={email} class="inline-flex items-center gap-1 font-mono text-[12px] text-ink hover:text-error-600" onclick={() => removeAttendee(email)} aria-label={t('calendar.create.removeAttendee', { email })}>
              {email}<Icon icon="lucide:x" class="text-[12px] text-ink-3" />
            </button>
          {/each}
          <input data-testid="calendar-create-attendee-input" bind:value={attendee_input} type="email" placeholder={attendees.length === 0 ? t('calendar.create.email') : ''} onkeydown={handleAttendeeKeydown} onblur={addAttendee} class="min-w-[8rem] flex-1 bg-transparent text-[14px] text-ink outline-none placeholder:text-ink-3" />
        </div>
      </div>
      <textarea data-testid="calendar-create-notes" bind:value={notes} placeholder={t('calendar.create.notes')} rows="3" class="mt-2 w-full resize-none bg-transparent py-1 text-[14px] leading-6 text-ink outline-none placeholder:text-ink-3"></textarea>
      <div class="mt-2 border-t border-line pt-3">
        <div class="mb-1.5 flex items-center gap-3">
          <span class={key}>{t('calendar.create.landsIn')}</span>
          <span class="text-[12.5px] text-ink-3">{targets.length > 1 ? t('calendar.create.linked', { count: targets.length }) : t('calendar.create.pick')}</span>
        </div>
        <div class="flex flex-col">
          {#each groups as group (group.provider_id)}
            <div class="flex items-center gap-2 pb-0.5 pt-1.5 text-[12px] text-ink-3">
              <ProviderIcon providers={[group.mark]} conversationType="email" />
              <span class="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{group.name}</span>
            </div>
            {#each group.calendars as calendar (calendar.id)}
              {@const on = targets.includes(calendar.id)}
              <button type="button" role="checkbox" data-testid="calendar-create-target" data-id={calendar.id} aria-checked={on} class="flex h-8 items-center gap-2.5 rounded-[0.5rem] px-2 text-left text-[14px] text-ink hover:bg-paper-2" onclick={() => toggleTarget(calendar.id)}>
                <span class="flex size-4 shrink-0 items-center justify-center rounded-[0.25rem] {on ? 'bg-accent text-accent-ink' : 'hairline'}">
                  {#if on}<Icon icon="lucide:check" class="text-[12px]" />{/if}
                </span>
                <span class="size-2 shrink-0 rounded-full" style:background={calendar.colour ?? 'var(--line-strong)'}></span>
                <span class="min-w-0 flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{calendar.name}</span>
                {#if calendar.is_primary}<span class="font-mono text-[11px] text-ink-3">{t('calendar.create.primary')}</span>{/if}
              </button>
            {/each}
          {/each}
          <button type="button" role="checkbox" data-testid="calendar-create-target" data-id={LOCAL_TARGET} aria-checked={localOn} class="mt-1 flex h-8 items-center gap-2.5 rounded-[0.5rem] px-2 text-left text-[14px] text-ink hover:bg-paper-2" onclick={() => toggleTarget(LOCAL_TARGET)}>
            <span class="flex size-4 shrink-0 items-center justify-center rounded-[0.25rem] {localOn ? 'bg-accent text-accent-ink' : 'hairline'}">
              {#if localOn}<Icon icon="lucide:check" class="text-[12px]" />{/if}
            </span>
            <Icon icon="lucide:hard-drive" class="shrink-0 text-[13px] text-ink-3" />
            <span class="flex-1">{t('calendar.create.local')}</span>
          </button>
        </div>
        {#if kinds.length > 0}
          <div class="mt-2 flex items-center gap-3 border-t border-line pt-2">
            <span class={key}>{t('calendar.create.online')}</span>
            <Icon icon={kinds[0] === 'meet' && kinds.length === 1 ? 'logos:google-meet' : 'logos:microsoft-teams'} class="shrink-0 text-[14px] grayscale" />
            <span class="flex-1 text-[14px] text-ink">{meetingLabel(kinds)}</span>
            <Switch testid="calendar-create-online" bind:checked={online_meeting} label={meetingLabel(kinds)} />
          </div>
        {/if}
        {#if attendees.length > 0 && targets.some((id) => id !== LOCAL_TARGET)}
          <p class="mt-2 text-[12.5px] leading-5 text-ink-3">{t(targets.filter((id) => id !== LOCAL_TARGET).length === 1 ? 'calendar.create.invitesOne' : 'calendar.create.invitesEach', { to: attendees.length === 1 ? attendees[0] : t('common.people', { count: attendees.length }) })}</p>
        {/if}
      </div>
    </div>
    {#snippet footer()}
      <span class="flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="esc" /> {t('calendar.create.close')}</span>
      <Button testid="calendar-create-cancel" class="ml-auto" onclick={onclose}>{t('common.cancel')}</Button>
      <Button testid="calendar-create-submit" variant="primary" keys="⌘↵" disabled={!title.trim() || targets.length === 0} onclick={handleSubmit}>{attendees.length > 0 && targets.some((id) => id !== LOCAL_TARGET) ? t('calendar.create.createInvite') : t('common.create')}</Button>
    {/snippet}
  </Sheet>
{/if}
