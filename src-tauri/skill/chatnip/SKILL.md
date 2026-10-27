---
name: chatnip
description: Drives the chatnip cli, the command line of the Chatnip desktop app that unifies Outlook, Teams, Gmail, Slack, IMAP and calendars on this Mac. Use when asked to read, search or summarise someone's mail and chats, look up a person and every address they write from, work with cases, tags and splits, read and write chats with their sources, grid views and attached actions, read or create calendar events, reuse replies, prompts, snippets and GIFs from Content, draft a reply with AI, or send a reply through Chatnip.
---

# chatnip

`chatnip` is the Chatnip app itself. With no arguments it opens the window. With arguments it runs one engine command and prints the result as JSON. When the app is open the command runs inside it, so the window updates at once. When it is closed the same engine starts without a window, answers and exits. Everything the app can do, the command line can do, because both call the same commands.

Everything belongs to the Mac user and the active profile. Tokens stay in the Keychain and never appear in any output.

## How commands are named

The grammar is `chatnip <resource> <verb> [--flags]`. The verbs are `get` (reads), `create` (adds), `update` (changes) and `delete` (removes). The engine names them in Rust as `get_`, `post_`, `put_` and `delete_`:

- `get_conversations` is `chatnip conversations get`
- `post_message` is `chatnip message create`
- `put_conversation_read` is `chatnip conversation-read update`
- `delete_person_identity` is `chatnip person-identity delete`

Flags are the parameter names in kebab case, for example `--conversation-id`. Values are read as JSON when they parse, so `--limit 20` is a number and `--ids '["a","b"]'` is a list. A body is given with `--set key=value` (repeatable, dotted keys nest: `--set input.conversation_id=ID --set input.content=Hi`), `--data '{...}'` or `--data @file.json`, or `--file path` (`-` reads stdin).

On a terminal the answer is a table; in a pipe, and to Claude through MCP, it is JSON. `--json` forces JSON on a terminal, `--raw` prints the whole answer compact, `--plain` prints tab separated lines. Every command that prints data takes `--limit N` and `--cursor N` (a cut list ends with the `--cursor` to continue from), `--format concise|detailed`, `--fields a,b.c`, `--sort field[:desc]`, `--query key=value` (keep rows whose field contains the value), `--jq EXPR` (through jq) and `--template TEXT|@FILE` (Jinja style, with `json`, `join`, `pluck`, `timeago`, `color` and the built in filters). Colour follows `NO_COLOR`, `CLICOLOR` and `TERM=dumb`.

Exit codes: 0 ok, 1 failure, 2 usage or rejected input, 3 refused by the provider, 4 not connected, expired or locked, 130 interrupted. Every error names the command that fixes it.

`chatnip help` prints this manual; `chatnip help <resource> <verb>` prints one command with an example. `chatnip commands` lists every command with its flags; `chatnip commands --json` adds the MCP tool name and input schema. `chatnip call <rust_name> --data '{...}'` runs any command by its engine name.

## Configs

`~/.chatnip/config.yaml` holds named configs, `app` (the app data) and `mock` (`home: ~/.chatnip-mock`, `use_debug: true`) to begin with. Keys are `home`, `use_debug` and `secrets`. A `.chatnip/config.yaml` in a repository, found walking up, pins it with `use: <name>` and may override single keys; secrets are refused there with exit 4. Values resolve flag, environment, `--env FILE`, the local file, then the named config picked by `--config`, `CHATNIP_CONFIG`, the local `use`, then `default`. `chatnip config get` shows each value and its layer; `chatnip config set KEY VALUE [--config NAME]`, `chatnip config use NAME`, `chatnip config local NAME|off`. `--global` ignores the local file.

## One model on every stack

Chatnip models each feature once and translates it to the stack behind each connection: Slack (Web API and Socket Mode), Microsoft Graph for Teams and Outlook, the Gmail API, and IMAP and SMTP. What a connection can do is in `capabilities` on `chatnip providers get`; a feature it cannot do natively either runs in Chatnip (send later, reminders, drafts, saved items on chat stacks) or is not offered.

