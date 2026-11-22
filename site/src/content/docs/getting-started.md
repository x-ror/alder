---
title: Getting started
description: Install Lamina, open a Rails view and, if you want, turn on format on save.
sidebar:
  order: 1
---

Lamina is one VS Code extension for Rails views in ERB, Haml and Slim. It highlights your templates, finds errors as you type, formats them, and understands your Rails app: partials, components, helpers, routes, instance variables, translations and Stimulus controllers. Pug and Liquid templates get checks, navigation and editing help without the Rails features; see [File types](/lamina/guides/file-types/).

## Install

Install **Lamina** from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=x-ror.lamina) or from [Open VSX](https://open-vsx.org/extension/x-ror/lamina).

There are no gems to install. The checks and the formatter are a language server written in Rust and bundled with the extension.

## Open a view

Open a Rails project and any view. Highlighting, checks, navigation and formatting work immediately.

While a template is open, the status bar shows its template language (`ERB`, `HAML`, `SLIM`, `ERB-JS`, …). If that item is missing, the file isn't one of the extension's; see [Troubleshooting](/lamina/guides/troubleshooting/).

## Format on save

Format on save is optional. Turn it on per language in your settings:

```json
{
  "[erb]": { "editor.formatOnSave": true },
  "[haml]": { "editor.formatOnSave": true },
  "[slim]": { "editor.formatOnSave": true }
}
```

The formatter changes only whitespace; [Formatting](/lamina/guides/formatting/) explains what it does and doesn't touch.

## Several Rails apps in one window

Multi-root workspaces and monorepos work side by side: each file resolves partials, routes, translations and gems against the app it belongs to.

## Requirements

- VS Code 1.100 or newer.
- Linux (x64, arm64), macOS (Apple silicon, Intel) or Windows (x64).
- Ruby only for _Preview Rendered HTML_, _Show Compiled Ruby Source_ and the converters, which call Ruby gems you install only if you use them. Everything else works without Ruby. See [Commands](/lamina/guides/commands/).

## Next steps

- [Errors as you type](/lamina/guides/checks/) and the [list of diagnostics](/lamina/reference/diagnostics/)
- [Rails navigation and completion](/lamina/guides/rails-navigation/)
- [Command line for CI](/lamina/guides/command-line/)
- [Settings](/lamina/guides/settings/)
