#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd)
repo=$(mktemp -d)
trap 'rm -rf "$repo"' EXIT HUP INT TERM

jj git init "$repo" >/dev/null
cd "$repo"
printf 'old\n' >tracked
jj --config 'debug.commit-timestamp="2030-01-01T00:00:00Z"' commit -m 'old bookmark' >/dev/null
jj bookmark create old -r @- >/dev/null
printf 'nearest\n' >>tracked
jj --config 'debug.commit-timestamp="2020-01-01T00:00:00Z"' commit -m 'nearest bookmark' >/dev/null
jj bookmark create main -r @- >/dev/null

test -z "$(jj log --no-graph -r @ -T bookmarks)"
output=$("$root/examples/vcs-status.sh")
[ "$(printf '%s\n' "$output" | grep -Fc ' main')" -eq 4 ]
if printf '%s\n' "$output" | grep -Fq ' old'; then
    exit 1
fi
printf 'jj bookmark status: ok\n'
