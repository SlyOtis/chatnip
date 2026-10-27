<script lang="ts">
  import {onMount} from "svelte"
  import {api, conversationStore, messagesStore} from "$lib/tauri"
  import type {PageProps} from "./$types"
  import {goto} from "$app/navigation"
  import {page} from "$app/state"
  import { t } from '$lib/i18n.svelte'

  let {data}: PageProps = $props();

  const loadConversation = async (): Promise<void> => {
    try {
      const conversation = await api.putCurrentConversation(data.conversationId)
      conversationStore.set({ data: { ...conversation, is_current: true }, is_loading: false })
      messagesStore.set(await api.getMessages())
      await goto(`/main/conversation/${data.conversationId}/chat${page.url.hash}`)
    } catch {
      await goto(`/main/conversation/${data.conversationId}/error`)
    }
  }

  onMount(() => loadConversation())
</script>

<div class="flex h-full flex-col items-center justify-center gap-3 bg-paper">
  <div class="h-px w-24 overflow-hidden bg-line">
    <div class="sweep h-full w-1/3 bg-accent"></div>
  </div>
  <p class="font-mono text-[11px] text-ink-3">{t('conversation.opening')}</p>
</div>

<style>
  .sweep {
    animation: sweep 1.1s var(--ease-out) infinite;
  }
  @keyframes sweep {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(300%);
    }
  }
</style>
