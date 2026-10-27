<script lang="ts">
  import { conversationStore } from '$lib/tauri'
  import { Kbd } from '$lib/components/ui'
  import { t } from '$lib/i18n.svelte'

  const conversation = $derived($conversationStore.data)
  const conversationName = $derived(conversation?.name || t('conversation.empty.thisConversation'))
</script>

<div class="relative flex h-full flex-col items-center justify-center bg-paper px-6 text-center">
  <p class="relative text-[15px] font-semibold tracking-tight text-ink">{t('conversation.empty.nothing')}</p>
  <p class="relative mt-1 max-w-sm text-[13px] text-ink-3">{t('conversation.empty.start', { name: conversationName })} <Kbd keys="R" /> {t('conversation.empty.startAfter')}</p>
</div>
