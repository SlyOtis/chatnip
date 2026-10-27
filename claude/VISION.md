# Chatnip Vision

Unified communication client for desktop. One app for all chats, emails, and calendars.

## Core Concept

Conversations in Chatnip are local constructs, independent of how they exist in each provider. A Chatnip conversation can map to a Teams group chat, a Slack channel, individual DMs to each participant, or any combination. The provider structure does not dictate the local structure.

All conversation metadata, grouping, threads, and relationships are stored locally in our database. Providers are delivery channels, not the source of truth for how conversations are organized.

## Sidebar

All active conversations grouped by:
- Person (1:1 across providers)
- Team or channel
- Thread

Each item shows the latest activity regardless of provider. Provider badges indicate where the conversation lives. A conversation can span multiple providers (e.g. Teams chat + related emails).

## Local Conversations and Threads

A conversation is a Chatnip concept. The user can:
- Create a group conversation that maps to individual DMs across different providers
- Bundle a Teams chat, a Slack channel, and email threads into one conversation
- Choose per send whether to deliver as separate DMs, create a provider group, or reply in an existing thread

Threads are also local. Any message can start a thread in Chatnip. When replying in a thread:
- If the underlying provider message is in a group chat, the thread reply goes to that chat
- If the underlying provider message is a DM, the thread reply is just a response in that DM
- If the message spans multiple providers, the user picks which channel to reply through

All of this is metadata in our local database. Providers never need to know about Chatnip's conversation structure.

## Message Actions

Every message has hover and right click actions contextual to its type and provider.

**Common actions (all message types):**
- React with emoji (mapped to provider reactions where supported)
- Reply in thread (local thread, delivery mapped to provider context)
- Copy, forward, bookmark

**Chat message actions:**
- Reply in thread
- React
- Edit or delete (if provider supports it and user is the sender)

**Email message actions:**
- Reply, reply all, forward (as email)
- Discuss: starts a new chat on a chosen provider (Teams, Slack) with selected users, shares the email content as the opening message in that chat. The invited users see the email body and can discuss it in real time.
- Link to existing conversation

**Provider specific actions** surface only when relevant (e.g. Teams message actions only appear on Teams messages).

## Chat Window

Messages render inline with provider indicators (Teams, Slack, email icon). Chat messages appear as standard bubbles. Emails appear as compact cards that expand to show the full email with subject, recipients, and formatted body.

When sending, the user chooses the delivery method:
- Which provider to send through
- Whether to send as separate messages to each participant or as a group
- Whether to create a group on the provider side to reflect the local conversation
- Which thread context to reply in

## Email Integration

Emails are first class citizens alongside chat messages. Users can:
- See emails appear in relevant conversations automatically (by participants)
- Manually move or link emails into any conversation
- Choose which conversations an email appears in
- Expand email cards to read full content inline
- Compose email replies from within any conversation

## Calendar

A joint calendar view combining all connected accounts. Shows events from every Teams account, Slack reminders, and email calendar invites in one view. When creating an event, the user chooses which calendar or account to create it on.

## Multi Account

Users can connect multiple Teams accounts, multiple Slack workspaces, and multiple email accounts. Each can have its own profile or share a single unified profile across all.

## Current State

### Built
- Microsoft Teams provider (sync, send, reactions)
- Outlook email sync into conversations
- Unified message rendering with type routing (chat, email, bot, system)
- Provider theme system with per-provider colors
- Multi-provider database schema (conversation_provider junction)
- Setup flow with OAuth for Microsoft
- Sidebar with conversation list, unread counts, provider badges
- Profile system with themes and avatars

### Not Yet Built
- Local conversation management (create, merge, split conversations independent of providers)
- Local threading system (thread any message, map replies to provider context)
- Message action menus (hover toolbar, right click context menu)
- Discuss email flow (create chat on provider, share email content, invite users)
- Send flow with delivery options (provider picker, group vs individual, thread context)
- Slack provider backend (UI scaffolding exists)
- Google provider backend (UI scaffolding exists)
- Calendar view (route placeholder only)
- Email composition UI
- Conversation grouping in sidebar (currently flat list)
- Email linking/moving between conversations
- Multi-account switching UI
- Message search
- Attachment handling
- Presence and typing indicators
