#!/bin/bash

# @raycast.schemaVersion 1
# @raycast.title Reply
# @raycast.mode compact
# @raycast.packageName Chatnip
# @raycast.icon ../src-tauri/icons/128x128.png
# @raycast.argument1 { "type": "text", "placeholder": "Conversation" }
# @raycast.argument2 { "type": "text", "placeholder": "Message" }
# @raycast.description Send a message to the conversation whose name matches

source "$(dirname "$0")/lib/chatnip.sh"

match=$(cn conversations get | jq -c --arg n "$1" "$JQ_DEFS"'
  rows | map(select(.deleted_at == null and (.name | ascii_downcase | contains($n | ascii_downcase))))
  | sort_by(((.name | ascii_downcase) != ($n | ascii_downcase)), (.last_activity | epoch | -.)) | .[0] // empty | {id, name}' | tail -1)
[ -n "$match" ] || { echo "No conversation matches $1"; exit 1; }

id=$(jq -r .id <<<"$match")
cn message create --data "$(jq -n --arg id "$id" --arg c "$2" '{input: {conversation_id: $id, content: $c, content_type: "markdown"}}')" >/dev/null
echo "Sent to $(jq -r .name <<<"$match")"
