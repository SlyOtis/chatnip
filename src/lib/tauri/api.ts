import { invoke } from '@tauri-apps/api/core'
import { featureShown } from '$lib/settings'
import type {
  HistoryEntry,
  HistoryOption,
  WorkspaceEmoji,
  MergeGroup,
  MergeGroupInput,
  ObjectOverride,
  PutObjectOverrideInput,
  DeleteObjectOverrideInput,
  LinkTarget,
  GetSharedInput,
  SharedFile,
  SharedLink,
  PersonProfile,
  ObjectTag,
  ObjectTagInput,
  ObjectType,
  TaggedObject,
  ProviderSetup,
  AiSetup,
  OllamaStatus,
  SectionInput,
  SectionSummaries,
  PostSectionSummaryInput,
  CliSetup,
  PluginOptions,
  Profile,
  ProfileResponse,
  ProfileSettings,
  Conversation,
  Message,
  Provider,
  Thread,
  SearchResults,
  ConversationsResponse,
  ConversationResponse,
  UsersResponse,
  ProvidersResponse,
  MessagesResponse,
  CreateProfileInput,
  UpdateProfileSettingsInput,
  CreateProfileAvatarInput,
  SendMessageInput,
  PostMessageForwardInput,
  SendMessageReactionInput,
  EditMessageInput,
  DeleteMessageInput,
  PinMessageInput,
  GetPinnedInput,
  BookmarkMessageInput,
  MarkMessageUnreadInput,
  MarkConversationReadInput,
  PinnedMessage,
  SetUserStatusInput,
  GetUserStatusInput,
  UserStatus,
  SetNotificationInput,
  UpdateSidebarSectionInput,
  DeleteSidebarSectionInput,
  SidebarSectionItemInput,
  RemoveSidebarSectionItemInput,
  SidebarSection,
  Tag,
  PostTagInput,
  PutTagInput,
  AssistantRun,
  CalendarEvent,
  EventRangeInput,
  PutEventInput,
  PostEventInput,
  PostEventResult,
  ProviderCalendar,
  Person,
  PostIdentityInput,
  PutPersonInput,
  MockClientKind,
  MockMode,
  MockServerState,
  CalendarResponse,
  ContentKind,
  ContentItem,
  ContentSort,
  ContentTag,
  ContentGroup,
  ContentGroupItemsInput,
  ContentUse,
  PostContentGroupInput,
  PutContentGroupInput,
  PostContentRenderInput,
  RenderedContent,
  ReactionUse,
  PutContentBulkInput,
  GifCursor,
  GifPage,
  LinkPreview,
  PostContentInput,
  PutContentInput,
  Group,
  AttachmentOpenInput,
  Chat,
  ChatItem,
  ChatMode,
  ChatPage,
  ChatPipe,
  ChatRoute,
  ChatView,
  GridItem,
  PostChatInput,
  PutChatInput,
  ViewSettings,
  GroupQuery,
  PostGroupInput,
  PutGroupInput,
  GroupItemsInput,
  StatsRangeInput,
  Stats,
  IntelligenceConnection,
  PostIntelligenceConnectionInput,
  PutIntelligenceConnectionInput,
  CaseFile,
  PostCaseInput,
  PutCaseInput,
  CaseItemsInput,
  PostExpectationInput,
  PutExpectationInput,
  PostCaseRuleInput,
  SettingRegistry,
  HoldRequest,
  OutboundHold,
  Sent,
} from './types'
export async function postStartup(): Promise<Profile | null> {
  return await invoke('post_startup')
}
export async function postProfile(input: CreateProfileInput): Promise<Profile> {
  return await invoke('post_profile', { input })
}
export async function getProfile(): Promise<ProfileResponse> {
  return await invoke('get_profile')
}
export async function putProfileSettings(input: UpdateProfileSettingsInput): Promise<ProfileSettings> {
  return await invoke('put_profile_settings', { input })
}
export async function postProfileAvatar(input: CreateProfileAvatarInput): Promise<string> {
  return await invoke('post_profile_avatar', { input })
}
export async function getConversations(): Promise<ConversationsResponse> {
  return await invoke('get_conversations')
}
export async function getConversation(conversation_id: string): Promise<Conversation> {
  return await invoke('get_conversation', { input: { conversation_id } })
}
export async function getCurrentConversation(): Promise<ConversationResponse> {
  return await invoke('get_current_conversation')
}
export async function getMessages(): Promise<MessagesResponse> {
  return await invoke('get_current_messages')
}
export async function postMessage(input: SendMessageInput, hold?: HoldRequest): Promise<Sent> {
  const every = input.delivery_mode === 'all' ? { deliveryMode: 'all' } : {}
  return await invoke('post_message', hold ? { input, hold, ...every } : { input, ...every })
}
export async function postMessageForward(input: PostMessageForwardInput, hold?: HoldRequest): Promise<Sent[]> {
  return await invoke('post_message_forward', hold ? { input, hold } : { input })
}
export async function getHistory(limit?: number): Promise<HistoryEntry[]> {
  return await invoke('get_history', { limit: limit ?? null })
}
export async function postHistory(input: { kind: HistoryEntry['kind']; title: string; detail?: string; conversation_id?: string | null; message_id?: string | null; href?: string | null }): Promise<HistoryEntry> {
  return await invoke('post_history', { input })
}
export async function putHistory(history_id: string, option: HistoryOption): Promise<HistoryEntry> {
  return await invoke('put_history', { historyId: history_id, option })
}
export async function getOutboundHolds(conversation_id?: string): Promise<OutboundHold[]> {
  return await invoke('get_outbound_holds', { conversationId: conversation_id ?? null })
}
export async function deleteOutboundHold(hold_id: string): Promise<OutboundHold> {
  return await invoke('delete_outbound_hold', { holdId: hold_id })
}
export async function getOutboundOpened(): Promise<OutboundHold | null> {
  return await invoke('get_outbound_opened')
}
export async function getMessageQuote(message_id: string): Promise<Message | null> {
  return await invoke('get_message_quote', { messageId: message_id })
}
export async function postMessageReaction(input: SendMessageReactionInput): Promise<void> {
  return await invoke('post_message_reaction', { input })
}
export async function getWorkspaceEmoji(): Promise<WorkspaceEmoji[]> {
  return await invoke('get_workspace_emoji')
}
export async function getMessageLink(message_id: string): Promise<string | null> {
  return await invoke('get_message_link', { input: { message_id } })
}
export async function getLinkTarget(url: string): Promise<LinkTarget | null> {
  return await invoke('get_link_target', { input: { url } })
}
export async function postSync(): Promise<ConversationsResponse> {
  return await invoke('post_sync')
}
export async function putCurrentConversation(conversation_id: string): Promise<Conversation> {
  return await invoke('put_current_conversation', { input: { conversation_id } })
}
export async function postProviderAuth(provider_name: string): Promise<string> {
  return await invoke('post_provider_auth', { input: { provider_name } })
}
export async function postSlackConnection(token: string): Promise<Provider> {
  return await invoke('post_slack_connection', { token })
}
export async function getConnectionSetup(): Promise<ProviderSetup[]> {
  return await invoke('get_connection_setup')
}
export async function postAdminConsent(): Promise<string> {
  return await invoke('post_admin_consent')
}
export interface ConnectImapInput {
  email: string
  password: string
  display_name?: string
  username?: string
  imap_host: string
  imap_port?: number
  smtp_host: string
  smtp_port?: number
  aliases?: string[]
}
export async function postImapConnection(input: ConnectImapInput): Promise<Provider> {
  return await invoke('post_imap_connection', { input })
}
export async function deleteProvider(provider_id: string): Promise<void> {
  return await invoke('delete_provider', { providerId: provider_id })
}
export async function postProviderSync(provider_id: string): Promise<void> {
  return await invoke('post_provider_sync', { providerId: provider_id })
}
let appSettings: Promise<Record<string, string>> | null = null
export async function getAppSettings(): Promise<Record<string, string>> {
  appSettings ??= Promise.resolve(invoke<Record<string, string>>('get_app_settings')).finally(() => (appSettings = null))
  return await appSettings
}
export async function putAppSetting(key: string, value: string): Promise<Record<string, string>> {
  return await invoke('put_app_setting', { key, value })
}
let settingRegistry: Promise<SettingRegistry> | null = null
export async function getSettingRegistry(): Promise<SettingRegistry> {
  settingRegistry ??= Promise.resolve(invoke<SettingRegistry>('get_setting_registry')).finally(() => (settingRegistry = null))
  return await settingRegistry
}
export async function getPlatform(): Promise<string> {
  return await invoke('get_platform')
}
export async function putWindowPosition(): Promise<void> {
  return await invoke('put_window_position')
}
export async function putWindowFrame(duration: number): Promise<void> {
  return await invoke('put_window_frame', { duration })
}
export async function postLog(level: string, message: string, source?: string): Promise<void> {
  return await invoke('post_log', { level, message, source })
}
export async function getProviders(): Promise<ProvidersResponse> {
  return await invoke('get_providers')
}
export async function getUsers(): Promise<UsersResponse> {
  return await invoke('get_users')
}
export async function postConversation(input: {
  name: string;
  conversation_type: string;
  group_type: string;
  member_ids: string[];
}): Promise<Conversation> {
  return await invoke('post_conversation', { input })
}
export async function postConversationLink(input: {
  source_conversation_id: string;
  target_conversation_id: string;
  link_type: string;
}): Promise<void> {
  return await invoke('post_conversation_link', { input })
}
export async function postThread(input: {
  message_id: string;
  name?: string;
}): Promise<Thread> {
  return await invoke('post_thread', { input })
}
export async function postThreadReply(input: {
  thread_id: string;
  content: string;
  content_type?: string;
  broadcast?: boolean;
  attachments?: import('./types').OutgoingAttachmentInput[];
}): Promise<Message> {
  return await invoke('post_thread_reply', { input })
}
export async function getThreadMessages(input: {
  thread_id: string;
}): Promise<Message[]> {
  return await invoke('get_thread_messages', { input })
}
export async function putConversation(input: {
  conversation_id: string;
  name?: string;
  rename_on_provider?: boolean;
  group_type?: string;
  is_pinned?: boolean;
  is_muted?: boolean;
  mode?: ChatMode;
  view?: ChatView;
  view_settings?: ViewSettings;
  is_archived?: boolean;
  snoozed_until?: string | null;
  deleted_at?: string | null;
  favourite_position?: number | null;
  metadata?: string;
}): Promise<Conversation> {
  return await invoke('put_conversation', { input })
}
export async function getSearch(input: {
  query: string;
  limit?: number;
  conversation_ids?: string[];
  tag_ids?: string[];
}): Promise<SearchResults> {
  return await invoke('get_search', { input })
}
export async function putMessage(input: EditMessageInput): Promise<Message> {
  return await invoke('put_message', { input })
}
export async function deleteMessage(input: DeleteMessageInput): Promise<void> {
  return await invoke('delete_message', { input })
}
export async function postMessagePin(input: PinMessageInput): Promise<void> {
  return await invoke('post_message_pin', { input })
}
export async function deleteMessagePin(input: PinMessageInput): Promise<void> {
  return await invoke('delete_message_pin', { input })
}
export async function getPinnedMessages(input: GetPinnedInput): Promise<PinnedMessage[]> {
  return await invoke('get_pinned_messages', { input })
}
export async function postBookmark(input: BookmarkMessageInput): Promise<void> {
  return await invoke('post_bookmark', { input })
}
export async function putMessageUnread(input: MarkMessageUnreadInput): Promise<void> {
  return await invoke('put_message_unread', { input })
}
export async function putConversationRead(input: MarkConversationReadInput): Promise<void> {
  return await invoke('put_conversation_read', { input })
}
export async function putUserStatus(input: SetUserStatusInput): Promise<void> {
  return await invoke('put_user_status', { input })
}
export async function getUserStatus(input: GetUserStatusInput): Promise<UserStatus> {
  return await invoke('get_user_status', { input })
}
export async function putConversationNotification(input: SetNotificationInput): Promise<void> {
  return await invoke('put_conversation_notification', { input })
}
export async function putSidebarSection(input: UpdateSidebarSectionInput): Promise<void> {
  return await invoke('put_sidebar_section', { input })
}
export async function deleteSidebarSection(input: DeleteSidebarSectionInput): Promise<void> {
  return await invoke('delete_sidebar_section', { input })
}
export async function postSidebarSectionItem(input: SidebarSectionItemInput): Promise<void> {
  return await invoke('post_sidebar_section_item', { input })
}
export async function deleteSidebarSectionItem(input: RemoveSidebarSectionItemInput): Promise<void> {
  return await invoke('delete_sidebar_section_item', { input })
}
export async function getSidebarSections(): Promise<SidebarSection[]> {
  return await invoke('get_sidebar_sections')
}
export async function getTags(): Promise<Tag[]> {
  return await invoke('get_tags')
}
export async function postTag(input: PostTagInput): Promise<Tag> {
  return await invoke('post_tag', { input })
}
export async function putTag(input: PutTagInput): Promise<Tag> {
  return await invoke('put_tag', { input })
}
export async function putTagFavourites(order: string[]): Promise<Tag[]> {
  return await invoke('put_tag_favourites', { order })
}
export async function deleteTag(tag_id: string): Promise<void> {
  return await invoke('delete_tag', { tagId: tag_id })
}
export async function getObjectTags(object_type?: ObjectType | null, object_id?: string | null): Promise<ObjectTag[]> {
  return await invoke('get_object_tags', { objectType: object_type ?? null, objectId: object_id ?? null })
}
export async function getTaggedObjects(tag_id?: string | null): Promise<TaggedObject[]> {
  return await invoke('get_tagged_objects', { tagId: tag_id ?? null })
}
export async function putObjectTag(input: ObjectTagInput): Promise<ObjectTag[]> {
  return await invoke('put_object_tag', { input })
}
export async function deleteObjectTag(input: ObjectTagInput): Promise<ObjectTag[]> {
  return await invoke('delete_object_tag', { input })
}
export async function getAssistantRuns(limit?: number): Promise<AssistantRun[]> {
  return await invoke('get_assistant_runs', { limit })
}
export async function getEvents(input: EventRangeInput): Promise<CalendarEvent[]> {
  return await invoke('get_events', { input })
}
export async function getProposedEvents(): Promise<CalendarEvent[]> {
  return await invoke('get_proposed_events')
}
export async function postEvent(input: PostEventInput): Promise<PostEventResult> {
  return await invoke('post_event', { input })
}
export async function getCalendars(): Promise<ProviderCalendar[]> {
  return await invoke('get_calendars')
}
export async function putCalendar(calendar_id: string, enabled: boolean): Promise<ProviderCalendar> {
  return await invoke('put_calendar', { calendarId: calendar_id, enabled })
}
export async function putEventMerge(event_ids: string[], merged: boolean): Promise<void> {
  return await invoke('put_event_merge', { eventIds: event_ids, merged })
}
export async function putEvent(input: PutEventInput): Promise<CalendarEvent> {
  return await invoke('put_event', { input })
}
export async function putEventResponse(event_id: string, response: CalendarResponse): Promise<CalendarEvent> {
  return await invoke('put_event_response', { eventId: event_id, response })
}
export async function deleteEvent(event_id: string): Promise<void> {
  return await invoke('delete_event', { eventId: event_id })
}
export async function getCalendarExport(): Promise<string> {
  return await invoke('get_calendar_export')
}
export async function postExport(name: string, content: string): Promise<string> {
  return await invoke('post_export', { name, content })
}
export async function getConversationMessages(conversation_ids: string[]): Promise<Message[]> {
  return await invoke('get_conversation_messages', { conversationIds: conversation_ids })
}
export async function getPeople(): Promise<Person[]> {
  return await invoke('get_people')
}
export async function getPerson(person_id: string): Promise<Person> {
  return await invoke('get_person', { personId: person_id })
}
export async function getPersonMessages(person_id: string, limit?: number): Promise<Message[]> {
  return await invoke('get_person_messages', { personId: person_id, limit })
}
export async function putPerson(input: PutPersonInput): Promise<Person> {
  return await invoke('put_person', { input })
}
export async function postPersonMerge(source_person_id: string, target_person_id: string): Promise<Person> {
  return await invoke('post_person_merge', { sourcePersonId: source_person_id, targetPersonId: target_person_id })
}
export async function deletePersonIdentity(identity_id: string): Promise<Person> {
  return await invoke('delete_person_identity', { identityId: identity_id })
}
export async function postPersonIdentity(input: PostIdentityInput): Promise<Person> {
  return await invoke('post_person_identity', { input })
}
export async function postMockConnection(kind: MockClientKind): Promise<Provider> {
  return await invoke('post_mock_connection', { kind })
}
export async function getGifs(query: string, limit?: number, cursor?: GifCursor | null): Promise<GifPage> {
  return await invoke('get_gifs', { query, limit, cursor: cursor ?? null })
}
export async function getContent(kind?: ContentKind, query?: string, sort?: ContentSort): Promise<ContentItem[]> {
  return await invoke('get_content', { kind, query, sort })
}
export async function getContentTags(): Promise<ContentTag[]> {
  return await invoke('get_content_tags')
}
export async function putContentBulk(input: PutContentBulkInput): Promise<ContentItem[]> {
  return await invoke('put_content_bulk', { input })
}
export async function postContentFromMessage(message_id: string, attachment_index?: number): Promise<ContentItem> {
  return await invoke('post_content_from_message', { messageId: message_id, attachmentIndex: attachment_index })
}
export async function postContentFromUrl(url: string, name?: string, tags?: string[], keywords?: string[]): Promise<ContentItem> {
  return await invoke('post_content_from_url', { url, name, tags, keywords })
}
export async function postContentSend(content_id: string, conversation_id: string, provider_id?: string): Promise<Message> {
  return await invoke('post_content_send', { contentId: content_id, conversationId: conversation_id, providerId: provider_id })
}
export async function postContentRender(input: PostContentRenderInput): Promise<RenderedContent> {
  return await invoke('post_content_render', { input })
}
export async function getContentUses(content_id: string): Promise<ContentUse[]> {
  return await invoke('get_content_uses', { contentId: content_id })
}
export async function getContentGroups(): Promise<ContentGroup[]> {
  return await invoke('get_content_groups')
}
export async function postContentGroup(input: PostContentGroupInput): Promise<ContentGroup> {
  return await invoke('post_content_group', { input })
}
export async function putContentGroup(input: PutContentGroupInput): Promise<ContentGroup> {
  return await invoke('put_content_group', { input })
}
export async function deleteContentGroup(content_group_id: string): Promise<void> {
  return await invoke('delete_content_group', { contentGroupId: content_group_id })
}
export async function putContentGroupItems(input: ContentGroupItemsInput): Promise<ContentGroup> {
  return await invoke('put_content_group_items', { input })
}
export async function deleteContentGroupItems(input: ContentGroupItemsInput): Promise<ContentGroup> {
  return await invoke('delete_content_group_items', { input })
}
export async function getReactionUses(limit?: number): Promise<ReactionUse[]> {
  return await invoke('get_reaction_uses', { limit })
}
export async function postContent(input: PostContentInput): Promise<ContentItem> {
  return await invoke('post_content', { input })
}
export async function putContent(input: PutContentInput): Promise<ContentItem> {
  return await invoke('put_content', { input })
}
export async function putContentOrder(content_ids: string[]): Promise<void> {
  return await invoke('put_content_order', { contentIds: content_ids })
}
export async function deleteContent(content_id: string): Promise<void> {
  return await invoke('delete_content', { contentId: content_id })
}
export async function getLinkPreview(url: string): Promise<LinkPreview | null> {
  return await invoke('get_link_preview', { url })
}
export async function getGroups(): Promise<Group[]> {
  return await invoke('get_groups')
}
export async function postGroup(input: PostGroupInput): Promise<Group> {
  return await invoke('post_group', { input })
}
export async function putGroup(input: PutGroupInput): Promise<Group> {
  return await invoke('put_group', { input })
}
export async function deleteGroup(group_id: string): Promise<void> {
  return await invoke('delete_group', { groupId: group_id })
}
export async function putGroupItems(input: GroupItemsInput): Promise<Group> {
  return await invoke('put_group_items', { input })
}
export async function deleteGroupItems(input: GroupItemsInput): Promise<Group> {
  return await invoke('delete_group_items', { input })
}
export async function getQueryConversations(query: GroupQuery): Promise<string[]> {
  return await invoke('get_query_conversations', { query })
}
export async function getStats(input: StatsRangeInput): Promise<Stats> {
  return await invoke('get_stats', { input })
}
export async function getIntelligenceConnections(): Promise<IntelligenceConnection[]> {
  return await invoke('get_intelligence_connections')
}
export async function postIntelligenceConnection(input: PostIntelligenceConnectionInput): Promise<IntelligenceConnection> {
  return await invoke('post_intelligence_connection', { input })
}
export async function putIntelligenceConnection(input: PutIntelligenceConnectionInput): Promise<IntelligenceConnection> {
  return await invoke('put_intelligence_connection', { input })
}
export async function deleteIntelligenceConnection(connection_id: string): Promise<void> {
  return await invoke('delete_intelligence_connection', { connectionId: connection_id })
}
export async function getAiSetup(refresh = false): Promise<AiSetup> {
  return await invoke('get_ai_setup', { refresh })
}
export async function getSectionSummaries(scope: string, sections: SectionInput[]): Promise<SectionSummaries> {
  return await invoke('get_section_summaries', { input: { scope, sections } })
}
export async function postSectionSummary(input: PostSectionSummaryInput): Promise<SectionSummaries> {
  return await invoke('post_section_summary', { input })
}
export async function postOllamaPull(model: string): Promise<void> {
  return await invoke('post_ollama_pull', { model })
}
export async function postOllamaStart(): Promise<OllamaStatus> {
  return await invoke('post_ollama_start')
}
export async function getCliSetup(): Promise<CliSetup> {
  return await invoke('get_cli_setup')
}
export async function postClaudePlugin(options: PluginOptions): Promise<string[]> {
  return await invoke('post_claude_plugin', { options })
}
export async function postCliPath(): Promise<string> {
  return await invoke('post_cli_path')
}
export async function postIntelligenceConnectionTest(connection_id: string): Promise<string> {
  return await invoke('post_intelligence_connection_test', { connectionId: connection_id })
}
export async function getIntelligenceModels(connection_id: string): Promise<string[]> {
  return await invoke('get_intelligence_models', { connectionId: connection_id })
}
export async function getMyUserIds(): Promise<string[]> {
  return await invoke('get_my_user_ids')
}
export async function getCases(): Promise<CaseFile[]> {
  return await invoke('get_cases')
}
export async function getCase(case_id: string): Promise<CaseFile> {
  return await invoke('get_case', { caseId: case_id })
}
export async function postCase(input: PostCaseInput): Promise<CaseFile> {
  return await invoke('post_case', { input })
}
export async function putCase(input: PutCaseInput): Promise<CaseFile> {
  return await invoke('put_case', { input })
}
export async function deleteCase(case_id: string): Promise<void> {
  return await invoke('delete_case', { caseId: case_id })
}
export async function putCaseItems(input: CaseItemsInput): Promise<CaseFile> {
  return await invoke('put_case_items', { input })
}
export async function deleteCaseItems(input: CaseItemsInput): Promise<CaseFile> {
  return await invoke('delete_case_items', { input })
}
export async function getCaseTimeline(case_id: string, limit?: number): Promise<Message[]> {
  return await invoke('get_case_timeline', { caseId: case_id, limit })
}
export async function postExpectation(input: PostExpectationInput): Promise<CaseFile> {
  return await invoke('post_expectation', { input })
}
export async function putExpectation(input: PutExpectationInput): Promise<CaseFile> {
  return await invoke('put_expectation', { input })
}
export async function deleteExpectation(expectation_id: string): Promise<void> {
  return await invoke('delete_expectation', { expectationId: expectation_id })
}
export async function postCaseRule(input: PostCaseRuleInput): Promise<CaseFile> {
  return await invoke('post_case_rule', { input })
}
export async function deleteCaseRule(rule_id: string): Promise<void> {
  return await invoke('delete_case_rule', { ruleId: rule_id })
}
export async function getLockState(): Promise<import('./types').LockState> {
  return await invoke('get_lock_state')
}
export async function postLock(): Promise<import('./types').LockState> {
  return await invoke('post_lock')
}
export async function deleteLock(): Promise<import('./types').LockState> {
  return await invoke('delete_lock')
}
export async function putLock(enabled: boolean, idle_minutes?: number): Promise<import('./types').LockState> {
  return await invoke('put_lock', { enabled, idleMinutes: idle_minutes })
}
export async function getActionGroups(): Promise<import('./types').ActionGroup[]> {
  return await invoke('get_action_groups')
}
export async function postActionGroup(input: import('./types').PostActionGroupInput): Promise<import('./types').ActionGroup> {
  return await invoke('post_action_group', { input })
}
export async function putActionGroup(input: import('./types').PutActionGroupInput): Promise<import('./types').ActionGroup> {
  return await invoke('put_action_group', { input })
}
export async function deleteActionGroup(group_id: string): Promise<void> {
  return await invoke('delete_action_group', { groupId: group_id })
}
export async function getActions(): Promise<import('./types').Action[]> {
  return (await invoke<import('./types').Action[]>('get_actions')).filter((action) => !action.key || featureShown(action.key))
}
export async function getAction(action_id: string): Promise<import('./types').Action> {
  return await invoke('get_action', { actionId: action_id })
}
export async function postAction(input: import('./types').PostActionInput): Promise<import('./types').Action> {
  return await invoke('post_action', { input })
}
export async function putAction(input: import('./types').PutActionInput): Promise<import('./types').Action> {
  return await invoke('put_action', { input })
}
export async function deleteAction(action_id: string): Promise<void> {
  return await invoke('delete_action', { actionId: action_id })
}
export async function postActionCheck(input: import('./types').ActionCheckInput): Promise<import('./types').ActionCheckOutcome> {
  return await invoke('post_action_check', { input })
}
export async function postActionRender(draft: import('./types').ActionDraft): Promise<string> {
  return await invoke('post_action_render', { draft })
}
export async function postActionRun(action_id: string, input?: Record<string, unknown> | null, dry_run = false, message_id?: string | null): Promise<import('./types').ActionRun> {
  return await invoke('post_action_run', { actionId: action_id, input: input ?? null, dryRun: dry_run, messageId: message_id ?? null })
}
export async function postActionNodeRun(action_id: string, node_id: string, input?: Record<string, unknown> | null, dry_run = true, message_id?: string | null): Promise<import('./types').ActionRun> {
  return await invoke('post_action_node_run', { actionId: action_id, nodeId: node_id, input: input ?? null, dryRun: dry_run, messageId: message_id ?? null })
}
export async function getActionHook(action_id: string, node_id: string): Promise<string> {
  return await invoke('get_action_hook', { actionId: action_id, nodeId: node_id })
}

