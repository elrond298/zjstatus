# Architecture

## Repository map

- `src/bin/zjstatus.rs`: visible plugin lifecycle, events, timers, hints, idle row, and interaction.
- `src/bin/zjframes.rs`: standalone frame controller.
- `src/config.rs`: KDL parsing, main-row composition, responsive selection, and validation.
- `src/render.rs`: formatted-part parsing and width-aware rendering.
- `src/widgets/`: mode, tabs, datetime, notifications, commands, pipes, session, and swap layout.
- `src/frames.rs`: shared frame conditions.
- `examples/`: bundled command scripts and layouts.

## Render pipeline

Configuration becomes cached formatted parts. Zellij events update application state; timers refresh hints and command widgets; the renderer composes hint, idle, main, and border rows. Main-row compression, command variants, and hint pagination are separate state machines that share terminal-width constraints.

Commands are submitted asynchronously with one in-flight run per widget. Run identifiers prevent an older result from replacing a newer one. Pipe messages invalidate or update state without spawning a command.

## Frames

The shared frame engine evaluates single-pane, search, fullscreen, and scroll conditions. `zjstatus` can call it during normal rendering; `zjframes` runs the same kind of decision loop without a visible bar.

When changing behavior, update the relevant tests and the corresponding guide/reference page. Keep producer integrations at their boundary: consume their existing output schema instead of duplicating producer logic.
