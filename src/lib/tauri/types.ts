export interface Profile {
  name: string;
  user_id: string | null;
  avatar: Asset | null;
  setup_step: number;
  last_id: string | null;
  settings: ProfileSettings | null;
  theme: ProfileTheme | null;
}

export interface WindowPosition {
  x: number;
  y: number;
  width: number;
  height: number;
  monitor: string;
  scale: number;
  maximized: boolean;
  fullscreen: boolean;
}

export interface ProfileSettings {
  email_content_threshold: number;
  navigation_width: number;
  navigation_collapsed: boolean;
  details_width: number;
  details_panel_open: boolean;
  window_position: WindowPosition | null;
  message_timestamp_interval: number;
  time_format: string;
  window_controls_style: string;
  max_input_lines: number;
}

export interface ProfileTheme extends Theme {
  theme_name: string;
  is_active: boolean;
}


export interface Conversation {
  id: string;
  name: string;
  provider_name: string;
  image: string | null;
  overrides: LocalFields;
  nicknames: Record<string, string>;
  type: ConversationType;
  last_id: string | null;
  last_activity: string;
  providers: Provider[];
  theme: ProfileTheme | null;
  namespace: string | null;
  metadata: string | null;
  unread_count: number;
  members: User[];
  is_current: boolean;
  summary: string | null;
  presentation?: Presentation | null;
  group_type: string;
  is_pinned: boolean;
  is_muted: boolean;
  mode: ChatMode;
  view: ChatView;
  view_settings: ViewSettings;
  is_archived: boolean;
  snoozed_until: string | null;
  deleted_at: string | null;
  person_id: string | null;
  favourite_position: number | null;
  tags: ObjectTag[];
  notification_level: string;
  threads: Thread[];
  created_at: string;
  updated_at: string;
}

export interface Message {
  id: string;
  external_id?: string;
  reply_to_id?: string | null;
  type: MessageType;
  conversation_id: string;
  provider_id: string;
  provider_name: string;
  sender_id: string;
  sender_name: string | null;
  sender_email: string | null;
  subject: string | null;
  recipients: EmailRecipient[];
  cc_recipients: EmailRecipient[];
  bcc_recipients: EmailRecipient[];
  content: string;
  content_type: string;
  summary?: string | null;
  presentation?: Presentation | null;
  timestamp: string;
  metadata: string | null;
  cached_at: string | null;
  thread_id: string | null;
  has_attachments: boolean;
  attachments: Attachment[];
  last_modified_datetime: string;
  deleted_datetime: string | null;
  is_read: boolean;
  created_at: string;
  updated_at: string;
}

export interface EmailRecipient {
  name: string | null;
  address: string;
}

export type SystemEventKind = 'member_joined' | 'member_left' | 'member_added' | 'member_removed' | 'renamed' | 'topic_changed' | 'call_started' | 'call_ended' | 'pinned' | 'unpinned' | 'chat_created';

export interface SystemPerson {
  user_id: string | null;
  external_id?: string | null;
  name: string;
}

export interface SystemEvent {
  event: SystemEventKind;
  actors: SystemPerson[];
  targets: SystemPerson[];
  detail?: string | null;
}

export interface Thread {
  id: string;
  conversation_id: string;
  root_message_id: string;
  name: string | null;
  last_activity: string;
  message_count: number;
  is_resolved: boolean;
  created_at: string;
  updated_at: string;
}

export interface MergeGroup {
  id: string;
  lead_conversation_id: string;
  name: string | null;
  parent_id: string | null;
  conversation_ids: string[];
  created_at: string;
  updated_at: string;
}

export interface MergeGroupInput {
  lead_conversation_id: string;
  conversation_ids: string[];
  group_lead_ids: string[];
}

export interface MessageSnippet {
  message_id: string;
  snippet: string;
}

export interface SearchResults {
  conversations: Conversation[];
  messages: Message[];
  snippets: MessageSnippet[];
}

export type ConversationType = 'direct_message' | 'group_message' | 'channel' | 'email' | 'local' | 'unknown';

export type MessageType = 'chat' | 'email' | 'system' | 'bot' | 'unknown';

export interface Theme {
  theme_name: string;
  primary_50: string;
  primary_100: string;
  primary_200: string;
  primary_300: string;
  primary_400: string;
  primary_500: string;
  primary_600: string;
  primary_700: string;
  primary_800: string;
  primary_900: string;
  primary_950: string;
  secondary_50: string;
  secondary_100: string;
  secondary_200: string;
  secondary_300: string;
  secondary_400: string;
  secondary_500: string;
  secondary_600: string;
  secondary_700: string;
  secondary_800: string;
  secondary_900: string;
  secondary_950: string;
  accent_50: string;
  accent_100: string;
  accent_200: string;
  accent_300: string;
  accent_400: string;
  accent_500: string;
  accent_600: string;
  accent_700: string;
  accent_800: string;
  accent_900: string;
  accent_950: string;
  background_50: string;
  background_100: string;
  background_200: string;
  background_300: string;
  background_400: string;
  background_500: string;
  background_600: string;
  background_700: string;
  background_800: string;
  background_900: string;
  background_950: string;
  content_50: string;
  content_100: string;
  content_200: string;
  content_300: string;
  content_400: string;
  content_500: string;
  content_600: string;
  content_700: string;
  content_800: string;
  content_900: string;
  content_950: string;
  control_50: string;
  control_100: string;
  control_200: string;
  control_300: string;
  control_400: string;
  control_500: string;
  control_600: string;
  control_700: string;
  control_800: string;
  control_900: string;
  control_950: string;
  error_50: string;
  error_100: string;
  error_200: string;
  error_300: string;
  error_400: string;
  error_500: string;
  error_600: string;
  error_700: string;
  error_800: string;
  error_900: string;
  error_950: string;
  warn_50: string;
  warn_100: string;
  warn_200: string;
  warn_300: string;
  warn_400: string;
  warn_500: string;
  warn_600: string;
  warn_700: string;
  warn_800: string;
  warn_900: string;
  warn_950: string;
  success_50: string;
  success_100: string;
  success_200: string;
  success_300: string;
  success_400: string;
  success_500: string;
  success_600: string;
  success_700: string;
  success_800: string;
  success_900: string;
  success_950: string;
}

export interface ConversationsResponse {
  data: Conversation[];
  is_loading: boolean;
}

export interface ConversationResponse {
  data: Conversation | null;
  is_loading: boolean;
}

export interface ProfileResponse {
  data: Profile | null;
  is_loading: boolean;
}


export interface UsersResponse {
  data: User[];
  is_loading: boolean;
}

export interface ProvidersResponse {
  data: Provider[];
  is_loading: boolean;
}

export interface MessagesResponse {
  data: Message[];
  is_loading: boolean;
}

export interface Provider {
  id: string;
  profile_id: string;
  provider_type: string;
  name: string;
  logo_id: string | null;
  logo: Asset | null;
  external_id: string | null;
  capabilities: ProviderCapabilities;
  is_enabled: boolean;
  is_connected: boolean;
  sync_interval: number;
  namespace: string | null;
  last_sync_at: string | null;
  created_at: string;
  updated_at: string;
  theme: Theme;
  missing_scopes: string[];
  account: Account | null;
  last_used_at: string | null;
}

export interface Account {
  user_id: string;
  display_name: string | null;
  email: string | null;
  avatar_url: string | null;
  avatar_data: string | null;
}

export interface ProviderCapabilities {
  can_send_messages: boolean;
  can_edit_messages: boolean;
  can_delete_messages: boolean;
  can_create_conversations: boolean;
  can_add_members: boolean;
  supports_reactions: boolean;
  supports_threads: boolean;
  supports_attachments: boolean;
  supports_presence: boolean;
  supports_typing_indicators: boolean;
  supports_read_receipts: boolean;
  supports_reply_quote: boolean;
  supports_forward: boolean;
  reply_quote: Delivery;
  forward: Delivery;
  supports_calendar_responses: boolean;
  live: boolean;
  supports_unread_sync: boolean;
  thread_broadcast: Delivery;
  drafts: Delivery;
  archive: Delivery;
  supports_flags: boolean;
  supports_channel_create: boolean;
  supports_channel_join: boolean;
  supports_channel_archive: boolean;
  supports_topic: boolean;
  supports_purpose: boolean;
  supports_search: boolean;
  supports_user_groups: boolean;
  supports_aliases: boolean;
  signature_sync: boolean;
  supports_auto_reply: boolean;
}

