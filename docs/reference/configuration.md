# Configuration reference

Configuration is supplied as KDL plugin attributes. Names written as `<name>` are dynamic widget names. Unless noted otherwise, an omitted format is empty and an omitted boolean is `false`.

## Main layout

| Key | Default | Meaning |
| --- | --- | --- |
| `format_left` | empty | Left main-row format. |
| `format_center` | empty | Center main-row format. |
| `format_right` | empty | Right main-row format. |
| `format_space` | empty | Spacer between populated regions. |
| `format_hide_on_overlength` | `false` | In non-responsive mode, trim regions when the bar is too wide. |
| `format_shrink_levels` | unset | Space-separated semantic levels after the base level. |
| `format_shrink_order` | `right center left` | Region order for responsive rounds; when enabled it must contain each region exactly once. |

For each level `compact`, define `format_left_compact`, `format_center_compact`, and `format_right_compact` as needed. A missing region format inherits the preceding level; an explicitly empty format hides that region. Responsive output measures terminal display width and advances regions in synchronized rounds.

## Frame and border settings

| Key | Default | Values / meaning |
| --- | --- | --- |
| `hide_frame_for_single_pane` | `false` | Hide frames when the tab has one pane. |
| `hide_frame_except_for_search` | `false` | Keep frames visible in search/rename mode when frame hiding is active. |
| `hide_frame_except_for_fullscreen` | `false` | Keep frames visible for a focused fullscreen pane. |
| `hide_frame_except_for_scroll` | `false` | Keep frames visible in scroll mode. |
| `border_enabled` | `false` | Draw a border line. |
| `border_char` | `─` | Character repeated across the terminal width. |
| `border_format` | empty | Format applied to the border. |
| `border_position` | `top` | `top` or `bottom`; any other value falls back to `top`. |

A border consumes one additional terminal row.

## Contextual hints and idle row

| Key | Default | Meaning |
| --- | --- | --- |
| `hint_mode_format` | blue/default/bold | Mode header format. |
| `hint_key_format` | yellow/default/bold | Key label format. |
| `hint_desc_format` | white/default | Description format. |
| `hint_space_format` | default background | Hint spacing format. |
| `hint_idle_left` | unset | Space-separated command names on the idle row's left side. |
| `hint_idle_right` | unset | Space-separated command names on the idle row's right side. |
| `hint_idle_separator` | empty | Separator between idle-row sides. |
| `hint_idle_shrink_order` | unset | Repeated command names that advance newline-delimited variants. |
| `hint_idle_format` | empty | Legacy/non-responsive left idle format. |
| `hint_idle_right_format` | empty | Legacy/non-responsive right idle format. |

See [key hints](../guides/key-hints.md) and [responsive command rows](../guides/responsive-command-row.md) for lifecycle and width rules.

## Widget prefixes

### Mode

- `mode_<mode>`: format for a Zellij input mode, including `mode_normal`, `mode_resize`, `mode_locked`, and other supported modes.
- `mode_default_to_mode`: fallback mode name when a specific format is absent.

The `{name}` placeholder is the human-readable mode name.

### Session

`session` is the format containing `{name}` for the current Zellij session. The `{session}` placeholder in another format expands to the session widget.

### Tabs

| Key | Default / meaning |
| --- | --- |
| `tab_normal` | Normal tab format. |
| `tab_active` | Active tab format; defaults to the normal format. |
| `tab_normal_fullscreen`, `tab_active_fullscreen` | Fullscreen variants; fall back to their normal variants. |
| `tab_normal_sync`, `tab_active_sync` | Synchronized-pane variants; fall back to normal variants. |
| `tab_normal_bell`, `tab_normal_flashing_bell` | Optional bell variants. |
| `tab_rename` | Rename-mode active format; falls back to active. |
| `tab_separator` | Optional separator between tabs. |
| `tab_display_count` | Optional maximum tab window size. |
| `tab_truncate_start_format`, `tab_truncate_end_format` | Formats for hidden tabs; support `{count}`. |
| `tab_zero_based_index` | `false`; use zero-based tab indexes. |
| `tab_locator_format`, `tab_locator_compact_format` | Active-tab position fallback; support `{left_arrow}`, `{index}`, `{right_arrow}`. |
| `tab_bell_indicator`, `tab_flashing_bell_indicator` | Bell indicator text. |
| `tab_sync_indicator`, `tab_fullscreen_indicator`, `tab_floating_indicator` | State indicator text. |

