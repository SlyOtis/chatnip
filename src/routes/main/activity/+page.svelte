<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { api } from '$lib/tauri'
  import type { ActivityItem } from '$lib/tauri/types'
  import { plainText } from '$lib/messageKind'
  import { t } from '$lib/i18n.svelte'
  import MessageViewList, { type ViewItem } from '$lib/components/inbox/MessageViewList.svelte'

  const ICONS: Record<ActivityItem['kind'], string> = { mention: 'lucide:at-sign', reply: 'lucide:reply', reaction: 'lucide:smile-plus' }

  let found = $state<ActivityItem[]>([])
  let cursor = $state<string | null>(null)
  let loading = $state(true)

  async function load(more = false) {
    loading = true
    try {
      const page = await api.getActivity(more ? cursor : null)
      found = more ? [...found, ...page.items] : page.items
      cursor = page.cursor
    } catch {
      if (!more) found = []
    } finally {
      loading = false
    }
  }

  function title(item: ActivityItem): string {
    const who = item.actor ?? item.message.sender_name ?? t('common.unknown')
    if (item.kind === 'reaction') return t('inbox.activity.reacted', { name: who, emoji: item.detail ?? '' })
    if (item.kind === 'reply') return t('inbox.activity.replied', { name: who })
    return t('inbox.activity.mentioned', { name: who })
  }

  const items = $derived(
    found.map((item, i): ViewItem => ({
      id: `${item.kind}:${item.message.id}:${i}`,
      conversationId: item.conversation_id,
      messageId: item.message.id,
      icon: ICONS[item.kind],
      title: title(item),
      detail: [item.conversation_name, plainText(item.message.content)].filter(Boolean).join(' · '),
      at: item.at
    }))
  )

  onMount(() => {
    load()
    const unlisten = listen('conversations-updated', () => load())
    return () => {
      unlisten.then((stop) => stop())
    }
  })
</script>

<MessageViewList title={t('inbox.view.activity')} icon="lucide:at-sign" testid="activity" {items} {loading} empty={t('inbox.activity.emptyTitle')} hint={t('inbox.activity.emptyHint')} more={cursor ? () => load(true) : null} />
