import { describe, it, expect, vi, beforeEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}))

const mockedInvoke = vi.mocked(invoke)

beforeEach(() => {
  mockedInvoke.mockReset()
  mockedInvoke.mockResolvedValue({})
})

describe('api', () => {

  describe('connection setup', () => {
    it('reads the registration details and opens the admin approval link', async () => {
      const { getConnectionSetup, postAdminConsent } = await import('./api')
      await getConnectionSetup()
      expect(mockedInvoke).toHaveBeenCalledWith('get_connection_setup')
      await postAdminConsent()
      expect(mockedInvoke).toHaveBeenCalledWith('post_admin_consent')
    })
  })
  describe('initializeApp', () => {
    it('calls invoke with initialize_app', async () => {
      const { postStartup } = await import('./api')
      await postStartup()
      expect(mockedInvoke).toHaveBeenCalledWith('post_startup')
    })
  })

  describe('postProfile', () => {
    it('calls invoke with post_profile and input', async () => {
      const { postProfile } = await import('./api')
      const input = { name: 'Test User' }
      await postProfile(input)
      expect(mockedInvoke).toHaveBeenCalledWith('post_profile', { input })
    })
  })

  describe('getProfile', () => {
    it('calls invoke with get_profile', async () => {
      const { getProfile } = await import('./api')
      await getProfile()
      expect(mockedInvoke).toHaveBeenCalledWith('get_profile')
    })
  })

  describe('putProfileSettings', () => {
    it('calls invoke with put_profile_settings and input', async () => {
      const { putProfileSettings } = await import('./api')
      const input = {
        settings: {
          email_content_threshold: 100,
          navigation_width: 280,
          navigation_collapsed: false,
          details_width: 320,
          details_panel_open: true,
          message_timestamp_interval: 5,
          time_format: '12h',
          window_controls_style: 'native',
          max_input_lines: 10,
        },
      }
      await putProfileSettings(input)
      expect(mockedInvoke).toHaveBeenCalledWith('put_profile_settings', { input })
    })
  })

  describe('postProfileAvatar', () => {
    it('calls invoke with post_profile_avatar and input', async () => {
      const { postProfileAvatar } = await import('./api')
      const input = { avatar_data: 'base64data' }
      await postProfileAvatar(input)
      expect(mockedInvoke).toHaveBeenCalledWith('post_profile_avatar', { input })
    })
  })

  describe('getConversations', () => {
    it('calls invoke with get_conversations', async () => {
      const { getConversations } = await import('./api')
      await getConversations()
      expect(mockedInvoke).toHaveBeenCalledWith('get_conversations')
    })
  })

  describe('getConversation', () => {
    it('calls invoke with get_conversation and wrapped input', async () => {
      const { getConversation } = await import('./api')
      await getConversation('conv-001')
      expect(mockedInvoke).toHaveBeenCalledWith('get_conversation', {
        input: { conversation_id: 'conv-001' },
      })
    })
  })

  describe('getCurrentConversation', () => {
    it('calls invoke with get_current_conversation', async () => {
      const { getCurrentConversation } = await import('./api')
      await getCurrentConversation()
      expect(mockedInvoke).toHaveBeenCalledWith('get_current_conversation')
    })
  })

  describe('getMessages', () => {
    it('calls invoke with get_current_messages', async () => {
      const { getMessages } = await import('./api')
      await getMessages()
      expect(mockedInvoke).toHaveBeenCalledWith('get_current_messages')
    })
  })

  describe('sendMessage', () => {
    it('calls invoke with send_message and input', async () => {
      const { postMessage } = await import('./api')
      const input = { conversation_id: 'conv-001', content: 'Hello world' }
      await postMessage(input)
      expect(mockedInvoke).toHaveBeenCalledWith('post_message', { input })
    })
  })

  describe('sendMessageReaction', () => {
    it('calls invoke with send_message_reaction and input', async () => {
      const { postMessageReaction } = await import('./api')
      const input = { conversation_id: 'conv-001', message_id: 'msg-001', reaction: '👍' }
      await postMessageReaction(input)
      expect(mockedInvoke).toHaveBeenCalledWith('post_message_reaction', { input })
    })
  })

  describe('syncAll', () => {
    it('calls invoke with sync_all', async () => {
      const { postSync } = await import('./api')
      await postSync()
      expect(mockedInvoke).toHaveBeenCalledWith('post_sync')
    })
  })

  describe('setCurrentConversation', () => {
    it('calls invoke with set_current_conversation and wrapped input', async () => {
      const { putCurrentConversation } = await import('./api')
      await putCurrentConversation('conv-002')
      expect(mockedInvoke).toHaveBeenCalledWith('put_current_conversation', {
        input: { conversation_id: 'conv-002' },
      })
    })
  })

  describe('startProviderAuth', () => {
    it('calls invoke with start_provider_auth and wrapped input', async () => {
      const { postProviderAuth } = await import('./api')
      await postProviderAuth('microsoft')
      expect(mockedInvoke).toHaveBeenCalledWith('post_provider_auth', {
        input: { provider_name: 'microsoft' },
      })
    })
  })

  describe('connectSlackToken', () => {
    it('calls invoke with connect_slack_token and the token', async () => {
      const { postSlackConnection } = await import('./api')
      await postSlackConnection('xoxp-1')
      expect(mockedInvoke).toHaveBeenCalledWith('post_slack_connection', { token: 'xoxp-1' })
    })
  })

  describe('connectImap', () => {
    it('calls invoke with connect_imap and the wrapped input', async () => {
      const { postImapConnection } = await import('./api')
      const input = { email: 'a@x', password: 'p', imap_host: 'imap.x', smtp_host: 'smtp.x' }
      await postImapConnection(input)
      expect(mockedInvoke).toHaveBeenCalledWith('post_imap_connection', { input })
    })
  })

  describe('deleteProvider', () => {
    it('calls invoke with delete_provider and the id', async () => {
      const { deleteProvider } = await import('./api')
      await deleteProvider('p1')
      expect(mockedInvoke).toHaveBeenCalledWith('delete_provider', { providerId: 'p1' })
    })
  })

  describe('syncProvider', () => {
    it('calls invoke with sync_provider and the id', async () => {
      const { postProviderSync } = await import('./api')
      await postProviderSync('p1')
      expect(mockedInvoke).toHaveBeenCalledWith('post_provider_sync', { providerId: 'p1' })
    })
  })

  describe('app settings', () => {
    it('reads and writes settings by key', async () => {
      const { getAppSettings, putAppSetting } = await import('./api')
      await getAppSettings()
      expect(mockedInvoke).toHaveBeenCalledWith('get_app_settings')
      await putAppSetting('microsoft.client_id', 'abc')
      expect(mockedInvoke).toHaveBeenCalledWith('put_app_setting', { key: 'microsoft.client_id', value: 'abc' })
    })
  })

  describe('getPlatform', () => {
    it('calls invoke with get_platform', async () => {
      const { getPlatform } = await import('./api')
      await getPlatform()
      expect(mockedInvoke).toHaveBeenCalledWith('get_platform')
    })
  })

  describe('window frame', () => {
    it('saves the frame the backend reads from the window', async () => {
      const { putWindowPosition } = await import('./api')
      await putWindowPosition()
      expect(mockedInvoke).toHaveBeenCalledWith('put_window_position')
    })

    it('opens the window to its frame over the launch duration', async () => {
      const { putWindowFrame } = await import('./api')
      await putWindowFrame(400)
      expect(mockedInvoke).toHaveBeenCalledWith('put_window_frame', { duration: 400 })
    })
  })

  describe('postLog', () => {
    it('calls invoke with log and level, message, source', async () => {
      const { postLog } = await import('./api')
      await postLog('info', 'test message', 'frontend')
      expect(mockedInvoke).toHaveBeenCalledWith('post_log', {
        level: 'info',
        message: 'test message',
        source: 'frontend',
      })
    })

    it('calls invoke with log without source', async () => {
      const { postLog } = await import('./api')
      await postLog('error', 'something broke')
      expect(mockedInvoke).toHaveBeenCalledWith('post_log', {
        level: 'error',
        message: 'something broke',
        source: undefined,
      })
    })
  })

  describe('getProviders', () => {
    it('calls invoke with get_providers', async () => {
      const { getProviders } = await import('./api')
      await getProviders()
      expect(mockedInvoke).toHaveBeenCalledWith('get_providers')
    })
  })

  describe('getUsers', () => {
    it('calls invoke with get_users', async () => {
      const { getUsers } = await import('./api')
      await getUsers()
      expect(mockedInvoke).toHaveBeenCalledWith('get_users')
    })
  })

  describe('createConversation', () => {
    it('calls invoke with create_conversation and input', async () => {
      const { postConversation } = await import('./api')
      const input = {
        name: 'New Chat',
        conversation_type: 'group_message',
        group_type: 'team',
        member_ids: ['user-001', 'user-002'],
      }
      await postConversation(input)
      expect(mockedInvoke).toHaveBeenCalledWith('post_conversation', { input })
    })
  })

  describe('linkConversations', () => {
    it('calls invoke with link_conversations and input', async () => {
      const { postConversationLink } = await import('./api')
      const input = {
        source_conversation_id: 'conv-001',
        target_conversation_id: 'conv-003',
        link_type: 'related',
      }
      await postConversationLink(input)
      expect(mockedInvoke).toHaveBeenCalledWith('post_conversation_link', { input })
    })
  })

  describe('createThread', () => {
    it('calls invoke with create_thread and input', async () => {
      const { postThread } = await import('./api')
      const input = { message_id: 'msg-001', name: 'Discussion' }
      await postThread(input)
      expect(mockedInvoke).toHaveBeenCalledWith('post_thread', { input })
    })

    it('calls invoke with create_thread without optional name', async () => {
      const { postThread } = await import('./api')
      const input = { message_id: 'msg-002' }
      await postThread(input)
      expect(mockedInvoke).toHaveBeenCalledWith('post_thread', { input })
    })
  })

  describe('replyInThread', () => {
    it('calls invoke with reply_in_thread and input', async () => {
      const { postThreadReply } = await import('./api')
      const input = { thread_id: 'thread-001', content: 'Thread reply' }
      await postThreadReply(input)
      expect(mockedInvoke).toHaveBeenCalledWith('post_thread_reply', { input })
    })

    it('calls invoke with reply_in_thread including content_type', async () => {
      const { postThreadReply } = await import('./api')
      const input = { thread_id: 'thread-001', content: '<p>Rich reply</p>', content_type: 'text/html' }
      await postThreadReply(input)
      expect(mockedInvoke).toHaveBeenCalledWith('post_thread_reply', { input })
    })
  })

  describe('getThreadMessages', () => {
    it('calls invoke with get_thread_messages and input', async () => {
      const { getThreadMessages } = await import('./api')
      const input = { thread_id: 'thread-001' }
      await getThreadMessages(input)
      expect(mockedInvoke).toHaveBeenCalledWith('get_thread_messages', { input })
    })
  })

  describe('updateConversation', () => {
    it('calls invoke with update_conversation and full input', async () => {
      const { putConversation } = await import('./api')
      const input = {
        conversation_id: 'conv-001',
        name: 'Renamed Chat',
        group_type: 'team',
        is_pinned: true,
        is_muted: false,
      }
      await putConversation(input)
      expect(mockedInvoke).toHaveBeenCalledWith('put_conversation', { input })
    })

    it('calls invoke with update_conversation and partial input', async () => {
      const { putConversation } = await import('./api')
      const input = { conversation_id: 'conv-001', is_pinned: true }
      await putConversation(input)
      expect(mockedInvoke).toHaveBeenCalledWith('put_conversation', { input })
    })
  })

  describe('searchConversations', () => {
    it('calls invoke with search_conversations and input', async () => {
      const { getSearch } = await import('./api')
      const input = { query: 'project update', limit: 10 }
      await getSearch(input)
      expect(mockedInvoke).toHaveBeenCalledWith('get_search', { input })
    })

    it('calls invoke with search_conversations without limit', async () => {
      const { getSearch } = await import('./api')
      const input = { query: 'meeting notes' }
      await getSearch(input)
      expect(mockedInvoke).toHaveBeenCalledWith('get_search', { input })
    })
  })
})

