#!/bin/sh
set -eu

repo=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
config_dir=${ZELLIJ_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/zellij}

(
    cd "$repo"
    cargo build --release --target wasm32-wasip1 --target-dir "$repo/target" --bin zjstatus
)
mkdir -p "$config_dir/plugins" "$config_dir/scripts"
install -m 0644 "$repo/target/wasm32-wasip1/release/zjstatus.wasm" "$config_dir/plugins/zjstatus.wasm"
for script in vcs-status pi-status host-load; do
    install -m 0755 "$repo/examples/$script.sh" "$config_dir/scripts/$script.sh"
done

printf 'Installed zjstatus to %s\n' "$config_dir"
