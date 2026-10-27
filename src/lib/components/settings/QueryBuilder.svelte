<script lang="ts">
  import Icon from '@iconify/svelte'
  import { api, tagRecordsStore, peopleStore, providersStore } from '$lib/tauri'
  import type { ConversationType, GroupQuery } from '$lib/tauri/types'
  import { GROUP_SORTS, describeQuery, serializeQuery, validRegex } from '$lib/tauri/groups'
  import { providerLabel } from '$lib/providerIcons'
  import { Field, Segmented, Select, TagMark } from '$lib/components/ui'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  interface Props {
    query: GroupQuery
    onchange: (query: GroupQuery) => void
  }

  let { query, onchange }: Props = $props()

  const types: { value: ConversationType; label: MessageKey }[] = [
    { value: 'direct_message', label: 'settings.query.type.direct' },
    { value: 'group_message', label: 'settings.query.type.group' },
    { value: 'channel', label: 'settings.query.type.channel' },
    { value: 'email', label: 'settings.query.type.mail' }
  ]

  const providerTypes = $derived([...new Set($providersStore.data.map((p) => p.provider_type))])
  const regexError = $derived(validRegex(query.regex))
  const summary = $derived(describeQuery(serializeQuery(query), $tagRecordsStore))

  let preview = $state<number | null>(null)
  let previewing = $state(false)
  let previewFailure = $state<string | null>(null)
  let timer: ReturnType<typeof setTimeout> | undefined

  function update(patch: Partial<GroupQuery>) {
    onchange({ ...query, ...patch })
  }

  function toggle<T>(list: T[] | undefined, value: T): T[] {
    const current = list ?? []
    return current.includes(value) ? current.filter((v) => v !== value) : [...current, value]
  }

  $effect(() => {
    const snapshot = serializeQuery(query)
    if (regexError) return
    clearTimeout(timer)
    timer = setTimeout(async () => {
      previewing = true
      try {
        preview = (await api.getQueryConversations(snapshot)).length
        previewFailure = null
      } catch (error) {
        preview = null
        previewFailure = String(error)
      } finally {
        previewing = false
      }
    }, 350)
    return () => clearTimeout(timer)
  })

  const pick = 'h-7 rounded-[0.5rem] px-2 text-[13px]'
  function pickState(on: boolean): string {
    return on ? 'bg-accent-soft font-medium text-accent' : 'text-ink-2 hover:bg-paper-3 hover:text-ink'
  }
</script>

