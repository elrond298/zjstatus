<h1 align="center">zjstatus & zjframes</h1>

<p align="center">
  A configurable, themable status bar and frame controller for Zellij.
</p>

<p align="center">
  <img src="./assets/demo.png" alt="Screenshot of the statusbar" />
</p>

## What is included

- **zjstatus** — a visible status bar with widgets, formatted output, contextual key hints, responsive rows, and asynchronous command integrations.
- **zjframes** — a background-only plugin that hides or shows pane frames based on Zellij state.

This fork adds three independently responsive systems: contextual key-hint compression, main status-bar compression, and a responsive idle command row. Slow status commands retain their last completed value instead of blocking Zellij or the active Pi session.

## Documentation

Start with the [documentation home](docs/README.md):

- [Installation](docs/getting-started/installation.md)
- [First status bar](docs/getting-started/first-status-bar.md)
- [Contextual key hints](docs/guides/key-hints.md)
- [Main status-bar compression](docs/guides/status-bar-compression.md)
- [Responsive command row](docs/guides/responsive-command-row.md)
- [Commands and pipes](docs/guides/commands-and-pipes.md)
- [Configuration reference](docs/reference/configuration.md)
- [Troubleshooting](docs/operations/troubleshooting.md)

## Installation

Download a release WASM artifact, place it where Zellij can read it, and load `zjstatus` from a layout. Load `zjframes` through Zellij's background `load_plugins` configuration; it is not a visible pane.

To build this repository and install the plugin plus bundled scripts:

```sh
rustup target add wasm32-wasip1
./install.sh
```

The installer resolves its destination as `$ZELLIJ_CONFIG_DIR`, then `$XDG_CONFIG_HOME/zellij`, then `~/.config/zellij`. It installs the plugin under `plugins/` and VCS, Pi, and host-load scripts under `scripts/`. See the [installation guide](docs/getting-started/installation.md).

## Minimal layout

After `./install.sh`, adapt the plugin path if you use a non-default config directory:

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

`zjstatus` renders a hint/idle row followed by the main status row, even when the first row is empty. Reserve two rows for the plugin; add one more row for a top or bottom border.

## Examples

The repository includes example layouts under [`examples/`](examples/), including compact, tmux-style, slanted, conky, and swap-layout configurations. Bundled VCS, Pi, and Linux host metrics are documented in [Status script examples](docs/guides/status-scripts.md).

## Development

See [building and testing](docs/development/building-and-testing.md), [architecture](docs/development/architecture.md), and [contributing](docs/development/contributing.md).

## Community and license

- [Community showcase](https://github.com/dj95/zjstatus/discussions/44)
- [Issue tracker](https://github.com/dj95/zjstatus/issues)
- [MIT License](LICENSE)