`{focused_pane_title}` is available in tab formats and resolves to the focused pane title for that tab, or empty when none is available.
Responsive levels automatically move from the configured tab window toward the active tab and its locator.


## Placeholder summary

| Widget / context | Placeholders |
| --- | --- |
| mode | `{mode}`, `{name}` |
| session | `{session}` |
| tabs | `{tabs}`, `{name}`, `{index}`, `{count}`, `{left_arrow}`, `{right_arrow}`, `{focused_pane_title}` |
| datetime | `{datetime}`, `{format}`, `{date}`, `{time}` |
| notifications | `{notifications}`, `{message}` |
| command | `{stdout}`, `{stderr}`, `{exit_code}`, `{focused_pane_cwd}` |
| pipe | `{output}` |
| swap_layout | `{swap_layout}`, `{name}` |
### Datetime

| Key | Default |
| --- | --- |
| `datetime` | empty format containing `{format}`, `{date}`, or `{time}` |
| `datetime_format` | `%H:%M` |
| `datetime_date_format` | `%Y-%m-%d` |
| `datetime_time_format` | `%H:%M` |
| `datetime_timezone` | `Etc/UTC` |

Invalid timezone names fall back to UTC. Formats use Chrono syntax.

### Notifications

| Key | Default | Meaning |
| --- | --- | --- |
| `notification_format_unread` | empty | Format for an incoming message; `{message}` is available. |
| `notification_format_no_notifications` | empty | Empty-state format. |
| `notification_show_interval` | `5` seconds | How long an incoming message remains visible. |

Notifications are retained during responsive main-row reduction when the widget is configured in a region.

### Commands

For `command_<name>_*`:

| Suffix | Default | Meaning |
| --- | --- | --- |
| `command` | empty | Process command. Use `sh -c` for shell syntax. |
| `format` | empty | Format containing `{stdout}`, `{stderr}`, `{exit_code}`. |
| `interval` | `1` second | Poll interval; `0` runs once after the first result. |
| `cwd` | process default | Working directory; `{focused_pane_cwd}` follows the focused pane. |
| `env` | unset | KDL child entries containing one string each become environment variables. |
| `rendermode` | `static` | `static`, `dynamic`, or `raw`. |
| `clickaction` | empty | Command executed when the rendered command region is clicked. |
| `hideonemptystdout` | `false` | Hide the command when stdout is empty. |

Commands are asynchronous and never overlap for one widget. A hung process can leave a stale last result; wrap it with an OS `timeout` when needed.

### Pipes

For `pipe_<name>_*`:

| Suffix | Default | Meaning |
| --- | --- | --- |
| `format` | empty | Format containing `{output}`. |
| `rendermode` | `static` | `static`, `dynamic`, or `raw`. |

Pipe widgets are updated through the [plugin message protocol](integration-protocols.md) and are not clickable.

### Swap layout


`swap_layout_format` formats the active swap-layout name with `{name}`. `swap_layout_hide_if_empty` (`false`) hides it when no name is active. The widget is clickable and advances to the next swap layout.
`swap_layout` renders the current swap-layout state and advances to the next layout when clicked.

## Removed keys

The parser rejects these old forms:

| Removed | Replacement |
| --- | --- |
| `format_responsive` | `format_shrink_levels` and named region formats |
| `format_precedence` | `format_shrink_order` |
| numeric `format_left_1`-style levels | semantic level names |
| `hint_idle_<item>_command` | `command_<name>_command` plus `hint_idle_left/right` |
| `hint_idle_<item>_reductions` | repeated names in `hint_idle_shrink_order` |

## Permissions

| Plugin | Requests |
| --- | --- |
| `zjstatus` | `ReadApplicationState`, `ChangeApplicationState`, `RunCommands` |
| `zjframes` | `ReadApplicationState`, `ChangeApplicationState` |
Zellij remains authoritative for permission prompts and may add requirements in newer releases. Keybind-driven `MessagePluginId` delivery is handled by Zellij's plugin-message action.
