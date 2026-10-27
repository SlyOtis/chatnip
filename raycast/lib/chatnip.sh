CHATNIP_BIN=$(command -v chatnip || echo /Applications/Chatnip.app/Contents/MacOS/chatnip)

cn() {
  local out code
  out=$("$CHATNIP_BIN" "$@" --json 2>/dev/null)
  code=$?
  case $code in
    0) printf '%s' "$out" ;;
    3) echo "The network refused the request" >&2; exit 3 ;;
    4) echo "Chatnip is locked or the account is not connected" >&2; exit 4 ;;
    *) echo "chatnip $1 $2 failed with exit $code" >&2; exit "$code" ;;
  esac
}

JQ_DEFS='
def epoch: if length == 10 then . + "T00:00:00Z" else . end
  | sub("\\.[0-9]+"; "")
  | capture("^(?<b>.{19})(?<z>Z|[+-][0-9]{2}:[0-9]{2})?$")
  | (.b + "Z" | fromdateiso8601) - (if (.z // "Z") == "Z" then 0 else ((.z[1:3] | tonumber) * 3600 + (.z[4:6] | tonumber) * 60) * (if .z[0:1] == "+" then 1 else -1 end) end);
def clock: epoch | strflocaltime("%H:%M");
def stamp: epoch | strflocaltime("%a %d %b %H:%M");
def rows: if type == "object" and has("data") then .data else . end;
def account: [.providers[]?.name] | unique | join(", ");
def line: gsub("\\s+"; " ") | if length > 90 then .[0:89] + "…" else . end;
'

utc() {
  date -u -r "$1" +%Y-%m-%dT%H:%M:%SZ
}
