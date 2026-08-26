# Plugin model

## Two binaries

`zjstatus` is the visible status-bar plugin. It subscribes to Zellij application state, renders rows, runs configured command widgets, and handles mouse or plugin-message interaction.

`zjframes` is a background-only plugin that changes pane-frame visibility according to application state. It does not render a status bar. Do not load it as a visible pane.

`zjstatus` can apply the same frame conditions while rendering, so load standalone `zjframes` only when that separation is useful. Avoid running two controllers that make opposite frame decisions.

## Configuration boundary

Zellij passes plugin configuration as KDL attributes. `zjstatus` parses static widget formats once, then updates state from events, timers, commands, and pipe messages. Commands run asynchronously through Zellij's background command API; a slow command keeps its last completed value instead of blocking the bar.

## Rows and state

A configured `zjstatus` pane renders:

1. a contextual key-hint row, or the idle command row when hints are not visible;
2. the main left/center/right status row;
3. an optional top or bottom border.

The plugin pane must be tall enough for both rows. The first row is intentionally empty only when neither hints nor idle formats produce output. The three responsive systems are independent; changing the main row's shrink level does not directly advance key hints or command output variants.

## Permissions

The plugin requests only the Zellij capabilities needed by its features. Commands, mouse actions, plugin messages, pane state, and frame changes may each require permission. A denied permission usually appears as missing behavior rather than a Rust error; check the Zellij permission prompt and [troubleshooting](../operations/troubleshooting.md).
