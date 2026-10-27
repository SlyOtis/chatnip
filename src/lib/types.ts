import type { Message } from '$lib/tauri'

export interface SearchModalData {
  query: string;
  conversation_id?: string;
  filters?: {
    provider?: string;
    conversation_type?: string;
    dateRange?: {
      start: string;
      end: string;
    };
  };
}
export interface ImageModalData {
  source: string;
  alt?: string;
  width?: number;
  height?: number;
}
export interface DetailsModalData {
  conversation_id?: string;
  message_id?: string;
  type?: 'conversation' | 'message' | 'user';
}
export interface ImageUploadModalData {
  currentImage?: string | null;
  newImage?: string;
  title?: string;
  library?: boolean;
  resettable?: boolean;
  onpick?: (image: string | null) => void;
}
export interface EmailComposeModalData {
  mode: 'new' | 'reply' | 'reply_all' | 'forward';
  original_message?: Message;
  to?: string[];
  subject?: string;
  conversation_id?: string;
  provider_id?: string;
  body?: string;
  cc?: string[];
  bcc?: string[];
  references?: Record<string, unknown>;
}
export interface DiscussModalData {
  message: Message;
  conversation_id: string;
}
export interface StatusModalData {
  initial_emoji?: string;
  initial_text?: string;
  provider_id?: string;
}
export interface ShortcutsModalData {
  shortcuts: true;
}
export interface ChannelBrowserModalData {
  browseChannels: true;
}
export interface ModalsState {
  search: SearchModalData | null
  image: ImageModalData | null
  details: DetailsModalData | null
  isOpen: boolean
  imageUpload: ImageUploadModalData | null
  emailCompose: EmailComposeModalData | null
  discuss: DiscussModalData | null
  setStatus: StatusModalData | null
  shortcuts: ShortcutsModalData | null
  channelBrowser: ChannelBrowserModalData | null
}
