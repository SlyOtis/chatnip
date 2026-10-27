<script lang="ts">
  import { onMount } from 'svelte'
  import { api, conversationsById } from '$lib/tauri'
  import type { BookmarkedMessage } from '$lib/tauri/types'
  import { plainText } from '$lib/messageKind'
  import { t } from '$lib/i18n.svelte'
  import MessageViewList, { type ViewItem } from '$lib/components/inbox/MessageViewList.svelte'

  let saved = $state<BookmarkedMessage[]>([])
  let loading = $state(true)

  async function load() {
    loading = true
    try {
      saved = await api.getBookmarks()
    } catch {
      saved = []
    } finally {
      loading = false
    }
  }

  async function unsave(message_id: string) {
    await api.deleteBookmark({ message_id }).catch(() => {})
    saved = saved.filter((entry) => entry.message_id !== message_id)
  }

  const items = $derived(
    saved.map((entry): ViewItem => {
      const conversation = entry.message ? $conversationsById.get(entry.message.conversation_id) : undefined
      return {
        id: entry.id,
        conversationId: entry.message?.conversation_id ?? null,
        messageId: entry.message_id,
        icon: entry.message?.type === 'email' ? 'lucide:mail' : 'lucide:message-square-text',
        title: entry.message?.sender_name ?? t('common.unknown'),
        detail: [conversation?.name, entry.note, plainText(entry.message?.content)].filter(Boolean).join(' · '),
        at: entry.created_at,
        action: { icon: 'lucide:bookmark-x', label: t('inbox.saved.remove'), run: () => unsave(entry.message_id) }
      }
    })
  )

  onMount(load)
</script>

<MessageViewList title={t('inbox.view.saved')} icon="lucide:bookmark" testid="saved" {items} {loading} empty={t('inbox.saved.emptyTitle')} hint={t('inbox.saved.emptyHint')} />