export type Delivery = 'unsupported' | 'native' | 'repost' | 'mail'

export interface ForwardTarget {
  conversation_id?: string | null;
  person_id?: string | null;
  provider_id?: string | null;
  to?: string[];
}

export interface PostMessageForwardInput {
  message_ids: string[];
  targets: ForwardTarget[];
  comment?: string | null;
}

export interface PostMessageReplyInput {
  message_id: string;
  content: string;
  content_type?: string;
  provider_id?: string;
  reply_all?: boolean;
}

export interface User {
  id: string;
  email: string | null;
  phone: string | null;
  display_name: string | null;
  first_name: string | null;
  last_name: string | null;
  avatar_url: string | null;
  avatar_data: string | null;
  job_title: string | null;
  department: string | null;
  company: string | null;
  office_location: string | null;
  first_seen: string;
  last_updated: string;
}

export interface Asset {
  id: string;
  profile_id: string;
  name: string;
  url: string | null;
  type: string;
  mime_type: string;
  path: string | null;
  created_at: string;
  updated_at: string;
}

export interface Attachment {
  id: string;
  name: string;
  content_type: string;
  size: number;
  is_inline: boolean;
  content_id: string | null;
  url?: string | null;
}

// Tauri Command Input Types
export interface CreateProfileInput {
  name: string;
}

export interface UpdateProfileInput {
  name?: string;
  avatar_id?: string;
  theme_id?: string;
  setup_step?: number;
}

export interface ProfileSettingInput {
  email_content_threshold: number;
  navigation_width: number;
  navigation_collapsed: boolean;
  details_width: number;
  details_panel_open: boolean;
  message_timestamp_interval: number;
  time_format: string;
  window_controls_style: string;
  max_input_lines: number;
}

export interface UpdateProfileSettingsInput {
  settings: ProfileSettingInput;
}

export interface ThemeInput {
  theme_name: string;
  is_active: boolean;
  primary_50: string;
  primary_100: string;
  primary_200: string;
  primary_300: string;
  primary_400: string;
  primary_500: string;
  primary_600: string;
  primary_700: string;
  primary_800: string;
  primary_900: string;
  primary_950: string;
  secondary_50: string;
  secondary_100: string;
  secondary_200: string;
  secondary_300: string;
  secondary_400: string;
  secondary_500: string;
  secondary_600: string;
  secondary_700: string;
  secondary_800: string;
  secondary_900: string;
  secondary_950: string;
  accent_50: string;
  accent_100: string;
  accent_200: string;
  accent_300: string;
  accent_400: string;
  accent_500: string;
  accent_600: string;
  accent_700: string;
  accent_800: string;
  accent_900: string;
  accent_950: string;
  background_50: string;
  background_100: string;
  background_200: string;
  background_300: string;
  background_400: string;
  background_500: string;
  background_600: string;
  background_700: string;
  background_800: string;
  background_900: string;
  background_950: string;
  content_50: string;
  content_100: string;
  content_200: string;
  content_300: string;
  content_400: string;
  content_500: string;
  content_600: string;
  content_700: string;
  content_800: string;
  content_900: string;
  content_950: string;
  control_50: string;
  control_100: string;
  control_200: string;
  control_300: string;
  control_400: string;
  control_500: string;
  control_600: string;
  control_700: string;
  control_800: string;
  control_900: string;
  control_950: string;
  error_50: string;
  error_100: string;
  error_200: string;
  error_300: string;
  error_400: string;
  error_500: string;
  error_600: string;
  error_700: string;
  error_800: string;
  error_900: string;
  error_950: string;
  warn_50: string;
  warn_100: string;
  warn_200: string;
  warn_300: string;
  warn_400: string;
  warn_500: string;
  warn_600: string;
  warn_700: string;
  warn_800: string;
  warn_900: string;
  warn_950: string;
  success_50: string;
  success_100: string;
  success_200: string;
  success_300: string;
  success_400: string;
  success_500: string;
  success_600: string;
  success_700: string;
  success_800: string;
  success_900: string;
  success_950: string;
}

export interface CreateProfileThemeInput {
  theme: ThemeInput;
}

export interface UpdateProfileThemeInput {
  theme_id: string;
  theme: ThemeInput;
}

export interface CreateProfileAvatarInput {
  avatar_data: string;
}

export interface UpdateProfileAvatarInput {
  avatar_data: string;
}

export interface SendMessageInput {
  conversation_id: string;
  content: string;
  content_type?: string;
  provider_id?: string;
  delivery_mode?: 'selected' | 'all';
  metadata?: Record<string, unknown>;
  attachments?: OutgoingAttachmentInput[];
}

export interface SendMessageReactionInput {
  conversation_id: string;
  message_id: string;
  reaction: string;
}

export interface EditMessageInput {
  message_id: string;
  content: string;
}

export interface DeleteMessageInput {
  message_id: string;
}

export interface PinMessageInput {
  conversation_id: string;
  message_id: string;
}

export interface GetPinnedInput {
  conversation_id: string;
}

export interface BookmarkMessageInput {
  message_id: string;
  note?: string;
}

export interface UnbookmarkMessageInput {
  message_id: string;
}

export interface MarkMessageUnreadInput {
  message_id: string;
}

export interface MarkConversationReadInput {
  conversation_id: string;
}

export interface SetUserStatusInput {
  status: string;
  status_text: string;
  status_emoji: string;
  status_expiration: string | null;
  provider_id?: string | null;
}

export interface GetUserStatusInput {
  user_id: string;
}

export interface UserStatus {
  user_id: string;
  status: string;
  status_text: string;
  status_emoji: string;
  status_expiration: string | null;
  updated_at: string;
}

export interface PinnedMessage {
  id: string;
  conversation_id: string;
  message_id: string;
  pinned_by: string | null;
  created_at: string;
  message: Message | null;
}

export interface BookmarkedMessage {
  id: string;
  message_id: string;
  profile_id: string;
  note: string | null;
  created_at: string;
  message: Message | null;
}

export interface SetNotificationInput {
  conversation_id: string;
  notification_level: string;
}

export interface CreateSidebarSectionInput {
  name: string;
  position?: number;
}

export interface UpdateSidebarSectionInput {
  id: string;
  name?: string;
  position?: number;
  is_collapsed?: boolean;
}

export interface DeleteSidebarSectionInput {
  id: string;
}

export interface SidebarSectionItemInput {
  section_id: string;
  conversation_id: string;
  position?: number;
}

export interface RemoveSidebarSectionItemInput {
  section_id: string;
  conversation_id: string;
}

export interface SidebarSection {
  id: string;
  profile_id: string;
  name: string;
  position: number;
  is_collapsed: boolean;
  conversations: Conversation[];
  created_at: string;
}
export type TagKind = 'split' | 'tag';

export interface Tag {
  id: string;
  profile_id: string;
  name: string;
  color: string;
  kind: TagKind;
  position: number;
  description: string;
  icon: string | null;
  favourite: boolean;
  favourite_position: number | null;
  created_at: string;
  updated_at: string;
}

export type ObjectType = 'conversation' | 'message' | 'person' | 'identity' | 'content' | 'action' | 'action_group' | 'attachment' | 'event' | 'group' | 'inbox' | 'merge_group';

export interface ObjectTag {
  tag_id: string;
  object_type: ObjectType;
  object_id: string;
  source: string;
  rule_id: string | null;
  reason: string | null;
  created_at: string;
}

