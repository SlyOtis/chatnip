#!/bin/bash

# @raycast.schemaVersion 1
# @raycast.title Today's Agenda
# @raycast.mode fullOutput
# @raycast.packageName Chatnip
# @raycast.icon ../src-tauri/icons/128x128.png
# @raycast.description Today's events from every calendar

source "$(dirname "$0")/lib/chatnip.sh"

start=$(date -j -f "%Y-%m-%d %H:%M:%S" "$(date +%F) 00:00:00" +%s)
end=$((start + 86400 + 3600))
end=$(date -j -f "%Y-%m-%d %H:%M:%S" "$(date -r "$end" +%F) 00:00:00" +%s)

cn events get --data "$(jq -n --arg s "$(utc "$start")" --arg e "$(utc "$end")" '{input: {start: $s, end: $e}}')" | jq -r "$JQ_DEFS"'
  rows | map(select(.status != "cancelled" and .status != "dismissed")) | sort_by((.all_day | not), (.starts_at | epoch))
  | if length == 0 then "Nothing today" else .[] | (if .all_day then "all day     " else "\(.starts_at | clock)–\(.ends_at | clock)" end) + "  \(.title | line)" + (if .location != "" then "  ·  \(.location | line)" else "" end) + (if .online_meeting_url then "  ·  \(.online_meeting_url)" else "" end) end'