describe('content', () => {
  it('getGifs passes query, limit and cursor', async () => {
    const { getGifs } = await import('./api')
    await getGifs('cats', 20)
    expect(mockedInvoke).toHaveBeenCalledWith('get_gifs', { query: 'cats', limit: 20, cursor: null })
    await getGifs('cats', 20, { tenor: 'CAgQ', giphy: '20' })
    expect(mockedInvoke).toHaveBeenCalledWith('get_gifs', { query: 'cats', limit: 20, cursor: { tenor: 'CAgQ', giphy: '20' } })
  })

  it('putContentOrder and link preview pass their arguments', async () => {
    const { putContentOrder, getLinkPreview } = await import('./api')
    await putContentOrder(['a', 'b'])
    expect(mockedInvoke).toHaveBeenCalledWith('put_content_order', { contentIds: ['a', 'b'] })
    await getLinkPreview('https://github.com')
    expect(mockedInvoke).toHaveBeenCalledWith('get_link_preview', { url: 'https://github.com' })
  })

  it('getContent passes kind and query', async () => {
    const { getContent } = await import('./api')
    await getContent('gif', 'party')
    expect(mockedInvoke).toHaveBeenCalledWith('get_content', { kind: 'gif', query: 'party' })
  })

  it('postContent wraps the input', async () => {
    const { postContent } = await import('./api')
    const input = { kind: 'gif' as const, name: 'dance', source_url: 'https://media.tenor.com/dance.gif' }
    await postContent(input)
    expect(mockedInvoke).toHaveBeenCalledWith('post_content', { input })
  })

  it('putContent wraps the input', async () => {
    const { putContent } = await import('./api')
    const input = { content_id: 'm1', is_favourite: true }
    await putContent(input)
    expect(mockedInvoke).toHaveBeenCalledWith('put_content', { input })
  })

  it('deleteContent passes the id as contentId', async () => {
    const { deleteContent } = await import('./api')
    await deleteContent('m1')
    expect(mockedInvoke).toHaveBeenCalledWith('delete_content', { contentId: 'm1' })
  })

  it('content groups and rendering pass their input', async () => {
    const { putContentGroupItems, postContentRender, getContentUses } = await import('./api')
    await putContentGroupItems({ content_group_id: 'g1', content_ids: ['a'], position: 0 })
    expect(mockedInvoke).toHaveBeenCalledWith('put_content_group_items', { input: { content_group_id: 'g1', content_ids: ['a'], position: 0 } })
    await postContentRender({ content_id: 'c1', conversation_id: 'x' })
    expect(mockedInvoke).toHaveBeenCalledWith('post_content_render', { input: { content_id: 'c1', conversation_id: 'x' } })
    await getContentUses('c1')
    expect(mockedInvoke).toHaveBeenCalledWith('get_content_uses', { contentId: 'c1' })
  })
})
