<script lang="ts">
  import { conversationHref } from '$lib/tauri/conversations'
  import {  } from '$lib/tauri/conversations'
  import Icon from '@iconify/svelte'
  import { goto } from '$app/navigation'
  import { onMount, untrack } from 'svelte'
  import { modalsStore, modalsClose, modalsShow } from '$lib/modals'
  import { api, conversationsById, conversationsStore, conversationStore, peopleStore, groupsStore, splitsStore, tagsStore, casesStore, openCasePicker, openNewChat, loadGroups, loadTags } from '$lib/tauri'
  import type { Action, CalendarEvent, CaseFile, ContentItem, Conversation, Group, Inbox, Message, Person, Tag } from '$lib/tauri/types'
  import { buildTree, flattenTree } from '$lib/tauri/groups'
  import { myUserIds, personEmail } from '$lib/tauri/people'
  import { chatAvatar, personAvatar } from '$lib/avatar'
  import { activeSplit, inboxView, inboxSheet, markDone, moveToGroup, openGroup, openInbox, openMessage, openTag, toggleUnread, type InboxView } from '$lib/inbox'
  import { addMembers, inboxesStore } from '$lib/inboxes'
  import { setThemeChoice } from '$lib/theme-mode'
  import { getShortcuts, formatShortcutKeys } from '$lib/shortcuts'
  import { contentSrc, isVisual } from '$lib/media'
  import { plainText } from '$lib/messageKind'
  import { formatDate, t, type MessageKey } from '$lib/i18n.svelte'
  import { renderEmojiText } from '$lib/emoji'
  import { SETTING_SECTIONS, featureShown, settingHref } from '$lib/settings'
  import { SCOPES, cycleScope, filterAt, filterCompletions, highlightParts, inScope, operatorAt, rank, scopeOf, searchFilters, snippetParts, uniqueTokens, visibleItems, withFilter, withoutFilter, withoutOperator, type GroupKind, type Scope, type SearchFilter, type SnippetPart, type Token } from '$lib/palette'
  import { Avatar, Button, Kbd, Segmented, Sheet, TagMark } from '$lib/components/ui'
  import ProviderIcon from '$lib/components/icons/ProviderIcon.svelte'

  interface Command {
    id: string
    label: string
    category: string
    icon: string
    keywords: string[]
    hint?: string
    run: () => void
  }

  type Base = { id: string; open: () => void; alt?: () => void; altLabel?: string }

  type Entry = Base &
    (
      | { kind: 'person'; person: Person }
      | { kind: 'conversation'; conversation: Conversation }
      | { kind: 'message'; message: Message; parts: SnippetPart[] }
      | { kind: 'action'; action: Action }
      | { kind: 'case'; item: CaseFile }
      | { kind: 'place'; label: string; icon: string; detail: string }
      | { kind: 'tag'; tag: Tag }
      | { kind: 'content'; content: ContentItem }
      | { kind: 'event'; event: CalendarEvent }
      | { kind: 'command'; command: Command }
      | { kind: 'suggestion'; token: Token; detail: string }
      | { kind: 'filter'; key: string; value: string }
      | { kind: 'provider'; label: string }
      | { kind: 'more'; label: string }
    )

  interface ResultGroup {
    key: GroupKind | 'recent' | 'suggestions' | 'provider'
    label: string
    icon: string
    entries: Entry[]
    hidden: number
  }

  let searchQuery = $state('')
  let tokens = $state<Token[]>([])
  let scope = $state<Scope>('all')
  let isSearching = $state(false)
  let conversationResults = $state<Conversation[]>([])
  let messageResults = $state<Message[]>([])
  let snippets = $state<Map<string, string>>(new Map())
  let contentResults = $state<ContentItem[]>([])
  let actionList = $state<Action[]>([])
  let events = $state<CalendarEvent[]>([])
  let selectedIndex = $state(0)
  let searchInput = $state<HTMLInputElement | null>(null)
  let debounceTimer: ReturnType<typeof setTimeout> | null = null
  let resultsContainer = $state<HTMLDivElement | null>(null)
  let drafting = $state(false)
  let promptKind = $state<'rule' | 'group' | 'split'>('rule')
  let draftInstruction = $state('')

  const prompts = {
    rule: { title: 'modals.prompt.rule.title', label: 'modals.prompt.rule.label', placeholder: 'modals.prompt.rule.placeholder', submit: 'modals.prompt.rule.submit', busy: 'modals.prompt.rule.busy' },
    group: { title: 'modals.prompt.group.title', label: 'modals.prompt.group.label', placeholder: 'modals.prompt.group.placeholder', submit: 'modals.prompt.group.submit', busy: 'modals.prompt.creating' },
    split: { title: 'modals.prompt.split.title', label: 'modals.prompt.split.label', placeholder: 'modals.prompt.split.placeholder', submit: 'modals.prompt.split.submit', busy: 'modals.prompt.creating' }
  } as const satisfies Record<string, Record<string, MessageKey>>
  let draftInput = $state<HTMLInputElement | null>(null)
  let draftBusy = $state(false)
  let draftFailure = $state<string | null>(null)

  const current = $derived($conversationStore.data)
  const filterOperator = $derived(filterAt(searchQuery))
  const operator = $derived(filterOperator ? null : operatorAt(searchQuery))
  const text = $derived(operator ? withoutOperator(searchQuery, operator) : filterOperator ? searchQuery.slice(0, filterOperator.start).trim() : searchQuery.trim())
  const parsedFilters = $derived(searchFilters(text))
  const words = $derived(parsedFilters.words)
  const filtering = $derived(!!text || tokens.length > 0)
  let providerResults = $state<Message[]>([])
  let providerQuery = $state('')
  let providerSearching = $state(false)
  const tagIds = $derived(tokens.filter((token) => token.kind === 'tag').map((token) => token.id))
  const scopedIds = $derived.by((): string[] | undefined => {
    const scoped = tokens.filter((token) => token.kind !== 'tag')
    if (scoped.length === 0) return undefined
    const ids = new Set<string>()
    for (const token of scoped) {
      if (token.kind === 'chat') ids.add(token.id)
      else for (const id of $peopleStore.find((person) => person.id === token.id)?.conversation_ids ?? []) ids.add(id)
    }
    return [...ids]
  })
  const splitName = $derived($splitsStore.find((split) => split.id === $activeSplit)?.name ?? null)

  onMount(() => {
    const within = $modalsStore.search?.conversation_id
    const scoped = within ? $conversationsById.get(within) : undefined
    if (scoped) tokens = [chatToken(scoped)]
    if ($modalsStore.search?.query) searchQuery = $modalsStore.search.query
    api.getActions().then((list) => (actionList = list)).catch(() => {})
    const now = Date.now()
    api.getEvents({ start: new Date(now - 90 * 86400000).toISOString(), end: new Date(now + 180 * 86400000).toISOString() }).then((list) => (events = list)).catch(() => {})
  })

  function view(key: InboxView) {
    modalsClose()
    inboxView.set(key)
    goto('/main/conversation')
  }

  function go(path: string) {
    modalsClose()
    goto(path)
  }

  function closing(run: () => unknown) {
    return () => {
      modalsClose()
      run()
    }
  }

  const commands = $derived.by((): Command[] => {
    const list: Command[] = [
      { id: 'view-inbox', label: t('modals.command.inbox'), category: t('modals.category.goTo'), icon: 'lucide:inbox', hint: 'G I', run: () => view('inbox'), keywords: ['inbox', 'home'] },
      { id: 'view-notifications', label: t('modals.command.notifications'), category: t('modals.category.goTo'), icon: 'lucide:bell', hint: 'G N', run: () => view('notifications'), keywords: ['notifications', 'unread', 'new'] },
      { id: 'view-pipeline', label: t('modals.command.pipeline'), category: t('modals.category.goTo'), icon: 'lucide:git-pull-request-arrow', hint: 'G P', run: () => view('pipeline'), keywords: ['pipeline', 'scheduled', 'waiting'] },
      { id: 'view-snoozed', label: t('modals.command.snoozed'), category: t('modals.category.goTo'), icon: 'lucide:clock', hint: 'G H', run: () => view('snoozed'), keywords: ['snoozed', 'later', 'reminders'] },
      { id: 'view-done', label: t('modals.command.done'), category: t('modals.category.goTo'), icon: 'lucide:check', hint: 'G E', run: () => view('done'), keywords: ['done', 'archive', 'archived'] },
      { id: 'view-deleted', label: t('modals.command.deleted'), category: t('modals.category.goTo'), icon: 'lucide:trash-2', hint: 'G D', run: () => view('deleted'), keywords: ['deleted', 'trash', 'bin', 'removed'] },
      { id: 'go-calendar', label: t('modals.command.calendar'), category: t('modals.category.goTo'), icon: 'lucide:calendar', hint: '⌘ ⇧ C', run: () => go('/main/calendar'), keywords: ['calendar', 'schedule', 'events', 'dates'] },
      { id: 'go-stats', label: t('modals.command.statistics'), category: t('modals.category.goTo'), icon: 'lucide:chart-no-axes-column', run: () => go('/main/stats'), keywords: ['statistics', 'stats', 'analytics', 'patterns'] },
      { id: 'go-actions', label: t('modals.command.actions'), category: t('modals.category.goTo'), icon: 'lucide:workflow', run: () => go('/main/actions'), keywords: ['actions', 'automation', 'workflow', 'rules'] },
      { id: 'go-activity', label: t('inbox.view.activity'), category: t('modals.category.goTo'), icon: 'lucide:at-sign', hint: '⌘ ⇧ A', run: () => go('/main/activity'), keywords: ['activity', 'mentions', 'reactions', 'replies'] },
      { id: 'go-saved', label: t('inbox.view.saved'), category: t('modals.category.goTo'), icon: 'lucide:bookmark', hint: 'G S', run: () => go('/main/saved'), keywords: ['saved', 'bookmarks', 'starred', 'flagged'] },
      { id: 'go-reminders', label: t('inbox.view.reminders'), category: t('modals.category.goTo'), icon: 'lucide:alarm-clock', hint: 'G M', run: () => go('/main/reminders'), keywords: ['reminders', 'remind', 'later'] },
      ...SETTING_SECTIONS.map((s): Command => ({ id: 'settings-' + s.id, label: t('modals.command.settings', { name: t(s.label) }), category: t('common.settings'), icon: s.icon, hint: s.id === 'general' ? '⌘ ,' : undefined, run: () => go(settingHref(s.id)), keywords: ['settings', ...s.keywords, ...s.settings.flatMap((r) => [r.id, ...('keywords' in r ? r.keywords : [])])] })),
      { id: 'new-group', label: t('modals.command.newGroup'), category: t('modals.category.organise'), icon: 'lucide:folder-plus', run: () => startDraft('group'), keywords: ['new', 'group', 'folder', 'nest', 'create'] },
      { id: 'new-split', label: t('modals.command.newSplit'), category: t('modals.category.organise'), icon: 'lucide:columns-2', run: () => startDraft('split'), keywords: ['new', 'split', 'tab', 'main tab', 'create'] },
      { id: 'new-chat', label: t('modals.command.newChat'), category: t('modals.category.organise'), icon: 'lucide:messages-square', run: closing(() => openNewChat({ sources: current ? [{ kind: 'conversation', id: current.id }] : [] })), keywords: ['new', 'chat', 'read', 'write', 'feed', 'digest', 'files', 'receipts', 'create'] },
      { id: 'new-action', label: t('modals.command.newAction'), category: t('modals.category.organise'), icon: 'lucide:workflow', run: () => go('/main/actions?new=1'), keywords: ['new', 'action', 'rule', 'automation', 'workflow', 'filter', 'create'] },
      { id: 'new-case', label: t('modals.command.newCase'), category: t('modals.category.organise'), icon: 'lucide:briefcase', run: closing(() => openCasePicker({ conversationIds: current ? [current.id] : [], messageIds: [], create: true })), keywords: ['new', 'case', 'matter', 'project', 'create'] },
      { id: 'sync-all', label: t('modals.command.syncAll'), category: t('modals.category.run'), icon: 'lucide:refresh-cw', run: closing(() => api.postSync()), keywords: ['sync', 'refresh', 'reload', 'fetch'] },
      { id: 'resort', label: t('modals.command.resort'), category: t('modals.category.run'), icon: 'lucide:play', run: closing(() => api.postActionBackfill()), keywords: ['sort', 'resort', 'rules', 'tags', 'classify', 'actions'] },
      { id: 'scan-events', label: t('modals.command.scanEvents'), category: t('modals.category.run'), icon: 'lucide:calendar-search', run: closing(() => api.postActionBackfill('find_appointments')), keywords: ['scan', 'events', 'dates', 'calendar', 'parse', 'appointments'] },
      { id: 'export-calendar', label: t('modals.command.exportCalendar'), category: t('modals.category.run'), icon: 'lucide:download', run: closing(() => api.getCalendarExport()), keywords: ['export', 'ics', 'calendar'] },
      { id: 'theme-system', label: t('modals.command.themeSystem'), category: t('modals.category.appearance'), icon: 'lucide:monitor', run: closing(() => setThemeChoice('system')), keywords: ['theme', 'system', 'auto', 'os'] },
      { id: 'theme-day', label: t('modals.command.themeDay'), category: t('modals.category.appearance'), icon: 'lucide:sun', run: closing(() => setThemeChoice('day')), keywords: ['theme', 'day', 'light'] },
      { id: 'theme-night', label: t('modals.command.themeNight'), category: t('modals.category.appearance'), icon: 'lucide:moon', run: closing(() => setThemeChoice('night')), keywords: ['theme', 'night', 'dark'] },
      { id: 'shortcuts', label: t('modals.command.shortcuts'), category: t('modals.category.help'), icon: 'lucide:keyboard', hint: '?', run: () => modalsShow({ shortcuts: true }), keywords: ['shortcuts', 'keyboard', 'keys', 'help'] }
    ]
    if (current) {
      const id = current.id
      list.unshift(
        { id: 'conv-done', label: t(current.is_archived ? 'modals.command.moveToInbox' : 'modals.command.markDone'), category: t('modals.category.conversation'), icon: 'lucide:check', hint: 'E', run: closing(() => markDone([id], !current.is_archived)), keywords: ['done', 'archive', 'finish'] },
        { id: 'conv-snooze', label: t('modals.command.snooze'), category: t('modals.category.conversation'), icon: 'lucide:clock', hint: 'H', run: closing(() => inboxSheet.set('snooze')), keywords: ['snooze', 'later', 'remind'] },
        { id: 'conv-tag', label: t('modals.command.tag'), category: t('modals.category.conversation'), icon: 'lucide:tag', hint: 'L', run: closing(() => inboxSheet.set('tag')), keywords: ['tag', 'label'] },
        { id: 'conv-unread', label: t(current.unread_count > 0 ? 'modals.command.markRead' : 'modals.command.markUnread'), category: t('modals.category.conversation'), icon: 'lucide:mail-open', hint: 'U', run: closing(() => toggleUnread([id], $conversationsStore.data)), keywords: ['unread', 'read', 'mark'] },
        { id: 'conv-rule', label: t('modals.prompt.rule.title'), category: t('modals.category.conversation'), icon: 'lucide:sparkles', run: () => startDraft('rule'), keywords: ['rule', 'action', 'sort', 'draft', 'assistant', 'automation'] }
      )
    }
    const hinted = new Set(list.map((command) => command.hint).filter(Boolean))
    for (const shortcut of getShortcuts()) {
      const keys = formatShortcutKeys(shortcut).join(' ')
      if (hinted.has(keys)) continue
      hinted.add(keys)
      list.push({ id: `shortcut-${keys}`, label: t(shortcut.description), category: t(shortcut.category), icon: 'lucide:keyboard', hint: keys, run: closing(() => shortcut.action()), keywords: ['shortcut', t(shortcut.category).toLowerCase()] })
    }
    return list.filter((command) => featureShown(command.id))
  })

  $effect(() => {
    const query = text
    const ids = scopedIds
    const tags = tagIds
    const typing = !!operator || !!filterOperator
    if (query !== untrack(() => providerQuery)) untrack(() => (providerResults = []))
    untrack(() => {
      if (debounceTimer) clearTimeout(debounceTimer)
      if (typing) return
      if (!query && !ids && tags.length === 0) {
        conversationResults = []
        messageResults = []
        snippets = new Map()
        contentResults = []
        isSearching = false
        return
      }
      isSearching = true
      debounceTimer = setTimeout(async () => {
        try {
          const [results, found] = await Promise.all([
            api.getSearch({ query, limit: 40, conversation_ids: ids, tag_ids: tags.length ? tags : undefined }),
            query ? api.getContent(undefined, query, 'relevance').catch(() => [] as ContentItem[]) : Promise.resolve([] as ContentItem[])
          ])
          conversationResults = results.conversations
          messageResults = results.messages
          snippets = new Map(results.snippets.map((s) => [s.message_id, s.snippet]))
          contentResults = found.filter((item) => item.kind !== 'emoji' && isVisual(item))
        } catch {
          conversationResults = []
          messageResults = []
          snippets = new Map()
          contentResults = []
        } finally {
          isSearching = false
        }
      }, 220)
    })
  })

  function addToken(token: Token) {
    tokens = uniqueTokens(tokens, token)
    if (operator) {
      const rest = withoutOperator(searchQuery, operator)
      searchQuery = rest ? rest + ' ' : ''
    }
    selectedIndex = 0
    searchInput?.focus()
  }

  function personToken(person: Person): Token {
    return { kind: 'person', id: person.id, label: person.display_name, icon: 'lucide:user' }
  }

  function chatToken(conversation: Conversation): Token {
    return { kind: 'chat', id: conversation.id, label: conversation.name, icon: conversation.type === 'email' ? 'lucide:mail' : 'lucide:message-circle' }
  }

  function tagToken(tag: Tag): Token {
    return { kind: 'tag', id: tag.id, label: tag.name, icon: tag.icon ?? 'lucide:tag' }
  }

  function jump(message: Message) {
    modalsClose()
    if (message.conversation_id) openMessage(message.conversation_id, message.id.startsWith('remote:') ? null : message.id)
  }

  function addFilter(value: string) {
    if (!filterOperator) return
    searchQuery = withFilter(searchQuery, filterOperator, value)
    selectedIndex = 0
    searchInput?.focus()
  }

  function dropFilter(filter: SearchFilter) {
    searchQuery = withoutFilter(searchQuery, filter) + ' '
    searchInput?.focus()
  }

  async function searchProvider() {
    const query = text
    if (!query || providerSearching) return
    providerSearching = true
    providerQuery = query
    try {
      providerResults = await api.getProviderMessages(query, scopedIds?.length === 1 ? scopedIds[0] : null)
    } catch {
      providerResults = []
    } finally {
      providerSearching = false
    }
  }

  function filterNames(key: string): string[] {
    if (key === 'from') return $peopleStore.flatMap((person) => [person.primary_email ?? '', person.display_name])
    if (key === 'in') return $conversationsStore.data.map((conversation) => conversation.name)
    return []
  }

  function navigateToConversation(id: string) {
    modalsClose()
    api.putCurrentConversation(id)
    goto(conversationHref(id))
  }

  function latestThread(person: Person): string | null {
    const ids = new Set(person.conversation_ids)
    return $conversationsStore.data.filter((c) => ids.has(c.id)).sort((a, b) => b.last_activity.localeCompare(a.last_activity))[0]?.id ?? null
  }

  function openPlace(run: () => void) {
    return () => {
      modalsClose()
      run()
      goto('/main/conversation')
    }
  }

  function personEntry(person: Person): Entry {
    const thread = latestThread(person)
    return { kind: 'person', id: 'person-' + person.id, person, open: () => go(`/main/person/${person.id}`), alt: thread ? () => navigateToConversation(thread) : undefined, altLabel: t('modals.alt.openThread') }
  }

  function conversationEntry(conversation: Conversation): Entry {
    return { kind: 'conversation', id: 'conversation-' + conversation.id, conversation, open: () => navigateToConversation(conversation.id), alt: () => addToken(chatToken(conversation)), altLabel: t('modals.alt.searchWithin') }
  }

  function suggestionEntries(): Entry[] {
    if (!operator) return []
    const term = operator.term
    if (operator.op === '#') {
      return rank($tagsStore, term, (tag) => [tag.name]).slice(0, 12).map((tag) => ({ kind: 'suggestion', id: 'tag-' + tag.id, token: tagToken(tag), detail: t('modals.detail.tag'), open: () => addToken(tagToken(tag)) }))
    }
    const people: Entry[] = rank($peopleStore, term, (p) => [p.display_name, p.primary_email, ...p.identities.map((i) => i.email)], (p) => p.last_activity)
      .slice(0, 6)
      .map((person) => ({ kind: 'suggestion', id: 'person-' + person.id, token: personToken(person), detail: person.primary_email ?? t('modals.detail.person'), open: () => addToken(personToken(person)) }))
    const chats: Entry[] = rank($conversationsStore.data, term, (c) => [c.name], (c) => c.last_activity)
      .slice(0, 6)
      .map((conversation) => ({ kind: 'suggestion', id: 'chat-' + conversation.id, token: chatToken(conversation), detail: t(conversation.type === 'email' ? 'modals.detail.mailThread' : 'modals.detail.chat'), open: () => addToken(chatToken(conversation)) }))
    return [...people, ...chats]
  }

  const groups = $derived.by((): ResultGroup[] => {
    if (filterOperator) {
      const values = filterCompletions(filterOperator, filterNames(filterOperator.key))
      return [{ key: 'suggestions', label: t('modals.group.filters'), icon: 'lucide:filter', entries: values.map((value): Entry => ({ kind: 'filter', id: 'filter-' + filterOperator.key + value, key: filterOperator.key, value, open: () => addFilter(value) })), hidden: 0 }]
    }
    if (operator) {
      return [{ key: 'suggestions', label: t(operator.op === '#' ? 'modals.group.tags' : 'modals.group.peopleAndChats'), icon: operator.op === '#' ? 'lucide:hash' : 'lucide:at-sign', entries: suggestionEntries(), hidden: 0 }]
    }
    const out: ResultGroup[] = []
    const push = (key: GroupKind, label: string, icon: string, entries: Entry[]) => {
      if (!inScope(scope, key) || entries.length === 0) return
      const { shown, hidden } = visibleItems(entries, scope)
      out.push({ key, label, icon, entries: shown, hidden })
    }
    if (!filtering) {
      if (scope === 'all') {
        const recent = [...$conversationsStore.data].sort((a, b) => b.last_activity.localeCompare(a.last_activity)).slice(0, 5)
        if (recent.length) out.push({ key: 'recent', label: t('modals.group.recent'), icon: 'lucide:history', entries: recent.map(conversationEntry), hidden: 0 })
      }
      if (scope === 'all' || scope === 'commands') out.push({ key: 'commands', label: t('modals.scope.commands'), icon: 'lucide:command', entries: commands.map((command) => ({ kind: 'command', id: command.id, command, open: command.run })), hidden: 0 })
      return out
    }
    const scoped = scopedIds ? new Set(scopedIds) : null
    if (words) push('people', t('modals.scope.people'), 'lucide:users', rank($peopleStore, words, (p) => [p.display_name, p.primary_email, ...p.identities.map((i) => i.email)], (p) => p.last_activity).map(personEntry))
    const conversations = conversationResults.filter((c) => !scoped || scoped.has(c.id))
    push('chats', t('modals.scope.chats'), 'lucide:message-circle', conversations.filter((c) => c.type !== 'email').map(conversationEntry))
    push('mail', t('modals.scope.mail'), 'lucide:mail', conversations.filter((c) => c.type === 'email').map(conversationEntry))
    const messages = messageResults.filter((m) => (scope === 'mail' ? m.type === 'email' : scope === 'chats' ? m.type !== 'email' : true))
    push('messages', t('modals.group.messages'), 'lucide:text-search', messages.map((message): Entry => {
      const raw = snippets.get(message.id)
      const conversation = $conversationsById.get(message.conversation_id)
      return { kind: 'message', id: 'message-' + message.id, message, parts: raw ? snippetParts(raw) : highlightParts(plainText(message.content), words), open: () => jump(message), alt: conversation ? () => addToken(chatToken(conversation)) : undefined, altLabel: t('modals.alt.searchWithin') }
    }))
    if (words) {
      push('actions', t('modals.scope.actions'), 'lucide:workflow', rank(actionList, words, (a) => [a.name, a.description, a.key], (a) => a.last_run_at).map((action) => ({ kind: 'action', id: 'action-' + action.id, action, open: () => go(`/main/actions?id=${encodeURIComponent(action.id)}`), alt: closing(() => api.postActionRun(action.id, {}, true).catch(() => {})), altLabel: t('modals.alt.dryRun') })))
      push('cases', t('modals.scope.cases'), 'lucide:briefcase', rank($casesStore, words, (c) => [c.name, c.description], (c) => c.last_activity).map((item) => ({ kind: 'case', id: 'case-' + item.id, item, open: () => go('/main/case/' + item.id), alt: current ? closing(() => api.putCaseItems({ case_id: item.id, conversation_ids: [current.id] }).catch(() => {})) : undefined, altLabel: t('modals.alt.addThisChat') })))
      const inboxes = rank($inboxesStore, words, (inbox) => [inbox.name]).map((inbox: Inbox): Entry => ({ kind: 'place', id: 'inbox-' + inbox.id, label: inbox.name, icon: inbox.icon || 'lucide:inbox', detail: inbox.is_default ? t('modals.detail.inbox') : t('modals.detail.inboxLinked', { count: inbox.members.length }), open: openPlace(() => openInbox(inbox.id)), alt: current && !inbox.is_default ? closing(() => addMembers(inbox, [{ kind: 'conversation', member_id: current.id }]).catch(() => {})) : undefined, altLabel: t('modals.alt.linkThisChat') }))
      const folders = rank(flattenTree(buildTree($groupsStore)).map((node) => node.group), words, (group) => [group.name]).map((group: Group): Entry => ({ kind: 'place', id: 'group-' + group.id, label: group.name, icon: group.icon || 'lucide:folder', detail: t('modals.detail.group', { count: group.conversation_ids.length }), open: openPlace(() => openGroup(group)), alt: current && group.kind === 'manual' ? closing(() => moveToGroup([current.id], group)) : undefined, altLabel: t('modals.alt.linkThisChat') }))
      const splits = rank($splitsStore, words, (split) => [split.name]).map((split): Entry => ({ kind: 'place', id: 'split-' + split.id, label: split.name, icon: split.icon || 'lucide:columns-2', detail: t('modals.detail.split'), open: openPlace(() => { inboxView.set('inbox'); activeSplit.set(split.id) }) }))
      push('places', t('modals.group.places'), 'lucide:folder-tree', [...inboxes, ...folders, ...splits])
      push('tags', t('modals.group.tags'), 'lucide:tag', rank($tagsStore, words, (tag) => [tag.name, tag.description]).map((tag) => ({ kind: 'tag', id: 'tag-' + tag.id, tag, open: openPlace(() => openTag(tag.id)), alt: () => addToken(tagToken(tag)), altLabel: t('modals.alt.searchWithin') })))
      push('content', t('content.title'), 'lucide:library', contentResults.slice(0, 24).map((content) => ({ kind: 'content', id: 'content-' + content.id, content, open: current ? closing(() => api.postContentSend(content.id, current.id).catch(() => {})) : () => go('/main/content') })))
      push('events', t('modals.scope.events'), 'lucide:calendar', rank(events.filter((e) => e.status !== 'dismissed'), words, (e) => [e.title, e.location, e.notes, e.organizer], (e) => e.starts_at).map((event) => ({ kind: 'event', id: 'event-' + event.id, event, open: () => go('/main/calendar'), alt: event.conversation_id ? () => navigateToConversation(event.conversation_id!) : undefined, altLabel: t('modals.alt.openThread') })))
      push('commands', t('modals.scope.commands'), 'lucide:command', rank(commands, words, (c) => [c.label, c.category, ...c.keywords]).map((command) => ({ kind: 'command', id: command.id, command, open: command.run })))
    }
    if (text && scope !== 'commands') {
      const remote: Entry[] = providerResults.length
        ? providerResults.map((message): Entry => ({ kind: 'message', id: 'provider-' + message.id, message, parts: highlightParts(plainText(message.content), words), open: () => jump(message) }))
        : [{ kind: 'provider', id: 'provider-search', label: t(providerSearching ? 'modals.provider.searching' : 'modals.provider.search', { query: text }), open: () => searchProvider() }]
      out.push({ key: 'provider', label: t('modals.group.provider'), icon: 'lucide:cloud', entries: remote, hidden: 0 })
    }
    return out
  })

  function showAll(key: GroupKind) {
    scope = key === 'messages' ? (messageResults.some((m) => m.type !== 'email') ? 'chats' : 'mail') : scopeOf(key)
    selectedIndex = 0
  }

  const selectable = $derived.by((): Entry[] => {
    const list: Entry[] = []
    for (const group of groups) {
      list.push(...group.entries)
      if (group.hidden > 0 && group.key !== 'recent' && group.key !== 'suggestions') {
        const key = group.key
        list.push({ kind: 'more', id: 'more-' + key, label: t('modals.showAll', { count: group.entries.length + group.hidden }), open: () => showAll(key) })
      }
    }
    return list
  })

  const counts = $derived(groups.filter((group) => group.key !== 'suggestions' && group.key !== 'recent' && group.key !== 'provider').map((group) => `${group.entries.length + group.hidden} ${group.label.toLowerCase()}`))
  const selected = $derived(selectable[selectedIndex] ?? null)
  const scopeOptions = $derived(SCOPES.map((entry) => ({ value: entry.value, label: t(entry.label) })))

  $effect(() => {
    if ($modalsStore.search && $modalsStore.isOpen && searchInput && !drafting) searchInput.focus()
  })

  $effect(() => {
    if (drafting && draftInput) draftInput.focus()
  })

  $effect(() => {
    const size = selectable.length
    untrack(() => (selectedIndex = Math.min(selectedIndex, Math.max(0, size - 1))))
  })

  $effect(() => {
    selectedIndex
    resultsContainer?.querySelector('[data-selected="true"]')?.scrollIntoView({ block: 'nearest' })
  })

  function startDraft(kind: 'rule' | 'group' | 'split' = 'rule') {
    promptKind = kind
    drafting = true
    draftFailure = null
    draftInstruction = ''
  }

  async function submitDraft() {
    const value = draftInstruction.trim()
    if (!value || draftBusy || (promptKind === 'rule' && !current)) return
    draftBusy = true
    draftFailure = null
    try {
      if (promptKind === 'group') {
        await api.postGroup({ name: value, kind: 'manual' })
        await loadGroups()
        drafting = false
        go(settingHref('organise.groups'))
      } else if (promptKind === 'split') {
        await api.postTag({ name: value, kind: 'split', position: $splitsStore.length })
        await loadTags()
        drafting = false
        modalsClose()
      } else if (current) {
        drafting = false
        go(`/main/actions?conversation=${encodeURIComponent(current.id)}&assist=${encodeURIComponent(value)}`)
      }
    } catch (e) {
      draftFailure = String(e)
    } finally {
      draftBusy = false
    }
  }

  function handleDraftKeydown(e: KeyboardEvent) {
    e.stopPropagation()
    if (e.key === 'Enter') {
      e.preventDefault()
      submitDraft()
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (drafting) return
    if (e.key === 'Tab') {
      e.preventDefault()
      if ((operator || filterOperator) && (selected?.kind === 'suggestion' || selected?.kind === 'filter')) {
        selected.open()
        return
      }
      scope = cycleScope(scope, e.shiftKey ? -1 : 1)
      selectedIndex = 0
      return
    }
    if (e.key === 'Backspace' && !searchQuery && tokens.length > 0) {
      e.preventDefault()
      tokens = tokens.slice(0, -1)
      return
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (selectable.length > 0) selectedIndex = (selectedIndex + 1) % selectable.length
      return
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (selectable.length > 0) selectedIndex = (selectedIndex - 1 + selectable.length) % selectable.length
      return
    }
    if (e.key === 'Enter') {
      e.preventDefault()
      if (!selected) return
      if ((e.metaKey || e.ctrlKey) && selected.alt) selected.alt()
      else selected.open()
    }
  }

  function when(iso: string | null | undefined): string {
    if (!iso) return ''
    const date = new Date(iso)
    return formatDate(date, date.getFullYear() === new Date().getFullYear() ? { day: 'numeric', month: 'short' } : { day: 'numeric', month: 'short', year: '2-digit' })
  }

  function marks(conversation: Conversation) {
    return conversation.providers.map((provider) => ({ ...provider, conversation_type: conversation.type }))
  }

  function conversationName(id: string): string {
    return $conversationsById.get(id)?.name ?? ''
  }

  function inboxesOf(conversation: Conversation): string {
    return $inboxesStore.filter((inbox) => !inbox.is_default && inbox.members.some((member) => member.member_id === conversation.id)).map((inbox) => inbox.name).join(', ')
  }

  const row = 'grid w-full grid-cols-[1.75rem_minmax(0,1fr)_auto] items-center gap-x-2.5 rounded-[0.5rem] px-2 py-1.5 text-left'
  const line = 'overflow-hidden text-ellipsis whitespace-nowrap'
</script>

{#snippet highlighted(parts: SnippetPart[])}
  {#each parts as part, i (i)}{#if part.hit}<mark class="rounded-[0.2rem] bg-accent-soft px-0.5 text-ink">{part.text}</mark>{:else}{part.text}{/if}{/each}
{/snippet}

{#snippet glyph(icon: string)}
  <span class="flex size-7 items-center justify-center rounded-[0.4rem] bg-paper-3 text-[14px] text-ink-2"><Icon {icon} /></span>
{/snippet}

{#snippet dot(color: string)}
  <span class="flex size-7 items-center justify-center"><span class="size-2.5 rounded-full" style:background={color}></span></span>
{/snippet}

<Sheet testid="search" label={t('modals.palette')} width="46rem" emphasis="strong" onclose={modalsClose}>
  {#if drafting && (promptKind !== 'rule' || current)}
    {@const prompt = prompts[promptKind]}
    <div class="flex flex-col gap-3 px-5 pb-4 pt-5">
      <div class="flex items-baseline gap-3">
        <h2 class="text-[15px] font-semibold tracking-tight text-ink">{t(prompt.title)}</h2>
        {#if promptKind === 'rule' && current}
          <span class="ml-auto max-w-[50%] {line} text-[13px] text-ink-3">{current.name}</span>
        {/if}
      </div>
      <label class="text-[13.5px] text-ink-3" for="draft-instruction">{t(prompt.label)}</label>
      <input
        id="draft-instruction"
        bind:this={draftInput}
        bind:value={draftInstruction}
        type="text"
        placeholder={t(prompt.placeholder)}
        class="w-full border-b border-line bg-transparent py-2 text-[15px] text-ink outline-none placeholder:text-ink-3 focus:border-accent"
        onkeydown={handleDraftKeydown}
        disabled={draftBusy}
      />
      {#if draftFailure}
        <p class="text-[13px] text-error-500">{draftFailure}</p>
      {/if}
      <div class="flex items-center justify-end gap-2">
        <Button onclick={() => (drafting = false)}>{t('common.back')}</Button>
        <Button variant="primary" keys="↵" busy={draftBusy} disabled={!draftInstruction.trim()} onclick={submitDraft}>{t(draftBusy ? prompt.busy : prompt.submit)}</Button>
      </div>
    </div>
  {:else}
    <div class="flex flex-col gap-2.5 border-b border-line px-4 pb-2.5 pt-3.5">
      <div class="flex min-w-0 items-center gap-2.5">
        <Icon icon="lucide:search" class="shrink-0 text-[18px] text-ink-3" />
        <div class="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
          {#each tokens as token, i (token.kind + token.id)}
            <span class="inline-flex h-6 max-w-[14rem] shrink-0 items-center gap-1 rounded-[0.4rem] bg-accent-soft pl-1.5 pr-0.5 text-[12.5px] text-ink">
              <span class="font-mono text-[11px] text-ink-3">{token.kind === 'tag' ? '#' : '@'}</span>
              <span class="min-w-0 {line}">{token.label}</span>
              <button type="button" class="flex size-5 items-center justify-center rounded-[0.3rem] text-ink-3 hover:text-ink" aria-label={t('modals.removeToken', { name: token.label })} onclick={() => (tokens = tokens.filter((_, index) => index !== i))}><Icon icon="lucide:x" class="text-[11px]" /></button>
            </span>
          {/each}
          {#each parsedFilters.filters as filter (filter.raw)}
            <span class="inline-flex h-6 max-w-[14rem] shrink-0 items-center gap-1 rounded-[0.4rem] bg-paper-3 pl-1.5 pr-0.5 text-[12.5px] text-ink" data-testid="search-filter" data-key={filter.key}>
              <span class="font-mono text-[11px] text-ink-3">{filter.key}:</span>
              <span class="min-w-0 {line}">{filter.value}</span>
              <button type="button" class="flex size-5 items-center justify-center rounded-[0.3rem] text-ink-3 hover:text-ink" aria-label={t('modals.removeToken', { name: filter.raw })} onclick={() => dropFilter(filter)}><Icon icon="lucide:x" class="text-[11px]" /></button>
            </span>
          {/each}
          <input
            data-testid="search-input"
            type="text"
            bind:this={searchInput}
            bind:value={searchQuery}
            placeholder={tokens.length ? t('modals.placeholder.within') : t('modals.placeholder.everything')}
            class="h-8 min-w-[8rem] flex-1 bg-transparent text-[18px] tracking-tight text-ink outline-none placeholder:text-ink-3"
            onkeydown={handleKeydown}
          />
        </div>
        {#if isSearching}
          <Icon icon="lucide:loader-circle" class="shrink-0 animate-spin text-[15px] text-ink-3" />
        {/if}
      </div>
      <div class="flex w-full items-center gap-2">
        <div class="min-w-0 flex-1">
          <Segmented testid="search-scope" label={t('modals.scope.label')} size="sm" stretch value={scope} options={scopeOptions} onchange={(next) => { scope = next as Scope; selectedIndex = 0 }} />
        </div>
        <Kbd keys="⇥" class="shrink-0" />
      </div>
      <div class="flex min-w-0 items-center gap-2 whitespace-nowrap text-[12px] text-ink-3">
        {#if filtering && !operator}
          <span class="min-w-0 shrink {line} font-mono">{counts.join(' · ') || t(isSearching ? 'modals.search.searching' : 'modals.search.noResults')}</span>
        {/if}
        <span class="ml-auto flex min-w-0 shrink items-center gap-1.5">
          <span class="shrink-0">{t('modals.in')}</span>
          {#if current}
            <ProviderIcon providers={marks(current)} size="xs" max={2} class="shrink-0" />
            <span class="min-w-0 {line} text-ink-2">{current.name}</span>
          {:else}
            <span class="min-w-0 {line} text-ink-2">{splitName ?? t('modals.everything')}</span>
          {/if}
        </span>
      </div>
    </div>

    <div bind:this={resultsContainer} class="min-h-0 flex-1 overflow-y-auto px-2 py-1.5" role="menu">
      {#each groups as group (group.key)}
        <div class="flex items-center gap-2 px-2 pb-1 pt-3 text-[12px] font-medium text-ink-3 first:pt-1.5">
          <Icon icon={group.icon} class="text-[13px]" />
          <span>{group.label}</span>
          <span class="font-mono text-[11px]">{group.entries.length + group.hidden}</span>
        </div>
        {#if group.key === 'content'}
          <div class="flex gap-1.5 overflow-x-auto px-2 pb-1">
            {#each group.entries as entry (entry.id)}
              {#if entry.kind === 'content'}
                {@const index = selectable.indexOf(entry)}
                <button type="button" role="menuitem" data-testid="search-result" data-kind={entry.kind} data-id={entry.id} data-selected={index === selectedIndex} class="aspect-square w-[4.5rem] shrink-0 overflow-hidden rounded-[0.5rem] bg-paper-3 {index === selectedIndex ? 'ring-2 ring-accent' : ''}" title={entry.content.name} onclick={entry.open} onmouseenter={() => (selectedIndex = index)}>
                  <img src={contentSrc(entry.content)} alt={entry.content.name} class="size-full object-cover" loading="lazy" />
                </button>
              {/if}
            {/each}
          </div>
        {:else}
          {#each group.entries as entry (entry.id)}
            {@const index = selectable.indexOf(entry)}
            <button type="button" role="menuitem" data-testid="search-result" data-kind={entry.kind} data-id={entry.id} data-selected={index === selectedIndex} class="{row} {index === selectedIndex ? 'bg-accent-soft text-ink' : 'text-ink hover:bg-paper-3'}" onclick={entry.open} onmouseenter={() => (selectedIndex = index)}>
              {#if entry.kind === 'person'}
                {@const person = entry.person}
                <Avatar source={personAvatar(person)} size="md" />
                <span class="flex min-w-0 flex-col">
                  <span class="{line} text-[14px] {person.unread_count > 0 ? 'font-semibold' : ''}">{person.display_name}</span>
                  <span class="{line} text-[12px] text-ink-3">{personEmail(person) ?? ''}</span>
                </span>
                <ProviderIcon providers={person.identities} size="xs" max={4} />
              {:else if entry.kind === 'conversation'}
                {@const conversation = entry.conversation}
                <Avatar source={chatAvatar(conversation, $peopleStore, $myUserIds)} size="md" />
                <span class="flex min-w-0 flex-col">
                  <span class="{line} text-[14px] {conversation.unread_count > 0 ? 'font-semibold' : ''}">{conversation.name}</span>
                  <span class="{line} text-[12px] text-ink-3">{renderEmojiText([inboxesOf(conversation), conversation.summary].filter(Boolean).join(' · '))}</span>
                </span>
                <span class="flex items-center gap-2">
                  <ProviderIcon providers={marks(conversation)} size="xs" max={3} />
                  <span class="font-mono text-[11px] text-ink-3">{when(conversation.last_activity)}</span>
                </span>
              {:else if entry.kind === 'message'}
                {@const message = entry.message}
                {@const conversation = $conversationsById.get(message.conversation_id)}
                {@render glyph(message.type === 'email' ? 'lucide:mail' : 'lucide:message-square-text')}
                <span class="flex min-w-0 flex-col gap-0.5">
                  <span class="flex min-w-0 items-baseline gap-1.5 text-[13px]">
                    <span class="shrink-0 font-medium">{message.sender_name || t('common.unknown')}</span>
                    <span class="min-w-0 {line} text-ink-3">{conversationName(message.conversation_id)}</span>
                  </span>
                  <span class="line-clamp-2 text-[12.5px] leading-[1.4] text-ink-2">{@render highlighted(entry.parts)}</span>
                </span>
                <span class="flex items-center gap-2 self-start pt-0.5">
                  {#if conversation}<ProviderIcon providers={marks(conversation)} size="xs" max={1} />{/if}
                  <span class="font-mono text-[11px] text-ink-3">{when(message.timestamp)}</span>
                </span>
              {:else if entry.kind === 'action'}
                {@const action = entry.action}
                {@render glyph(action.icon || 'lucide:workflow')}
                <span class="flex min-w-0 flex-col">
                  <span class="{line} text-[14px] {action.enabled ? '' : 'text-ink-3'}">{action.name}</span>
                  <span class="{line} text-[12px] text-ink-3">{[action.enabled ? '' : t('modals.detail.disabled'), action.description].filter(Boolean).join(' · ')}</span>
                </span>
                <span class="font-mono text-[11px] text-ink-3">{action.last_run_at ? t('modals.detail.ran', { when: when(action.last_run_at) }) : t('modals.detail.runs', { count: action.run_count })}</span>
              {:else if entry.kind === 'case'}
                {@const item = entry.item}
                {@render dot(item.color)}
                <span class="flex min-w-0 flex-col">
                  <span class="{line} text-[14px]">{item.name}</span>
                  <span class="{line} text-[12px] text-ink-3">{item.status} · {t('modals.detail.chats', { count: item.conversation_count })}{item.due_at ? ' · ' + t('modals.detail.due', { when: when(item.due_at) }) : ''}</span>
                </span>
                <span class="font-mono text-[11px] text-ink-3">{when(item.last_activity)}</span>
              {:else if entry.kind === 'place'}
                {@render glyph(entry.icon)}
                <span class="flex min-w-0 flex-col">
                  <span class="{line} text-[14px]">{entry.label}</span>
                  <span class="{line} text-[12px] text-ink-3">{entry.detail}</span>
                </span>
                <span></span>
              {:else if entry.kind === 'tag'}
                <span class="flex size-7 items-center justify-center"><TagMark tag={entry.tag} size="md" /></span>
                <span class="{line} text-[14px]">{entry.tag.name}</span>
                <span class="text-[11.5px] text-ink-3">{t('modals.detail.filter')}</span>
              {:else if entry.kind === 'event'}
                {@const event = entry.event}
                {@render glyph('lucide:calendar')}
                <span class="flex min-w-0 flex-col">
                  <span class="{line} text-[14px]">{event.title}</span>
                  <span class="{line} text-[12px] text-ink-3">{[event.location, event.status === 'proposed' ? t('modals.detail.proposed') : ''].filter(Boolean).join(' · ')}</span>
                </span>
                <span class="font-mono text-[11px] text-ink-3">{when(event.starts_at)}{event.all_day ? '' : ' ' + formatDate(event.starts_at, { hour: '2-digit', minute: '2-digit' })}</span>
              {:else if entry.kind === 'command'}
                {@const command = entry.command}
                {@render glyph(command.icon)}
                <span class="flex min-w-0 items-baseline gap-2">
                  <span class="{line} text-[14px]">{command.label}</span>
                  <span class="shrink-0 text-[11.5px] text-ink-3">{command.category}</span>
                </span>
                {#if command.hint}<Kbd keys={command.hint} />{:else}<span></span>{/if}
              {:else if entry.kind === 'suggestion'}
                {@render glyph(entry.token.icon ?? 'lucide:at-sign')}
                <span class="{line} text-[14px]"><span class="font-mono text-ink-3">{entry.token.kind === 'tag' ? '#' : '@'}</span>{entry.token.label}</span>
                <span class="text-[11.5px] text-ink-3">{entry.detail}</span>
              {:else if entry.kind === 'filter'}
                {@render glyph('lucide:filter')}
                <span class="{line} text-[14px]"><span class="font-mono text-ink-3">{entry.key}:</span>{entry.value}</span>
                <span class="text-[11.5px] text-ink-3">{t('modals.detail.filter')}</span>
              {:else if entry.kind === 'provider'}
                {@render glyph(providerSearching ? 'lucide:loader-circle' : 'lucide:cloud')}
                <span class="{line} text-[14px]" data-testid="search-provider">{entry.label}</span>
                <span></span>
              {/if}
            </button>
          {/each}
        {/if}
        {#if group.hidden > 0}
          {@const more = selectable.find((entry) => entry.id === 'more-' + group.key)}
          {#if more && more.kind === 'more'}
            {@const index = selectable.indexOf(more)}
            <button type="button" role="menuitem" data-selected={index === selectedIndex} class="flex h-7 w-full items-center gap-2 rounded-[0.5rem] pl-[2.75rem] pr-2 text-left text-[12.5px] {index === selectedIndex ? 'bg-accent-soft text-ink' : 'text-ink-3 hover:bg-paper-3 hover:text-ink'}" onclick={more.open} onmouseenter={() => (selectedIndex = index)}>
              {more.label}
              <Icon icon="lucide:arrow-right" class="text-[12px]" />
            </button>
          {/if}
        {/if}
      {/each}

      {#if groups.length === 0 && !isSearching}
        <div class="px-4 py-8 text-center text-[13px] text-ink-3">{operator ? t('modals.empty.noMatches') : filtering ? t('modals.empty.nothingFound', { query: text }) : t('modals.empty.startTyping')}</div>
      {/if}
    </div>
  {/if}
  {#snippet footer()}
    {#if !drafting}
      <span class="flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="↑ ↓" /> {t('modals.footer.move')}</span>
      <span class="flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="↵" /> {t(selected?.kind === 'suggestion' ? 'modals.footer.add' : 'modals.footer.open')}</span>
      {#if selected?.alt}
        <span class="flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="⌘ ↵" /> {selected.altLabel}</span>
      {/if}
      <span class="flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="@ #" /> {t('modals.footer.scope')}</span>
      <span class="flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="from: is:" /> {t('modals.footer.filters')}</span>
      <span class="ml-auto flex items-center gap-1.5 text-[12px] text-ink-3"><Kbd keys="esc" /> {t('modals.footer.close')}</span>
    {/if}
  {/snippet}
</Sheet>
