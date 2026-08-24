#!/bin/sh

max=${VCS_STATUS_DESC_MAX:-60}
case $max in ''|*[!0-9]*) max=60 ;; esac
test "$max" -ge 2 || max=60

shorten() {
    printf '%s\n' "$1" | awk -v max="$max" 'NR == 1 { print (length($0) > max ? substr($0, 1, max - 1) "…" : $0); exit }'
}

if command -v jj >/dev/null 2>&1 && jj root >/dev/null 2>&1; then
    dirty=
    test -z "$(jj diff --summary 2>/dev/null)" || dirty=' ±'
    meta=$(jj log --no-graph -r @ -T 'if(bookmarks, bookmarks ++ "  ", "") ++ change_id.shortest(8)' 2>/dev/null)
    current=$(jj log --no-graph -r @ -T 'description.first_line()' 2>/dev/null)
    printf 'jj  %s  @ %s' "$meta" "$(shorten "$current")"
    if test -z "$current"; then
        parent=$(jj log --no-graph -r @- -T 'description.first_line()' 2>/dev/null)
        printf '  @- %s' "$(shorten "$parent")"
    fi
    printf '%s' "$dirty"
elif git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    branch=$(git branch --show-current 2>/dev/null)
    test -n "$branch" || branch=$(git rev-parse --short HEAD 2>/dev/null)
    dirty=
    test -z "$(git status --porcelain 2>/dev/null)" || dirty=' ±'
    description=$(git log -1 --format=%s 2>/dev/null)
    printf 'git  %s  %s  %s%s' "$branch" "$(git rev-parse --short HEAD 2>/dev/null)" "$(shorten "$description")" "$dirty"
fi
