# Status-bar anatomy

## Rows

- **Hint/idle row:** shows contextual bindings when hints are visible; otherwise it can show external commands such as VCS, Pi, and host metrics. It is the first line rendered by `zjstatus`.
- **Main status row:** composes configured left, center, and right regions and is rendered below the hint/idle row.
- **Border:** optional top or bottom decoration, counted as an additional terminal row.

## Main regions

`format_left`, `format_center`, and `format_right` contain formatted text and widget placeholders. `format_space` fills the gap between regions when the bar has room. If no center format is configured, the left and right regions are joined with the configured spacer.

## Widgets

Built-in widgets read Zellij state (`mode`, `session`, and `tabs`), time in `Etc/UTC` by default (`datetime`), layout state (`swap_layout`), notifications, or external data (`command_*` and `pipe_*`). See the [widget reference](../reference/widgets.md).

## Interaction

Tabs and configured command or swap-layout regions can be clickable. Hit testing uses terminal cell positions. Responsive fallback output may intentionally remove or disable ambiguous click targets; do not assume every visible character is interactive.

## Three responsive systems

- [Key hints](../guides/key-hints.md) compress headers and paginate complete key-description pairs.
- [Main-row compression](../guides/status-bar-compression.md) advances named semantic levels for left, center, and right regions in synchronized rounds.
- [Responsive command rows](../guides/responsive-command-row.md) advance newline-delimited command variants according to repeated command names.
