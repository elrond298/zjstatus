#!/bin/sh

session=${ZELLIJ_SESSION_NAME-}
script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
panel=\$panel
vcs_start=$(printf '\357\270\202')
vcs_end=$(printf '\357\270\203')

vcs() {
    output=$("$script_dir/vcs-status.sh")
    [ -z "$output" ] || printf '%s%s%s' "$vcs_start" "$output" "$vcs_end"
    exit 0
}

[ -n "$session" ] || vcs
session=session-$(printf '%s' "$session" | sed 's/[^A-Za-z0-9_.-]/_/g')
dir=${XDG_RUNTIME_DIR:-/tmp}/pi-zellij-status-$(id -u)/$session
[ -d "$dir" ] || vcs

set -- "$dir"/*.json
[ -e "$1" ] || vcs

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

[ -n "$valid" ] || vcs
# Filenames cannot contain spaces: both the session name and PID are sanitized.
# shellcheck disable=SC2086
pi=$(jq -sr '
    def clean:
        tostring | explode | map(
            if . < 32 or (. >= 127 and . <= 159)
                or (. >= 57344 and . <= 57363) or (. >= 65024 and . <= 65039)
                or (. >= 917760 and . <= 917999)
                or . == 57526 or . == 57524 or . == 9474 or . == 8194 then 32
            else . end
        ) | implode | gsub("#\\["; "#〔");
    def paint($color; $text): "#[bg=$panel,fg=" + $color + "]" + $text;
    def optional_detail($text): "\uFE00" + $text + "\uFE01";
    def optional_progress($text): "\uFE06" + $text + "\uFE07";
    def todo:
        if . == null then ""
        elif type == "object" then
            optional_progress(
                paint("#C6A0F6"; " " + ((.completed // 0) | clean) + "/" + ((.total // 0) | clean)) +
                (if (.active // 0) > 0 and .detail then
                    optional_detail(paint("#EED49F"; " ▶ " + (.detail | clean)))
                else "" end)
            )
        else optional_progress(paint("#EED49F"; " " + (. | clean)))
        end;
    def instance:
        paint("#C6A0F6"; "π" + (if (.sessionName // .instanceName) then " [" + ((.sessionName // .instanceName) | clean) + "]" else "" end)) +
        (if .busy then paint("#EED49F"; " ●") else paint("#A6DA95"; " ○") end) +
        (.todo | todo) +
        (if (.subagents | length) > 0 then optional_detail(paint("#8AADF4"; " 󰓻 " + (.subagents | length | tostring))) else "" end) +
        (if .busy and .tool then optional_detail(paint("#8BD5CA"; " " + (.tool | clean))) else "" end) +
        (if .goal then optional_detail(paint("#C6A0F6"; " 󰘳 " + (.goal | clean))) else "" end);
    def aggregate:
        length as $total |
        (map(select(.busy)) | length) as $busy |
        ($total - $busy) as $idle |
        paint("#C6A0F6"; "π" + (if $total > 1 then ($total | tostring) else "" end)) +
        (if $busy > 0 then paint("#EED49F"; " ●" + (if $total > 1 then ($busy | tostring) else "" end)) else "" end) +
        (if $idle > 0 then paint("#A6DA95"; " ○" + (if $total > 1 then ($idle | tostring) else "" end)) else "" end);
    sort_by(.busy | not) |
    . as $items |
    "\uFE08" + ($items | map(instance) | join("#[bg=$panel,fg=#494D64] · ")) + "\uFE09" +
    "\uFE0A" + ($items | aggregate) + "\uFE0B"
' $valid 2>/dev/null) || vcs
vcs_output=$("$script_dir/vcs-status.sh")
[ -z "$vcs_output" ] || printf '%s%s#[bg=%s]  %s' "$vcs_start" "$vcs_output" "$panel" "$vcs_end"
printf '%s' "$pi"