export interface TaggedObject {
  tag_id: string;
  object_type: ObjectType;
  object_id: string;
  title: string;
  detail: string | null;
  conversation_id: string | null;
  conversation_name: string | null;
  message_id: string | null;
  person_id: string | null;
  author: string | null;
  timestamp: string | null;
  excerpt: string | null;
  created_at: string;
}

export interface PostTagInput {
  name: string;
  color?: string;
  kind?: TagKind;
  position?: number;
  description?: string;
}

export interface PutTagInput {
  tag_id: string;
  name?: string;
  color?: string;
  position?: number;
  description?: string;
  icon?: string;
  favourite?: boolean;
}

export interface ObjectTagInput {
  object_type: ObjectType;
  object_id: string;
  tag_id: string;
}

export type RuleField =
  | 'provider_type'
  | 'connection_name'
  | 'conversation_type'
  | 'conversation_name'
  | 'sender_email'
  | 'sender_domain'
  | 'sender_name'
  | 'subject'
  | 'body'
  | 'member_count'
  | 'namespace';

export type RuleOperator =
  | 'is'
  | 'is_not'
  | 'contains'
  | 'not_contains'
  | 'starts_with'
  | 'ends_with'
  | 'matches'
  | 'greater_than'
  | 'less_than';

export interface RuleCondition {
  field: RuleField;
  operator: RuleOperator;
  value: string;
}

export interface AssistantRun {
  id: string;
  kind: string;
  model: string;
  output: unknown;
  cost_usd: number;
  duration_ms: number;
  created_at: string;
}

export interface CalendarEvent {
  id: string;
  profile_id: string;
  title: string;
  notes: string;
  location: string;
  starts_at: string;
  ends_at: string;
  all_day: boolean;
  status: 'proposed' | 'confirmed' | 'dismissed';
  origin: 'manual' | 'parsed' | 'synced';
  confidence: number;
  source_message_id: string | null;
  conversation_id: string | null;
  attendees: string[];
  created_at: string;
  updated_at: string;
  provider_id: string | null;
  calendar_id: string | null;
  external_id: string | null;
  ical_uid: string | null;
  online_meeting_url: string | null;
  organizer: string | null;
  response_status: string | null;
  link_id: string | null;
  unmerged: boolean;
  sources: EventSource[];
}

export interface EventSource {
  event_id: string;
  calendar_id: string | null;
  calendar_name: string;
  colour: string | null;
  provider_id: string | null;
  provider_type: string | null;
  provider_external_id: string | null;
  response_status: string | null;
  online_meeting_url: string | null;
}

export interface ProviderCalendar {
  id: string;
  profile_id: string;
  provider_id: string;
  provider_type: string;
  provider_name: string;
  provider_external_id: string | null;
  external_id: string;
  name: string;
  colour: string | null;
  is_primary: boolean;
  can_write: boolean;
  enabled: boolean;
}

export interface PostEventInput {
  draft: EventDraft;
  targets?: string[];
  online_meeting?: boolean;
}

export interface PostEventResult {
  events: CalendarEvent[];
  failures: string[];
}

export interface EventDraft {
  title: string;
  notes?: string;
  location?: string;
  starts_at: string;
  ends_at?: string | null;
  all_day?: boolean;
  attendees?: string[];
}

export interface EventRangeInput {
  start: string;
  end: string;
  include_dismissed?: boolean;
}

export interface PutEventInput {
  event_id: string;
  draft?: EventDraft;
  status?: 'proposed' | 'confirmed' | 'dismissed';
}

export interface PersonIdentity {
  id: string;
  person_id: string;
  user_id: string | null;
  provider_type: string;
  external_id: string | null;
  email: string | null;
  display_name: string | null;
  manual: boolean;
}

export interface PostIdentityInput {
  person_id: string;
  provider_type: string;
  email?: string | null;
  external_id?: string | null;
  display_name?: string | null;
}

export interface Person {
  id: string;
  profile_id: string;
  display_name: string;
  provider_name: string;
  primary_email: string | null;
  avatar_url: string | null;
  avatar_data: string | null;
  overrides: LocalFields;
  nicknames: Record<string, string>;
  is_human: boolean;
  favourite_position: number | null;
  identities: PersonIdentity[];
  conversation_ids: string[];
  provider_types: string[];
  last_activity: string | null;
  unread_count: number;
  created_at: string;
  updated_at: string;
}

export interface PutPersonInput {
  person_id: string;
  display_name?: string;
  is_human?: boolean;
  favourite_position?: number | null;
}

export interface FavouriteOrderItem {
  kind: 'person' | 'conversation';
  id: string;
}

export type MockClientKind = 'teams' | 'outlook' | 'slack' | 'gmail' | 'google_chat' | 'imap';

export type MockServerState = 'ok' | 'expired' | 'rate_limited' | 'offline' | 'denied';

export interface MockMode {
  enabled: boolean;
  forced: boolean | null;
  states: Record<string, MockServerState>;
}

export type CalendarResponse = 'accepted' | 'declined' | 'tentativelyAccepted';

export interface ProviderSetup {
  provider_type: 'microsoft' | 'google' | 'slack';
  ready: boolean;
  redirect_uri: string;
  scopes: string[];
  admin_scopes: string[];
  tenant: string | null;
  admin_consent_url: string | null;
  console_url: string;
}

export type ContentKind = 'gif' | 'emoji' | 'image' | 'video' | 'sticker' | 'file' | 'reply' | 'email' | 'prompt' | 'snippet' | 'signature';

export type ContentTemplateKind = 'reply' | 'email' | 'prompt' | 'snippet' | 'signature';

export type ContentSourceKind = 'upload' | 'message' | 'tenor' | 'giphy' | 'library' | 'url' | 'conversation' | 'person' | 'action' | 'manual';

export type ContentSort = 'relevance' | 'used' | 'recent' | 'newest';

export interface ContentItem {
  id: string;
  profile_id: string;
  kind: ContentKind;
  name: string;
  source_url: string | null;
  path: string;
  mime_type: string;
  width: number | null;
  height: number | null;
  tags: string[];
  is_favourite: boolean;
  used_count: number;
  last_used_at: string | null;
  created_at: string;
  favourite_position: number | null;
  query: string | null;
  keywords: string[];
  body: string | null;
  source_kind: ContentSourceKind | null;
  source_message_id: string | null;
  source_conversation_id: string | null;
  source_provider_id: string | null;
  source_person_id: string | null;
  source_action_id: string | null;
  content_hash: string | null;
  size_bytes: number | null;
  duration_ms: number | null;
  group_ids: string[];
  file_path: string | null;
}

export interface ContentTag {
  name: string;
  count: number;
}

export type ContentGroupKind = 'manual' | 'kind' | 'tag';

export interface ContentGroup {
  id: string;
  profile_id: string;
  parent_id: string | null;
  name: string;
  kind: ContentGroupKind;
  rule: string | null;
  position: number;
  is_collapsed: boolean;
  icon: string | null;
  content_ids: string[];
  created_at: string;
}

export interface PostContentGroupInput {
  name: string;
  kind?: ContentGroupKind;
  parent_id?: string | null;
  rule?: string | null;
  icon?: string | null;
  content_ids?: string[];
}

export interface PutContentGroupInput {
  content_group_id: string;
  name?: string;
  parent_id?: string | null;
  rule?: string | null;
  position?: number;
  is_collapsed?: boolean;
  icon?: string | null;
}

export interface ContentGroupItemsInput {
  content_group_id: string;
  content_ids: string[];
  position?: number;
}

export interface TemplateVariable {
  name: string;
  kind: 'text' | 'list' | 'flag';
  fields: string[];
  sample: string;
}

export interface PostContentRenderInput {
  content_id?: string;
  body?: string;
  values?: Record<string, unknown>;
  conversation_id?: string;
  message_id?: string;
  used?: boolean;
  sample?: boolean;
}

export interface RenderedContent {
  text: string;
  variables: TemplateVariable[];
  missing: string[];
}

