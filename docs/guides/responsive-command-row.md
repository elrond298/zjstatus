# Responsive command row

The idle command row is configured with `hint_idle_left` and `hint_idle_right`. It is not the main status row.

## Variant protocol

A command emits one display variant per line, widest first. A line containing exactly `@hide` hides it. Every occurrence of a command name in `hint_idle_shrink_order` advances that command to its next line.

```kdl
hint_idle_left         "vcs pi"
hint_idle_right        "load"
hint_idle_shrink_order "vcs pi pi pi load load vcs pi"
```

The command names refer directly to `command_<name>_command` widgets. Old `hint_idle_<item>_command` aliases and numeric `_reductions` keys are rejected.

Blank or missing lines inherit the previous valid output. A later variant that is wider than the previous one is rejected or ignored, so write variants from detailed to compact. Commands omitted from the shrink order remain at their first variant.

## Design a custom command

```sh
printf 'full detail\ncompact detail\nminimal\n@hide\n'
```

Use `rendermode "raw"` only when the command deliberately emits trusted `#[...]` formatting. Otherwise use normal rendering. Keep semantic labels in every stage; shorten values before removing the field.

## Width and fitting

The row advances the configured sequence until the combined left and right output fits. If content still cannot fit, zjstatus truncates according to the row's final fitting rules. Put high-priority stable information on the side you want to survive longest, and test the bar at realistic terminal widths.

See [commands and pipes](commands-and-pipes.md) for execution behavior and [bundled status scripts](status-scripts.md) for complete examples.
