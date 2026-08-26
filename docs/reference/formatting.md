# Formatting reference

Formats use Zellij-style directives:

```text
#[fg=#c6d0f5,bg=#303446,bold]text {session}
```

Supported color fields include `fg`, `bg`, and `us` (underline). Colors can be named, bright/named variants, hexadecimal RGB (`#rrggbb`), ANSI-256 numeric values, `default`, or configuration aliases such as `$green`. Text effects include attributes supported by the Zellij tile formatter, such as `bold` and `italic`.

Widget placeholders are expanded inside formats, for example `{mode}`, `{session}`, `{tabs}`, `{datetime}`, `{stdout}`, and `{name}` where supported. See [widgets](widgets.md) for the complete placeholder context.
## Escaping

There is no separate backslash-escape syntax in the formatter. Text is literal until it contains `#[`, which starts a styled part; avoid a literal `#[` in a configured format or emit it through a command's normal (non-raw) output. In a style directive, `]` ends the directive.
Unknown `{word}` sequences are treated as widget placeholders and render `Use of uninitialized widget` when no widget with that name exists. There is no brace-doubling escape; use text that does not match a placeholder or emit it through normal command output when a literal brace sequence is required.

## Render modes

Command and pipe widgets choose how output is interpreted:

- **static:** output is inserted as text into the configured formatted parts;
- **dynamic:** formatting directives in output are parsed again;
- **raw:** output is treated as complete zjstatus markup.

Raw output is a trust boundary. Do not pass untrusted user or network content through raw mode. Use normal rendering and escaping when content is not controlled by the configuration author.

Responsive levels select complete formatted strings before rendering. Use semantic variants rather than relying on byte-based truncation; zjstatus measures terminal display width and keeps grapheme clusters together where supported.