- Live: Slack and IMAP push changes (Slack needs the `slack.app_token` setting); Teams, Outlook and Gmail are checked for what changed and only those conversations are fetched
- `chatnip activity get [--before AT]`: mentions of you, `@here`, `@channel`, replies to you and reactions to your messages
- `chatnip search get --input '{"query":"from:anna in:design has:file after:2026-10-01 invoice"}'`: local search with `from:`, `in:`, `has:link|file|reaction|mention`, `is:unread|saved|pinned|thread`, `before:` and `after:`; `chatnip provider-messages get --input '{"query":"…"}'` asks the providers themselves
- `chatnip drafts get`, `chatnip draft update --input '{"conversation_id":"ID","markdown":"…"}'`, `chatnip draft delete --conversation-id ID`: one draft per conversation and thread; mail drafts are mirrored to Gmail, Outlook or the IMAP drafts folder
- `chatnip reminder create --input '{"message_id":"ID","at":"2026-10-28T09:00:00+01:00"}'`, `chatnip reminders get`, `chatnip reminder delete --reminder-id ID`
- `chatnip bookmark create --input '{"message_id":"ID"}'`, `chatnip bookmarks get`: saved items; on mail they are the provider's star or flag
- `chatnip joinable-conversations get`, `chatnip conversation-membership create --input '{"provider_id":"ID","external_id":"C123"}'`, `chatnip conversation-membership delete --conversation-id ID`, `chatnip conversation-topic update --input '{"conversation_id":"ID","topic":"…","purpose":"…"}'`, `chatnip conversation-state update --conversation-id ID --archived true`, `chatnip provider-conversation create --input '{…}'`
- `chatnip presences get --user-ids '["ID"]'`, `chatnip user-groups get --provider-id ID`
- Mentions are written `[@Name](mention:USER_ID)`, `[@here](mention:here)`, `[@group](mention:group:ID)` and `[#name](channel:CONVERSATION_ID)`; Chatnip translates them to each stack and across stacks through the person behind the mention
- `chatnip thread-reply create --input '{"thread_id":"ID","content":"…","content_type":"markdown","broadcast":true}'`: reply in a thread, and with `broadcast` also in the conversation where the stack allows it

## Mail

- Signatures are Content items of kind `signature`, templates with `.account.email`, `.account.alias` and `.account.provider` beside the usual variables. `chatnip signature update --input '{"content_id":"ID","provider_id":"ID","alias":"me@x.no","use_new":true,"use_reply":true,"use_forward":false}'` binds one to an account and address; every mail sent from it is signed below `-- `, unless the send carries `"signature": false` in its metadata, or `"signature_id"` to pick another. `chatnip signatures get`, `chatnip signature get --input '{"provider_id":"ID","alias":"…","mode":"reply"}'` renders what will be used, `chatnip signature-import create --provider-id ID` takes Gmail's signatures in, and saving a bound signature writes it back to Gmail
- `chatnip aliases get --provider-id ID`: the addresses a mail account sends as; `"from"` in a send's metadata picks one
- `chatnip auto-reply get --provider-id ID`, `chatnip auto-reply update --provider-id ID --reply '{"enabled":true,"body":"…","starts_at":null,"ends_at":null,"external_only":false,"subject":null}'`: automatic replies on Outlook and Gmail
- Done writes back as archive on Gmail, Outlook and IMAP

## Useful reads

