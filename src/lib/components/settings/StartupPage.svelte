<script lang="ts">
  import { onMount } from 'svelte'
  import { conversationsStore, groupsStore, tagRecordsStore } from '$lib/tauri'
  import { userInboxes } from '$lib/inboxes'
  import { VIEW_TITLES, type InboxView } from '$lib/inbox'
  import { Row, Segmented, Select } from '$lib/components/ui'
  import { getCachedStartup, inboxRoute, INBOX_ROUTE, loadStartupSettings, putStartupPage, STARTUP_LAST } from '$lib/startup'
  import { featureShown } from '$lib/settings'
  import { t, type MessageKey } from '$lib/i18n.svelte'

  interface Choice {
    route: string
    label: string
    icon: string
    color?: string | null
  }

  interface Part {
    title: MessageKey
    items: Choice[]
  }

  const VIEW_ICONS: Record<Exclude<InboxView, 'inbox'>, string> = { notifications: 'lucide:bell', done: 'lucide:circle-check', snoozed: 'lucide:clock', deleted: 'lucide:trash-2', pipeline: 'lucide:git-pull-request-arrow' }

  let choice = $state(getCachedStartup()?.choice ?? STARTUP_LAST)
  let specific = $state(false)
  let picking = $state(false)
  let failure = $state<string | null>(null)

  const mode = $derived(specific || choice !== STARTUP_LAST ? 'page' : 'last')

  onMount(() => {
    loadStartupSettings().then((settings) => (choice = settings.choice)).catch(() => {})
  })

  const chats = $derived(
    $conversationsStore.data
      .filter((conversation) => !conversation.deleted_at)
      .sort((a, b) => b.last_activity.localeCompare(a.last_activity))
  )

  const parts = $derived<Part[]>([
    {
      title: 'settings.startup.inboxes',
      items: [{ route: INBOX_ROUTE, label: t('inbox.view.inbox'), icon: 'lucide:inbox' }, ...$userInboxes.map((inbox) => ({ route: inboxRoute('inbox', inbox.id), label: inbox.name, icon: inbox.icon ?? 'lucide:inbox' }))]
    },
    {
      title: 'settings.startup.views',
      items: (Object.keys(VIEW_ICONS) as (keyof typeof VIEW_ICONS)[]).map((view) => ({ route: inboxRoute('view', view), label: t(VIEW_TITLES[view]), icon: VIEW_ICONS[view] }))
    },
    {
      title: 'settings.startup.pages',
      items: [
        { route: '/main/actions', label: t('app.rail.actions'), icon: 'lucide:workflow' },
        { route: '/main/content', label: t('content.title'), icon: 'lucide:library' },
        { route: '/main/calendar', label: t('app.rail.calendar'), icon: 'lucide:calendar' },
        { route: '/main/stats', label: t('app.rail.statistics'), icon: 'lucide:chart-no-axes-column' },
        { route: '/main/cases', label: t('app.rail.cases'), icon: 'lucide:briefcase' },
        { route: '/main/profile', label: t('common.settings'), icon: 'lucide:settings' }
      ].filter((item) => featureShown(item.route))
    },
    { title: 'settings.startup.groups', items: $groupsStore.map((group) => ({ route: inboxRoute('group', group.id), label: group.name, icon: group.icon || 'lucide:folder' })) },
    { title: 'settings.startup.tags', items: $tagRecordsStore.filter((tag) => tag.kind === 'tag').map((tag) => ({ route: inboxRoute('tag', tag.id), label: tag.name, icon: 'lucide:tag', color: tag.color })) },
    { title: 'settings.startup.chats', items: chats.filter((conversation) => conversation.mode !== 'read').map((conversation) => ({ route: `${INBOX_ROUTE}/${conversation.id}`, label: conversation.name, icon: 'lucide:message-square' })) },
    { title: 'settings.startup.readChats', items: chats.filter((conversation) => conversation.mode === 'read').map((conversation) => ({ route: `${INBOX_ROUTE}/${conversation.id}`, label: conversation.name, icon: 'lucide:book-open' })) }
  ])

  const choices = $derived(parts.flatMap((part) => part.items.map((item) => ({ value: item.route, label: item.label, icon: item.icon, color: item.color, group: t(part.title) }))))
  const known = $derived(choices.some((item) => item.value === choice))

  async function put(next: string) {
    try {
      choice = await putStartupPage(next)
      failure = null
    } catch (error) {
      failure = String(error)
    }
  }

  function pickMode(value: string) {
    specific = value === 'page'
    if (value === 'last') put(STARTUP_LAST)
    else {
      if (choice === STARTUP_LAST) put(getCachedStartup()?.last ?? INBOX_ROUTE)
      picking = true
    }
  }
</script>

<Row id="general.startup-page" label={t('settings.startup.label')} hint={t('settings.startup.hint')}>
  <Segmented label={t('settings.startup.label')} size="sm" value={mode} options={[{ value: 'last', label: t('settings.startup.last') }, { value: 'page', label: t('settings.startup.specific') }]} onchange={pickMode} />
</Row>
{#if mode === 'page'}
  <Row label={t('settings.startup.page')} hint={t('settings.startup.find')}>
    <Select size="sm" class="w-[clamp(12rem,40%,20rem)]" testid="startup-choice" dialog={t('settings.startup.label')} placeholder={t('settings.startup.gone')} value={known ? choice : ''} options={choices} bind:open={picking} onchange={put} />
  </Row>
{/if}
{#if failure}
  <p class="py-2 text-[13px] text-error-600">{failure}</p>
{/if}
