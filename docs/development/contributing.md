# Contributing

Keep changes small and source-backed. Reuse existing widgets and integration boundaries before adding abstractions.

For behavior changes:

1. Read every caller and the relevant tests.
2. Add or update the smallest regression test that proves the behavior.
3. Update the canonical reference and the user guide when configuration or visible behavior changes.
4. Run `cargo fmt --check`, `cargo clippy`, `cargo test`, and relevant script checks.
5. Check Markdown links and `git diff --check`.

Document terminology consistently: main-row `format_shrink_*` levels, idle-row `hint_idle_shrink_order`, and key-hint pagination are different mechanisms. Keep examples runnable and avoid documenting unsupported legacy keys as current configuration.

See the repository [Code of Conduct](../../CODE_OF_CONDUCT.md) before opening a contribution.