- `chatnip conversations get`: the inbox with splits, unread counts and tags
- `chatnip conversation get --input '{"conversation_id":"ID"}'`: one conversation
- `chatnip search get --input '{"query":"TEXT"}'`: search conversations and messages
- `chatnip people get` and `chatnip person get --person-id ID`: a person with every address and alias they use
- `chatnip person-messages get --person-id ID`: mail and chat from one person across every source
- `chatnip action-run create --action-id summarise_mail --message-id ID`: the one line AI summary of a mail, made once and kept
- `chatnip cases get`, `chatnip case get --case-id ID`, `chatnip case-timeline get --case-id ID`
- `chatnip events get --input '{"start":"2026-10-01T00:00:00Z","end":"2026-11-01T00:00:00Z"}'` and `chatnip calendars get`: the merged calendar and its calendars
- `chatnip event create --input '{"draft":{"title":"Planning","starts_at":"2026-10-28T09:00:00+01:00","ends_at":"2026-10-28T10:00:00+01:00","attendees":["anna@client.example"],"notes":"","location":""},"targets":["CALENDAR_ID"],"online_meeting":true}'`: create an event in one or more calendars; attendees get invites from Outlook, Teams or Google, so confirm first. Without `targets` it stays a local event
- `chatnip event update --input '{"event_id":"ID","draft":{…}}'`, `chatnip event delete --event-id ID`, `chatnip event-response update --event-id ID --response accepted` (`declined`, `tentativelyAccepted`), `chatnip calendar update --calendar-id ID --enabled false`, `chatnip proposed-events get` (meetings found in mail), `chatnip calendar-export get` (writes an .ics and returns its path)
- `chatnip app-settings get`, `chatnip app-setting update --key slack.app_token --value xapp-…`: every app setting, such as `sync.interval_seconds`, `sync.live_interval_seconds` and the client ids under Settings, Connections; secrets show as a marker and are kept in the Keychain
- `chatnip bookmark delete --input '{"message_id":"ID"}'`, `chatnip signature delete --content-id ID --provider-id ID --alias me@x.no`, `chatnip signature-sync update --content-id ID`, `chatnip drafts get --conversation-id ID`, `chatnip reminders get --pending false`, `chatnip joinable-conversations get --provider-id ID`
- `chatnip tags get`, `chatnip groups get`, `chatnip providers get`, and `chatnip stats get --input '{"start":"…","end":"…"}'`
- `chatnip prompts get`, `chatnip ai-setup get`: the default prompts script nodes start from, and the AI connections with the default model

Many commands take one `--input` object; its fields are snake case. `chatnip commands --json` shows every field and which ones are required.

Run `chatnip commands` to see the exact flags before calling a command for the first time.

## Actions

Everything Chatnip does on its own is an action: sorting into splits, finding appointments, summarising mail, deciding human or service, the user's old sorting rules, and anything the user builds. An action has a signature (typed inputs and outputs) and a body that is a graph of nodes. A node is a trigger, a script in Rhai, a built in step, or another action used as a node.

- `chatnip actions get`, `chatnip action get --action-id ID_OR_KEY`: the actions. Starter actions answer to their key: `sort_splits`, `find_appointments`, `summarise_mail`, `draft_reply`, `recognise_invites`. Every built in is a read only action in the nested groups under Built in (`builtin_kind` trigger, step, landing or action): triggers keyed `trigger:<on>` (`arrival`, `sent`, `edited`, `deleted`, `reaction`, `mention`, `pattern`, `ai`, `filter`, `schedule`, `event_soon`, `chat_read`, `manual`, `called`), steps and landings keyed `builtin:<step>` (plus `builtin:condition`, `builtin:script`, `builtin:chat_read`, `builtin:chat_write`). A built in trigger, step or landing never runs on its own; put its node in an action. No built in names a connection: a trigger's scope narrows it with `scope.providers` (kinds of connection), `scope.connection_ids`, `scope.conversation_ids`, `scope.tag_ids`, `scope.person_ids`, `scope.has_attachments`, `scope.humans_only`, `scope.types`, and on the trigger `group_ids`, `direction` (`incoming`, `outgoing`, `any`), `message_kinds` (`email`, `chat`, `channel`, `other`) and, for `event_soon`, `minutes_before`. Built in actions, `classify_senders` (Human or service, on by default) and `builtin:receipts` (Receipts, off by default), can be switched on and off with `chatnip action update --input '{"action_id":"classify_senders","enabled":false}'` but not edited. Receipts: then Extract (body, HTML, PDF and image text per file), Match (editable regex patterns with weights in the copy), Confirm (a small local model through Ollama, batched, passing candidates on unconfirmed when no local model answers) and Organise (one record per receipt with vendor, date, total, currency, VAT, order number, card ending, source, message, attachment path, excerpt and confidence); duplicate it to change it
- `chatnip action-catalog get`: types, the script functions with signatures, built in steps, every action with its kind, description, signature and required settings, and the action groups the picker mirrors
- `chatnip action-code get --action-id ID_OR_KEY`: the action as code, the form to read and edit
- `chatnip action-check create --input '{"code":"…"}'`: compile and type check code or a draft before saving; returns diagnostics with line numbers
- `chatnip action create --input '{"name":"…","code":"…"}'` and `chatnip action update --input '{"action_id":"ID","code":"…"}'`: create or change an action from its code
- `chatnip action-run create --action-id ID_OR_KEY --dry-run true --message-id MESSAGE`: run it against a message without changing anything; the log says what each node would do
- `chatnip action-runs get --action-id ID`, `chatnip action-backfill create --action-id KEY --limit 50`
- `chatnip action-groups get`, `chatnip action-group create --input '{"name":"Mail"}'`
- `chatnip message-schedule create --input '{"conversation_id":"ID","content":"…","at":"2026-10-15T08:00:00+02:00"}'`