export interface ContentUse {
  action_id: string;
  action_name: string;
  node_id: string;
  node_label: string;
  via: 'prompt' | 'script';
}

export interface ReactionUse {
  emoji: string;
  used_count: number;
  last_used_at: string;
}

export interface PutContentBulkInput {
  content_ids: string[];
  add_tags?: string[];
  remove_tags?: string[];
  add_group_ids?: string[];
  remove_group_ids?: string[];
  is_favourite?: boolean;
}

export type GifSource = 'tenor' | 'giphy' | 'library';

export type GifCursor = Record<string, string>;

export interface GifResult {
  id: string;
  source: GifSource;
  kind: ContentKind | 'sticker';
  title: string;
  url: string;
  preview_url: string;
  width: number;
  height: number;
  content_id: string | null;
}

export interface GifPage {
  results: GifResult[];
  next: GifCursor | null;
  notice: string | null;
}

export type LinkPreviewMode = 'on' | 'off' | 'known-only';

export type LinkPreviewKind = 'article' | 'image' | 'video' | 'audio' | 'pdf' | 'repo' | 'pull' | 'issue' | 'post';

export interface LinkPreview {
  url: string;
  kind: LinkPreviewKind;
  site_name: string | null;
  title: string | null;
  description: string | null;
  image_url: string | null;
  image: string | null;
  image_width: number | null;
  image_height: number | null;
  favicon_url: string | null;
  favicon: string | null;
  theme_color: string | null;
  author: string | null;
  duration: number | null;
  state: string | null;
  repo: string | null;
  number: number | null;
  file_name: string | null;
  size: number | null;
}

export interface LinkPreviewEntry {
  url: string;
  preview: LinkPreview | null;
  error: string | null;
}

export interface PostContentInput {
  kind: ContentKind | 'auto';
  name: string;
  source_url?: string;
  data?: string;
  mime_type?: string;
  tags?: string[];
  query?: string;
  is_favourite?: boolean;
  group_ids?: string[];
  keywords?: string[];
  source_kind?: ContentSourceKind;
  body?: string;
  source_message_id?: string;
  source_conversation_id?: string;
  source_person_id?: string;
  source_action_id?: string;
}

export interface PutContentInput {
  content_id: string;
  name?: string;
  is_favourite?: boolean;
  tags?: string[];
  keywords?: string[];
  body?: string;
  kind?: ContentTemplateKind;
}

export interface OutgoingAttachmentInput {
  content_id?: string;
  name?: string;
  mime_type?: string;
  data?: string;
}

export type GroupKind = 'manual' | 'tag' | 'query';

export type GroupSort = 'activity' | 'name' | 'unread' | 'messages' | 'tags' | 'created';

export interface GroupQuery {
  conversation_ids?: string[];
  providers?: string[];
  connection_ids?: string[];
  types?: ConversationType[];
  tag_ids?: string[];
  person_ids?: string[];
  unread?: boolean;
  has_attachments?: boolean;
  humans_only?: boolean;
  from?: string | null;
  to?: string | null;
  sender?: string | null;
  text?: string | null;
  regex?: string | null;
  sort: GroupSort;
  direction: 'asc' | 'desc';
  limit?: number | null;
}

export interface Group {
  id: string;
  profile_id: string;
  parent_id: string | null;
  name: string;
  kind: GroupKind;
  tag_id: string | null;
  query: GroupQuery | null;
  position: number;
  is_collapsed: boolean;
  icon: string | null;
  color: string | null;
  mode: ChatMode;
  conversation_ids: string[];
  created_at: string;
}

export interface PostGroupInput {
  name: string;
  kind?: GroupKind;
  parent_id?: string | null;
  tag_id?: string | null;
  query?: GroupQuery | null;
  icon?: string | null;
  color?: string | null;
}

export interface PutGroupInput {
  group_id: string;
  name?: string;
  parent_id?: string | null;
  tag_id?: string | null;
  query?: GroupQuery | null;
  position?: number;
  is_collapsed?: boolean;
  icon?: string | null;
  color?: string | null;
  mode?: ChatMode;
}

export interface GroupItemsInput {
  group_id: string;
  conversation_ids: string[];
}

export interface StatsRangeInput {
  start: string;
  end: string;
  person_ids?: string[];
  provider_ids?: string[];
  tag_ids?: string[];
}

export interface PersonStat {
  person_id: string | null;
  name: string;
  messages: number;
  characters: number;
  avg_length: number;
  to_me: number;
  from_me: number;
  median_response_minutes: number | null;
  my_median_response_minutes: number | null;
  first_seen: string | null;
  last_seen: string | null;
  provider_types: string[];
  busiest_hour: number | null;
  busiest_weekday: number | null;
}

export interface ProviderStat {
  provider_id: string;
  provider_type: string;
  name: string;
  messages: number;
  conversations: number;
  people: number;
  unread: number;
}

export interface ConversationStat {
  conversation_id: string;
  name: string;
  type: ConversationType;
  messages: number;
  people: number;
  characters: number;
  last_activity: string;
}

export interface TimelinePoint {
  date: string;
  sent: number;
  received: number;
  unread: number;
}

export interface ResponseBucket {
  label: string;
  minutes: number;
  count: number;
  theirs: number;
  mine: number;
}

export type TrafficKind = 'chat' | 'channel' | 'mail' | 'invite' | 'notification' | 'bot';

export interface KindStat {
  kind: TrafficKind;
  messages: number;
  sent: number;
  received: number;
}

export type FileKind = 'image' | 'pdf' | 'document' | 'spreadsheet' | 'presentation' | 'archive' | 'calendar' | 'audio' | 'video' | 'other';

export interface FileTypeStat {
  kind: FileKind;
  files: number;
  bytes: number;
}

export interface WeekdayReply {
  weekday: number;
  theirs: number | null;
  mine: number | null;
  replies: number;
}

export interface Origin {
  ip: string;
  lat: number;
  lon: number;
  city: string | null;
  country: string | null;
  count: number;
  person_name: string | null;
  provider_type: string;
  last_seen: string;
}

export interface Stats {
  range: { start: string; end: string };
  totals: {
    messages: number;
    sent: number;
    received: number;
    conversations: number;
    people: number;
    services: number;
    attachments: number;
    characters: number;
    human_messages: number;
    service_messages: number;
    median_response_minutes: number | null;
    my_median_response_minutes: number | null;
  };
  by_person: PersonStat[];
  by_provider: ProviderStat[];
  by_conversation: ConversationStat[];
  by_label: { tag_id: string; name: string; color: string; conversations: number; messages: number }[];
  by_hour: number[];
  by_weekday: number[];
  heatmap: number[][];
  timeline: TimelinePoint[];
  response_times: ResponseBucket[];
  words: { word: string; count: number }[];
  origins: Origin[];
  rankings: Ranking[];
  groups: GroupStat[];
  by_kind: KindStat[];
  by_file_type: FileTypeStat[];
  reply_by_weekday: WeekdayReply[];
}

export type IntelligenceKind = 'claude_cli' | 'ollama' | 'replicate' | 'openai_compatible' | 'command';

export interface IntelligenceConnection {
  id: string;
  kind: IntelligenceKind;
  name: string;
  config: Record<string, string>;
  is_default: boolean;
  available: boolean;
  detail: string | null;
  models: string[];
  created_at: string;
}

export interface PostIntelligenceConnectionInput {
  kind: IntelligenceKind;
  name: string;
  config: Record<string, string>;
  is_default?: boolean;
}

export interface PutIntelligenceConnectionInput {
  connection_id: string;
  name?: string;
  config?: Record<string, string>;
  is_default?: boolean;
}

export interface Prompt {
  key: string;
  name: string;
  description: string;
  connection_id: string | null;
  model: string | null;
  system: string;
  template: string;
  output_schema: string | null;
  variables: TemplateVariable[];
}

export type SectionKind = 'section' | 'all' | 'selection';

export interface SectionInput {
  kind: SectionKind;
  start_message_id: string;
  end_message_id: string;
  message_ids: string[];
}

