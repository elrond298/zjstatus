# Repository Agent Guide

## Scope and structure

This is a Rust 2024 Zellij plugin repository with two binaries:

- `src/bin/zjstatus.rs`: visible status plugin lifecycle and responsive rows.
- `src/bin/zjframes.rs`: background frame controller.
- `src/config.rs` owns KDL parsing/validation and responsive selection; `src/render.rs` owns width-aware formatting; `src/widgets/` owns widgets; `src/frames.rs` contains shared frame decisions.
- `examples/` contains shipped layouts and status scripts. `tests/zjstatus/` and `tests/zjframes/` are interactive fixture layouts.

Read `docs/development/architecture.md` before moving responsibilities between these areas. Main-row compression, idle-command variants, and key-hint pagination are separate state machines. Keep producer integrations at their existing output/schema boundary.

## Toolchain and prerequisites

`rust-toolchain.toml` pins Rust 1.95.0 and `wasm32-wasip1`. `.cargo/config.toml` makes that target the default and uses Wasmtime as the test runner. Install the target and ensure `wasmtime` is available before WASM tests. CI additionally uses `cargo-nextest`.

The shell checks directly invoke extra tools: `tests/pi-status.sh` uses `jq` and `python3`; `tests/vcs-status.sh` uses `jj`.

## Commands

Use the smallest applicable check first:

```sh
cargo test <test-name>                 # one Rust test
cargo test                             # repository-configured WASM test suite
cargo nextest run --lib                # exact CI test command
cargo fmt --check
RUSTFLAGS=-Dwarnings cargo clippy --all-features --lib  # CI lint behavior
tests/pi-status.sh
tests/vcs-status.sh
```

If Wasmtime is unavailable on Linux, the documented native fallback is:

```sh
cargo test --target x86_64-unknown-linux-gnu
```

Build individual plugins explicitly:

```sh
cargo build --target wasm32-wasip1 --bin zjstatus
cargo build --target wasm32-wasip1 --bin zjframes
```

Run `cargo bench --features=bench` only for width measurement, rendering, or widget hot-path changes. See `docs/development/building-and-testing.md`.

Do not use `install.sh` merely to verify a change: it builds a release and writes the plugin and bundled scripts into the user's Zellij configuration directory. Use it only when installation is requested.

## Change rules

Follow `docs/development/contributing.md`: read all callers, add the smallest regression test, and update the canonical guide/reference for configuration or visible behavior changes. Keep `tests/` fixture layouts synchronized with configuration-key changes. `.gitignore` excludes generated `target/`; do not edit it.

Status commands must remain asynchronous and retain the last completed value rather than blocking Zellij. Preserve each binary's `register_plugin!(State)` registration when changing WASM entry/build behavior.

Completion requires relevant focused checks, `cargo fmt --check`, CI-equivalent Clippy/tests, applicable shell checks, documentation/link review, `git diff --check`, and explicit code-review approval.
