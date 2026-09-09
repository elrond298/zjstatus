# Building and testing

## Toolchain

Use the repository Rust toolchain and install the active WASI target:

```sh
rustup target add wasm32-wasip1
cargo build --target wasm32-wasip1 --bin zjstatus
cargo build --target wasm32-wasip1 --bin zjframes
```

For release plugins:

```sh
cargo build --release --target wasm32-wasip1 --bin zjstatus
cargo build --release --target wasm32-wasip1 --bin zjframes
```

`./install.sh` builds and installs the standard release `zjstatus` artifact and bundled scripts.

## Tests

The repository's `.cargo/config.toml` sets `wasm32-wasip1` as the default target and runs test binaries through Wasmtime with the target directory and `/tmp` mounted:

```sh
cargo test
```

The configured runner is equivalent to:

```sh
wasmtime --dir './target::target' --dir /tmp target/wasm32-wasip1/debug/deps/<test-binary>.wasm
```

Use the native test suite when Wasmtime is unavailable:

```sh
cargo test --target x86_64-unknown-linux-gnu
```

Run the shell checks individually:

```sh
tests/pi-status.sh
tests/vcs-status.sh
```

The fixture layouts exercise the plugins interactively:

```sh
zellij -s zjstatus-dev --config ./tests/zjstatus/config.kdl -n ./tests/zjstatus/layout.kdl
zellij -s zjframes-dev --config ./tests/zjframes/config.kdl -n ./tests/zjframes/layout.kdl
```

## Reloading in a live session

`zellij action start-or-reload-plugin` re-instantiates an existing plugin in place, but Zellij matches a running instance by resolved location **and exact configuration equality**. A reload that omits the configuration, or differs from the layout's plugin block by a single key or value, does not match: Zellij treats the plugin as absent and *starts a new one* — a full tiled pane in the active tab. An instance without configuration renders this repository's own `No configuration found` error and the original status bar loses its asynchronous command output until it is reloaded or the session restarts.

To reload a configured `zjstatus` in place, pass the layout's configuration verbatim as `key=value` pairs:

```sh
zellij action start-or-reload-plugin "file:$HOME/.config/zellij/plugins/zjstatus.wasm" \
  -c "$(cat zjstatus-config.txt)"
```

Generate `zjstatus-config.txt` from the layout's plugin block instead of writing it by hand:

```sh
python3 - <<'EOF' > zjstatus-config.txt
import re
src = open("layout.kdl").read()
block = re.search(r'plugin location="file:[^"]*"[^{]*\{(.*?)\n {8}\}', src, re.S).group(1)
print(",".join(f"{k}={v}" for k, v in re.findall(r'^ {12}([a-z0-9_]+) +"((?:[^"\\]|\\.)*)"', block, re.M)))
EOF
```

If a reload already spawned a duplicate pane, remove it with the namespaced pane id (a bare number is read as a terminal id and silently does nothing):

```sh
zellij action dump-layout | grep 'plugin location'   # identify the spawned pane
zellij action close-pane --pane-id plugin_2
```

For quick iteration, restarting the fixture session (`zellij -s zjstatus-dev ...`) is simpler than maintaining an exact configuration copy: the reload path exists for sessions that cannot be restarted.

The test layouts under `tests/` exercise rendering and responsive behavior. Keep fixture layouts in sync with configuration-key changes.

## Benchmarks

Benchmarks live under `benches/`. Run them when changing width measurement, rendering, or widget hot paths:

```sh
cargo bench --features=bench
```

## Tracing

Build with the optional tracing feature:

```sh
cargo build --features tracing
```

Tracing is written outside normal plugin output. `zjstatus` writes `/host/.zjstatus.log` and `zjframes` writes `/host/.zjframes.log` when the feature is enabled; confirm the host path is writable before diagnosing runtime behavior.
