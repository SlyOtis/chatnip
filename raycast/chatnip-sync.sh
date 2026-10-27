#!/bin/bash

# @raycast.schemaVersion 1
# @raycast.title Sync Now
# @raycast.mode compact
# @raycast.packageName Chatnip
# @raycast.icon ../src-tauri/icons/128x128.png
# @raycast.description Fetch what changed on every connected account

source "$(dirname "$0")/lib/chatnip.sh"

cn sync create >/dev/null
echo "Synced"
