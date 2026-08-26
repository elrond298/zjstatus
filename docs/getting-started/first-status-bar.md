# Your first status bar

This minimal layout creates the two lines that `zjstatus` renders: a hint/idle row followed by the main status row. Reserve a two-cell plugin pane. Add one more row when enabling a top or bottom border.

```kdl
layout {
    pane split_direction="vertical" {
        pane
    }
    pane size=2 borderless=true {
        plugin location="file:~/.config/zellij/plugins/zjstatus.wasm" {
            format_left " {mode} {session} {tabs}"
            format_right "{datetime} "
            format_space " "

            mode_normal "#[bg=#89B4FA] {name} "
            mode_default_to_mode "normal"
            tab_normal " {index} {name}"
            tab_active "#[bold] {index} {name}"
            datetime "{format}"
            datetime_format "%H:%M"
        }
    }
}
```

Adjust the plugin path and add the plugin to `load_plugins` if your layout uses a named plugin load block. `zjstatus` renders the hint/idle row even when it is empty, followed by the main row; a top or bottom border adds one more row.

## Verify it

1. Start a new Zellij session with the layout.
2. Confirm the mode, session, tabs, and time render.
3. Enter another mode and check that the mode widget changes.
4. If nothing appears, check [troubleshooting](../operations/troubleshooting.md).

Next steps:

- [formatting](../reference/formatting.md)
- [widgets](../reference/widgets.md)
- [main-row compression](../guides/status-bar-compression.md)
- [contextual key hints](../guides/key-hints.md)
