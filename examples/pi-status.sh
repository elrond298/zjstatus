#!/bin/sh

session=${ZELLIJ_SESSION_NAME-}
script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
panel="\$panel"

fallback() {
    vcs=$("$script_dir/vcs-status.sh")
    [ -z "$vcs" ] || printf "#[bg=$panel,fg=#A6DA95,bold]VCS #[bg=$panel,fg=#CAD3F5]%s" "$vcs"
    exit 0
}

[ -n "$session" ] || fallback
session=$(printf '%s' "$session" | sed 's/[^A-Za-z0-9_.-]/_/g')
dir=${XDG_RUNTIME_DIR:-/tmp}/pi-zellij-status-$(id -u)/$session
[ -d "$dir" ] || fallback

set -- "$dir"/*.json
[ -e "$1" ] || fallback

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

[ -n "$valid" ] || fallback
# Filenames cannot contain spaces: both the session name and PID are sanitized.
# shellcheck disable=SC2086
pi=$(jq -sr '
    def short: tostring | gsub("[\\n\\t]"; " ") | if length > 48 then .[:47] + "…" else . end;
    sort_by(.busy | not) |
    map(
        "#[bg=$panel,fg=#8BD5CA,bold]PI "
        + (if .sessionName then "#[bg=$panel,fg=#C6A0F6][" + (.sessionName | short) + "] " else "" end)
        + "#[bg=$panel,fg=#CAD3F5]" + (.model // "?")
        + (if .busy then " #[bg=$panel,fg=#EED49F]●" else " #[bg=$panel,fg=#A6DA95]○" end)
        + (if .busy and .tool then " " + .tool else "" end)
        + "#[bg=$panel,fg=#CAD3F5]"
        + (if .goal then "  #[bg=$panel,fg=#C6A0F6,bold]goal: #[bg=$panel,fg=#CAD3F5]" + (.goal | short) else "" end)
        + (if .todo then "  #[bg=$panel,fg=#EED49F,bold]todo: #[bg=$panel,fg=#CAD3F5]" + (.todo | short) else "" end)
        + (if (.subagents | length) > 0 then "  #[bg=$panel,fg=#8AADF4,bold]agents: #[bg=$panel,fg=#CAD3F5]" + (.subagents | length | tostring) else "" end)
    ) | join("  |  ")
' $valid 2>/dev/null) || fallback
printf '%s' "$pi"
vcs=$("$script_dir/vcs-status.sh")
[ -z "$vcs" ] || printf "#[bg=$panel,fg=#6E738D]  |  #[bg=$panel,fg=#A6DA95,bold]VCS #[bg=$panel,fg=#CAD3F5]%s" "$vcs"
