#!/bin/sh

panel=\$panel
full_max=${VCS_STATUS_DESC_MAX:-0}
short_max=${VCS_STATUS_DESC_SHORT:-32}
for value in "$full_max" "$short_max"; do
    case $value in ''|*[!0-9]*) exit 0 ;; esac
done
if [ "$full_max" -gt 0 ] && { [ "$short_max" -eq 0 ] || [ "$short_max" -gt "$full_max" ]; }; then
    short_max=$full_max
fi

clean() {
    printf '%s' "$1" | jq -Rrs --argjson max "${2:-0}" '
        (split("\n")[0] // "")
        | explode
        | map(
            if . < 32 or (. >= 127 and . <= 159) then 32
            elif (. >= 57344 and . <= 57363) or (. >= 65024 and . <= 65039)
                or (. >= 917760 and . <= 917999)
                or . == 57526 or . == 57524 or . == 9474 or . == 8194 then empty
            else . end
        )
        | implode
        | gsub("#\\["; "#〔")
        | gsub("^\\s+|\\s+$"; "")
        | if $max > 1 and length > $max then .[:$max - 1] + "…" else . end
    '
}

semantic() {
    printf '%s\n' "$1" | awk '{ word = $1; sub(/[(:!].*/, "", word); print word }'
}

paint() {
    printf "#[bg=$panel,fg=%s]%s" "$1" "$2"
}

diff_counts() {
    awk '{
        if ($1 ~ /^[0-9]+$/) add += $1
        if ($2 ~ /^[0-9]+$/) del += $2
    } END { printf "+%d -%d", add, del }'
}

kind=
bookmark=
revision=
description=
counts=

if jj root >/dev/null 2>&1; then
    kind=jj
    bookmark=$(clean "$(jj log --no-graph -r @ -T 'bookmarks' 2>/dev/null)")
    description=$(clean "$(jj log --no-graph -r @ -T 'description.first_line()' 2>/dev/null)")
    if [ -n "$description" ]; then
        revision='@ '
    else
        description=$(clean "$(jj log --no-graph -r @- -T 'description.first_line()' 2>/dev/null)")
        if [ -n "$description" ]; then revision='@- '; else revision='@()'; fi
    fi
    if [ -n "$(jj diff --summary 2>/dev/null)" ]; then
        counts=$(jj diff --git --color never 2>/dev/null | awk '
            /^diff --git / { hunk = 0; next }
            /^@@/ { hunk = 1; next }
            hunk && /^\+/ { add++ }
            hunk && /^-/ { del++ }
            END { printf "+%d -%d", add, del }
        ')
    fi
elif git rev-parse --show-toplevel >/dev/null 2>&1; then
    kind=git
    bookmark=$(clean "$(git branch --show-current 2>/dev/null)")
    revision=$(clean "$(git rev-parse --short HEAD 2>/dev/null)")
    description=$(clean "$(git log -1 --format=%s 2>/dev/null)")
    if [ -n "$(git status --porcelain --untracked-files=normal 2>/dev/null)" ]; then
        if git rev-parse --verify HEAD >/dev/null 2>&1; then
            counts=$(git diff --numstat HEAD 2>/dev/null | diff_counts)
        else
            counts=$(git diff --numstat 4b825dc642cb6eb9a060e54bf8d69288fbee4904 2>/dev/null | diff_counts)
        fi
        untracked=$(git ls-files --others --exclude-standard 2>/dev/null | awk 'END { print NR + 0 }')
        [ "$untracked" -eq 0 ] || counts="$counts ?$untracked"
    fi
else
    exit 0
fi

render() {
    mode=$1
    case $mode in
        full) shown=$(clean "$description" "$full_max") ;;
        short) shown=$(clean "$description" "$short_max") ;;
        compact|minimal) shown=$(semantic "$description") ;;
    esac

    if [ "$kind" = jj ]; then
        [ -z "$bookmark" ] || paint '#A6DA95' " $bookmark"
        [ -z "$bookmark" ] || paint '#CAD3F5' ' '
        paint '#8AADF4' "$revision"
        [ -z "$shown" ] || paint '#8AADF4' "$shown"
    else
        [ -z "$bookmark" ] || paint '#A6DA95' " $bookmark "
        if [ -n "$revision" ]; then
            paint '#8AADF4' "@ $revision"
            [ -z "$shown" ] || paint '#8AADF4' " $shown"
        fi
    fi
    if [ "$mode" != minimal ] && [ -n "$counts" ]; then
        paint '#EED49F' "  $counts"
    fi
}

render full
printf '\n'
render short
printf '\n'
render compact
printf '\n'
render minimal
printf '\n@hide'
