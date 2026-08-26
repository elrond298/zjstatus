# Status script examples

The scripts in [`examples/`](../../examples/) turn VCS, Pi, and Linux host state into raw zjstatus command-widget output. Each script emits several display variants, from most detailed to smallest or hidden, so the responsive command row can keep useful information as the terminal narrows. See the [responsive command-row guide](responsive-command-row.md) for the selection protocol.

## Quick setup

Run the repository installer:

```sh
./install.sh
```

It builds zjstatus and installs these scripts under `${ZELLIJ_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/zellij}/scripts/`:

- `vcs-status.sh`
- `pi-status.sh`
- `host-load.sh`

The configuration below assumes the default `$HOME/.config/zellij` location. If the installer prints a different directory, replace that prefix with the printed path.

Configure them as raw command widgets:

```kdl
hint_idle_left         "vcs pi"
hint_idle_right        "load"
hint_idle_separator    "#[bg=$panel]  "
hint_idle_shrink_order "vcs pi pi pi pi load load load vcs vcs vcs pi pi pi"

command_vcs_command    "sh -c '$HOME/.config/zellij/scripts/vcs-status.sh'"
command_vcs_cwd        "{focused_pane_cwd}"
command_vcs_rendermode "raw"
command_vcs_interval   "2"
command_pi_command     "sh -c '$HOME/.config/zellij/scripts/pi-status.sh'"
command_pi_rendermode  "raw"
command_pi_interval    "2"
command_load_command   "sh -c '$HOME/.config/zellij/scripts/host-load.sh'"
command_load_rendermode "raw"
command_load_interval  "2"
```

The examples use Nerd Font glyphs. Install a Nerd Font in the terminal if icons appear as empty boxes.

## How responsive output works

A script prints one variant per line, widest first. Each occurrence of its name in `hint_idle_shrink_order` advances it by one line. A line containing exactly `@hide` removes the command from the row.

For the configuration above, zjstatus:

1. shortens VCS once;
2. progressively shortens Pi detail;
3. removes Load detail and then VCS detail;
4. reduces Pi to progress, state, and finally aggregate counts.

Blank or missing lines inherit the previous valid variant. zjstatus also rejects a later variant if it is wider than the preceding one.

## VCS status

[`vcs-status.sh`](../../examples/vcs-status.sh) detects Jujutsu first, then Git, in the focused pane's working directory. Outside a repository it emits nothing.

### Display

| Text | Meaning |
| --- | --- |
| ` main` | nearest Jujutsu bookmark or current Git branch |
| `@ description` | Jujutsu working-copy commit with a description |
| `@- description` | described Jujutsu parent when the working-copy description is empty |
| `@()` | neither the Jujutsu working copy nor its parent has a description |
| `@ abc1234 description` | Git revision and latest commit subject |
| `+A -D` | added and deleted lines in the working copy |
| `?N` | untracked Git files |

For Jujutsu, the bookmark is the graph-nearest local bookmark in `heads(::@ & bookmarks())`; the working-copy commit itself is often an unbookmarked child.

### Variants

1. Full description and change counts.
2. Short description and change counts.
3. First semantic word of the description and change counts.
4. First semantic word without change counts.
5. Hidden.

### Configuration and requirements

| Variable | Default | Meaning |
| --- | ---: | --- |
| `VCS_STATUS_DESC_MAX` | `0` | Maximum full-description characters; `0` or `1` means unlimited. |
| `VCS_STATUS_DESC_SHORT` | `32` | Maximum short-description characters; `1` means unlimited, while `0` adopts a positive full limit. |

Both values must be non-negative integers; only limits of `2` or more truncate text. When `VCS_STATUS_DESC_MAX` is positive, a zero short limit or one larger than the full limit is capped to the full limit.

The script requires `jq`, `awk`, and either `jj` or `git`. It strips control and formatting characters from repository text before emitting raw zjstatus markup.

## Pi session status

[`pi-status.sh`](../../examples/pi-status.sh) reads JSON status files produced by `zellij-pi-dashboard/extensions/zellij-status.ts`. It does not start or query Pi directly.

### Enable the producer

