---
title: Settings
description: Every alder setting with its default value and what it does.
sidebar:
  order: 9
---

All settings live under `alder.` and can be set per user or per workspace. For example, in `.vscode/settings.json`:

```json
{
  "alder.erb.disabledDiagnostics": ["Html/ImgAlt"],
  "alder.slim.extraEngines": ["mjml"],
  "alder.preview.useRails": true
}
```

In Zed the engine settings go under `initialization_options`; see [Zed](/alder/guides/zed/#settings).

A `.alder.toml` in the project sets the diagnostics, formatting and Slim engine settings below too, and wins over them where it says something; see the [rules reference](/alder/reference/rules/#configuration).

Editor settings for one template language use its language id, such as `[erb]`, `[haml]`, `[slim]` or `[erb-js]`; [File types](/alder/guides/file-types/) lists the ids.

## Diagnostics

| Setting                              | Default | Description                                                                                  |
| ------------------------------------ | ------- | -------------------------------------------------------------------------------------------- |
| `alder.erb.disabledDiagnostics`      | `[]`    | Built-in ERB diagnostics to turn off, by rule name.                                          |
| `alder.haml.disabledDiagnostics`     | `[]`    | Built-in Haml diagnostics to turn off, by rule name.                                         |
| `alder.jbuilder.disabledDiagnostics` | `[]`    | Built-in diagnostics to turn off in Jbuilder and Builder views, by rule name.                |
| `alder.liquid.disabledDiagnostics`   | `[]`    | Built-in Liquid diagnostics to turn off, by rule name.                                       |
| `alder.pug.disabledDiagnostics`      | `[]`    | Built-in Pug diagnostics to turn off, by rule name.                                          |
| `alder.slim.disabledDiagnostics`     | `[]`    | Built-in Slim diagnostics to turn off, by rule name.                                         |
| `alder.erb.diagnostics.enabled`      | `true`  | Show the language server's diagnostics in ERB files. Turn off to silence them all.           |
| `alder.haml.diagnostics.enabled`     | `true`  | Show the language server's diagnostics in Haml files. Turn off to silence them all.          |
| `alder.slim.diagnostics.enabled`     | `true`  | Show the language server's diagnostics in Slim and Skim files. Turn off to silence them all. |
| `alder.pug.diagnostics.enabled`      | `true`  | Show the language server's diagnostics in Pug files. Turn off to silence them all.           |

Each `disabledDiagnostics` setting takes the rule names of that language's checks (`Html/ImgAlt`); the [diagnostics reference](/alder/reference/diagnostics/) lists them. A rule that a `.alder.toml` enables with `enabled = true` stays on even when the list names it; the file's `enabled = false` also applies. [Errors as you type](/alder/guides/checks/#turning-checks-off) shows examples.

## Formatting

| Setting                          | Default | Description                                                                                                                                                                                                        |
| -------------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `alder.erb.format.enabled`       | `true`  | Let the language server format ERB files (Format Document and Format Selection with the built-in formatter). Turn off for no edits.                                                                                |
| `alder.erb.format.alignComments` | `false` | Move the lines inside an ERB template's HTML comments (`<!-- … -->`) and multi-line `<%# … %>` comments with the line the comment opens on, keeping their relative indentation. Off: those lines keep their bytes. |
| `alder.haml.format.enabled`      | `true`  | Let the language server format Haml files (Format Document and Format Selection). Turn off for no edits.                                                                                                           |
| `alder.slim.format.enabled`      | `true`  | Let the language server format Slim and Skim files (Format Document and Format Selection). Turn off for no edits.                                                                                                  |
| `alder.pug.format.enabled`       | `true`  | Let the language server format Pug files (Format Document: indentation, trailing whitespace, final newline). Turn off for no edits.                                                                                |

The indentation width isn't a setting of its own: it comes from the editor's tab size, then `.editorconfig`, then 2. See [Formatting](/alder/guides/formatting/#indentation-width-and-line-endings).

## Editing

| Setting                   | Default | Description                                                                                                                 |
| ------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------- |
| `alder.erb.autoCloseTags` | `true`  | Typing the `>` of an HTML start tag in ERB inserts its end tag after the cursor (like html.autoClosingTags for plain HTML). |

## Rails

| Setting                | Default | Description                                                                                                                                                                                                                                                                                                                                                                            |
| ---------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `alder.routes.enabled` | `true`  | The language server reads the app's routes with `bin/rails runner` (boots the app once per app, and again when config/routes.rb or config/routes/*.rb changes) for completion, hover and go to definition of `*_path` / `*_url` helpers in views, and for the `Rails/UnknownRoute` check. Only in a trusted workspace. A boot that fails is logged and leaves the routes as they were. |

Routes are read in trusted workspaces only.

## Slim

| Setting                   | Default | Description                                                                                             |
| ------------------------- | ------- | ------------------------------------------------------------------------------------------------------- |
| `alder.slim.extraEngines` | `[]`    | Embedded engine names registered by your app (e.g. `mjml`) that the unknown-engine check should accept. |

`.skim` files use the `slim` settings.

## Preview and compiled source

| Setting                       | Default | Description                                                                                                                                                                                                                                       |
| ----------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `alder.slim.compileForRails`  | `false` | Show Compiled Ruby Source for Slim runs `slimrb --rails`: ActionView output buffer and no capture, as Rails compiles it.                                                                                                                          |
| `alder.preview.refreshOnSave` | `true`  | Re-render the HTML preview whenever the template is saved.                                                                                                                                                                                        |
| `alder.preview.useBundler`    | `false` | Prefix render commands with `bundle exec` when a Gemfile exists in the workspace.                                                                                                                                                                 |
| `alder.preview.useRails`      | `false` | Render the preview through the app's `bin/rails runner` (`ApplicationController.render(inline:, type:)`) when the file is inside a Rails app, so helpers, routes and partials work. Boots the app on every refresh and gives up after 60 seconds. |

## Ruby tools

| Setting                    | Default       | Description                                                                                                                                                                                                                                                                                               |
| -------------------------- | ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `alder.commands.slimrb`    | `"slimrb"`    | Executable used to render Slim (gem install slim).                                                                                                                                                                                                                                                        |
| `alder.commands.haml`      | `"haml"`      | Executable used to render Haml (gem install haml).                                                                                                                                                                                                                                                        |
| `alder.commands.html2haml` | `"html2haml"` | Executable for ERB → Haml (gem install html2haml).                                                                                                                                                                                                                                                        |
| `alder.commands.haml2slim` | `"haml2slim"` | Executable for Haml → Slim (gem install haml2slim).                                                                                                                                                                                                                                                       |
| `alder.commands.erb2slim`  | `"erb2slim"`  | Executable for ERB → Slim (gem install html2slim).                                                                                                                                                                                                                                                        |
| `alder.commands.ruby`      | `"ruby"`      | Ruby used to preview and compile ERB through Erubi (gem install erubi; stdlib ERB without it) and for Haml → ERB through the haml_to_erb library (gem install haml_to_erb). Also the Ruby that runs `bin/rails runner` for the routes; on Windows it must be a real `ruby.exe`, not a `.bat`/`.cmd` shim. |

These are the executables that preview, _Show Compiled Ruby Source_ and the converters call; see [Commands](/alder/guides/commands/#ruby-gems).

## Language server

| Setting                | Default | Description                                                                                                                    |
| ---------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `alder.server.enabled` | `true`  | Run the Rust language server (diagnostics, go to partial, completions, hover, and the built-in ERB, Haml and Slim formatting). |
| `alder.server.path`    | `""`    | Path to the alder-ls binary. Leave empty to use the bundled one.                                                               |