<div class="flex flex-col divide-y divide-line">
  <div class="flex min-h-10 flex-wrap items-center gap-1 py-1.5">
    <span class="w-20 text-[13px] text-ink-3">{t('settings.query.source')}</span>
    {#each providerTypes as type (type)}
      {@const on = (query.providers ?? []).includes(type)}
      <button type="button" aria-pressed={on} class="{pick} {pickState(on)}" onclick={() => update({ providers: toggle(query.providers, type) })}>{providerLabel(type)}</button>
    {/each}
    {#if providerTypes.length === 0}
      <span class="text-[13px] text-ink-3">{t('settings.query.noConnections')}</span>
    {/if}
  </div>

  <div class="flex min-h-10 flex-wrap items-center gap-1 py-1.5">
    <span class="w-20 text-[13px] text-ink-3">{t('settings.group.kindLabel')}</span>
    {#each types as type (type.value)}
      {@const on = (query.types ?? []).includes(type.value)}
      <button type="button" aria-pressed={on} class="{pick} {pickState(on)}" onclick={() => update({ types: toggle(query.types, type.value) })}>{t(type.label)}</button>
    {/each}
  </div>

  <div class="flex min-h-10 flex-wrap items-center gap-1 py-1.5">
    <span class="w-20 text-[13px] text-ink-3">{t('settings.inbox.tags')}</span>
    {#each $tagRecordsStore as label (label.id)}
      {@const on = (query.tag_ids ?? []).includes(label.id)}
      <button type="button" aria-pressed={on} class="{pick} flex items-center gap-1.5 {pickState(on)}" onclick={() => update({ tag_ids: toggle(query.tag_ids, label.id) })}>
        <TagMark tag={label} size="xs" label ink={on ? 'ink' : 'ink-2'} />
      </button>
    {/each}
  </div>

  {#if $peopleStore.length > 0}
    <div class="flex min-h-10 flex-wrap items-center gap-1 py-1.5">
      <span class="w-20 text-[13px] text-ink-3">{t('settings.inbox.people')}</span>
      {#each $peopleStore.filter((p) => p.is_human).slice(0, 24) as person (person.id)}
        {@const on = (query.person_ids ?? []).includes(person.id)}
        <button type="button" aria-pressed={on} class="{pick} {pickState(on)}" onclick={() => update({ person_ids: toggle(query.person_ids, person.id) })}>{person.display_name}</button>
      {/each}
    </div>
  {/if}

  <div class="flex min-h-10 flex-wrap items-center gap-1 py-1.5">
    <span class="w-20 text-[13px] text-ink-3">{t('settings.query.only')}</span>
    <button type="button" aria-pressed={!!query.unread} class="{pick} {pickState(!!query.unread)}" onclick={() => update({ unread: !query.unread })}>{t('settings.query.unread')}</button>
    <button type="button" aria-pressed={!!query.has_attachments} class="{pick} {pickState(!!query.has_attachments)}" onclick={() => update({ has_attachments: !query.has_attachments })}>{t('settings.query.withAttachments')}</button>
    <button type="button" aria-pressed={!!query.humans_only} class="{pick} {pickState(!!query.humans_only)}" onclick={() => update({ humans_only: !query.humans_only })}>{t('settings.query.humansOnly')}</button>
  </div>

  <div class="grid grid-cols-2 gap-x-4 gap-y-2 py-3 sm:grid-cols-4">
    <label class="flex flex-col gap-1 text-[12px] text-ink-3">
      {t('settings.query.from')}
      <Field size="sm" mono placeholder={t('settings.query.datePlaceholder')} value={query.from ?? ''} onchange={(e) => update({ from: e.currentTarget.value || null })} />
    </label>
    <label class="flex flex-col gap-1 text-[12px] text-ink-3">
      {t('settings.query.to')}
      <Field size="sm" mono placeholder={t('settings.query.datePlaceholder')} value={query.to ?? ''} onchange={(e) => update({ to: e.currentTarget.value || null })} />
    </label>
    <label class="flex flex-col gap-1 text-[12px] text-ink-3 sm:col-span-2">
      {t('settings.query.sender')}
      <Field size="sm" placeholder={t('settings.query.senderPlaceholder')} value={query.sender ?? ''} oninput={(e) => update({ sender: e.currentTarget.value })} />
    </label>
    <label class="flex flex-col gap-1 text-[12px] text-ink-3 sm:col-span-2">
      {t('settings.query.text')}
      <Field size="sm" placeholder={t('settings.query.textPlaceholder')} value={query.text ?? ''} oninput={(e) => update({ text: e.currentTarget.value })} />
    </label>
    <label class="flex flex-col gap-1 text-[12px] text-ink-3 sm:col-span-2">
      <span class="flex items-center gap-2">
        Regex
        <span class="font-mono text-[11px] text-ink-3">{t('settings.query.regexFields')}</span>
      </span>
      <Field size="sm" mono class={regexError ? 'ring-2 ring-error-500' : ''} placeholder="^(invoice|faktura)\b" value={query.regex ?? ''} oninput={(e) => update({ regex: e.currentTarget.value })} spellcheck="false" />
      {#if regexError}
        <span class="text-[12px] text-error-500">{regexError}</span>
      {/if}
    </label>
  </div>

  <div class="flex min-h-10 flex-wrap items-center gap-2 py-1.5">
    <span class="w-20 text-[13px] text-ink-3">{t('settings.query.orderBy')}</span>
    <Select size="sm" value={query.sort} options={GROUP_SORTS.map((s) => ({ value: s.value, label: t(s.label) }))} onchange={(v) => update({ sort: v as GroupQuery['sort'] })} />
    <Segmented label={t('settings.query.direction')} size="sm" value={query.direction} options={[{ value: 'desc', label: t('settings.query.newest') }, { value: 'asc', label: t('settings.query.oldest') }]} onchange={(v) => update({ direction: v as 'asc' | 'desc' })} />
    <label class="flex items-center gap-2 text-[13px] text-ink-3">
      {t('settings.query.top')}
      <Field size="sm" mono type="number" min="0" class="w-16" value={query.limit == null ? '' : String(query.limit)} oninput={(e) => update({ limit: e.currentTarget.value ? Number(e.currentTarget.value) : null })} />
    </label>
  </div>

  <div class="flex h-10 items-center gap-2 text-[13px]">
    <Icon icon="lucide:filter" class="text-[14px] text-ink-3" />
    <span class="min-w-0 flex-1 truncate text-ink-2">{summary}</span>
    {#if previewing}
      <Icon icon="lucide:loader-circle" class="animate-spin text-[14px] text-ink-3" />
    {:else if previewFailure}
      <span class="font-mono text-[11px] text-error-500" title={previewFailure}>{t('settings.query.noPreview')}</span>
    {:else if preview !== null}
      <span class="font-mono text-[11px] text-accent">{t('settings.query.matches', { count: preview })}</span>
    {/if}
  </div>
</div>
