<p align="center"><img src="images/icon.png" alt="Lamina" width="128"></p>

# Lamina: Ruby Templates (ERB, Haml, Slim, Pug, Liquid)

[![VS Marketplace](https://vsmarketplacebadges.dev/version-short/x-ror.lamina.svg)](https://marketplace.visualstudio.com/items?itemName=x-ror.lamina)
[![Open VSX](https://img.shields.io/open-vsx/v/x-ror/lamina?label=Open%20VSX)](https://open-vsx.org/extension/x-ror/lamina)
[![npm](https://img.shields.io/npm/v/@x-ror/lamina?label=npm%20CLI)](https://www.npmjs.com/package/@x-ror/lamina)
[![Socket](https://socket.dev/api/badge/npm/package/@x-ror/lamina)](https://socket.dev/npm/package/@x-ror/lamina)
[![build](https://github.com/x-ror/lamina/actions/workflows/build.yml/badge.svg)](https://github.com/x-ror/lamina/actions/workflows/build.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Rails view tooling for VS Code: one extension for ERB, Haml and Slim.** It highlights your templates, finds errors as you type, formats them, and understands your Rails app: partials, components, helpers, routes, instance variables, translations and Stimulus controllers. Pug and Liquid templates get a smaller share, without the Rails features.

Built for Rails developers who work in `app/views` every day, and for teams that want the same template checks and formatting in the editor and in CI.

- **No gems to install.** The checks and the formatter are a fast language server written in Rust and bundled with the extension. They follow Erubi, Rails, Haml 7 and Slim 5 and report errors with those projects' own messages.
- **Rails-aware.** <kbd>F12</kbd> on `render "users/card"` opens the partial; completion knows your partials, ViewComponents, model columns, `*_path` helpers, `@ivars` from the controller and `t(".keys")`.
- **Safe formatting.** It changes only whitespace and checks that the template still compiles to the same thing; if it is unsure, it leaves the file alone.
- **Pug and Liquid.** Pug (`.pug`, `.jade`, `pug-rails` `.jst.pug`) keeps VS Code's own highlighting and gets snippets, Pug 3's syntax errors, the HTML checks, an outline, `include`/`extends` navigation, whitespace formatting and the editing commands. Liquid gets highlighting, navigation to the files `render` and `include` name, and a check of its tags.
- **Works with Ruby LSP.** Install both: Ruby LSP keeps your `.rb` files, this extension adds what templates need.

## Getting started

1. Install **Lamina** from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=x-ror.lamina) or [Open VSX](https://open-vsx.org/extension/x-ror/lamina).
2. Open a Rails project and any view. That's it: highlighting, checks, navigation and formatting work immediately.
3. Optional: turn on format on save, e.g. `"[erb]": { "editor.formatOnSave": true }` (also `[haml]`, `[slim]`, and `[jade]` for Pug).

Requires VS Code 1.100 or newer, on Linux (x64, arm64), macOS (Apple silicon, Intel) or Windows (x64).

## Documentation

The full documentation is at [x-ror.github.io/lamina](https://x-ror.github.io/lamina/):

- [Getting started](https://x-ror.github.io/lamina/getting-started/)
- Guides: [errors as you type](https://x-ror.github.io/lamina/guides/checks/), [Rails navigation and completion](https://x-ror.github.io/lamina/guides/rails-navigation/), [file types](https://x-ror.github.io/lamina/guides/file-types/), [formatting](https://x-ror.github.io/lamina/guides/formatting/), [commands](https://x-ror.github.io/lamina/guides/commands/), [command line for CI](https://x-ror.github.io/lamina/guides/command-line/), [settings](https://x-ror.github.io/lamina/guides/settings/), [supported syntax](https://x-ror.github.io/lamina/guides/syntax/), [compatibility](https://x-ror.github.io/lamina/guides/compatibility/) and [troubleshooting](https://x-ror.github.io/lamina/guides/troubleshooting/)
- [Diagnostics reference](https://x-ror.github.io/lamina/reference/diagnostics/): every check, with its code
- [Rules and `.lamina.toml`](https://x-ror.github.io/lamina/reference/rules/#configuration): turn checks off per project, folder or file; it wins over the VS Code settings
- [Changelog](https://x-ror.github.io/lamina/changelog/)

[Contributing](CONTRIBUTING.md) covers building, testing and the project layout. Found a bug or missing a Rails feature? [Open an issue](https://github.com/x-ror/lamina/issues).

## License

[MIT](LICENSE)
