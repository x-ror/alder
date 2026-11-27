---
title: Zed
description: Run Alder's language server in Zed, with its settings, what differs from VS Code and how to troubleshoot it.
sidebar:
  order: 13
---

The Zed extension runs the same language server as VS Code and adds its snippets. It does not provide syntax highlighting, and it has none of the features that the VS Code extension implements itself.

## Install

Install **Alder** from Zed's Extensions page once it is published. Until then, and for development, open the Extensions page, choose _Install Dev Extension_ and pick the repository's `zed/` directory. This needs Rust installed with rustup; Zed adds the `wasm32-wasip2` target itself.

Highlighting comes from other Zed extensions, which you install yourself:

- Ruby: ERB, HTML+ERB, JS+ERB, YAML+ERB, and Ruby for `.jbuilder` files
- Haml
- Slim
- Pug
- Liquid

Alder starts only for the languages whose extension is installed.

For Ruby-language files Alder is opt-in, because only `.jbuilder` and `.builder` files and Rails controllers concern it. Name it in Zed's `settings.json` to turn it on:

```json
{
  "languages": {
    "Ruby": { "language_servers": ["alder-ls", "..."] }
  }
}
```

## Server binary

The extension uses `alder-ls` from your `PATH` when there is one. Otherwise it downloads the `alder-ls-<platform>` archive from the latest [GitHub release](https://github.com/x-ror/alder/releases) (`linux-x64`, `linux-arm64`, `darwin-arm64`, `darwin-x64` or `win32-x64`). Releases up to and including 1.1.3 don't carry these archives, so the download works from the first release after them.

To use your own binary:

```json
{
  "lsp": {
    "alder-ls": {
      "binary": { "path": "/path/to/alder-ls" }
    }
  }
}
```

## Settings

The engine settings of the [settings reference](/alder/guides/settings/), without the `alder.` prefix, go in Zed's `settings.json` under `initialization_options`:

```json
{
  "lsp": {
    "alder-ls": {
      "initialization_options": {
        "slim": { "extraEngines": ["mjml"] },
        "erb": {
          "disabledDiagnostics": ["Html/ImgAlt"],
          "diagnostics": { "enabled": true },
          "format": { "enabled": true, "alignComments": false }
        }
      }
    }
  }
}
```

| Option                                                             | Type     | Default |
| ------------------------------------------------------------------ | -------- | ------- |
| `slim.extraEngines`                                                | string[] | `[]`    |
| `<slim\|erb\|haml\|jbuilder\|liquid\|pug>.disabledDiagnostics`     | string[] | `[]`    |
| `<erb\|haml\|slim\|pug>.diagnostics.enabled`, `<…>.format.enabled` | boolean  | `true`  |
| `erb.format.alignComments`                                         | boolean  | `false` |

The server reads them once at startup: run `editor: restart language server` after changing them. A `.alder.toml` in the project works unchanged and wins over these options; see the [rules reference](/alder/reference/rules/#configuration).

Semantic tokens are off by default in Zed. To use them, set `"semantic_tokens": "combined"` (or `"full"`) in Zed's settings.

## What works

Everything the language server does works in Zed: diagnostics, formatting and format on save, go to definition (partials, components, helpers), completion, hover, signature help, references, rename of strict locals, partial renames that update `render` calls when you rename the file, document links, folding, document symbols and quick fixes. See [Rails navigation and completion](/alder/guides/rails-navigation/) and [Errors as you type](/alder/guides/checks/).

Zed formats with the first language server that offers formatting, which for a language can be another server than Alder. To format with Alder, name it as the formatter for that language:

```json
{
  "languages": {
    "HTML+ERB": { "formatter": [{ "language_server": { "name": "alder-ls" } }] }
  }
}
```

## What differs from VS Code

The VS Code extension implements these itself, so Zed doesn't have them:

- The [commands](/alder/guides/commands/): toggle and wrap, extract partial, switch to related file, preview and compiled source, the converters.
- Tag auto-closing in ERB.
- Route awareness. The VS Code extension reads the routes and hands them to the server, so in Zed the `Rails/UnknownRoute` check and the completion, hover and go to definition of `*_path` and `*_url` helpers are missing.

## Troubleshooting

- Alder doesn't start for a file: check that Zed's extension for its language is installed and that the file opens in that language.
- Logs: `zed: open log` for the extension (server download and start), `dev: open language server logs` for the server.
- Settings have no effect: restart the server with `editor: restart language server`.
- Diagnostics or completion from Ruby LSP as well: see [Compatibility](/alder/guides/compatibility/#ruby-lsp).