export interface SectionSummary {
  id: string;
  scope: string;
  kind: SectionKind;
  start_message_id: string;
  end_message_id: string;
  content_hash: string;
  message_count: number;
  summary: string;
  action_items: string[];
  connection: string;
  model: string | null;
  created_at: string;
  updated_at: string;
  stale: boolean;
}

export interface SectionSummaries {
  summaries: SectionSummary[];
  requirements: ActionDiagnostic[];
}

export interface PostSectionSummaryInput extends SectionInput {
  scope: string;
  conversation_id: string | null;
  title: string | null;
  force: boolean;
  options?: SummaryOptions;
}

export interface SummaryOptions {
  focus?: string;
  include?: string;
  exclude?: string;
  without_files?: boolean;
}

export interface ContentPrompt {
  id: string;
  name: string;
  body: string;
}

export interface PromptOverride {
  system?: string;
  template?: string;
  content_id?: string;
  output_schema?: string;
  connection_id?: string;
  model?: string;
}

export interface RankingEntry {
  person_id: string | null;
  name: string;
  value: number;
  unit: 'messages' | 'characters' | 'minutes' | 'percent' | 'count';
  detail: string | null;
}

export interface Ranking {
  key: string;
  title: string;
  description: string;
  entries: RankingEntry[];
}

export interface GroupStat {
  key: string;
  kind: 'bucket' | 'split' | 'tag' | 'group';
  name: string;
  color: string | null;
  messages: number;
  conversations: number;
  people: number;
  characters: number;
  median_response_minutes: number | null;
  busiest_hour: number | null;
  busiest_weekday: number | null;
  top: RankingEntry[];
  by_hour: number[];
  by_weekday: number[];
}

export type CaseStatus = 'open' | 'waiting' | 'done';

export type ExpectationKind = 'file' | 'message' | 'reply';

export type ExpectationStatus = 'pending' | 'arrived' | 'overdue';

export interface CaseItem {
  id: string;
  case_id: string;
  conversation_id: string | null;
  message_id: string | null;
  note: string | null;
  title: string;
  conversation_type: string | null;
  added_at: string;
}

export interface CaseExpectation {
  id: string;
  case_id: string;
  title: string;
  kind: ExpectationKind;
  from_match: string | null;
  subject_match: string | null;
  filename_match: string | null;
  mime_match: string | null;
  due_at: string | null;
  status: ExpectationStatus;
  arrived_message_id: string | null;
  arrived_attachment_name: string | null;
  arrived_at: string | null;
  arrived_from: string | null;
  arrived_conversation_id: string | null;
  created_at: string;
}

export interface CaseRule {
  id: string;
  case_id: string;
  match: 'all' | 'any';
  conditions: RuleCondition[];
  enabled: boolean;
  created_at: string;
}

export interface CaseFile {
  id: string;
  profile_id: string;
  name: string;
  description: string;
  status: CaseStatus;
  color: string;
  due_at: string | null;
  items: CaseItem[];
  expectations: CaseExpectation[];
  rules: CaseRule[];
  conversation_count: number;
  message_count: number;
  pending_count: number;
  arrived_count: number;
  overdue_count: number;
  last_activity: string | null;
  created_at: string;
  updated_at: string;
}

export interface PostCaseInput {
  name: string;
  description?: string | null;
  color?: string | null;
  due_at?: string | null;
  conversation_ids?: string[];
  message_ids?: string[];
}

export interface PutCaseInput {
  case_id: string;
  name?: string;
  description?: string;
  status?: CaseStatus;
  color?: string;
  due_at?: string | null;
}

export interface CaseItemsInput {
  case_id: string;
  conversation_ids?: string[];
  message_ids?: string[];
}

export interface PostExpectationInput {
  case_id: string;
  title: string;
  kind?: ExpectationKind;
  from_match?: string | null;
  subject_match?: string | null;
  filename_match?: string | null;
  mime_match?: string | null;
  due_at?: string | null;
}

export interface PutExpectationInput {
  expectation_id: string;
  title?: string;
  kind?: ExpectationKind;
  from_match?: string | null;
  subject_match?: string | null;
  filename_match?: string | null;
  mime_match?: string | null;
  due_at?: string | null;
  status?: ExpectationStatus;
}

export interface PostCaseRuleInput {
  case_id: string;
  match?: 'all' | 'any';
  conditions: RuleCondition[];
}

export interface ClaudeStatus {
  installed: boolean;
  binary: string | null;
  version: string | null;
  signed_in: boolean;
  account: string | null;
  plan: string | null;
  model: string | null;
  models: string[];
}

export interface OllamaModel {
  name: string;
  size_bytes: number;
  parameter_size: string | null;
  family: string | null;
}

export interface OllamaStatus {
  installed: boolean;
  binary: string | null;
  running: boolean;
  version: string | null;
  host: string;
  models: OllamaModel[];
  download_url: string;
}

export interface ModelSuggestion {
  name: string;
  size: string;
  good_for: string;
  command: string;
}

export interface AiSetup {
  claude: ClaudeStatus;
  ollama: OllamaStatus;
  suggestions: ModelSuggestion[];
  claude_model: string;
}

export interface ModelFeature {
  id: string;
  prompts: string[];
  models: [IntelligenceKind, string][];
}

export interface SettingRegistry {
  defaults: Record<string, string>;
  values: Record<string, string>;
  features: ModelFeature[];
}

export interface PullProgress {
  model: string;
  status: string;
  completed: number | null;
  total: number | null;
  done: boolean;
  error: string | null;
}

export const SECRET_MARKER = 'keychain';

export interface LockState {
  enabled: boolean;
  locked: boolean;
  idle_minutes: number;
}

export interface PluginOptions {
  home: string | null;
  mock: boolean;
}

export interface CliSetup {
  executable: string;
  link: string;
  linked: boolean;
  on_path: boolean;
  claude: string | null;
  plugin_dir: string;
  plugin_version: string | null;
  plugin_current: string;
  plugin_installed: boolean;
  plugin_options: PluginOptions | null;
}


export type PortType = 'message' | 'conversation' | 'person' | 'text' | 'number' | 'bool' | 'list' | 'json' | 'tag' | 'any';

export interface ActionPort {
  name: string;
  type: PortType;
  description?: string;
  label?: string;
  node?: string | null;
  source_port?: string | null;
}

export type TriggerOn = 'arrival' | 'sent' | 'edited' | 'deleted' | 'reaction' | 'mention' | 'filter' | 'schedule' | 'event_soon' | 'manual' | 'called' | 'custom';

export type TriggerDirection = 'incoming' | 'outgoing' | 'any';

export type BuiltinKind = 'trigger' | 'step' | 'landing' | 'action';

export type NodeRole = 'trigger' | 'step' | 'landing' | 'chat';

export interface ActionUsage {
  standalone: boolean;
  chat: boolean;
  step: boolean;
  landing: boolean;
}

export interface TriggerSchedule {
  every_minutes: number | null;
  at: string | null;
  days: number[];
}

export interface TriggerConfig {
  on: TriggerOn;
  scope?: GroupQuery;
  group_ids?: string[];
  direction?: TriggerDirection | null;
  message_kinds?: string[];
  minutes_before?: number | null;
  schedule?: TriggerSchedule | null;
  disabled?: boolean;
  labels?: Record<string, string>;
  watch?: TriggerWatch | null;
}

export interface TriggerWatch {
  mode: 'webhook' | 'poll';
  url?: string | null;
  every_minutes?: number | null;
  pick?: string | null;
  headers?: Record<string, string>;
  token?: string | null;
}

export interface ActionNode {
  id: string;
  type: string;
  label: string;
  config: Record<string, unknown> | null;
  body: string;
  x: number;
  y: number;
}

export interface ActionEdge {
  id: string;
  source: string;
  source_port: string;
  target: string;
  target_port: string;
}

export interface ActionGraph {
  nodes: ActionNode[];
  edges: ActionEdge[];
}

export interface AiChoice {
  connection_id: string | null;
  model: string | null;
}

