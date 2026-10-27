import { getContext, setContext } from 'svelte';

export interface MessageGroupContext {
  is_grouped: boolean;
  position: 'first' | 'middle' | 'last' | 'single';
  showAvatar: boolean;
  showSenderName: boolean;
  showDateHeader: boolean;
  dateHeaderText?: string;
  isCurrentUser: boolean;
}

const MESSAGE_GROUP_KEY = Symbol('message-group');

export function setMessageGroupContext(context: MessageGroupContext) {
  setContext(MESSAGE_GROUP_KEY, context);
}

export function getMessageGroupContext(): MessageGroupContext | undefined {
  return getContext<MessageGroupContext>(MESSAGE_GROUP_KEY);
}