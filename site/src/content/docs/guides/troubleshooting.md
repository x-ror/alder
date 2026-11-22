---
title: Troubleshooting
description: Fix overlaps with other extensions for the same languages and check which grammar, language and server are active.
sidebar:
  order: 12
---

## Other extensions for the same languages

| Extension                                                                | Overlap                                                                    |
| ------------------------------------------------------------------------ | -------------------------------------------------------------------------- |
| Ruby LSP (`Shopify.ruby-lsp`)                                            | Registers `erb`; may add its own ERB diagnostics, completion or formatting |
| ERB formatters (e.g. `aliariff.vscode-erb-beautify`)                     | Formatter for `erb`                                                        |
| Other ERB grammars                                                       | Grammar for `erb`                                                          |
| Other Haml or Slim extensions (e.g. `karunamurti.haml`, `sianglim.slim`) | Grammar, snippets and often a formatter for `haml` / `slim`                |

Symptoms are doubled diagnostics or completions, a formatter that asks which one to use, or different colours than the examples on this site.

### Formatter

Choose this extension's formatter per language:

```json
{
  "[erb]": { "editor.defaultFormatter": "x-ror.lamina" },
  "[haml]": { "editor.defaultFormatter": "x-ror.lamina" },
  "[slim]": { "editor.defaultFormatter": "x-ror.lamina" }
}
```

To keep the other one, set it as `editor.defaultFormatter` for the language. `lamina.<erb|haml|slim|pug>.format.enabled` set to `false` (or `format.enabled = false` in a `.lamina.toml`) makes this extension return no edits, but it stays in the list VS Code asks from.

### Grammar

VS Code uses one grammar per language id, so only one extension's grammar applies. Disable the other extension for the workspace, or move the files to another id with `files.associations`:

```json
{
  "files.associations": { "*.html.erb": "erb" }
}
```

### Diagnostics and completion

Turn off the duplicates in the other extension's settings. This extension's are `lamina.<erb|haml|slim>.diagnostics.enabled` and `lamina.<erb|haml|slim|jbuilder>.disabledDiagnostics`; see [Errors as you type](/lamina/guides/checks/#turning-checks-off).

## Check what is active

### Language mode

The language is shown in the status bar; click it to change. This extension adds a status bar item with a preview icon and the template language (`ERB`, `HAML`, `SLIM`, `ERB-JS`, …) while a template is open. If it is missing, the file isn't one of this extension's.

### Grammar

Run _Developer: Inspect Editor Tokens and Scopes_ on a token. Scopes ending in `.erb`, `.haml` or `.slim` (`text.html.erb`, `text.haml`, `text.slim`) are this extension's.

### Language server

- The _Lamina_ channel in the Output panel logs the language server, including why a format was skipped.
- _Lamina: Restart Language Server_ restarts it.
- _Developer: Show Running Extensions_ lists every extension active for the window.

## Report a problem

Found a bug or missing a Rails feature? [Open an issue on GitHub](https://github.com/x-ror/lamina/issues).
