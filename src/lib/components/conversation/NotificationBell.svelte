<script lang="ts">
  import { NOTIFY_LEVELS } from '$lib/tauri/chats';
  import { anchored, boxOf } from '$lib/popover';
  import { conversationStore, api } from '$lib/tauri';
  import { IconButton, MenuItem } from '$lib/components/ui';
  import { t } from '$lib/i18n.svelte';

  let dropdownOpen = $state(false);
  let bellEl = $state<HTMLDivElement>();

  let notificationLevel = $derived($conversationStore.data?.notification_level || 'all');


  let bellIcon = $derived.by(() => {
    switch (notificationLevel) {
      case 'mentions': return 'lucide:bell-ring';
      case 'nothing': return 'lucide:bell-off';
      default: return 'lucide:bell';
    }
  });

  async function setLevel(level: string) {
    if (!$conversationStore.data) return;
    dropdownOpen = false;
    try {
      await api.putConversationNotification({ conversation_id: $conversationStore.data.id, notification_level: level });
      conversationStore.update(state => {
        if (!state.data) return state;
        return { ...state, data: { ...state.data, notification_level: level } };
      });
    } catch {}
  }

</script>

<div class="relative" bind:this={bellEl}>
  <IconButton testid="chat-notify" icon={bellIcon} label={t('conversation.notify.label')} active={notificationLevel !== 'all'} onclick={() => dropdownOpen = !dropdownOpen} />
  {#if dropdownOpen}
    <div class="menu w-48" use:anchored={{ at: () => boxOf(bellEl ?? null), onclose: () => (dropdownOpen = false) }} role="menu">
      {#each NOTIFY_LEVELS as level (level.value)}
        <MenuItem testid={`chat-notify-${level.value}`} icon={level.icon} checked={notificationLevel === level.value} onclick={() => setLevel(level.value)}>{t(level.label)}</MenuItem>
      {/each}
    </div>
  {/if}
</div>
