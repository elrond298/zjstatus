#!/bin/sh

max=${VCS_STATUS_DESC_MAX:-0}
panel=\$panel
desc_start=$(printf '\357\270\204')
desc_end=$(printf '\357\270\205')
changes_start=$(printf '\363\240\204\200')
changes_end=$(printf '\363\240\204\201')
case $max in ''|*[!0-9]*) max=0 ;; esac

clean() {
    printf '%s' "$1" | jq -Rrs --argjson max "$max" '
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

paint() {
    printf "#[bg=$panel,fg=%s]%s" "$1" "$2"
}

paint_changes() {
    printf '%s' "$changes_start"
    paint '#EED49F' "  $1"
    printf '%s' "$changes_end"
}

diff_counts() {
    awk '{
        if ($1 ~ /^[0-9]+$/) add += $1
        if ($2 ~ /^[0-9]+$/) del += $2
    } END { printf "+%d -%d", add, del }'
}

if jj root >/dev/null 2>&1; then
    bookmark=$(clean "$(jj log --no-graph -r @ -T 'bookmarks' 2>/dev/null)")
    description=$(clean "$(jj log --no-graph -r @ -T 'description.first_line()' 2>/dev/null)")
    [ -z "$bookmark" ] || paint '#A6DA95' " $bookmark"
    [ -z "$bookmark" ] || paint '#CAD3F5' ' '
    if [ -n "$description" ]; then
        paint '#8AADF4' '@ '
        paint '#8AADF4' "$desc_start$description$desc_end"
    else
        parent=$(clean "$(jj log --no-graph -r @- -T 'description.first_line()' 2>/dev/null)")
        if [ -n "$parent" ]; then
            paint '#8AADF4' '@- '
            paint '#8AADF4' "$desc_start$parent$desc_end"
        else
            paint '#8AADF4' '@()'
        fi
    fi
    if [ -n "$(jj diff --summary 2>/dev/null)" ]; then
        counts=$(jj diff --git --color never 2>/dev/null | awk '
            /^diff --git / { hunk = 0; next }
            /^@@/ { hunk = 1; next }
            hunk && /^\+/ { add++ }
            hunk && /^-/ { del++ }
            END { printf "+%d -%d", add, del }
        ')
        paint_changes "$counts"
    fi
elif git rev-parse --show-toplevel >/dev/null 2>&1; then
    branch=$(clean "$(git branch --show-current 2>/dev/null)")
    commit=$(clean "$(git rev-parse --short HEAD 2>/dev/null)")
    description=$(clean "$(git log -1 --format=%s 2>/dev/null)")
    [ -z "$branch" ] || paint '#A6DA95' " $branch "
    if [ -n "$commit" ]; then
        paint '#8AADF4' "@ $commit"
        [ -z "$description" ] || paint '#8AADF4' " $desc_start$description$desc_end"
    fi
    if [ -n "$(git status --porcelain --untracked-files=normal 2>/dev/null)" ]; then
        if git rev-parse --verify HEAD >/dev/null 2>&1; then
            counts=$(git diff --numstat HEAD 2>/dev/null | diff_counts)
        else
            counts=$(git diff --numstat 4b825dc642cb6eb9a060e54bf8d69288fbee4904 2>/dev/null | diff_counts)
        fi
        untracked=$(git ls-files --others --exclude-standard 2>/dev/null | awk 'END { print NR + 0 }')
        [ "$untracked" -eq 0 ] || counts="$counts ?$untracked"
        paint_changes "$counts"
    fi
fi
