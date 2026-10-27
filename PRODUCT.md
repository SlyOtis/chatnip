# Product

<!-- impeccable:product-schema 1 -->

## Platform

macOS desktop (Tauri 2, WebKit webview)

## Users

Milo, the single user and owner, working across several organisations at once: more than one Microsoft 365 tenant, more than one Slack workspace, and mail accounts outside Microsoft 365. He reads at a desk display by day and in a dim room at night, so the OS colour scheme decides the rendition. One person on one machine; a profile is a workspace of his, not another user.

## Product Purpose

Chatnip pulls every conversation he is part of, mail, Teams chats and channels, Slack channels and direct messages, from every tenant and workspace into one inbox. From there he reads, answers, parses and organises all of it, and anything with a time attached lands in a shared calendar automatically. Success is that he stops opening Outlook, Teams and Slack per tenant and works the day from Chatnip.

## Positioning

One inbox across tenants. Outlook, Teams and Slack each show one organisation at a time and nothing from the others; Chatnip's unit is the conversation regardless of where it lives, with the source as a fact about it, not a place to go. A conversation is a local construct: it can span a Teams chat, a Slack channel and a mail thread, and replies go back through whichever provider each message came from.

## Operating Context

A Tauri 2 desktop app on macOS: Rust backend with SQLite, Svelte 5 frontend, run locally only. No cloud deployment and no provisioned infrastructure. Accounts are connected from the app's own settings: the user adds a tenant or workspace, is sent through the provider's consent flow in the browser, and the connection is stored locally with its tokens. Microsoft 365 is read and written through Microsoft Graph with delegated permissions; Slack through a Slack app; Gmail through the Gmail API; other mail through IMAP and SMTP. Providers are polled on an interval. AI work, sorting, rule drafting and parsing, is done by invoking the Claude Code CLI already installed on the machine, so it costs nothing extra; no hosted model API is called by the product.

## Capabilities and Constraints

