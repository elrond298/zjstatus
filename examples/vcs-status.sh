#!/bin/sh

max=${VCS_STATUS_DESC_MAX:-0}
panel=\$panel
case $max in ''|*[!0-9]*) max=0 ;; esac

clean() {
    printf '%s\n' "$1" | awk -v max="$max" 'NR == 1 {
        gsub(/[[:cntrl:]]/, " "); gsub(/#\[/, "#〔"); gsub(/[│ ]/, "");
        print (max > 1 && length($0) > max ? substr($0, 1, max - 1) "…" : $0);
        exit
    }'
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

if jj root >/dev/null 2>&1; then
    bookmark=$(jj log --no-graph -r @ -T 'bookmarks' 2>/dev/null)
    description=$(jj log --no-graph -r @ -T 'description.first_line()' 2>/dev/null)
    [ -z "$bookmark" ] || paint '#A6DA95' " $(clean "$bookmark")"
    [ -z "$bookmark" ] || paint '#CAD3F5' ' '
    if [ -n "$description" ]; then
        paint '#8AADF4' "@ $(clean "$description")"
    else
        paint '#8AADF4' '@()'
        parent=$(jj log --no-graph -r @- -T 'description.first_line()' 2>/dev/null)
        paint '#CAD3F5' "  @-($(clean "$parent"))"
    fi
    if [ -n "$(jj diff --summary 2>/dev/null)" ]; then
        counts=$(jj diff --git --color never 2>/dev/null | awk '
            /^diff --git / { hunk = 0; next }
            /^@@/ { hunk = 1; next }
            hunk && /^\+/ { add++ }
            hunk && /^-/ { del++ }
            END { printf "+%d -%d", add, del }
        ')
        paint '#EED49F' "  $counts"
    fi
elif git rev-parse --show-toplevel >/dev/null 2>&1; then
    branch=$(git branch --show-current 2>/dev/null)
    commit=$(git rev-parse --short HEAD 2>/dev/null)
    description=$(git log -1 --format=%s 2>/dev/null)
    [ -z "$branch" ] || paint '#A6DA95' " $(clean "$branch") "
    [ -z "$commit" ] || paint '#8AADF4' "@ $(clean "$commit") $(clean "$description")"
    if [ -n "$(git status --porcelain --untracked-files=normal 2>/dev/null)" ]; then
        if git rev-parse --verify HEAD >/dev/null 2>&1; then
            counts=$(git diff --numstat HEAD 2>/dev/null | diff_counts)
        else
            counts=$(git diff --numstat 4b825dc642cb6eb9a060e54bf8d69288fbee4904 2>/dev/null | diff_counts)
        fi
        untracked=$(git ls-files --others --exclude-standard 2>/dev/null | awk 'END { print NR + 0 }')
        [ "$untracked" -eq 0 ] || counts="$counts ?$untracked"
        paint '#EED49F' "  $counts"
    fi
fi
