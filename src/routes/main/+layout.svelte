<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import type { Snippet } from 'svelte'
  import { page } from '$app/state'
  import { goto } from '$app/navigation'
  import { api, profileSettingsStore, conversationsStore, messagesStore, casePickerStore, openCasePicker } from '$lib/tauri'
  import { putDetailsOpen, putProfilePatch } from '$lib/tauri/profile'
  import { closeThread, threadPanelStore } from '$lib/tauri/threads'
  import WindowManager from '$lib/components/window/WindowManager.svelte'
  import Modals from '$lib/components/modals/Modals.svelte'
  import CasePicker from '$lib/components/cases/CasePicker.svelte'
  import NewChatModal from '$lib/components/modals/NewChatModal.svelte'
  import ChatDialog from '$lib/components/conversation/ChatDialog.svelte'
  import Tooltip from '$lib/tooltip/Tooltip.svelte'
  import Rail from '$lib/components/rail/Rail.svelte'
  import InboxList from '$lib/components/inbox/InboxList.svelte'
  import { DraggableEdge } from 'sly-svelte-pane'
  import { shellPanel } from '$lib/shell.svelte'
  import { PaneGrid } from '$lib/components/ui'
  import { modalsShow, modalsClose, modalsStore } from '$lib/modals'
  import { armSequence, registerShortcut, handleKeydown as shortcutKeydown, setShortcutOverrides } from '$lib/shortcuts'
  import { FEATURES } from '$lib/settings'
  import { foldAll, folds, toggleFocused } from '$lib/collapse'
  import { chooseSummary, toggleAllSections } from '$lib/sections.svelte'
  import { clearSelection, selection, shortcutForward, shortcutReply } from '$lib/reply.svelte'
  import { copySelected } from '$lib/export.svelte'
  import {
    inboxView,
    inboxSheet,
    selectedIndex,
    selectedIds,
    moveSelection,
    toggleSelected,
    clearSelected,
    cycleSplit,
    targetIds,
    focusedItem,
    itemId,
    itemPath,
    markDone,
    remove,
    toggleUnread,
    listCollapsed,
    toggleList,
    uiScale,
    applyUiScale,
    stepUiScale,
    paneSize,
    holdPane,
    activeGroup,
    activeTag,
    openAdjacent,
    type InboxView
  } from '$lib/inbox'
  import { get } from 'svelte/store'
  import { loadAppearance, shell } from '$lib/appearance'
  import { putLaunchStep } from '$lib/window'
  import { layoutSettings, loadLayoutSettings } from '$lib/layoutSettings'
  import { activeInbox, defaultInbox, listenInboxes } from '$lib/inboxes'
  import { listenHolds } from '$lib/outbound.svelte'
  import { flushLastRoute, putLastRoute, trackedRoute } from '$lib/startup'
  import type { MessageKey } from '$lib/i18n.svelte'

  interface Props {
    children?: Snippet
  }

  let { children }: Props = $props()
  let sidebarSize = $state(paneSize('chatnip.sidebar-size', '15vw'))

  const inboxRoute = $derived(page.url.pathname.startsWith('/main/conversation') || (page.url.pathname.startsWith('/main/person') && !page.url.pathname.endsWith('/profile')) || page.url.pathname.startsWith('/main/merged') || page.url.pathname === '/main')
  const panelOpen = $derived(!!$shellPanel && $shellPanel.open)
  const inboxPanes = $derived([
    { key: 'list', size: '28vw', min: '16rem', max: '45vw', open: !$listCollapsed, edge: 'end' as const, class: 'bg-paper' },
    { key: 'content' },
    { key: $shellPanel?.key ?? 'panel', size: $shellPanel?.size ?? '26rem', min: $shellPanel?.min ?? '18rem', max: $shellPanel?.max ?? '45vw', open: panelOpen, edge: 'start' as const, class: 'bg-paper' }
  ])
  const collapsed = $derived($profileSettingsStore?.navigation_collapsed ?? false)
  const glass = $derived($shell.glass)
  const pane = $derived(glass ? $shell.frost : `${$shell.frost} border-r border-line`)

  $effect(() => {
    if (!$conversationsStore.is_loading) putLaunchStep('ready')
  })

  $effect(() => {
    putLastRoute(trackedRoute(page.url, { inbox: $activeInbox, view: $inboxView, group: $activeGroup, tag: $activeTag }, $defaultInbox?.id ?? null))
  })

  onMount(() => {
    loadAppearance().catch(() => {})
    loadLayoutSettings().catch(() => {})
    const stopInboxes = listenInboxes()
    const stopHolds = listenHolds()
    const flush = () => void flushLastRoute()
    window.addEventListener('pagehide', flush)
    window.addEventListener('beforeunload', flush)
    return () => {
      stopInboxes.then((stop) => stop()).catch(() => {})
      stopHolds.then((stop) => stop()).catch(() => {})
      window.removeEventListener('pagehide', flush)
      window.removeEventListener('beforeunload', flush)
      flush()
    }
  })

  const initialize = async (): Promise<void> => {
    await api.postSync()
  }

  function modalOpen() {
    return get(modalsStore).isOpen
  }

  function viewShortcut(key: string, view: InboxView, description: MessageKey) {
    registerShortcut({
      key,
      after: 'g',
      description,
      category: 'app.shortcutCategory.navigation',
      action: () => {
        inboxView.set(view)
        selectedIndex.set(0)
        if (!inboxRoute) goto('/main/conversation')
      }
    })
  }

  function pageShortcut(key: string, path: string, description: MessageKey) {
    registerShortcut({
      key,
      after: 'g',
      description,
      category: 'app.shortcutCategory.navigation',
      action: () => {
        goto(path)
      }
    })
  }

  function findInConversation() {
    const current = get(conversationsStore).data.find((c) => c.is_current)
    if (modalOpen() || !current) return false
    modalsShow({ query: '', conversation_id: current.id })
  }

  function setSidebarCollapsed(next: boolean) {
    putProfilePatch({ navigation_collapsed: next }).catch(() => {})
  }

  function toggleSidebar() {
    setSidebarCollapsed(!collapsed)
  }

  let pointerAt: Element | null = null

  function copyChecked(): boolean {
    if (!selection.ids.length || window.getSelection()?.toString()) return false
    void copySelected()
    return true
  }

  function selectAll(): boolean {
    if (!/^\/main\/(conversation|person|merged)\//.test(location.pathname)) return false
    const anchor = window.getSelection()?.anchorNode
    const anchored = anchor instanceof Element ? anchor : (anchor?.parentElement ?? null)
    const inside = anchored?.closest<HTMLElement>('[data-testid="message"], [data-message-id]')
    if (inside) {
      const range = document.createRange()
      range.selectNodeContents(inside)
      window.getSelection()?.removeAllRanges()
      window.getSelection()?.addRange(range)
      return true
    }
    const thread = [document.activeElement, anchored, pointerAt].map((node) => node?.closest('[data-testid="thread-panel"]')).find(Boolean)
    if (thread) {
      const ids = [...thread.querySelectorAll<HTMLElement>('[data-message-id]')].map((node) => node.dataset.messageId ?? '').filter(Boolean)
      if (!ids.length) return false
      selection.ids = ids
      return true
    }
    const loaded = get(messagesStore).data ?? []
    if (!loaded.length) return false
    selection.ids = loaded.map((message) => message.id)
    return true
  }

  let showing = false
  function stepSidebars() {
    const rail = !collapsed
    const list = !get(listCollapsed)
    if (rail && list) showing = false
    if (!rail && !list) showing = true
    if (showing) {
      if (!list) toggleList()
      else setSidebarCollapsed(false)
    } else if (rail) setSidebarCollapsed(true)
    else toggleList()
  }

  function settleSidebar(value: string) {
    const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16
    const pixels = value.endsWith('vw') ? (parseFloat(value) / 100) * window.innerWidth : parseFloat(value)
    if (pixels < rem * 8) {
      localStorage.setItem('chatnip.sidebar-size', sidebarSize)
      document.getElementById('shell')?.style.setProperty('--sidebar-size', sidebarSize)
      setSidebarCollapsed(true)
      return
    }
    sidebarSize = value
    setSidebarCollapsed(false)
  }

  function registerAllShortcuts() {
    registerShortcut({ key: 'c', description: 'app.shortcut.newMessage', category: 'app.shortcutCategory.conversation', action: () => !modalOpen() && modalsShow({ mode: 'new' }) })
    registerShortcut({ key: 'o', description: 'app.shortcut.toggleFocused', category: 'app.shortcutCategory.conversation', action: () => !modalOpen() && toggleFocused() })
    registerShortcut({ key: 'O', shift: true, description: 'app.shortcut.toggleAll', category: 'app.shortcutCategory.conversation', action: () => !modalOpen() && foldAll(get(folds).all === 'open' ? 'closed' : 'open') })
    registerShortcut({ key: 'z', description: 'app.shortcut.toggleSections', category: 'app.shortcutCategory.conversation', action: () => !modalOpen() && toggleAllSections() })
    registerShortcut({ key: 'Z', shift: true, description: 'app.shortcut.summarizeAll', category: 'app.shortcutCategory.conversation', action: () => !modalOpen() && chooseSummary() })
    if (FEATURES.cases) registerShortcut({
      key: 'C',
      shift: true,
      description: 'app.shortcut.addToCase',
      category: 'app.shortcutCategory.conversation',
      action: () => {
        if (modalOpen() || get(casePickerStore)) return
        const focused = targetIds(focusedItem())
        const current = get(conversationsStore).data.find((c) => c.is_current)
        const ids = page.url.pathname.startsWith('/main/case/') ? [] : focused.length ? focused : current ? [current.id] : []
        openCasePicker({ conversationIds: ids, messageIds: [] })
      }
    })
    registerShortcut({ key: 'Tab', description: 'app.shortcut.nextSplit', category: 'app.shortcutCategory.inbox', action: () => cycleSplit(1) })
    registerShortcut({ key: 'Tab', shift: true, description: 'app.shortcut.previousSplit', category: 'app.shortcutCategory.inbox', action: () => cycleSplit(-1) })
    registerShortcut({ key: 'k', meta: true, description: 'app.shortcut.search', category: 'app.shortcutCategory.navigation', action: () => !modalOpen() && modalsShow({ query: '' }) })
    registerShortcut({ key: '/', meta: true, description: 'app.shortcut.shortcuts', category: 'app.shortcutCategory.navigation', action: () => !modalOpen() && modalsShow({ shortcuts: true }) })
    registerShortcut({ key: '?', shift: true, description: 'app.shortcut.shortcuts', category: 'app.shortcutCategory.navigation', action: () => !modalOpen() && modalsShow({ shortcuts: true }) })
    registerShortcut({ key: ',', meta: true, description: 'app.shortcut.settings', category: 'app.shortcutCategory.navigation', action: () => goto('/main/profile') })
    registerShortcut({ key: 'C', meta: true, shift: true, description: 'app.shortcut.calendar', category: 'app.shortcutCategory.navigation', action: () => goto('/main/calendar') })
    registerShortcut({ key: '|', meta: true, shift: true, description: 'app.shortcut.collapseSidebar', category: 'app.shortcutCategory.view', action: toggleSidebar })
    registerShortcut({ key: 'L', meta: true, shift: true, description: 'app.shortcut.toggleList', category: 'app.shortcutCategory.view', action: toggleList })
    registerShortcut({ key: 'S', meta: true, shift: true, description: 'app.shortcut.stepSidebars', category: 'app.shortcutCategory.view', action: stepSidebars })
    registerShortcut({ key: 'a', meta: true, fields: false, description: 'app.shortcut.selectAll', category: 'app.shortcutCategory.conversation', action: selectAll })
    registerShortcut({ key: 'c', meta: true, fields: false, description: 'app.shortcut.copySelected', category: 'app.shortcutCategory.conversation', action: copyChecked })
    registerShortcut({ key: '=', meta: true, description: 'app.shortcut.largerText', category: 'app.shortcutCategory.view', action: () => stepUiScale(1) })
    registerShortcut({ key: '-', meta: true, description: 'app.shortcut.smallerText', category: 'app.shortcutCategory.view', action: () => stepUiScale(-1) })
    registerShortcut({ key: '0', meta: true, description: 'app.shortcut.resetText', category: 'app.shortcutCategory.view', action: () => stepUiScale(0) })
    registerShortcut({ key: '[', meta: true, description: 'app.shortcut.back', category: 'app.shortcutCategory.navigation', action: () => history.back() })
    registerShortcut({ key: ']', meta: true, description: 'app.shortcut.forward', category: 'app.shortcutCategory.navigation', action: () => history.forward() })
    registerShortcut({ key: '{', meta: true, shift: true, description: 'app.shortcut.previousSplit', category: 'app.shortcutCategory.inbox', action: () => cycleSplit(-1) })
    registerShortcut({ key: '}', meta: true, shift: true, description: 'app.shortcut.nextSplit', category: 'app.shortcutCategory.inbox', action: () => cycleSplit(1) })
    registerShortcut({ key: 'g', description: 'app.shortcut.goTo', category: 'app.shortcutCategory.navigation', action: () => armSequence('g') })
    viewShortcut('i', 'inbox', 'app.shortcut.goToInbox')
    viewShortcut('n', 'notifications', 'app.shortcut.goToNotifications')
    viewShortcut('p', 'pipeline', 'app.shortcut.goToPipeline')
    viewShortcut('h', 'snoozed', 'app.shortcut.goToSnoozed')
    viewShortcut('e', 'done', 'app.shortcut.goToDone')
    viewShortcut('d', 'deleted', 'app.shortcut.goToDeleted')
    pageShortcut('a', '/main/activity', 'app.shortcut.goToActivity')
    pageShortcut('s', '/main/saved', 'app.shortcut.goToSaved')
    pageShortcut('m', '/main/reminders', 'app.shortcut.goToReminders')
    registerShortcut({ key: 'A', meta: true, shift: true, description: 'app.shortcut.activity', category: 'app.shortcutCategory.navigation', action: () => goto('/main/activity') })
    registerShortcut({ key: 'f', meta: true, description: 'app.shortcut.findInConversation', category: 'app.shortcutCategory.conversation', action: findInConversation })
    registerShortcut({ key: 'ArrowDown', alt: true, description: 'app.shortcut.nextConversation', category: 'app.shortcutCategory.navigation', action: () => !modalOpen() && openAdjacent(1) })
    registerShortcut({ key: 'ArrowUp', alt: true, description: 'app.shortcut.previousConversation', category: 'app.shortcutCategory.navigation', action: () => !modalOpen() && openAdjacent(-1) })
    registerShortcut({ key: 'ArrowDown', alt: true, shift: true, description: 'app.shortcut.nextUnread', category: 'app.shortcutCategory.navigation', action: () => !modalOpen() && openAdjacent(1, true) })
    registerShortcut({ key: 'ArrowUp', alt: true, shift: true, description: 'app.shortcut.previousUnread', category: 'app.shortcutCategory.navigation', action: () => !modalOpen() && openAdjacent(-1, true) })

    registerShortcut({ key: 'j', description: 'app.shortcut.nextRow', category: 'app.shortcutCategory.inbox', action: () => moveSelection(1) })
    registerShortcut({ key: 'k', description: 'app.shortcut.previousRow', category: 'app.shortcutCategory.inbox', action: () => moveSelection(-1) })
    registerShortcut({ key: 'ArrowDown', description: 'app.shortcut.nextRow', category: 'app.shortcutCategory.inbox', action: () => moveSelection(1) })
    registerShortcut({ key: 'ArrowUp', description: 'app.shortcut.previousRow', category: 'app.shortcutCategory.inbox', action: () => moveSelection(-1) })
    registerShortcut({
      key: 'Enter',
      description: 'app.shortcut.openRow',
      category: 'app.shortcutCategory.inbox',
      action: () => {
        const item = focusedItem()
        if (item && !modalOpen() && !get(inboxSheet)) goto(itemPath(item))
      }
    })
    registerShortcut({
      key: 'x',
      description: 'app.shortcut.selectRow',
      category: 'app.shortcutCategory.inbox',
      action: () => {
        const item = focusedItem()
        if (item) toggleSelected(itemId(item))
      }
    })
    registerShortcut({
      key: 'e',
      description: 'app.shortcut.markDone',
      category: 'app.shortcutCategory.inbox',
      action: () => {
        markDone(targetIds(focusedItem()), get(inboxView) !== 'done')
      }
    })
    registerShortcut({
      key: 'h',
      description: 'app.shortcut.snooze',
      category: 'app.shortcutCategory.inbox',
      action: () => {
        if (focusedItem()) inboxSheet.set('snooze')
      }
    })
    registerShortcut({
      key: 'l',
      description: 'app.shortcut.tag',
      category: 'app.shortcutCategory.inbox',
      action: () => {
        if (focusedItem()) inboxSheet.set('tag')
      }
    })
    registerShortcut({
      key: '#',
      shift: true,
      description: 'app.shortcut.delete',
      category: 'app.shortcutCategory.inbox',
      action: () => remove(targetIds(focusedItem()), get(conversationsStore).data)
    })
    registerShortcut({
      key: 'u',
      description: 'app.shortcut.toggleRead',
      category: 'app.shortcutCategory.inbox',
      action: () => toggleUnread(targetIds(focusedItem()), get(conversationsStore).data)
    })
    registerShortcut({
      key: 'r',
      description: 'app.shortcut.reply',
      category: 'app.shortcutCategory.conversation',
      action: () => !modalOpen() && shortcutReply()
    })
    registerShortcut({ key: 'R', shift: true, description: 'app.shortcut.replyAll', category: 'app.shortcutCategory.conversation', action: () => !modalOpen() && shortcutReply(true) })
    registerShortcut({ key: 'f', description: 'app.shortcut.forwardMessage', category: 'app.shortcutCategory.conversation', action: () => !modalOpen() && shortcutForward() })
    registerShortcut({
      key: 'M',
      meta: true,
      shift: true,
      description: 'app.shortcut.mute',
      category: 'app.shortcutCategory.conversation',
      action: async () => {
        const current = get(conversationsStore).data.find((c) => c.is_current)
        if (current) await api.putConversation({ conversation_id: current.id, is_muted: !current.is_muted })
      }
    })
    registerShortcut({
      key: 'Escape',
      description: 'app.shortcut.close',
      category: 'app.shortcutCategory.conversation',
      action: () => {
        if (modalOpen()) return modalsClose()
        if (get(inboxSheet)) return inboxSheet.set(null)
        if (selection.ids.length > 0) return clearSelection()
        if (get(selectedIds).size > 0) return clearSelected()
        if (get(threadPanelStore).open) return closeThread()
        putDetailsOpen(false)
      }
    })
    const toggleDetails = () => putDetailsOpen(null)
    registerShortcut({ key: 'D', meta: true, shift: true, description: 'app.shortcut.toggleDetails', category: 'app.shortcutCategory.conversation', action: toggleDetails })
    registerShortcut({ key: 'i', description: 'app.shortcut.toggleDetails', category: 'app.shortcutCategory.conversation', action: () => !modalOpen() && toggleDetails() })
  }

  $effect(() => setShortcutOverrides($layoutSettings))

  function handleGlobalKeydown(e: KeyboardEvent) {
    shortcutKeydown(e)
  }

  onMount(() => {
    applyUiScale(get(uiScale))
    registerAllShortcuts()
    initialize()
    window.addEventListener('keydown', handleGlobalKeydown)
    window.addEventListener('pointerover', trackPointer, true)
  })

  function trackPointer(e: PointerEvent) {
    pointerAt = e.target instanceof Element ? e.target : null
  }

  onDestroy(() => {
    if (typeof window === 'undefined') return
    window.removeEventListener('keydown', handleGlobalKeydown)
    window.removeEventListener('pointerover', trackPointer, true)
  })
</script>

<WindowManager />
<Modals />
{#if FEATURES.cases}
  <CasePicker />
{/if}
<NewChatModal />
<ChatDialog />
<Tooltip />

<div
  id="shell"
  data-pane-grid
  class="grid-slide relative z-[1] grid h-full grid-rows-1 overflow-hidden"
  style:--icon-cell="2.25rem"
  style:--rail-pad="0.5rem"
  style:--sidebar-size={sidebarSize}
  style:--sidebar-size-min="10rem"
  style:--sidebar-size-max="24rem"
  style:--sidebar-open="clamp(var(--sidebar-size-min), var(--sidebar-size), var(--sidebar-size-max))"
  style:--sidebar-track={collapsed ? 'calc(var(--icon-cell) + var(--rail-pad) * 2)' : 'var(--sidebar-open)'}
  style:grid-template-columns="var(--sidebar-track) minmax(0, 1fr)"
>
  <div class="relative h-full min-w-0 overflow-clip {pane}">
    <Rail ontoggle={toggleSidebar} />
    {#if !collapsed}
      <div class="pane-edge right" role="presentation" onpointerdowncapture={(e) => holdPane(e, 'chatnip.sidebar-size', '--sidebar-size', settleSidebar)}>
      <DraggableEdge options={{ property: '--sidebar-size', anchor: 'right', gridSelector: '#shell' }} />
    </div>
    {/if}
  </div>
  <div class="relative min-h-0 min-w-0 overflow-hidden {glass ? '' : 'bg-paper'}" data-router>
    {#if inboxRoute}
      <PaneGrid name="inbox" panes={inboxPanes}>
        {#snippet pane(key)}
          {#if key === 'list'}
            <InboxList />
          {:else if key === 'content'}
            <div class="relative h-full min-h-0 min-w-0" data-content-pane>{@render children?.()}</div>
          {:else if $shellPanel}
            {@const Panel = $shellPanel.component}
            <Panel {...$shellPanel.props ?? {}} />
          {/if}
        {/snippet}
      </PaneGrid>
    {:else}
      <div class="relative h-full min-h-0 min-w-0" data-content-pane>{@render children?.()}</div>
    {/if}
  </div>
</div>
