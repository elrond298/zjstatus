#!/bin/sh
set -eu

repo=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
config_dir=${ZELLIJ_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/zellij}
jobs=${CARGO_BUILD_JOBS:-1}

printf 'Building zjstatus (jobs=%s)...\n' "$jobs"
(
    cd "$repo"
    cargo build --jobs "$jobs" --release --target wasm32-wasip1 --target-dir "$repo/target" --bin zjstatus
)
printf 'Preparing %s...\n' "$config_dir"
mkdir -p "$config_dir/plugins" "$config_dir/scripts"
printf 'Installing plugin: %s\n' "$config_dir/plugins/zjstatus.wasm"
install -m 0644 "$repo/target/wasm32-wasip1/release/zjstatus.wasm" "$config_dir/plugins/zjstatus.wasm"
for script in vcs-status pi-status host-load; do
    printf 'Installing script: %s\n' "$config_dir/scripts/$script.sh"
    install -m 0755 "$repo/examples/$script.sh" "$config_dir/scripts/$script.sh"
done

printf 'Installed zjstatus to %s\n' "$config_dir"
