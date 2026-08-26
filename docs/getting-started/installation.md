# Installation

## Choose a plugin

- **`zjstatus`** renders the status bar and can also hide frames conditionally.
- **`zjframes`** is a background-only frame controller. Use it when you want frame automation without a status bar.
- Load both only when you intentionally want both responsibilities and have avoided conflicting frame controllers.

## From source

Install Rust and the `wasm32-wasip1` target, then run:

```sh
rustup target add wasm32-wasip1
./install.sh
```

`install.sh` builds the release `zjstatus` plugin and installs it plus `vcs-status.sh`, `pi-status.sh`, and `host-load.sh` under:

```text
${ZELLIJ_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/zellij}/
```

The script preserves existing layouts and uses the same resolved directory for `plugins/` and `scripts/`. It does not build `zjframes`; build that binary separately and copy `target/wasm32-wasip1/release/zjframes.wasm` into the resolved `plugins/` directory:

```sh
cargo build --release --target wasm32-wasip1 --bin zjframes
install -m 0644 target/wasm32-wasip1/release/zjframes.wasm \
  "${ZELLIJ_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/zellij}/plugins/zjframes.wasm"
```

## Nix

Use the repository's flake environment when developing or building with Nix:

```sh
nix develop
cargo build --release --target wasm32-wasip1 --bin zjstatus
```

Copy the resulting WASM file to your Zellij plugin directory, or use `install.sh` after entering the environment.

## Release artifact

Copy the release `zjstatus.wasm` or `zjframes.wasm` to your Zellij `plugins/` directory and load it from a layout with a `file:` location. Build each plugin for the WASI target supported by your Zellij release.

## Permissions and cache

Zellij may ask for `ReadApplicationState`, `ChangeApplicationState`, or `RunCommands` when the plugin first loads. Approve the capabilities required by your configuration. Keybind message delivery is handled by Zellij's plugin-message action. If a rebuilt plugin appears unchanged, restart the session or clear the cached plugin instance before diagnosing the build.

For an existing installation, follow the [upgrade procedure](../operations/upgrading.md) after replacing the artifact.
Continue with [your first status bar](first-status-bar.md), then read the [configuration reference](../reference/configuration.md).
