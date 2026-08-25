#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
runtime=$(mktemp -d)
trap 'rm -rf "$runtime"' EXIT HUP INT TERM
session=goal-fixture
dir="$runtime/pi-zellij-status-$(id -u)/session-$session"
mkdir -p "$dir"

jq -n --argjson pid "$$" \
    --arg goal '重构自适应状态栏，确保长目标先截断再隐藏稳定区块，并完整保留中文内容' \
    --arg detail '完善👨‍👩‍👧‍👦任务详情截断并保留组合字符é，避免破坏终端显示' \
    '{version: 2, pid: $pid, busy: true, instanceName: "debug", tool: "replace", subagents: [], goal: $goal, todo: {completed: 2, total: 3, active: 1, detail: $detail}}' \
    >"$dir/$$.json"

XDG_RUNTIME_DIR="$runtime" ZELLIJ_SESSION_NAME="$session" panel='#24273A' \
    "$root/examples/pi-status.sh" >"$runtime/output"

python3 - "$runtime/output" <<'PY'
import re
import sys
import unicodedata

lines = open(sys.argv[1], encoding="utf-8").read().splitlines()
plain = [re.sub(r"#\[[^]]*\]", "", line) for line in lines]
family = "👨‍👩‍👧‍👦"

def cells(text):
    text = text.replace(family, "🙂")
    return sum(
        0 if unicodedata.combining(char) else 2 if unicodedata.east_asian_width(char) in "WF" else 1
        for char in text
    )

assert len(plain) == 8, plain
assert all("󰘳" in line and "▶" in line for line in plain[:5]), plain
for line, limit in zip(plain[1:5], (64, 40, 24, 12), strict=True):
    goal = line.split("󰘳 ", 1)[1]
    detail = line.split("▶ ", 1)[1].split(" replace ", 1)[0]
    assert cells(goal) <= limit and cells(detail) <= limit, (limit, goal, detail)
for line in plain[:5]:
    detail = line.split("▶ ", 1)[1].split(" replace ", 1)[0]
    residue = detail.replace(family, "").replace("é", "")
    assert not any(char in residue for char in "👨👩👧👦‍e"), detail
assert "…" in plain[1] and "…" in plain[4], plain
assert "2/3" in plain[5] and "󰘳" not in plain[5] and "▶" not in plain[5], plain
assert "2/3" not in plain[6], plain
print("pi status progressive truncation: ok")
PY