export interface Action {
  id: string;
  profile_id: string;
  key: string | null;
  builtin: boolean;
  group_id: string | null;
  name: string;
  description: string;
  icon: string | null;
  enabled: boolean;
  position: number;
  inputs: ActionPort[];
  outputs: ActionPort[];
  graph: ActionGraph;
  max_sends_per_hour: number;
  run_count: number;
  last_run_at: string | null;
  last_status: string | null;
  usage: ActionUsage;
  builtin_kind?: BuiltinKind | null;
  ai: AiChoice;
  chats: string[];
  created_at: string;
  updated_at: string;
}

export interface ActionDraft {
  name: string;
  description: string;
  icon: string | null;
  max_sends_per_hour: number | null;
  inputs: ActionPort[];
  graph: ActionGraph;
}

export interface PostActionInput {
  name: string;
  description?: string | null;
  icon?: string | null;
  group_id?: string | null;
  enabled?: boolean;
  inputs?: ActionPort[];
  graph?: ActionGraph;
  code?: string;
  max_sends_per_hour?: number;
  ai?: AiChoice;
}

export interface PutActionInput {
  action_id: string;
  name?: string;
  description?: string;
  icon?: string | null;
  group_id?: string | null;
  enabled?: boolean;
  position?: number;
  inputs?: ActionPort[];
  graph?: ActionGraph;
  code?: string;
  max_sends_per_hour?: number;
  ai?: AiChoice;
}

export interface ActionGroup {
  id: string;
  profile_id: string;
  key: string | null;
  builtin: boolean;
  parent_id: string | null;
  name: string;
  description: string;
  icon: string | null;
  position: number;
}

export interface PostActionGroupInput {
  name: string;
  parent_id?: string | null;
  description?: string | null;
  icon?: string | null;
}

export interface PutActionGroupInput {
  group_id: string;
  name?: string;
  parent_id?: string | null;
  description?: string;
  icon?: string | null;
  position?: number;
}

export interface ActionLogEntry {
  node: string;
  kind: string;
  status: 'done' | 'skipped' | 'would' | 'failed' | 'waiting';
  detail: string;
}

export interface ActionRun {
  id: string;
  action_id: string;
  action_name: string;
  status: 'running' | 'done' | 'failed' | 'waiting';
  dry_run: boolean;
  trigger: unknown;
  log: ActionLogEntry[];
  outputs: Record<string, unknown>;
  state: unknown;
  error: string | null;
  resume_at: string | null;
  depth: number;
  started_at: string;
  finished_at: string | null;
  result: ActionResult;
  requirement?: ActionDiagnostic | null;
}

export interface ActionResult {
  kind: 'command' | 'content' | 'data' | 'none';
  message: string;
  commands: { function: string; detail: string; status: 'done' | 'would' }[];
  content: { text: string } | null;
  outputs: Record<string, unknown>;
}

export interface ActionSample {
  message?: Record<string, unknown> | null;
  conversation?: Record<string, unknown> | null;
  person?: Record<string, unknown> | null;
  text?: string | null;
  tag?: string | null;
}

export interface ActionDiagnostic {
  severity: 'error' | 'warning' | 'requirement';
  message: string;
  node: string | null;
  edge: string | null;
  line: number | null;
  column: number | null;
  setting?: string | null;
}

export interface ActionCheckInput {
  action_id?: string | null;
  code?: string | null;
  draft?: ActionDraft | null;
}

export interface ActionCheckOutcome {
  diagnostics: ActionDiagnostic[];
  draft: ActionDraft | null;
  outputs: ActionPort[];
  code: string | null;
}

export interface ActionFunction {
  name: string;
  params: [string, PortType][];
  required: number;
  returns: PortType;
  description: string;
  effect: boolean;
  signature: string;
  requires: string[];
}

export interface ActionBuiltin {
  key: string;
  label: string;
  icon: string;
  description: string;
  inputs: ActionPort[];
  outputs: ActionPort[];
  body: string;
  kind: BuiltinKind;
}


export interface ActionCatalogEntry {
  id: string;
  name: string;
  icon: string | null;
  key: string | null;
  builtin: boolean;
  builtin_kind: BuiltinKind | null;
  description: string;
  position: number;
  usage: ActionUsage;
  role: NodeRole;
  requires: string[];
  enabled: boolean;
  inputs: ActionPort[];
  outputs: ActionPort[];
  group_id: string | null;
  graph: ActionGraph;
  diagnostics: ActionDiagnostic[];
}

export interface ActionCatalog {
  types: PortType[];
  chat_node: ChatNodeSpec;
  functions: ActionFunction[];
  builtins: ActionBuiltin[];
  prompts: Prompt[];
  content_prompts: ContentPrompt[];
  tags: { id: string; name: string; kind: string }[];
  splits: { id: string; name: string }[];
  groups: { id: string; name: string; kind: string }[];
  actions: ActionCatalogEntry[];
  action_groups: ActionGroup[];
  enabled: boolean;
  requirements: ActionRequirementStatus[];
  unmet: ActionUnmetRequirement[];
  result_schema: Record<string, unknown>;
}

export interface ActionRequirementStatus {
  key: string;
  label: string;
  met: boolean;
}

export interface ActionUnmetRequirement {
  action_id: string;
  action: string;
  node: string;
  setting: string;
  label: string;
}

export interface ActionAssistMessage {
  role: 'user' | 'assistant';
  content: string;
  status?: 'applied' | 'discarded' | 'undone' | null;
}

export interface ActionAssistInput {
  action_id?: string | null;
  draft?: ActionDraft | null;
  instruction: string;
  messages: ActionAssistMessage[];
}

export type ActionOperationKind = 'create' | 'update' | 'copy' | 'split' | 'merge' | 'delete' | 'move' | 'rename' | 'restore';

export interface ActionPatch {
  name?: string;
  description?: string;
  icon?: string;
  max_sends_per_hour?: number;
  inputs?: ActionPort[];
  graph?: ActionGraph;
}

export type ActionOperation =
  | { op: 'create'; ref?: string; group_id?: string | null; action: ActionDraft }
  | { op: 'update'; action_id: string; action: ActionPatch }
  | { op: 'copy'; action_id: string; name?: string | null; without_trigger?: boolean; ref?: string }
  | { op: 'split'; action_id: string; node_ids: string[]; name: string; ref?: string }
  | { op: 'merge'; action_ids: string[]; name: string; remove_sources?: boolean; ref?: string }
  | { op: 'delete'; action_id: string }
  | { op: 'move'; action_ids: string[]; group_id?: string | null }
  | { op: 'rename'; action_id: string; name: string }
  | { op: 'restore'; action: Action };

export interface ActionOperationPreview {
  index: number;
  op: ActionOperationKind;
  action_id: string;
  name: string;
  group_before: string | null;
  group_after: string | null;
  before: ActionDraft | null;
  after: ActionDraft | null;
  diagnostics: ActionDiagnostic[];
}

export interface ActionAssistOutcome {
  reply: string;
  operations: ActionOperation[];
  previews: ActionOperationPreview[];
  errors: string[];
  requirement?: ActionDiagnostic | null;
}

export interface PutActionsInput {
  operations: ActionOperation[];
}

export interface ActionsOutcome {
  actions: Action[];
  undo: ActionOperation[];
  ids: Record<string, string>;
}

export interface PostMessageScheduleInput {
  conversation_id: string;
  provider_id?: string | null;
  content: string;
  content_type?: string | null;
  at?: string | null;
  action_id?: string | null;
  metadata?: Record<string, unknown> | null;
  attachments?: OutgoingAttachmentInput[] | null;
}

export interface MessageSchedule {
  id: string;
  conversation_id: string;
  conversation_name: string | null;
  provider_id: string | null;
  content: string;
  content_type: string;
  metadata: Record<string, unknown> | null;
  attachments: OutgoingAttachmentInput[] | null;
  at: string | null;
  action_id: string | null;
  action_name: string | null;
  status: 'pending' | 'sent' | 'failed' | 'cancelled';
  error: string | null;
  created_at: string;
}

