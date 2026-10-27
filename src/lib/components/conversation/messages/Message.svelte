<script lang="ts">
  import { myUserIds, isMine } from '$lib/tauri/people'
  import type { Message } from '$lib/tauri'
  import { setMessageGroupContext } from './messageContext'
  import { get } from 'svelte/store'
  import ChatMessage from './ChatMessage.svelte'
  import EmailMessage from './EmailMessage.svelte'
  import BotMessage from './BotMessage.svelte'
  import SystemMessage from './SystemMessage.svelte'
  import TextMessage from './TextMessage.svelte'

  interface Props {
    message: Message;
    showAvatar?: boolean;
    showSenderName?: boolean;
    position?: 'first' | 'middle' | 'last' | 'single';
  }

  let { message, showAvatar = true, showSenderName = true, position = 'single' }: Props = $props();

  const isMe = isMine(get(myUserIds), message.sender_id);

  setMessageGroupContext({
    is_grouped: position !== 'single',
    position,
    showAvatar,
    showSenderName,
    showDateHeader: false,
    dateHeaderText: undefined,
    isCurrentUser: isMe
  });
</script>

{#if message.type === 'system'}
  <SystemMessage {message} />
{:else if message.type === 'email'}
  <EmailMessage {message} />
{:else if message.type === 'bot'}
  <BotMessage {message} />
{:else if message.type === 'chat'}
  <ChatMessage {message} />
{:else}
  <TextMessage {message} />
{/if}
