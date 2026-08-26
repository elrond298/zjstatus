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
