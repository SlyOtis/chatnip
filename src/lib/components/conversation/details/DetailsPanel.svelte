<script lang="ts">
  import Header from './Header.svelte';
  import ConversationInfo from './ConversationInfo.svelte';
  import PinnedMessages from './PinnedMessages.svelte';
  import Identities from './Identities.svelte';
  import Shared from './Shared.svelte';
  import SectionActions from '../SectionActions.svelte';
  import ExportMenu from '../ExportMenu.svelte';
  import { chatExport } from '$lib/export.svelte';
  import { sections } from '$lib/sections.svelte';
  import { conversationStore, conversationsStore } from '$lib/tauri/conversations';
  import { peopleStore } from '$lib/tauri/people';
  import type { GetSharedInput } from '$lib/tauri/types';
  import { page } from '$app/state';
  import Appearance from '$lib/components/settings/Appearance.svelte';
  import { conversationKey, personKey } from '$lib/appearance';
  import { t } from '$lib/i18n.svelte';

  const personId = $derived(page.url.pathname.match(/^\/main\/person\/([^/]+)/)?.[1] ?? null);
  const mergedId = $derived(page.url.pathname.match(/^\/main\/merged\/([^/]+)/)?.[1] ?? null);
  const appearanceKey = $derived(personId ? personKey(decodeURIComponent(personId)) : $conversationStore.data ? conversationKey($conversationStore.data.id) : null);
  const sharedScope = $derived.by((): GetSharedInput | null => {
    if (mergedId) return { lead_conversation_id: decodeURIComponent(mergedId) };
    if (personId) {
      const id = decodeURIComponent(personId);
      const person = $peopleStore.find((p) => p.id === id);
      const ids = [...new Set([...(person?.conversation_ids ?? []), ...$conversationsStore.data.filter((c) => c.person_id === id).map((c) => c.id)])];
      return ids.length ? { conversation_ids: ids } : null;
    }
    return $conversationStore.data ? { conversation_ids: [$conversationStore.data.id] } : null;
  });
  const sharedKey = $derived(sharedScope ? JSON.stringify(sharedScope) : '');
</script>

<aside data-testid="details-panel" class="relative h-full min-w-0 overflow-hidden bg-paper" aria-label={t('conversation.details.title')}>
    {#if personId || mergedId || $conversationStore.data}
      <div class="flex h-full flex-col overflow-y-auto">
        {#if $conversationStore.data}
          <Header />
        {/if}
        <div class="px-5 pb-5">
          {#if personId}
            <Identities personId={decodeURIComponent(personId)} />
          {/if}
          {#if sections.timeline?.sections.length}
            <div data-testid="details-sections" class="border-b border-line py-3">
              <p class="text-[13px] text-ink-3">{t('conversation.details.sections')} <span class="font-mono text-[11px]">{sections.timeline.sections.length}</span></p>
              <div class="flex flex-wrap items-center gap-1.5 pt-2"><SectionActions labelled /></div>
            </div>
          {/if}
          {#if chatExport.ids.length}
            <div class="border-b border-line py-3"><ExportMenu labelled /></div>
          {/if}
          {#if $conversationStore.data}
            <ConversationInfo conversation={$conversationStore.data} people={!personId} />
            <PinnedMessages />
          {/if}
          {#if sharedScope}
            {#key sharedKey}
              <Shared scope={sharedScope} grouped={!!mergedId || !!personId} />
            {/key}
          {/if}
          {#if appearanceKey}
            <div class="pt-4">
              <p class="text-[13px] font-semibold text-ink">{t('conversation.details.appearance')}</p>
              <p class="text-[12.5px] text-ink-3">{personId ? t('conversation.details.appliesPerson') : t('conversation.details.appliesChat')}</p>
              <Appearance scope={appearanceKey} />
            </div>
          {/if}
        </div>
      </div>
    {:else}
      <p class="px-5 pt-8 text-[13px] text-ink-3">{t('conversation.details.noneOpen')}</p>
    {/if}
</aside>
