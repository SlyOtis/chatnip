<script lang="ts">
  import { api, peopleById, peopleStore, loadPeople, identityInput, identityLabel, mergeCandidates, personInitials } from '$lib/tauri'
  import type { Person } from '$lib/tauri/types'
  import Icon from '@iconify/svelte'
  import { Button, Field, IconButton, Section, Segmented, Select, Sheet } from '$lib/components/ui'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'
  import { goto } from '$app/navigation'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    personId: string
  }

  let { personId }: Props = $props()

  const PROVIDERS = $derived([
    { value: 'email', label: t('conversation.identities.anyMail') },
    { value: 'microsoft', label: 'Microsoft 365' },
    { value: 'google', label: 'Google' },
    { value: 'slack', label: 'Slack' },
    { value: 'imap', label: 'IMAP' }
  ])

  let provider = $state('email')
  let value = $state('')
  let name = $state('')
  let query = $state('')
  let adding = $state<'address' | 'merge' | null>(null)
  let busy = $state(false)
  let failure = $state('')

  const person = $derived($peopleById.get(personId) ?? null)
  const candidates = $derived(adding === 'merge' ?mergeCandidates($peopleStore, personId, query) : [])
  const draft = $derived(identityInput(personId, provider, value, name))

  async function run(action: () => Promise<unknown>) {
    busy = true
    failure = ''
    try {
      await action()
      await loadPeople()
    } catch (e) {
      failure = String(e)
    } finally {
      busy = false
    }
  }

  function add() {
    const input = draft
    if (!input) return
    run(async () => {
      await api.postPersonIdentity(input)
      close()
    })
  }

  function close() {
    adding = null
    value = ''
    name = ''
    query = ''
    failure = ''
  }

  function remove(identityId: string) {
    run(() => api.deletePersonIdentity(identityId))
  }

  function kind(next: string) {
    run(() => api.putPerson({ person_id: personId, is_human: next === 'person' }))
  }

  function merge(source: Person) {
    run(async () => {
      await api.postPersonMerge(source.id, personId)
      close()
    })
  }
</script>

