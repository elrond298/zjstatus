#!/bin/sh

session=${ZELLIJ_SESSION_NAME-}
script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
panel=\$panel

vcs() {
    "$script_dir/vcs-status.sh"
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
    def short:
        tostring | gsub("[\u0000-\u001F\u007F-\u009F│ ]"; " ") | gsub("#\\["; "#〔") |
        if length > 24 then .[:23] + "…" else . end;
    def paint($color; $text): "#[bg=$panel,fg=" + $color + "]" + $text;
    sort_by(.busy | not) |
    map(
        paint("#C6A0F6"; "π" + (if .sessionName then " [" + (.sessionName | short) + "]" else "" end)) +
        (if .busy then paint("#EED49F"; " ●") else paint("#A6DA95"; " ○") end) +
        (if .todo then paint("#EED49F"; " 󰄬 " + (.todo | short)) else "" end) +
        (if (.subagents | length) > 0 then paint("#8AADF4"; " 󰓻 " + (.subagents | length | tostring)) else "" end) +
        (if .busy and .tool then paint("#8BD5CA"; " " + (.tool | short)) else "" end) +
        (if .goal then paint("#C6A0F6"; " 󰘳 " + (.goal | short)) else "" end)
    ) | join("#[bg=$panel,fg=#494D64] · ")
' $valid 2>/dev/null) || vcs
printf '%s' "$pi"
vcs_output=$("$script_dir/vcs-status.sh")
[ -z "$vcs_output" ] || printf "#[bg=$panel]  %s" "$vcs_output"
