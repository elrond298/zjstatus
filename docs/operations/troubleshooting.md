# Troubleshooting

## Nothing renders

Check the WASM path, plugin target, plugin permissions, and whether the pane has a nonzero size. `zjstatus` renders two plugin rows even when the first is empty, so reserve `size=2` plus one extra row for a border. Restart the session after replacing a plugin so Zellij does not reuse an older cached instance.

## Only one row appears

The hint row and idle command row are alternative contents of the same first rendered row, not separate panes. The plugin's first row shows hints when active and the idle command row otherwise; the main status row is always below it. Reserve two rows for the plugin pane.

## Border clips output

A top or bottom border consumes another terminal row. Reduce the layout or disable the border before debugging text width. Check that the border character and formatted output have the expected terminal display width.

## Permission errors or missing interaction

`zjstatus` requests `ReadApplicationState`, `ChangeApplicationState`, and `RunCommands`; `zjframes` requests the first two. Accept the Zellij permission prompt for the features you use. A denied permission often looks like missing updates rather than a Rust error.

## Configuration errors

Read the exact error printed by the plugin. Named responsive configuration requires `format_shrink_levels` and a `format_shrink_order` containing left, center, and right exactly once. Removed legacy responsive keys must be migrated; see [upgrading](upgrading.md).

## Commands are blank, stale, or never finish

Run the command manually in the configured working directory. Check `sh -c` quoting, dependencies, `cwd`, and permissions. Slow commands intentionally retain their last completed value; wrap a command with `timeout` if it must terminate. Interval `0` runs once. A command using `{focused_pane_cwd}` is invalidated only when the focused pane directory changes.

## Raw markup appears literally

Use `rendermode "dynamic"` to reparse formatting from trusted output, or `raw` for complete trusted markup. Static mode treats returned directives as text. Never pass untrusted user or network content through dynamic or raw mode.

## Focused-directory status does not update

Use `command_<name>_cwd "{focused_pane_cwd}"` and ensure the command is a configured command widget listed in the idle row or main format. Check that Zellij is delivering focus/CWD updates and that the script can run in the focused directory.

## Bundled scripts emit nothing

Check the required dependencies and platform: VCS needs Git or Jujutsu plus `jq`; Pi needs the producer extension, `jq`, and a live process in the current Zellij session; host-load reads Linux `/proc` and `/sys` files. The VCS script emits nothing outside a repository. See [status scripts](../guides/status-scripts.md).

## Host metrics on non-Linux systems

`host-load.sh` is Linux-specific and reads `/proc/loadavg`, `/proc/net/dev`, and `/sys/block/*/stat`. Replace it with a platform-specific command widget instead of treating an empty result as a zjstatus failure.

## Icons are missing

Install a Nerd Font in the terminal, or replace icon glyphs in the script/configuration with ASCII labels. Check display width again after changing fonts.

## Frames flicker

Do not run conflicting `zjstatus`, `zjframes`, or external frame controllers. Check Zellij's own frame settings and test each enabled frame condition separately.

## Plugin cache contains an old build

Use a new Zellij session or restart the plugin after replacing a WASM file. Confirm the layout points to the intended absolute or `file:` path and inspect the installed file's timestamp/size before debugging source changes.
