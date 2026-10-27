#!/bin/bash

# @raycast.schemaVersion 1
# @raycast.title Search Messages
# @raycast.mode fullOutput
# @raycast.packageName Chatnip
# @raycast.icon ../src-tauri/icons/128x128.png
# @raycast.argument1 { "type": "text", "placeholder": "from:anna has:file invoice" }
# @raycast.description Search every conversation with Chatnip's filters

source "$(dirname "$0")/lib/chatnip.sh"

cn search get --data "$(jq -n --arg q "$1" '{input: {query: $q, limit: 30}}')" | jq -r "$JQ_DEFS"'
  (.snippets | map({key: .message_id, value: .snippet}) | from_entries) as $snip
  | (.conversations | map("▸ " + ([(.name | line), account] | map(select(. != "")) | join("  ·  ")))) + (.messages | map("\(.sender_name // .sender_email // "?")  ·  \($snip[.id] // .subject // .content // "" | gsub("<[^>]*>"; "") | line)"))
  | if length == 0 then "Nothing found" else .[] end'
