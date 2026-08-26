# Releasing

A release should contain a WASM artifact built for the target supported by the declared Zellij version. Before publishing:

1. Run formatting, lint, Rust tests, script tests, and documentation link checks.
2. Build both binaries when the release includes `zjframes`.
3. Verify the installed plugin loads in a fresh Zellij session.
4. Check permissions, responsive rows, command widgets, and frame behavior.
5. Publish the artifact and update release notes with configuration migrations.

Keep the version in `Cargo.toml`, release workflow, and generated notes consistent. Include user-visible changes to key hints, compression, integrations, scripts, and removed configuration keys.