export async function getPresentationTemplate(action_id: string, node_id: string): Promise<import('./types').PresentationTemplate> {
  return await invoke('get_presentation_template', { actionId: action_id, nodeId: node_id })
}
export async function postPresentationTemplateError(action_id: string, node_id: string, message_id: string, error: string): Promise<import('./types').ActionRun> {
  return await invoke('post_presentation_template_error', { actionId: action_id, nodeId: node_id, messageId: message_id, error })
}
export async function postTemplateAssist(input: import('./types').TemplateAssistInput): Promise<import('./types').TemplateAssistOutcome> {
  return await invoke('post_template_assist', { input })
}
export async function getActionRuns(action_id?: string | null, limit?: number): Promise<import('./types').ActionRun[]> {
  return await invoke('get_action_runs', { actionId: action_id ?? null, limit: limit ?? null })
}
export async function getActionSample(message_id?: string | null, person_id?: string | null): Promise<import('./types').ActionSample | null> {
  return await invoke('get_action_sample', { messageId: message_id ?? null, personId: person_id ?? null })
}
export async function getActionCatalog(): Promise<import('./types').ActionCatalog> {
  const catalog = await invoke<import('./types').ActionCatalog>('get_action_catalog')
  return { ...catalog, actions: catalog.actions.filter((action) => !action.key || featureShown(action.key)) }
}
export async function postActionAssist(input: import('./types').ActionAssistInput): Promise<import('./types').ActionAssistOutcome> {
  return await invoke('post_action_assist', { input })
}
export async function postMessageSchedule(input: import('./types').PostMessageScheduleInput): Promise<import('./types').MessageSchedule> {
  return await invoke('post_message_schedule', { input })
}
export async function getMessageSchedules(conversation_id?: string): Promise<import('./types').MessageSchedule[]> {
  return await invoke('get_message_schedules', { conversationId: conversation_id ?? null })
}
export async function deleteMessageSchedule(schedule_id: string): Promise<void> {
  return await invoke('delete_message_schedule', { scheduleId: schedule_id })
}
export async function getInboxes(): Promise<import('./types').Inbox[]> {
  return await invoke('get_inboxes')
}
export async function postInbox(input: import('./types').PostInboxInput): Promise<import('./types').Inbox> {
  return await invoke('post_inbox', { input })
}
export async function putInbox(input: import('./types').PutInboxInput): Promise<import('./types').Inbox> {
  return await invoke('put_inbox', { input })
}
export async function deleteInbox(inbox_id: string): Promise<void> {
  return await invoke('delete_inbox', { inboxId: inbox_id })
}
export async function putInboxMembers(input: import('./types').PutInboxMembersInput): Promise<import('./types').Inbox> {
  return await invoke('put_inbox_members', { input })
}
export async function postActionBackfill(action_id?: string | null, limit?: number): Promise<import('./types').ActionBackfillReport> {
  return await invoke('post_action_backfill', { actionId: action_id ?? null, limit: limit ?? null })
}
export async function getMockMode(): Promise<MockMode> {
  return await invoke('get_mock_mode')
}
export async function putMockState(provider_id: string, state: MockServerState): Promise<MockMode> {
  return await invoke('put_mock_state', { providerId: provider_id, state })
}
export async function getMergeGroups(): Promise<MergeGroup[]> {
  return await invoke('get_merge_groups')
}
export async function postMergeGroup(input: MergeGroupInput): Promise<MergeGroup[]> {
  return await invoke('post_merge_group', { input })
}
export async function deleteMergeGroupItems(input: MergeGroupInput): Promise<MergeGroup[]> {
  return await invoke('delete_merge_group_items', { input })
}
export async function getMergeGroupMessages(conversation_id: string, limit?: number, offset?: number): Promise<Message[]> {
  return await invoke('get_merge_group_messages', { conversationId: conversation_id, limit, offset })
}
export async function getSharedFiles(input: GetSharedInput): Promise<SharedFile[]> {
  return await invoke('get_shared_files', { input })
}
export async function getSharedLinks(input: GetSharedInput): Promise<SharedLink[]> {
  return await invoke('get_shared_links', { input })
}
export async function getSidebarLayout(): Promise<import('./types').SidebarLayout> {
  return await invoke('get_sidebar_layout')
}
export async function putSidebarLayout(layout: import('./types').SidebarLayout): Promise<import('./types').SidebarLayout> {
  return await invoke('put_sidebar_layout', { layout })
}
export async function getPersonProfile(person_id: string): Promise<PersonProfile> {
  return await invoke('get_person_profile', { personId: person_id })
}
export async function putObjectOverride(input: PutObjectOverrideInput): Promise<ObjectOverride[]> {
  return await invoke('put_object_override', { input })
}
export async function deleteObjectOverride(input: DeleteObjectOverrideInput): Promise<ObjectOverride[]> {
  return await invoke('delete_object_override', { input })
}
export async function getChats(): Promise<Chat[]> {
  return await invoke('get_chats')
}
export async function postChat(input: PostChatInput): Promise<Chat> {
  return await invoke('post_chat', { input })
}
export async function putChat(input: PutChatInput): Promise<Chat> {
  return await invoke('put_chat', { input })
}
export async function getChatItems(conversation_id: string, before?: string | null, limit?: number): Promise<ChatPage> {
  return await invoke('get_chat_items', { conversationId: conversation_id, before: before ?? null, limit: limit ?? null })
}
export async function getChatGrid(conversation_id: string): Promise<GridItem[]> {
  return await invoke('get_chat_grid', { conversationId: conversation_id })
}
export async function putChatAction(conversation_id: string, action_id: string): Promise<string[]> {
  return await invoke('put_chat_action', { input: { conversation_id, action_id } })
}
export async function deleteChatAction(conversation_id: string, action_id: string): Promise<string[]> {
  return await invoke('delete_chat_action', { input: { conversation_id, action_id } })
}
export async function postChatMessage(conversation_id: string, text: string, target_id?: string | null, reply_to_id?: string | null): Promise<ChatItem> {
  return await invoke('post_chat_message', { input: { conversation_id, text, target_id: target_id ?? null, reply_to_id: reply_to_id ?? null } })
}
export async function getChatPipe(conversation_id: string): Promise<ChatPipe> {
  return await invoke('get_chat_pipe', { conversationId: conversation_id })
}
export async function putChatRoutes(conversation_id: string, action_id: string, routes: ChatRoute[]): Promise<ChatPipe> {
  return await invoke('put_chat_routes', { input: { conversation_id, action_id, routes } })
}
export async function postChatBackfill(conversation_id: string, limit?: number): Promise<number> {
  return await invoke('post_chat_backfill', { conversationId: conversation_id, limit: limit ?? null })
}
export async function postAttachmentOpen(input: AttachmentOpenInput): Promise<string> {
  return await invoke('post_attachment_open', { input })
}
export async function postActionCopy(action_id: string, without_trigger: boolean, name?: string | null): Promise<import('./types').Action> {
  return await invoke('post_action_copy', { actionId: action_id, withoutTrigger: without_trigger, name: name ?? null })
}
export async function putActions(input: import('./types').PutActionsInput): Promise<import('./types').ActionsOutcome> {
  return await invoke('put_actions', { input })
}
export async function getVoiceSetup(): Promise<import('./types').VoiceSetup> {
  return await invoke('get_voice_setup')
}
export async function postVoiceSetup(force = false): Promise<void> {
  return await invoke('post_voice_setup', { force })
}
export async function postVoice(conversation_id?: string | null): Promise<string> {
  return await invoke('post_voice', { conversationId: conversation_id ?? null })
}
export async function deleteVoice(voice_id: string): Promise<string> {
  return await invoke('delete_voice', { voiceId: voice_id })
}
export async function getDrafts(conversation_id?: string | null): Promise<import('./types').Draft[]> {
  return await invoke('get_drafts', { conversationId: conversation_id ?? null })
}
export async function putDraft(input: import('./types').PutDraftInput): Promise<import('./types').Draft> {
  return await invoke('put_draft', { input })
}
export async function deleteDraft(conversation_id: string, thread_id?: string | null): Promise<void> {
  return await invoke('delete_draft', { conversationId: conversation_id, threadId: thread_id ?? null })
}
export async function getReminders(pending = true): Promise<import('./types').Reminder[]> {
  return await invoke('get_reminders', { pending })
}
export async function postReminder(input: import('./types').PostReminderInput): Promise<import('./types').Reminder> {
  return await invoke('post_reminder', { input })
}
export async function deleteReminder(reminder_id: string): Promise<boolean> {
  return await invoke('delete_reminder', { reminderId: reminder_id })
}
export async function getActivity(before?: string | null, limit?: number): Promise<import('./types').ActivityPage> {
  return await invoke('get_activity', { before: before ?? null, limit: limit ?? null })
}
export async function getProviderMessages(query: string, conversation_id?: string | null, provider_id?: string | null): Promise<Message[]> {
  return await invoke('get_provider_messages', { input: { query, conversation_id: conversation_id ?? null, provider_id: provider_id ?? null } })
}
export async function getJoinableConversations(provider_id?: string | null): Promise<import('./types').JoinableConversation[]> {
  return await invoke('get_joinable_conversations', { providerId: provider_id ?? null })
}
export async function postConversationMembership(input: { conversation_id?: string | null; provider_id?: string | null; external_id?: string | null }): Promise<boolean> {
  return await invoke('post_conversation_membership', { input })
}
export async function deleteConversationMembership(conversation_id: string): Promise<boolean> {
  return await invoke('delete_conversation_membership', { conversationId: conversation_id })
}
export async function putConversationState(conversation_id: string, archived: boolean): Promise<boolean> {
  return await invoke('put_conversation_state', { conversationId: conversation_id, archived })
}
export async function putConversationTopic(conversation_id: string, topic?: string | null, purpose?: string | null): Promise<boolean> {
  return await invoke('put_conversation_topic', { input: { conversation_id, topic: topic ?? null, purpose: purpose ?? null } })
}
export async function postProviderConversation(provider_id: string, conversation: import('./types').NewRemoteConversation, member_ids: string[] = []): Promise<Conversation | null> {
  return await invoke('post_provider_conversation', { input: { provider_id, conversation, member_ids } })
}
export async function getPresences(user_ids: string[]): Promise<import('./types').PresenceUpdate[]> {
  return await invoke('get_presences', { userIds: user_ids })
}
export async function getUserGroups(provider_id: string): Promise<import('./types').RemoteUserGroup[]> {
  return await invoke('get_user_groups', { providerId: provider_id })
}
export async function getSignature(input: { provider_id: string; alias?: string | null; mode?: 'new' | 'reply' | 'forward'; conversation_id?: string | null; content_id?: string | null }): Promise<import('./types').RenderedSignature | null> {
  return await invoke('get_signature', { input })
}
export async function getSignatures(content_id?: string | null): Promise<import('./types').ContentSignature[]> {
  return await invoke('get_signatures', { contentId: content_id ?? null })
}
export async function putSignature(input: import('./types').ContentSignature): Promise<import('./types').ContentSignature[]> {
  return await invoke('put_signature', { input })
}
export async function deleteSignature(content_id: string, provider_id: string, alias?: string | null): Promise<void> {
  return await invoke('delete_signature', { contentId: content_id, providerId: provider_id, alias: alias ?? null })
}
export async function postSignatureImport(provider_id: string): Promise<import('./types').ContentItem[]> {
  return await invoke('post_signature_import', { providerId: provider_id })
}
export async function getAliases(provider_id: string): Promise<import('./types').RemoteAlias[]> {
  return await invoke('get_aliases', { providerId: provider_id })
}
export async function getAutoReply(provider_id: string): Promise<import('./types').RemoteAutoReply | null> {
  return await invoke('get_auto_reply', { providerId: provider_id })
}
export async function putAutoReply(provider_id: string, reply: import('./types').RemoteAutoReply): Promise<boolean> {
  return await invoke('put_auto_reply', { providerId: provider_id, reply })
}
export async function putSignatureSync(content_id: string): Promise<number> {
  return await invoke('put_signature_sync', { contentId: content_id })
}
export async function getBookmarks(): Promise<import('./types').BookmarkedMessage[]> {
  return await invoke('get_bookmarks')
}
export async function deleteBookmark(input: { message_id: string }): Promise<void> {
  return await invoke('delete_bookmark', { input })
}
export async function postConversationMember(input: { conversation_id: string; user_id: string }): Promise<void> {
  return await invoke('post_conversation_member', { input })
}
export async function deleteConversationMember(input: { conversation_id: string; user_id: string }): Promise<void> {
  return await invoke('delete_conversation_member', { input })
}