Clone [`zellij-pi-dashboard`](https://github.com/elrond298/zellij-pi-dashboard), link its extension into Pi, then restart Pi:

```sh
git clone https://github.com/elrond298/zellij-pi-dashboard.git \
  "$HOME/.local/share/zellij-pi-dashboard"
mkdir -p "$HOME/.pi/agent/extensions"
ln -sfn "$HOME/.local/share/zellij-pi-dashboard/extensions/zellij-status.ts" \
  "$HOME/.pi/agent/extensions/zellij-status.ts"
```

The producer writes one file per Pi process under:

```text
${XDG_RUNTIME_DIR:-/tmp}/pi-zellij-status-<uid>/session-<zellij-session>/
```

The reader only shows processes from the current `ZELLIJ_SESSION_NAME`. Invalid files and files whose process has exited are removed.

### Symbol legend

| Display | Meaning |
| --- | --- |
| `π [name]` | Pi session and its generated or instance name |
| `[PLAN]` | Pi is in plan mode |
| `●` | busy |
| `○` | idle |
| `2/3` | completed/total Todos in the current task batch |
| `▶ detail` | active Todo detail |
| `󰓻 2` | active subagent count |
| `replace` | currently running tool |
| `󰘳 goal` | active Goal summary |
| `·` | separator between Pi processes |

Busy processes are listed before idle processes. Goal and Todo text is cleaned and shortened by terminal display width without splitting grapheme clusters such as emoji, combining characters, or CJK text.

### Variants

1. Full detail without a width limit.
2. Goal and active-Todo detail limited to 64 terminal cells each.
3. Detail limited to 40 cells.
4. Detail limited to 24 cells.
5. Detail limited to 12 cells.
6. Session name, mode, state, and Todo progress only.
7. Session name, mode, and state only.
8. Aggregate Pi, busy, and idle counts.

The producer asynchronously summarizes goals wider than 48 terminal cells. Until a summary is available, the reader still applies the variant's width limit to the original goal. Aggregate output intentionally omits individual names and plan-mode badges.

### Requirements

The script requires `jq`, a running Zellij session, and the Pi status extension above. It emits nothing when no live Pi process belongs to the current Zellij session.

## Host load

[`host-load.sh`](../../examples/host-load.sh) reports Linux disk throughput, network throughput, and the one-minute load average.

### Symbol legend

| Display | Meaning |
| --- | --- |
| `󰋊` | combined selected-block-device read/write rate |
| `󰖩` | combined non-loopback network receive/transmit rate |
| `` | one-minute system load average |
| Braille cells | recent one-minute peak history |

Rates are calculated from kernel counters between invocations. Disk totals exclude loop, RAM, zram, device-mapper, and software-RAID devices. A counter reset clears the affected history; a sampling gap longer than 120 seconds reports a zero rate for that invocation.

History contains ten one-minute peak buckets rendered as five Braille cells. Missing minutes are represented as zero; ten minutes without a sample resets history. State is stored per Zellij session under `${XDG_RUNTIME_DIR:-/tmp}/zjstatus-metrics-<uid>/` with owner-only directory permissions.

### Variants

1. Disk, network, and load values with history.
2. Disk, network, and load values without history.
3. Load value only.
4. Hidden.

### Configuration and requirements

| Variable | Default | Meaning |
| --- | ---: | --- |
| `ZJSTATUS_LOAD_SCALE` | `1` | Load-average value that fills the graph. |
| `ZJSTATUS_NET_SCALE_BPS` | `125000000` | Bytes per second that fill the network graph. |
| `ZJSTATUS_DISK_SCALE_BPS` | `1000000000` | Bytes per second that fill the disk graph. |

Scale values must be positive integers. Rates use binary units (`K`, `M`, `G`, `T`) and include `/s` in the display.

This script is Linux-specific: it reads `/proc/loadavg`, `/proc/net/dev`, and `/sys/block/*/stat`. It requires a POSIX shell and `awk`; unlike the VCS and Pi scripts, it does not require `jq`.

## Customizing the examples

These scripts are examples, not special built-ins. You can replace them with any command that follows the same line-per-variant protocol. Keep `rendermode` set to `raw` only when the command intentionally emits trusted zjstatus formatting directives such as `#[fg=...]`; otherwise use the normal command rendering mode.