The code form:

```
action(#{ "name": "Thank for invoices", "description": "Files invoices and drafts a thank you" });
output("text", "text");
node("trigger", "trigger", #{ "label": "Invoice arrives", "x": 0.0, "y": 0.0, "config": #{ "on": "arrival" } }, #"
regex_match(text, "invoice|faktura")
"#);
node("draft", "builtin:ai_reply", #{ "label": "", "x": 0.0, "y": 160.0, "config": #{ "values": #{} } }, "");
node("reply", "builtin:reply", #{ "label": "", "x": 0.0, "y": 320.0, "config": #{ "values": #{} } }, "");
connect("trigger", "conversation", "draft", "conversation");
connect("trigger", "conversation", "reply", "conversation");
connect("draft", "text", "reply", "text");
```

Scripts get their inputs as variables and return a map of outputs. Triggers only decide whether to run; functions that change things are refused there. A trigger node with `"disabled": true` in its config is switched off; the action's other triggers still fire. A script node that calls `prompt("key", …)`, `ai()` or `ai_json()` uses the default prompt from `chatnip prompts get` unless its config carries `"prompts": #{ "key": #{ "system": …, "template": …, "output_schema": …, "connection_id": …, "model": …, "enabled": false } }`, or `"content_id"` in place of `"template"` to take the prompt from Content; only the fields given replace the default, for that node only. To draft a reply, run `chatnip action-run create --action-id draft_reply --input '{"conversation":"ID","instruction":"…"}'`.

When you write an action for the user, sends and replies go out directly, so use a draft node where the user wants to read the text first, check it with `action-check create`, dry run it, then show the user before switching it on.

## Chats

Every chat has a mode, `read` or `write`. A read chat has no composer and shows what flows into it; a write chat is a normal chat. A group in read mode makes every chat inside it read. Chats made in Chatnip have the type `local` and a list of sources: chats, read chats, groups, tags, people and saved searches. A read chat can source from other read chats to any depth; a change that would make the sources loop is refused. A read chat picks its view in its details: `chat` (the timeline) or `grid` (every item that lands in or flows through the chat as tiles: messages, mail, files, images and videos, links, records, events and Content items; filtered by `show` (messages, files, media, links, records, events; empty is all), grouped by type, kind, modified, source, sender, tag, group, name or none, sorted by date, name, size, kind, sender or reactions, in scrolling or wrapped rows); the view and its settings are kept per chat.

An action without a start trigger can be attached to any chat; the chat then starts it, a write chat on every message that arrives or is sent there, a read chat on every item that flows in. A read chat is a pipe: its sources flow in, attached actions transform them, and each output lands where its routes say; a read chat made in Chatnip whose actions land in it shows what lands there, grouped by its view, instead of its raw sources. A write chat made in Chatnip merges the chats and DMs picked as its sources and sends through their connections. The Chat node (read and write variants, `chatnip chat-node get`) starts an action from a chat when nothing is wired into it, and delivers into a chat when something is.

