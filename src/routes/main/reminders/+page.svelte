<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { api } from '$lib/tauri'
  import type { Reminder } from '$lib/tauri/types'
  import { formatDate, t } from '$lib/i18n.svelte'
  import MessageViewList, { type ViewItem } from '$lib/components/inbox/MessageViewList.svelte'

  let reminders = $state<Reminder[]>([])
  let loading = $state(true)

  async function load() {
    loading = true
    try {
      reminders = await api.getReminders(true)
    } catch {
      reminders = []
    } finally {
      loading = false
    }
  }

  async function cancel(id: string) {
    await api.deleteReminder(id).catch(() => false)
    reminders = reminders.filter((reminder) => reminder.id !== id)
  }

  const items = $derived(
    reminders.map((reminder): ViewItem => ({
      id: reminder.id,
      conversationId: reminder.conversation_id,
      messageId: reminder.message_id,
      icon: 'lucide:alarm-clock',
      title: reminder.note || reminder.conversation_name || t('inbox.view.reminders'),
      detail: [t('inbox.reminders.at', { when: formatDate(reminder.at, { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) }), reminder.note ? reminder.conversation_name : null].filter(Boolean).join(' · '),
      at: reminder.at,
      action: { icon: 'lucide:alarm-clock-off', label: t('inbox.reminders.cancel'), run: () => cancel(reminder.id) }
    }))
  )

  onMount(() => {
    load()
    const unlisten = listen('reminders-updated', () => load())
    return () => {
      unlisten.then((stop) => stop())
    }
  })
</script>

<MessageViewList title={t('inbox.view.reminders')} icon="lucide:alarm-clock" testid="reminders" {items} {loading} empty={t('inbox.reminders.emptyTitle')} hint={t('inbox.reminders.emptyHint')} />
