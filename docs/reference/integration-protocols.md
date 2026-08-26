# Integration protocols

External processes and plugins can communicate with `zjstatus` through plugin messages.

## Rerun a command

```text
zjstatus::rerun::<command_name>
```

This invalidates the named command and requests a fresh run.

## Show a notification

```text
zjstatus::notify::<message>
```

The message becomes available to the notification widget according to its configured display and timeout behavior.

## Update a pipe

```text
zjstatus::pipe::<pipe_name>::<content>
```

The named pipe widget renders the supplied content. Choose static or dynamic rendering for normal text, or raw only for trusted complete markup.


## Delimiters

The parser splits each message on `::` and consumes only the documented fields. Do not include `::` in a command name, pipe name, notification, or pipe content; such text is split and later pieces are ignored.

## Click actions

A command widget can run a separate command when its rendered region is clicked:

```kdl
command_refresh_clickaction "sh -c 'printf clicked'"
```

The click action is parsed as an argv command and runs through Zellij; use `sh -c` when it needs shell syntax.
Command and pipe result placeholders are documented in the [widget reference](widgets.md); command click actions are configured separately and pipe widgets are not clickable. Plugin message permissions must be granted by Zellij.
