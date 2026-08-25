#!/bin/sh
# Reads status files exported by zellij-pi-dashboard/extensions/zellij-status.ts.

session=${ZELLIJ_SESSION_NAME-}
[ -n "$session" ] || exit 0
session=session-$(printf '%s' "$session" | sed 's/[^A-Za-z0-9_.-]/_/g')
dir=${XDG_RUNTIME_DIR:-/tmp}/pi-zellij-status-$(id -u)/$session
[ -d "$dir" ] || exit 0

set -- "$dir"/*.json
[ -e "$1" ] || exit 0

valid=
for file do
    pid=$(jq -r '.pid // empty' "$file" 2>/dev/null)
    case $pid in
        ''|*[!0-9]*) rm -f -- "$file" ;;
        *)
            if kill -0 "$pid" 2>/dev/null; then
                valid="$valid $file"
            else
                rm -f -- "$file"
            fi
            ;;
    esac
done
[ -n "$valid" ] || exit 0

# Filenames cannot contain spaces: both the session name and PID are sanitized.
# shellcheck disable=SC2086
jq -jrs '
    def clean:
        tostring | explode | map(
            if . < 32 or (. >= 127 and . <= 159)
                or (. >= 57344 and . <= 57363) or (. >= 65024 and . <= 65039)
                or (. >= 917760 and . <= 917999)
                or . == 57526 or . == 57524 or . == 9474 or . == 8194 then 32
            else . end
        ) | implode | gsub("#\\["; "#〔");
    def paint($color; $text): "#[bg=$panel,fg=" + $color + "]" + $text;
    def todo($details):
        if . == null then ""
        elif type == "object" then
            paint("#C6A0F6"; " " + ((.completed // 0) | clean) + "/" + ((.total // 0) | clean)) +
            (if $details and (.active // 0) > 0 and .detail then
                paint("#EED49F"; " ▶ " + (.detail | clean))
            else "" end)
        else paint("#EED49F"; " " + (. | clean))
        end;
    def instance($level):
        paint("#C6A0F6"; "π" + (if (.sessionName // .instanceName) then " [" + ((.sessionName // .instanceName) | clean) + "]" else "" end)) +
        (if .busy then paint("#EED49F"; " ●") else paint("#A6DA95"; " ○") end) +
        (if $level == "full" then
            (.todo | todo(true)) +
            (if (.subagents | length) > 0 then paint("#8AADF4"; " 󰓻 " + (.subagents | length | tostring)) else "" end) +
            (if .busy and .tool then paint("#8BD5CA"; " " + (.tool | clean)) else "" end) +
            (if .goal then paint("#C6A0F6"; " 󰘳 " + (.goal | clean)) else "" end)
        elif $level == "progress" then .todo | todo(false)
        else "" end);
    def aggregate:
        length as $total |
        (map(select(.busy)) | length) as $busy |
        ($total - $busy) as $idle |
        paint("#C6A0F6"; "π" + (if $total > 1 then ($total | tostring) else "" end)) +
        (if $busy > 0 then paint("#EED49F"; " ●" + (if $total > 1 then ($busy | tostring) else "" end)) else "" end) +
        (if $idle > 0 then paint("#A6DA95"; " ○" + (if $total > 1 then ($idle | tostring) else "" end)) else "" end);
    sort_by(.busy | not) as $items |
    [
        ($items | map(instance("full")) | join("#[bg=$panel,fg=#494D64] · ")),
        ($items | map(instance("progress")) | join("#[bg=$panel,fg=#494D64] · ")),
        ($items | map(instance("state")) | join("#[bg=$panel,fg=#494D64] · ")),
        ($items | aggregate)
    ] | .[] + "\n"
' $valid 2>/dev/null
