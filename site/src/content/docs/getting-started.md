---
title: Getting started
description: Install Alder in VS Code or Zed, open a Rails view and, if you want, turn on format on save.
sidebar:
  order: 1
---

Alder is one extension for Rails views in ERB, Haml and Slim. It highlights your templates, finds errors as you type, formats them, and understands your Rails app: partials, components, helpers, routes, instance variables, translations and Stimulus controllers. Pug and Liquid templates get checks, navigation and editing help without the Rails features; see [File types](/alder/guides/file-types/). It runs in VS Code and, with fewer features, in Zed.

## Install

Install **Alder** from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=x-ror.alder) or from [Open VSX](https://open-vsx.org/extension/x-ror/alder).

There are no gems to install. The checks and the formatter are a language server written in Rust and bundled with the extension.

### Zed

Install **Alder** from Zed's Extensions page once it is published; until then, choose _Install Dev Extension_ there and pick the repository's `zed/` directory (this needs Rust installed with rustup). Alder doesn't highlight in Zed: install the extensions for your template languages (Ruby for ERB, Haml, Slim, Pug, Liquid), and the language server starts for the ones you have. See [Zed](/alder/guides/zed/) for the server binary, settings and what differs from VS Code.

## Open a view

Open a Rails project and any view. Highlighting, checks, navigation and formatting work immediately.

While a template is open, the status bar shows its template language (`ERB`, `HAML`, `SLIM`, `ERB-JS`, …). If that item is missing, the file isn't one of the extension's; see [Troubleshooting](/alder/guides/troubleshooting/).

## Format on save

Format on save is optional. Turn it on per language in your settings:

```json
{
  "[erb]": { "editor.formatOnSave": true },
  "[haml]": { "editor.formatOnSave": true },
  "[slim]": { "editor.formatOnSave": true }
}
```

The formatter changes only whitespace; [Formatting](/alder/guides/formatting/) explains what it does and doesn't touch.

## Several Rails apps in one window

Multi-root workspaces and monorepos work side by side: each file resolves partials, routes, translations and gems against the app it belongs to.

## Requirements

- VS Code 1.100 or newer, or Zed with the extensions for your template languages.
- Linux (x64, arm64), macOS (Apple silicon, Intel) or Windows (x64).
- Ruby only for _Preview Rendered HTML_, _Show Compiled Ruby Source_ and the converters, which call Ruby gems you install only if you use them. Everything else works without Ruby. See [Commands](/alder/guides/commands/).

## Next steps

- [Errors as you type](/alder/guides/checks/) and the [list of diagnostics](/alder/reference/diagnostics/)
- [Rails navigation and completion](/alder/guides/rails-navigation/)
- [Command line for CI](/alder/guides/command-line/)
- [Settings](/alder/guides/settings/)
- [Zed](/alder/guides/zed/)
