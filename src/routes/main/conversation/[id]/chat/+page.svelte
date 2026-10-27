<script lang="ts">
  import TitleBar from '$lib/components/conversation/TitleBar.svelte';
  import History from '$lib/components/conversation/History.svelte';
  import ChatInput from '$lib/components/conversation/ChatInput.svelte';
  import TypingIndicator from '$lib/components/conversation/TypingIndicator.svelte';
  import {profileSettingsStore} from '$lib/tauri/profile';
  import {threadPanelStore} from '$lib/tauri/threads';
  import {conversationStore} from '$lib/tauri/conversations';
  import {readStore} from '$lib/tauri/groups';
  import {chatsStore} from '$lib/tauri/chats';
  import {putDetailsOpen} from '$lib/tauri/profile';
  import ChatTimeline from '$lib/components/conversation/ChatTimeline.svelte';
  import ChatGrid from '$lib/components/conversation/ChatGrid.svelte';
  import {appearanceSettings, globalAppearance, conversationKey, scoped, scopePresentation} from '$lib/appearance';
  import {themeChoice, resolvedTheme} from '$lib/theme-mode';
  import { composerSize, holdPane } from '$lib/inbox';
  import { holdConversationPanel } from '$lib/shell.svelte';
  import { DraggableEdge } from 'sly-svelte-pane';
  import type { Message } from '$lib/tauri/types';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { messagesStore, api } from '$lib/tauri';
  import { messageAnchor, messageHref, revealMessage } from '$lib/messageLink';

  let hash = $state(page.url.hash);
  $effect(() => {
    hash = page.url.hash;
  });

  const anchor = $derived(messageAnchor(hash));
  const ready = $derived(!$messagesStore.is_loading && ($messagesStore.data?.length ?? 0) > 0);

  $effect(() => {
    const id = anchor;
    const here = $conversationStore.data?.id;
    if (!id || !ready || !here) return;
    const run = revealMessage(id);
    run.done.then((found) => {
      if (found) return;
      api.getMessageQuote(id).then((message) => {
        if (message?.conversation_id && message.conversation_id !== here) goto(messageHref(message.conversation_id, id));
      }).catch(() => {});
    });
    return () => run.cancel();
  });

  let lastThread = $state<{ thread_id: string; root_message: Message } | null>(null);
  $effect(() => {
    const panel = $threadPanelStore;
    if (panel.open && panel.thread_id && panel.root_message) lastThread = { thread_id: panel.thread_id, root_message: panel.root_message };
  });

  holdConversationPanel(() => ({ thread: lastThread, threadOpen: $threadPanelStore.open, detailsOpen: !!$profileSettingsStore?.details_panel_open }));

  const conversationId = $derived($conversationStore.data?.id ?? '');
  const conversation = $derived($conversationStore.data);
  const local = $derived(conversation?.type === 'local');
  const read = $derived($readStore.has(conversationId));
  const grid = $derived(read && conversation?.view === 'grid');
  const attached = $derived($chatsStore.find((c) => c.conversation_id === conversationId)?.attachments.length ?? 0);

  const scope = $derived(scopePresentation($globalAppearance, conversationId ? scoped($appearanceSettings, [conversationKey(conversationId)]) : {}, resolvedTheme($themeChoice)));
</script>

<svelte:window onhashchange={() => (hash = location.hash)} />

<div class="relative isolate h-full overflow-hidden {scope.tinted ? `frost-${scope.tint}` : ''}" style={scope.style} style:--frost-alpha={scope.solid ? '1' : undefined}>
    {#if scope.backdrop}<div class="scope-backdrop" style:background={scope.backdrop}></div>{/if}
    {#if scope.tinted}<div class="scope-frost"></div>{/if}
    <div data-pane-grid
         class="relative grid h-full min-w-0 grid-cols-1 overflow-hidden"
         style:--composer-size={$composerSize}
         style:--composer-size-min="6rem"
         style:--composer-size-max="70vh"
         style:--composer-track="clamp(var(--composer-size-min), var(--composer-size), var(--composer-size-max))"
         style:grid-template-rows={read ? 'auto minmax(0, 1fr)' : 'auto minmax(0, 1fr) auto var(--composer-track)'}>
        <div class="relative z-0 min-w-0"><TitleBar/></div>
        <div class="relative z-10 flex min-h-0 min-w-0 flex-col bg-paper">{#if conversation && grid}<ChatGrid {conversation} onsources={local ? () => putDetailsOpen(true) : undefined}/>{:else if conversation && local}<ChatTimeline {conversation} {read} {attached}/>{:else}<History/>{/if}</div>
        {#if !read}
        <div class="relative z-10 min-w-0 bg-paper"><TypingIndicator/></div>
        <div class="relative z-10 flex min-h-0 min-w-0 flex-col bg-paper">
            <div class="pane-edge top" data-axis="y" role="presentation" onpointerdowncapture={(e) => holdPane(e, 'chatnip.composer-size', '--composer-size', (value) => composerSize.set(value))} ondblclick={() => { composerSize.set('18vh'); localStorage.removeItem('chatnip.composer-size') }}>
                <DraggableEdge options={{ property: '--composer-size', anchor: 'bottom', gridSelector: '[data-pane-grid]' }} />
            </div>
            <ChatInput/>
        </div>
        {/if}
    </div>
</div>
