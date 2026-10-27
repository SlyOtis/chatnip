# Chatnip for Raycast

Script commands that drive Chatnip from Raycast: Inbox, Search Messages, Reply, Today's Agenda, Add Event, Sync Now and Activity.

## Setup

1. Install Chatnip, so `chatnip` is on your PATH (`brew install --cask slyotis/chatnip/chatnip`) or the app is in `/Applications`.
2. In Raycast, open Settings, Extensions, press **+**, choose **Add Script Directory** and pick this `raycast` folder.

The commands call `chatnip` and read its JSON with `jq`, installed with `brew install jq` if your Mac lacks it. They work whether or not the app is open: with the app open the change shows in the window at once, otherwise the engine starts without a window. With the Touch ID lock on, a command asks for Touch ID first.

## Commands

| Command | Arguments | Does |
|---|---|---|
| Inbox | | Lists unread conversations, newest first |
| Search Messages | query | Searches with the same filters as the app: `from:`, `in:`, `has:`, `is:`, `before:`, `after:` |
| Reply | conversation, message | Sends the message, as markdown, to the conversation whose name best matches |
| Today's Agenda | | Today's events from every calendar, with meeting links |
| Add Event | title, time, minutes | Adds an event; the time is `14:00` for today or `2026-11-03 09:30`, the length 30 minutes unless given |
| Sync Now | | Fetches what changed on every account |
| Activity | | Mentions, replies and reactions aimed at you |

Each script is plain bash, so copying one is the quickest way to add your own: every `chatnip <resource> <verb>` from `chatnip commands` works the same way.
