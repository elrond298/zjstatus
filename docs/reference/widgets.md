# Widget reference

## Mode

`{mode}` selects a mode-specific format such as `mode_normal`, `mode_resize`, or `mode_locked`. `mode_default_to_mode` supplies a fallback when a mode-specific key is absent. `{name}` is the human-readable mode name.

## Session

`{session}` renders the current Zellij session name.

## Tabs

`{tabs}` renders the tab list. `tab_normal` and `tab_active` control tab text. Indicators include fullscreen, sync, floating, bell, and rename states where configured. Tab display-count and truncation formats control how the visible window narrows. Tab regions can be clickable when their positions remain unambiguous.

## Datetime

`{datetime}` uses `datetime_format` (`%H:%M`) by default. Configure `datetime_date_format` (`%Y-%m-%d`), `datetime_time_format` (`%H:%M`), and `datetime_timezone`; the timezone defaults to `Etc/UTC` and invalid timezone names fall back to UTC. Date/time format strings are interpreted by Chrono.

## Notifications

`{notifications}` selects unread and empty formats. Notifications receive priority in narrow main-row fallback so an active message is not silently discarded too early.

## Swap layout

`{swap_layout}` displays the current swap-layout state. A click action can request the next swap layout when configured.

## Commands

A command widget uses `command_<name>_command`, with optional `format`, `interval`, `cwd`, `env`, `rendermode`, and `clickaction`. Result placeholders include `{stdout}`, `{stderr}`, and `{exit_code}`. See [commands and pipes](../guides/commands-and-pipes.md).

## Pipes

A pipe widget renders a named pushed value. Its output placeholder is `{output}` and it supports the same static, dynamic, and raw rendering distinction.

## Borders

Border settings control whether a top or bottom border is drawn, its character, and its format. Borders are outside the main status regions and consume a row.
