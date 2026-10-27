<script lang="ts">
  import type { Message } from '$lib/tauri';
  import { getMessageGroupContext } from './messageContext';
  import SingleMessage from './SingleMessage.svelte';

  interface Props {
    message: Message;
  }

  let { message }: Props = $props();

  const context = getMessageGroupContext();
  const isMe = context?.isCurrentUser ?? false;
  const position = context?.position ?? 'single';
</script>

<SingleMessage {message} is_me={isMe} is_first={position === 'first' || position === 'single'} is_last={position === 'last' || position === 'single'} />
