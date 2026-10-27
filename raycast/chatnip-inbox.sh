#!/bin/bash

# @raycast.schemaVersion 1
# @raycast.title Inbox
# @raycast.mode fullOutput
# @raycast.packageName Chatnip
# @raycast.icon ../src-tauri/icons/128x128.png
# @raycast.description Unread conversations across every network

source "$(dirname "$0")/lib/chatnip.sh"

cn conversations get | jq -r "$JQ_DEFS"'
  rows
  | map(select(.unread_count > 0 and (.is_archived | not) and .deleted_at == null))
  | sort_by(.last_activity) | reverse
  | if length == 0 then "Inbox zero" else .[] | "\(.unread_count | tostring | (" " * (3 - length)) + .)  \(.name | line)  ·  \(account)  ·  \(.last_activity | stamp)" end'