export type HoldSurface = 'composer' | 'compose' | 'forward' | 'reply' | 'action'

export interface HoldRequest {
  surface: HoldSurface;
  conversation_id?: string | null;
  draft?: unknown;
}

export type Outbound =
  | { kind: 'send'; input: SendMessageInput }
  | { kind: 'reply'; input: PostMessageReplyInput }
  | { kind: 'forward'; input: PostMessageForwardInput }

export interface HistoryEntry {
  id: string;
  kind: 'sent' | 'failed' | 'error' | 'warning' | 'info';
  title: string;
  detail: string;
  conversation_id: string | null;
  message_id: string | null;
  href: string | null;
  status: 'open' | 'waiting' | 'resolved' | 'cancelled';
  options: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export type HistoryOption = 'send_when_online' | 'always_wait' | 'send_now' | 'cancel' | 'resolve';

export interface OutboundHold {
  id: string;
  surface: HoldSurface;
  conversation_id: string;
  subject: string;
  recipient: string;
  payload: Outbound;
  draft: unknown;
  due_at: string;
  status: 'pending' | 'sending' | 'sent' | 'draft' | 'cancelled' | 'failed';
  error: string | null;
  message_id: string | null;
  created_at: string;
}

export type Sent = Message | OutboundHold

export type InboxMemberKind = 'provider' | 'person' | 'conversation' | 'tag' | 'group';

export interface InboxMember {
  kind: InboxMemberKind;
  member_id: string;
}

export interface Inbox {
  id: string;
  profile_id: string;
  name: string;
  icon: string | null;
  position: number;
  is_default: boolean;
  members: InboxMember[];
  created_at: string;
}

export interface PostInboxInput {
  name: string;
  icon?: string | null;
  members?: InboxMember[];
}

export interface PutInboxInput {
  inbox_id: string;
  name?: string;
  icon?: string;
  position?: number;
}

export interface PutInboxMembersInput {
  inbox_id: string;
  add?: InboxMember[];
  remove?: InboxMember[];
  replace?: InboxMember[];
}

export interface ActionBackfillReport {
  messages: number;
  runs: number;
  failed: number;
}

export type SidebarEntry =
  | { kind: 'item'; key: string }
  | { kind: 'divider'; id: string }
  | { kind: 'heading'; id: string; name: string }
  | { kind: 'folder'; id: string; name: string; icon: string | null; collapsed: boolean; items: SidebarEntry[] };

export interface SidebarLayout {
  top: SidebarEntry[];
  footer: SidebarEntry[];
}

export type PresentationKind = 'auto' | 'chat' | 'mail' | 'invite' | 'notification' | 'bot' | 'digest' | 'system' | 'media' | 'hidden'

export interface PresentedInvite {
  confidence?: number;
  title?: string;
  starts_at?: string | null;
  ends_at?: string | null;
  location?: string;
  organizer?: string;
  join_url?: string | null;
  provider?: string | null;
}

export interface Presentation {
  kind?: PresentationKind;
  collapsed?: boolean;
  summary_only?: boolean;
  highlight?: string;
  pinned?: boolean;
  badge?: string;
  icon?: string;
  invite?: PresentedInvite;
  template?: PresentationTemplateRef | null;
}

export interface PresentationTemplateRef {
  action_id: string;
  node_id: string;
}

export interface PresentationTemplate {
  action_id: string;
  node_id: string;
  kind: string;
  source: string;
}

export interface TemplateAssistInput {
  action_id?: string | null;
  node_id?: string | null;
  kind?: string | null;
  instruction: string;
  source?: string | null;
}

export interface TemplateAssistOutcome {
  source: string;
  explanation: string;
  requirement?: ActionDiagnostic | null;
}

export interface MessagePresentation {
  message_id: string;
  own: Presentation;
  conversation: Presentation;
  effective: Presentation;
}

export type LocalField = 'display_name' | 'name' | 'avatar_data' | 'color' | 'job_title' | 'company' | 'notes' | 'description';

export type LocalFields = Partial<Record<LocalField, string>>;

export type OverrideObjectType = 'person' | 'conversation';

export interface ObjectOverride {
  object_type: OverrideObjectType;
  object_id: string;
  scope_type: '' | 'conversation';
  scope_id: string;
  field: LocalField;
  value: string;
  updated_at: string;
}

export interface PutObjectOverrideInput {
  object_type: OverrideObjectType;
  object_id: string;
  scope_type?: 'conversation' | null;
  scope_id?: string | null;
  field: LocalField;
  value: string | null;
}

export interface DeleteObjectOverrideInput {
  object_type: OverrideObjectType;
  object_id: string;
  scope_type?: 'conversation' | null;
  scope_id?: string | null;
  field?: LocalField | null;
}

export interface PersonThread {
  id: string;
  name: string;
  kind: ConversationType;
  mode: ChatMode;
  local: boolean;
  direct: boolean;
  provider_types: string[];
  provider_names: string[];
  message_count: number;
  unread_count: number;
  last_activity: string;
}

export interface SharedConversation {
  id: string;
  name: string;
  kind: string;
}

export interface IdentityProfile {
  identity: PersonIdentity;
  key: string;
  display_name: string | null;
  email: string | null;
  phone: string | null;
  job_title: string | null;
  department: string | null;
  company: string | null;
  office_location: string | null;
  avatar_url: string | null;
  avatar_data: string | null;
  presence: string | null;
  status_text: string | null;
  first_seen: string | null;
  last_seen: string | null;
  message_count: number;
  conversations: SharedConversation[];
}

export interface PersonSignature {
  id: string;
  person_id: string;
  hash: string;
  text: string;
  sender_email: string | null;
  provider_id: string | null;
  provider_name: string | null;
  conversation_id: string | null;
  message_id: string | null;
  first_seen: string;
  last_seen: string;
  use_count: number;
}

export interface PersonProfile {
  person: Person;
  tags: ObjectTag[];
  threads: PersonThread[];
  identities: IdentityProfile[];
  message_count: number;
  conversation_count: number;
  first_seen: string | null;
  last_seen: string | null;
  signatures: PersonSignature[];
}

export type ChatMode = 'read' | 'write';

export type ChatView = 'chat' | 'grid';

export type GridGroup = 'none' | 'kind' | 'type' | 'modified' | 'source' | 'sender' | 'name' | 'tag' | 'group';

export type GridSort = 'name' | 'date' | 'size' | 'kind' | 'sender' | 'reactions';

export type GridRows = 'scroll' | 'wrap';

export type GridShow = 'messages' | 'files' | 'media' | 'links' | 'records' | 'events';

export interface GridSettings {
  group: GridGroup;
  sort: GridSort;
  direction: 'asc' | 'desc';
  rows: GridRows;
  tags: boolean;
  show: GridShow[];
}

export interface TimelineSettings {
  sources: boolean;
}

export interface ViewSettings {
  chat: TimelineSettings;
  grid: GridSettings;
}

export type ChatSourceKind = 'conversation' | 'group' | 'tag' | 'person' | 'query';

export interface ChatSource {
  kind: ChatSourceKind;
  id?: string | null;
  query?: GroupQuery | null;
}

export interface Chat {
  id: string;
  profile_id: string;
  conversation_id: string;
  name: string;
  icon: string | null;
  mode: ChatMode;
  sources: ChatSource[];
  attachments: ChatAttachment[];
  created_at: string;
}

export interface ChatRoute {
  output: string;
  node: string;
  input: string;
  values: Record<string, unknown>;
}

export interface ChatAttachment {
  action_id: string;
  routes: ChatRoute[];
}

export interface LandingNode {
  node: string;
  label: string;
  icon: string;
  inputs: ActionPort[];
}

export interface RouteProblem {
  index: number;
  message: string;
}

export interface PipeStep {
  action_id: string;
  name: string;
  icon: string | null;
  enabled: boolean;
  inputs: ActionPort[];
  outputs: ActionPort[];
  routes: ChatRoute[];
  stored: boolean;
  problems: RouteProblem[];
}

export interface ChatPipe {
  conversation_id: string;
  mode: ChatMode;
  sources: ChatSource[];
  steps: PipeStep[];
  landings: LandingNode[];
}

export interface PostChatInput {
  name: string;
  icon?: string | null;
  mode: ChatMode;
  view?: ChatView;
  sources: ChatSource[];
  action_ids?: string[];
}

export interface PutChatInput {
  chat_id: string;
  name?: string;
  icon?: string | null;
  sources?: ChatSource[];
}

export type ChatItemKind = 'message' | 'record' | 'note';

export interface ChatItem {
  id: string;
  kind: ChatItemKind;
  message: Message | null;
  record: Record<string, unknown> | null;
  text: string | null;
  title: string;
  conversation_id: string;
  conversation_name: string;
  conversation_type: string;
  action_id: string | null;
  reactions: number;
  at: string;
}

export interface ChatPage {
  chat_id: string;
  items: ChatItem[];
  next_before: string | null;
}

export type GridKind = 'message' | 'mail' | 'file' | 'image' | 'video' | 'link' | 'record' | 'event' | 'content';

export interface GridItem {
  id: string;
  kind: GridKind;
  name: string;
  content_type: string;
  size: number;
  at: string;
  message_id: string | null;
  attachment_id: string | null;
  conversation_id: string;
  conversation_name: string;
  provider_type: string;
  sender: string;
  sender_id: string | null;
  excerpt: string;
  reactions: number;
  path: string | null;
  url: string | null;
  tags: string[];
  record: Record<string, unknown> | null;
  presentation: Record<string, unknown> | null;
}

export interface ChatNodeVariant {
  label: string;
  icon: string;
  description: string;
  start: string;
  end: string;
  inputs: ActionPort[];
  outputs: ActionPort[];
}

export interface ChatNodeSpec {
  key: string;
  label: string;
  icon: string;
  description: string;
  config: Record<string, unknown>;
  variants: Record<ChatMode, ChatNodeVariant>;
}

export interface AttachmentOpenInput {
  message_id?: string | null;
  attachment_id?: string | null;
  path?: string | null;
  quick_look?: boolean;
}

export type VoiceLanguage = 'auto' | 'no' | 'en';
export type VoiceCleanup = 'live' | 'stop' | 'off';
export type MicrophoneAccess = 'authorized' | 'undetermined' | 'denied' | 'restricted' | 'unknown';

export interface VoiceSettings {
  enabled: boolean;
  model: string;
  vad: string;
  language: VoiceLanguage;
  cleanup: VoiceCleanup;
  hotkey: string;
  model_url: string;
  vad_url: string;
  debounce_ms: number;
}

export interface VoiceModel {
  setting: string;
  name: string;
  url: string;
  path: string;
  present: boolean;
  bytes: number;
}

export interface VoiceRequirement {
  key: string;
  label: string;
}

export interface VoiceSetup {
  settings: VoiceSettings;
  models: VoiceModel[];
  unmet: VoiceRequirement[];
  ready: boolean;
  microphone: MicrophoneAccess;
  cleanup_available: boolean;
  setting_up: boolean;
  sessions: string[];
}

export interface VoiceSetupProgress {
  model: string;
  completed: number;
  total: number | null;
  done: boolean;
  finished: boolean;
  error: string | null;
}

export type VoiceEvent =
  | { event: 'level'; id: string; level: number; speech: boolean }
  | { event: 'partial'; id: string; state: 'recording'; committed: string; tentative: string; language: string | null; ms_first_word: number | null; ms_decode: number }
  | { event: 'final'; id: string; state: 'recording'; segment: number; text: string; committed: string; tentative: string; language: string | null; ms_first_word: number | null; ms_final: number | null; ms_decode: number }
  | { event: 'corrected'; id: string; from: number; upto: number; raw: string; text: string; ms_corrected: number }
  | { event: 'done'; id: string; state: 'done'; text: string; raw: string; cleaning: boolean; language: string | null; ms_first_word: number | null; ms_final: number | null }
  | { event: 'cleaned'; id: string; state: 'cleaned'; text: string; raw: string; model: string | null; language: string | null; ms_first_word: number | null; ms_final: number | null; ms_cleaned: number }
  | { event: 'failed'; id: string; state: 'failed'; error: string; reason: 'microphone' | null };

export interface WorkspaceEmoji {
  provider_id: string;
  provider_name: string;
  provider_type: string;
  name: string;
  alias_for: string | null;
  url: string | null;
  file_path: string | null;
  mime_type: string | null;
  updated_at: string;
}

export interface LinkTarget {
  conversation_id: string;
  message_id: string | null;
}

export interface GetSharedInput {
  conversation_ids?: string[];
  lead_conversation_id?: string | null;
  limit?: number;
}

export interface SharedFile {
  id: string;
  attachment_id: string;
  name: string;
  content_type: string;
  size: number;
  url: string | null;
  message_id: string;
  conversation_id: string;
  conversation_name: string;
  sender: string;
  at: string;
}

export interface SharedLink {
  id: string;
  url: string;
  title: string | null;
  site_name: string | null;
  domain: string | null;
  favicon: string | null;
  message_id: string;
  conversation_id: string;
  conversation_name: string;
  sender: string;
  at: string;
}

export interface Draft {
  id: string;
  conversation_id: string;
  thread_id: string;
  doc: unknown;
  markdown: string;
  metadata: Record<string, unknown> | null;
  remote_id: string | null;
  updated_at: string;
}

export interface PutDraftInput {
  conversation_id: string;
  thread_id?: string | null;
  doc: unknown;
  markdown: string;
  metadata?: Record<string, unknown> | null;
}

export interface Reminder {
  id: string;
  message_id: string | null;
  conversation_id: string | null;
  conversation_name: string | null;
  note: string;
  at: string;
  status: 'pending' | 'firing' | 'done' | 'cancelled';
  created_at: string;
}

export interface PostReminderInput {
  message_id?: string | null;
  conversation_id?: string | null;
  note?: string | null;
  at: string;
}

export interface ActivityItem {
  kind: 'mention' | 'reply' | 'reaction';
  at: string;
  actor: string | null;
  detail: string | null;
  conversation_id: string | null;
  conversation_name: string | null;
  message: Message;
}

export interface ActivityPage {
  items: ActivityItem[];
  cursor: string | null;
}

export interface RemoteConversation {
  external_id: string;
  name: string;
  conversation_type: 'direct_message' | 'group_message' | 'channel' | 'email' | 'unknown';
  members: unknown[];
  last_activity: string;
  metadata: Record<string, unknown>;
}

export interface JoinableConversation {
  provider_id: string;
  provider_name: string;
  conversation: RemoteConversation;
}

export interface NewRemoteConversation {
  conversation_type: 'direct_message' | 'group_message' | 'channel';
  name: string;
  members: string[];
  private: boolean;
  parent_external_id?: string | null;
}

export interface PresenceUpdate {
  user_id: string;
  status: 'online' | 'away' | 'dnd' | 'offline';
  status_text: string | null;
  status_emoji: string | null;
}

export interface TypingUpdate {
  conversation_id: string;
  user_id: string | null;
  user_name: string;
}

export interface RemoteUserGroup {
  external_id: string;
  handle: string;
  name: string;
  members: string[];
}

export interface RenderedSignature {
  content_id: string;
  name: string;
  text: string;
  html: string | null;
}

export interface ContentSignature {
  content_id: string;
  provider_id: string;
  alias: string;
  use_new: boolean;
  use_reply: boolean;
  use_forward: boolean;
}

export interface RemoteAlias {
  email: string;
  name: string | null;
  is_default: boolean;
  signature: string | null;
}

export interface RemoteAutoReply {
  enabled: boolean;
  subject: string | null;
  body: string;
  starts_at: string | null;
  ends_at: string | null;
  external_only: boolean;
}
