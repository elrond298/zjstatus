# Main status-bar compression

Main-row compression is the `format_shrink_*` system. It is separate from key hints and from the idle command row.

## Configure named levels

```kdl
format_shrink_levels "compact minimal locator tiny"
format_shrink_order  "right center left"
format_left          "{session} {tabs}"
format_center        "{notifications}"
format_right         "{mode} {datetime}"
format_left_compact  "{session} {tabs}"
format_center_compact "{notifications}"
format_right_compact "{mode}"
format_left_minimal  "{tabs}"
format_right_minimal "{mode}"
format_left_locator  "{tabs}"
format_left_tiny     "{tabs}"
```

Each name in `format_shrink_levels` creates `format_<region>_<level>` keys. `format_shrink_order` must contain left, center, and right exactly once. Regions complete the current synchronized compression round before any region advances to the next level.

A missing variant inherits the previous valid one. An empty variant is an intentional empty output. Legacy `format_responsive`, `format_precedence`, and numeric suffix configuration is rejected; use named levels instead.

## Narrow-width priorities

The renderer measures terminal display cells, not bytes. It preserves a non-empty notification and an active-tab position locator as long as possible. Notifications temporarily outrank persistent status fields. Tabs use their own fallback sequence: the configured window, one active tab with arrows, a truncated active-tab name with arrows, the full position locator, then its compact index.

When responsive fallback synthesizes a notification or collapses to a minimum layout, it disables mouse hit testing because the visible position no longer maps reliably to the original widget. Normal fitting responsive stages preserve click handling.

Configure short semantic text before relying on hard truncation. Keep stable labels, remove decorative detail, and make each stage smaller than the prior stage.

## Related systems

- [Contextual key hints](key-hints.md) reduce headers and paginate key pairs.
- [Responsive command rows](responsive-command-row.md) select script output variants.
- [Formatting](../reference/formatting.md) describes the text syntax used at every level.