{#if person}
  <Section title={t('conversation.identities.title')} description={t('conversation.identities.description', { name: person.display_name })}>
    <div class="flex flex-col gap-4">
      <Button size="sm" variant="secondary" icon="lucide:id-card" onclick={() => goto(`/main/person/${encodeURIComponent(personId)}/profile`)}>{t('conversation.identities.openProfile')}</Button>
      <Segmented label={t('conversation.identities.kind')} value={person.is_human ? 'person' : 'entity'} options={[{ value: 'person', label: t('conversation.identities.person'), icon: 'lucide:user' }, { value: 'entity', label: t('conversation.identities.entity'), icon: 'lucide:building-2' }]} onchange={kind} />

      <ul class="flex flex-col">
        {#each person.identities as identity (identity.id)}
          {@const label = identityLabel(identity)}
          <li class="group grid min-w-0 grid-cols-[1.5rem_minmax(0,1fr)_auto] items-center gap-2.5 border-t border-line py-2 first:border-t-0">
            <span class="flex items-center justify-center"><ProviderIcon providers={[{ ...identity, conversation_type: identity.email ? 'email' : 'direct_message' }]} size="md" max={1} /></span>
            <span class="flex min-w-0 flex-col">
              <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[13.5px] text-ink">{label.primary}</span>
              <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[12px] text-ink-3">{label.source}{label.secondary ? ` · ${label.secondary}` : ''}</span>
            </span>
            <IconButton icon="lucide:x" label={t('conversation.identities.removeFrom', { name: person.display_name })} size="sm" disabled={busy || person.identities.length < 2} onclick={() => remove(identity.id)} />
          </li>
        {/each}
      </ul>

      <Button testid="identities-add" size="sm" variant="secondary" icon="lucide:plus" onclick={() => (adding = 'address')}>{t('common.add')}</Button>

      {#if failure && !adding}
        <p class="text-[12.5px] text-error-600">{failure}</p>
      {/if}
    </div>
  </Section>

  {#if adding}
    <Sheet testid="identities-dialog" label={t('conversation.identities.addTitle', { name: person.display_name })} width="30rem" onclose={close}>
      <div class="flex flex-col gap-5 px-5 pb-5 pt-5">
        <header class="flex items-center gap-3">
          <span class="flex size-10 shrink-0 items-center justify-center rounded-[28%] bg-paper-3 text-[17px] text-ink-2"><Icon icon={adding === 'address' ? 'lucide:at-sign' : 'lucide:merge'} /></span>
          <div class="flex min-w-0 flex-col">
            <h2 class="overflow-hidden text-ellipsis whitespace-nowrap text-[16px] font-semibold tracking-tight text-ink">{t('conversation.identities.addTitle', { name: person.display_name })}</h2>
            <p class="text-[12.5px] text-ink-3">{adding === 'address' ? t('conversation.identities.addressHint') : t('conversation.identities.mergeHint')}</p>
          </div>
        </header>

        <Segmented testid="identities-mode" size="sm" label={t('conversation.identities.title')} value={adding} options={[{ value: 'address', label: t('conversation.identities.address'), icon: 'lucide:at-sign' }, { value: 'merge', label: t('conversation.identities.merge'), icon: 'lucide:merge' }]} onchange={(next) => { adding = next as 'address' | 'merge'; failure = '' }} />

        {#if adding === 'address'}
          <form class="flex flex-col gap-3" onsubmit={(e) => { e.preventDefault(); add() }}>
            <div class="grid grid-cols-[minmax(0,2fr)_minmax(0,1fr)] gap-2">
              <Field data-testid="identities-address" bind:value placeholder={t('conversation.identities.addressPlaceholder')} aria-label={t('conversation.identities.address')} autofocus />
              <Select value={provider} options={PROVIDERS} onchange={(next) => (provider = next)} />
            </div>
            <Field data-testid="identities-label" bind:value={name} size="sm" placeholder={t('conversation.identities.labelPlaceholder')} aria-label={t('conversation.identities.label')} />
            {#if draft}
              <div class="flex min-w-0 flex-col rounded-[0.6rem] bg-paper-2 px-3 py-2">
                <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[13.5px] text-ink">{value.trim()}</span>
                <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[12px] text-ink-3">{PROVIDERS.find((p) => p.value === provider)?.label}{name.trim() ? ` · ${name.trim()}` : ''}</span>
              </div>
            {/if}
          </form>
        {:else}
          <div class="flex min-h-0 flex-col gap-2">
            <Field data-testid="identities-merge-search" bind:value={query} placeholder={t('conversation.identities.searchPlaceholder')} aria-label={t('conversation.identities.mergeTarget')} autofocus />
            <div class="flex max-h-[min(40vh,20rem)] flex-col overflow-y-auto">
              {#each candidates as candidate (candidate.id)}
                <button type="button" data-testid="identities-merge-candidate" class="grid cursor-pointer grid-cols-[1.75rem_minmax(0,1fr)_auto] items-center gap-2.5 rounded-[0.5rem] px-2 py-2 text-left hover:bg-paper-2" onclick={() => merge(candidate)} disabled={busy}>
                  <span class="flex size-7 items-center justify-center rounded-full bg-paper-3 text-[10.5px] font-semibold text-ink-2">{personInitials(candidate.display_name)}</span>
                  <span class="flex min-w-0 flex-col">
                    <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[13.5px] text-ink">{candidate.display_name}</span>
                    <span class="overflow-hidden text-ellipsis whitespace-nowrap text-[12px] text-ink-3">{candidate.primary_email ?? t('conversation.overview.identities', { count: candidate.identities.length })}</span>
                  </span>
                  <Icon icon="lucide:merge" class="text-[13px] text-ink-3" />
                </button>
              {/each}
              {#if query.trim() && candidates.length === 0}
                <p class="px-2 py-2 text-[12.5px] text-ink-3">{t('conversation.identities.noMatch', { query: query.trim() })}</p>
              {/if}
            </div>
          </div>
        {/if}

        {#if failure}<p data-testid="identities-error" class="text-[12.5px] text-error-600">{failure}</p>{/if}
      </div>
      {#snippet footer()}
        <Button testid="identities-cancel" class="ml-auto" onclick={close}>{t('common.cancel')}</Button>
        {#if adding === 'address'}
          <Button testid="identities-submit" variant="primary" icon="lucide:plus" disabled={!draft} {busy} onclick={add}>{t('common.add')}</Button>
        {/if}
      {/snippet}
    </Sheet>
  {/if}
{/if}