- Connections: add, consent, refresh and remove Microsoft 365 tenants, Slack workspaces, Google accounts and IMAP accounts from settings, several of each side by side in one profile. Each connection carries its own credentials, namespace and sync state.
- Inbox: one list of conversations across all connections, newest activity first, with the source and organisation visible on every item; unread, pinned, muted, archived; search across everything; sidebar sections. Inside a split the list is grouped as the original vision set it (claude/VISION.md): starred, the user's own sections, people (one row per person regardless of provider), teams and channels, mail, open threads. A conversation is a local construct and may span providers. One row per person: identities across Teams, Slack and mail that share an email or a direct-message name merge into a person, and the person opens as one thread with every channel in time order. Humans come first; senders recognised as services (no-reply, notifications, newsletters, bots) are grouped apart. The sidebar holds the user's own inboxes first (each linking accounts, people, chats, labels or groups, overlap allowed, with nested groups), then a flat catch-all Inbox, Labels, Done, Snoozed, Notifications, Deleted and Pipeline, then Actions, Content, Calendar and Statistics; every entry has an icon, a chevron and a more menu. Provider identity shows as a small grey glyph, never as colour; the Teams look is what the product moves away from. The look is Superhuman's desktop as it ships, read from its screenshots: one-line rows without avatars, splits as text tabs in the top bar, bare paper panes with hairlines, ghost icon actions with shortcut tooltips, one filled button per surface. The user keeps every Chatnip feature; only the visual grammar is Superhuman's.
- Thread: a conversation opens as its messages in order with participants, attachments and reactions; reply or send from the thread through the conversation's own provider; local threads on any message.
- Organising: main tabs (splits) and groups are the user's own structure, managed in settings and from the command palette. Groups nest, a group can be a tag or a saved query (providers, types, labels, people, unread, attachments, humans only, dates, sender, text, regular expression, sort by activity, name, unread, messages, tags or created). Actions are the only automation: an action has typed inputs and outputs and a graph of nodes (triggers, provider neutral built in functions, Rhai script nodes, other actions); each script node is edited as code; actions are grouped, triggered on arrival, filter, regex, AI judgement, schedule or after sending, and send directly when their graph sends; a draft node keeps the text as a draft.
- Statistics: a page below Calendar reading every pattern in the user's own traffic: who writes the most and the longest, who answers fastest, the hour and weekday rhythm, channels and labels, reply time distribution, busiest conversations, most used words, and a globe of where mail came from (origin IP from the Received headers, resolved with a GeoLite2 file the user supplies). Any two people or two periods compare side by side.
- Intelligence: one place for every model origin: the local Claude Code CLI (the default when found), Replicate, any OpenAI compatible endpoint (Ollama, LM Studio, Jan, OpenAI) and any local command (the user's own tools such as llm). The page keeps only the connections and the default model. Prompts live inside the action nodes that use them: each script node that asks an AI starts from the code default Go template and may override the prompt, the origin and the model for that node alone.
- Calendar: a calendar in Chatnip's own database; events are created from messages automatically when a time is parsed out of them, and by hand; shown in the app and exported as an ICS feed.
- Content: one library of everything the user reuses, GIFs (Tenor or GIPHY with the user's own key), stickers, images, videos, shared files, custom emoji, and templates for message replies, email replies, AI prompts and snippets in Go template syntax. The page stacks it in the user's own nested groups, an item can sit in several at once, and every item shows where it came from and where it is used. Templates fill from the conversation, the person and the user in the composer, in actions and from the command line; media is attached as an upload when the provider cannot carry the original.
- Test clients: a mock adapter for every client kind (Teams, Outlook, Slack, Gmail, Google Chat, IMAP) behind the same provider port, switched on in settings, with a shared cast of people so merging and sorting can be exercised without real accounts.
- Assistant: the local Claude Code CLI, invoked by the Rust backend with the text in hand; used for sorting, rule drafting, parsing times and people, and summarising a thread. Never a chat with a model as a product surface.
- Existing: Teams chats over Graph (sync, send, reactions, presence), Slack (sync, send, reactions), Gmail (threads, send), local conversations that merge and link across providers, threads, pins, bookmarks, sidebar sections, search, setup flow per provider, profile themes.
- Undecided: which Teams channel conversations are pulled first (channels can be loud), and how far back history is synced on first connect; the first answer is thirty days and all conversation types.

## Brand Commitments

The name Chatnip. The user pinned the visual world and amended it in October 2026; this amendment wins over every earlier note. Superhuman's desktop grammar stays (keyboard first, split tabs as text, ghost icon actions with shortcut tooltips, one filled button per surface, hairlines instead of cards), but nothing may read as Teams: no purple or blue violet anywhere, no Teams style app bar. The accent is green (#1f9d55 by day, #4cc77e by night) with orange as its warm partner. The far left sidebar is frosted glass over an animated green to orange gradient done only in CSS (default backdrop "meadow-flow", drifting slowly, still under reduced motion); the list and reading panes are solid theme paper, never frosted and never gradient. Panes are flat and edge to edge: no rounded panels, no framed or outlined panes, no gaps or padding around panes. Adjacent panes (sidebar, list, reading pane, side panel) are separated by hairlines only, never by a drop shadow. Inputs are flat: no rounded borders, no outline box, no focus ring or border highlight; at most a hairline rule. Sidebar rows have a rounded selection fill that changes on hover and select, with contrasting text and icon on the selected row; every sidebar entry and list section carries an icon from the icon bank. Rows are compact with no extra left or right padding. Every layout is a CSS grid whose template is sized only by CSS variables, resized through sly-svelte-pane's DraggableEdge, and panels slide in by animating grid-template-columns; sizes are relative. Schibsted Grotesk with Martian Mono for times, codes and keys.


## Evidence on Hand

No marketing claims, no testimonials, no pricing. The content is the user's own mail, chats and calendar once connected; test fixtures under `src/test/fixtures` stand in until then. `claude/VISION.md` holds the original product notes and the earlier built and not built lists.

## Product Principles

1. The conversation is the unit; the source is a fact on it, never a mode or a separate place.
2. Everything arrives already sorted; the user teaches the sorting, he does not do it by hand twice.
3. Anything with a time becomes a calendar entry without being asked.
4. Nothing leaves the machine except through the provider the message belongs to; the assistant runs locally.
5. Connecting an account is a settings task, finished in the app, with consent given there.