- `chatnip chats get`: the chats made in Chatnip with their mode, sources and attached actions
- `chatnip chat create --input '{"name":"Receipts","mode":"read","view":"grid","sources":[{"kind":"query","query":{"types":["email"]}}],"action_ids":["ACTION_ID"]}'`: make a chat; `mode` is `read` or `write`
- `chatnip chat update --input '{"chat_id":"ID","sources":[{"kind":"conversation","id":"CHAT_ID"}]}'`, `chatnip chat delete --chat-id ID`
- `chatnip conversation update --input '{"conversation_id":"ID","mode":"read","view":"grid","view_settings":{"grid":{"group":"type","sort":"date","direction":"desc","rows":"scroll","tags":true,"show":["records","files"]}}}'`: the mode, view and view settings of any chat
- `chatnip chat-items get --conversation-id ID`, `chatnip chat-grid get --conversation-id ID`: what a chat shows, as timeline items or as grid tiles of every kind
- `chatnip action-copy create --action-id builtin:receipts --without-trigger true`: duplicate an action without its start trigger, so it can be attached
- `chatnip chat-action update --input '{"conversation_id":"ID","action_id":"ID"}'`, `chatnip chat-action delete --input '{…}'`: attach or detach an action
- `chatnip chat-backfill create --conversation-id ID --limit 200`: run the attached actions over what already flowed in
- `chatnip chat-message create --input '{"conversation_id":"ID","text":"…","target_id":"CHAT_ID"}'`: write in a write chat made in Chatnip. When the chat merges picked chats or DMs (its `conversation` sources), it sends through the target, by default the one written through last; with none picked it keeps a local note. Attached actions run on it
- `chatnip chat-pipe get --conversation-id ID`: the pipe of a chat: its sources, each attached action with its input and output schema, where each output lands, and any schema mismatch
- `chatnip chat-routes update --input '{"conversation_id":"ID","action_id":"ID","routes":[{"output":"summary","node":"chat","input":"item"},{"output":"conversation","node":"builtin:tag_add","input":"conversation","values":{"tag":"Kunde AS"}}]}'`: route an attached action's outputs to landings: `chat` (records or messages in this read chat) or a built in step such as `builtin:tag_add`, `builtin:group_add`, `builtin:move`, `builtin:mark_read`, `builtin:pin`, `builtin:notify` or `builtin:present_*`; refused when an output does not fit the landing's input; `"routes":[]` goes back to the default, every list or JSON output into the chat
- `chatnip attachment-open create --input '{"message_id":"ID","attachment_id":"ID","quick_look":true}'`: download an attachment to the cache and open it in its default app, or in Quick Look

Attaching, creating and changing chats change data; say what will happen and wait for the user to confirm, as for any command that is not a `get`.

## Content

The user's own library of everything he reuses: GIFs, stickers, images, videos and shared files, plus templates for message replies, email replies, AI prompts and snippets. Each item has a title, tags and keywords, sits in any number of the user's groups, counts how often it was sent, and records where it came from: a message, a conversation, a person, a URL, an upload, an action, or written by hand.

