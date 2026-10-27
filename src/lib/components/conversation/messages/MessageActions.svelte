<script lang="ts">
  import type { Message } from '$lib/tauri'
  import { IconButton, TagPicker } from '$lib/components/ui'
  import { selection } from '$lib/reply.svelte'
  import EmojiPicker from '$lib/components/EmojiPicker.svelte'
  import { boxOf, type Box } from '$lib/popover'
  import { t } from '$lib/i18n.svelte'

  interface Props {
    message: Message;
    is_me: boolean;
    onreact: (emoji: string) => void;
    onreply: () => void;
    onforward: () => void;
    replies?: boolean;
    forwards?: boolean;
    onthread: () => void;
    onmore: (event: MouseEvent) => void;
    read?: boolean;
    selected?: boolean;
    onselect?: () => void;
    onedit?: () => void;
    oncollapse?: () => void;
  }

  let { message, is_me, onreact, onreply, onforward, onthread, onmore, read = false, replies = !read, forwards = true, selected = false, onselect, onedit, oncollapse }: Props = $props()

  let picker_at = $state<Box | null>(null)
  let tag_at = $state<Box | null>(null)

  const tagTargets = $derived(selection.ids.includes(message.id) && selection.ids.length > 1 ? selection.ids : [message.id])

  function handleEmojiSelect(emoji: string) {
    picker_at = null
    onreact(emoji)
  }
</script>

<span class="flex items-center gap-0.5" role="toolbar" data-testid="message-actions" data-id={message.id} aria-label={t('conversation.actions.label')}>
  {#if replies}
    <IconButton testid="message-reply" icon="lucide:reply" label={t('conversation.actions.reply')} keys="R" size="sm" onclick={onreply} />
  {/if}
  {#if forwards}
    <IconButton testid="message-forward" icon="lucide:forward" label={t('conversation.actions.forward')} keys="F" size="sm" onclick={onforward} />
  {/if}
  {#if !read}
    <IconButton testid="message-react" icon="lucide:smile-plus" label={t('conversation.actions.react')} size="sm" onclick={(e) => (picker_at = picker_at ? null : boxOf(e.currentTarget))} />
  {/if}
  {#if picker_at}
    <EmojiPicker at={picker_at} providerId={message.provider_id} onselect={handleEmojiSelect} onclose={() => (picker_at = null)} />
  {/if}
  {#if message.type !== 'email' && !read}
    <IconButton testid="message-thread" icon="lucide:messages-square" label={t('conversation.actions.thread')} size="sm" onclick={onthread} />
  {/if}
  {#if onedit && is_me && !read && message.type === 'chat'}
    <IconButton testid="message-edit" icon="lucide:pencil" label={t('common.edit')} size="sm" onclick={onedit} />
  {/if}
  {#if onselect}
    <IconButton testid="message-select" icon="lucide:circle-check" label={t('conversation.actions.select')} active={selected} size="sm" onclick={onselect} />
  {/if}
  <IconButton testid="message-tag" icon="lucide:tag" label={t('conversation.menu.tags')} size="sm" onclick={(e) => (tag_at = tag_at ? null : boxOf(e.currentTarget))} />
  {#if tag_at}
    <TagPicker objectType="message" ids={tagTargets} at={tag_at} onclose={() => (tag_at = null)} />
  {/if}
  {#if oncollapse}
    <IconButton testid="message-collapse" icon="lucide:chevrons-down-up" label={t('conversation.actions.collapse')} keys="O" size="sm" onclick={oncollapse} />
  {/if}
  <IconButton testid="message-more" icon="lucide:ellipsis" label={t('common.more')} size="sm" onclick={(e) => onmore(e)} />
</span>
