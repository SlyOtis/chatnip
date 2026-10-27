#!/bin/bash

# @raycast.schemaVersion 1
# @raycast.title Activity
# @raycast.mode fullOutput
# @raycast.packageName Chatnip
# @raycast.icon ../src-tauri/icons/128x128.png
# @raycast.description Mentions, replies and reactions aimed at you

source "$(dirname "$0")/lib/chatnip.sh"

cn activity get --limit 25 | jq -r "$JQ_DEFS"'
  rows.items // [] | if length == 0 then "No activity" else .[] | "\(.at | stamp)  " + ([.kind, .actor, .conversation_name, .detail] | map(select(. != null and . != "") | line) | join("  ·  ")) end'
