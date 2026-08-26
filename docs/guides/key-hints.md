# Contextual key hints

The key-hint row is the first line of the `zjstatus` plugin pane, above the main status row. When hints are hidden it can render the configured idle command row; it stays active while empty so it can continue receiving mode updates.

## What it shows

The plugin consumes Zellij's `InitialKeybinds` event and supports bindings for Resize, Pane, Tab, Scroll, Search, Session, Move, and Tmux modes. Key labels are humanized while descriptions remain paired with their keys.

A mode change schedules the row after a short idle delay. Input postpones a pending reveal; once visible, the first subsequent input dismisses the one-shot display. A later mode change can show it again.

## Width behavior

The row reduces in this order:

1. full headers and descriptions;
2. compact headers;
3. page-only headers;
4. no header;
5. pagination of complete key-description pairs.

If one pair is wider than the available row, only that pair is truncated. When a pair fits alone, its description remains complete. Pagination never splits a key-description pair.

## Configuration

A typical two-row pane uses a key binding for page cycling:

```kdl
keybinds {
    shared_except "locked" {
        bind "Ctrl h" {
            MessagePluginId {
                name "key-hints-next-page"
            }
        }
    }
}
```

The exact hint format keys and supported options are listed in the [configuration reference](../reference/configuration.md). Style headers, key names, descriptions, separators, and the empty state independently.

## Troubleshooting

- No hints: ensure the plugin received `InitialKeybinds`, the pane is tall enough, and the plugin has application-state permissions.
- Hints flash repeatedly: keep the `zjstatus` pane persistent; do not replace empty rendering with `hide_self()`.
- Missing page cycling: verify the binding targets the loaded plugin with `MessagePluginId` and that the plugin is receiving pipe messages.
- Missing icons: install a Nerd Font, or use plain-text formats.
