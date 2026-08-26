# zjframes

`zjframes` is a background-only plugin that changes pane-frame visibility without rendering a status bar.

Use it when frame automation is useful on its own. If `zjstatus` is already loaded and configured with frame conditions, do not also load `zjframes` unless you intentionally coordinate the two controllers.

## Conditions

The frame engine can hide or show frames based on:

- a single-pane tab;
- search or rename mode;
- scroll mode;
- a focused fullscreen pane.

Conditions can be combined; test the combinations that matter for your layout. Zellij's own frame settings and another frame plugin can override or fight these decisions.

Load `zjframes` as a plugin, not as a visible pane, and grant the application-state and frame-changing permissions requested by the plugin. For symptoms such as flicker or frames returning unexpectedly, see [troubleshooting](../operations/troubleshooting.md).
