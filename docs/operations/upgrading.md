# Upgrading

1. Install the new WASM artifact or run `./install.sh`.
2. Start a new Zellij session, or restart the plugin after clearing an old cached instance.
3. Recheck permissions when plugin capabilities change.
4. Validate responsive configuration and command scripts at narrow widths.

## Responsive configuration migration

Replace removed keys:

| Old | New |
| --- | --- |
| `format_responsive` | `format_shrink_levels` plus named `format_<region>_<level>` values |
| `format_precedence` | `format_shrink_order` |
| numeric `format_left_1`-style levels | named levels such as `format_left_compact` |
| `hint_idle_<item>_command` | `command_<name>_command` plus the item name in `hint_idle_left/right` |
| `hint_idle_<item>_reductions` | repeated names in `hint_idle_shrink_order` |

Keep old configuration in version control until the new named stages render correctly. See [main-row compression](../guides/status-bar-compression.md) and [responsive command rows](../guides/responsive-command-row.md).
