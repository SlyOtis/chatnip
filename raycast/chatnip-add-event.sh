#!/bin/bash

# @raycast.schemaVersion 1
# @raycast.title Add Event
# @raycast.mode compact
# @raycast.packageName Chatnip
# @raycast.icon ../src-tauri/icons/128x128.png
# @raycast.argument1 { "type": "text", "placeholder": "Title" }
# @raycast.argument2 { "type": "text", "placeholder": "14:00 or 2026-11-03 09:30" }
# @raycast.argument3 { "type": "text", "placeholder": "Minutes", "optional": true }
# @raycast.description Add an event to the default calendar

source "$(dirname "$0")/lib/chatnip.sh"

case $2 in
  [0-9][0-9]:[0-9][0-9]|[0-9]:[0-9][0-9]) when="$(date +%F) $2:00" ;;
  *) when="$2:00" ;;
esac
starts=$(date -j -f "%Y-%m-%d %H:%M:%S" "$when" +%s 2>/dev/null) || { echo "Could not read the time $2"; exit 2; }
minutes=${3:-30}
[[ $minutes =~ ^[0-9]+$ ]] || { echo "Minutes is a number, got $minutes"; exit 2; }
ends=$((starts + minutes * 60))

cn event create --data "$(jq -n --arg t "$1" --arg s "$(utc "$starts")" --arg e "$(utc "$ends")" '{input: {draft: {title: $t, starts_at: $s, ends_at: $e}}}')" >/dev/null
echo "Added $1, $(date -r "$starts" '+%a %d %b %H:%M')"
