# Commands and pipes

Use a command widget when zjstatus should poll a process. Use a pipe widget when another plugin or process pushes values into zjstatus.

## Commands

A dynamic widget is named `command_<name>_*`:

```kdl
command_weather_command   "sh -c 'printf %s 21C'"
command_weather_format    "weather {stdout}"
command_weather_interval  "30"
command_weather_rendermode "static"
```

Commands run through Zellij's background command API. There is at most one in-flight run per widget; a slow or hung run does not block Zellij, Pi, or the bar, and the last completed value remains visible. Use an operating-system `timeout` when a process must be forcibly terminated. Interval `0` is a run-once command.

`{stdout}`, `{stderr}`, and `{exit_code}` can be used in command formats. `{focused_pane_cwd}` is available for commands whose working directory follows the focused pane; changing focus invalidates the cached result.

Use `sh -c` when the command needs pipes, redirects, variables, or shell operators.

## Render modes

- `static` inserts command output into the configured format.
- `dynamic` reparses formatting directives contained in the result.
- `raw` uses the result as zjstatus markup; treat it as trusted input.

Click actions can be attached with `command_<name>_clickaction`. See the [reference](../reference/configuration.md) for exact keys.

## Pipes

Pipe widgets render data delivered through the plugin message protocol. They do not spawn the producer. Use them for push-based integrations and choose `static`, `dynamic`, or `raw` rendering just as with commands.

See [integration protocols](../reference/integration-protocols.md) for message syntax and [status scripts](status-scripts.md) for polling examples.