- `chatnip content get --query "monday" --sort relevance`: search the library; sorts are `relevance`, `used`, `recent`, `newest`; `--kind reply` (or `gif`, `sticker`, `image`, `video`, `file`, `email`, `prompt`, `snippet`) narrows it
- `chatnip content-tags get`: every tag with how many items carry it
- `chatnip content-groups get`: the user's group tree. A group has a `parent_id` and a `position`; `manual` groups hold `content_ids` in order, `kind` and `tag` groups fill themselves from their `rule`. An item can sit in several groups at once
- `chatnip content-group create --input '{"name":"Customers","parent_id":"ID"}'`, `chatnip content-group update --input '{"content_group_id":"ID","parent_id":null,"position":0}'`, `chatnip content-group delete --content-group-id ID` (its children move up, its items stay in their other groups)
- `chatnip content-group-items update --input '{"content_group_id":"ID","content_ids":["A"],"position":0}'`: add items to a group or reorder them there; they stay in every other group. `content-group-items delete` takes them out of that one group only
- `chatnip gifs get --query "slow clap"`: search Tenor and GIPHY as well (needs a key in Settings, Media)
- `chatnip content create --input '{"kind":"reply","name":"Thanks","body":"Thanks {{ .person.first_name | default \"there\" }}!"}'`: write a template; `source_message_id`, `source_conversation_id`, `source_person_id` or `source_action_id` record where it came from
- `chatnip content update --input '{"content_id":"ID","tags":["monday"],"body":"…"}'`: retitle, retag or rewrite one item
- `chatnip content-bulk update --input '{"content_ids":["A","B"],"add_tags":["team"],"add_group_ids":["G"]}'`: tag or group many at once
- `chatnip content-from-message create --message-id ID`: save the first image, GIF or video in a received message; `--attachment-index 1` picks the next one
- `chatnip content-from-url create --url https://…/party.gif --tags '["party"]'`: save from a link
- `chatnip content-render create --input '{"content_id":"ID","conversation_id":"ID"}'`: fill a template from the conversation, its person and the user; `values` overrides any variable, `"sample":true` without a conversation fills sample values. The answer lists the template's variables and the ones nothing filled
- `chatnip content-uses get --content-id ID`: the actions and nodes that use an item
- `chatnip content-send create --content-id ID --conversation-id ID`: send an item to a chat; a reply, email or snippet is filled in from that conversation first

Templates use Go template syntax: `{{ .person.first_name }}`, `{{ if .x }}…{{ else }}…{{ end }}`, `{{ range .messages }}{{ .sender }}{{ end }}`, and pipes to `lower`, `upper`, `default "fallback"` and `date "2 Jan 2006"`. The variables are `me` (name, first_name, email), `person` (name, first_name, email), `conversation` (name, type, provider, members), `message` (subject, from_name, from_email, date, text), `messages`, `today` and `now`.

In actions, `content("Thanks", #{ conversation: c })` returns a filled template and `prompt("Polite decline", #{ conversation: c })` runs an AI prompt from Content and answers `#{ answer }`. A node can also take the prompt behind `prompt("key")`, `ai()` or `ai_json()` from Content with `"prompts": #{ "key": #{ "content_id": "ID" } }`; it then holds no template of its own.

Saving is safe and idempotent: the same file is stored once. Sending reaches other people: name the item and the chat, and wait for the user to confirm before running `content-send create`.

## Safety

Reads are safe to run freely. Every other verb changes data, and many reach other people:

- sending, replying, reacting, editing or deleting messages
- creating events with attendees, which sends invites through Outlook, Teams or Google
- connecting or removing accounts, merging people, creating, changing or switching on actions
- joining, leaving or archiving channels, changing topics, and writing signatures or automatic replies to a provider

Before running any command that is not a `get`, say what it will do and wait for the user to confirm. Never send a message or an invite on your own initiative. Never call commands that start sign-in flows or change the lock unless asked.

When Chatnip is locked, a command asks for Touch ID first. If the user refuses, the command fails with "Chatnip stayed locked". Do not retry in a loop.

## The plugin

`chatnip plugin install` writes `~/.chatnip/plugin` and installs `chatnip@chatnip` through the claude cli. The plugin carries this skill and `chatnip mcp`, which serves every engine command as a tool named `<resource>_<verb>`, such as `conversations_get` or `message_create`. Tools carry `readOnlyHint`, `destructiveHint` and `idempotentHint`; sends and deletes also ask for the user's confirmation. `chatnip skill install` installs only this skill. `chatnip path install` links the binary into `~/.local/bin`. `chatnip completion --auto` writes shell completion into the data folder.

`chatnip plugin install --config mock` points the mcp server at the `mock` config, the mock home `~/.chatnip-mock` with the mock clients on; without `--config` the server reads the app data. The config's home and mode are written into the plugin manifest as the server's `CHATNIP_HOME` and `CHATNIP_USE_DEBUG`, nothing else, and `chatnip plugin status` reads them back as `mcp home` and `mock mode`. Settings, Developer, Claude Code plugin offers the same choice: App data, the mock data home from the `mock.home` setting, or a custom folder, and a Mock mode switch; the choice is kept in the `plugin.home`, `plugin.home_path` and `plugin.mock` settings.
