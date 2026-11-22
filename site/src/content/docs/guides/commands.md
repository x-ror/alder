---
title: Commands
description: The Alder commands, their keyboard shortcuts, and which ones need Ruby gems.
sidebar:
  order: 7
---

Run any command from the Command Palette (<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd>, or <kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd> on macOS); they are listed under _Alder_.

## Editing commands

| Command                                             | Keys (Windows, Linux)                       | Keys (macOS)                               | What it does                                                                                                     |
| --------------------------------------------------- | ------------------------------------------- | ------------------------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| Toggle ERB Tag / Haml Prefix / Slim Line / Pug Line | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>E</kbd> | <kbd>Cmd</kbd>+<kbd>Alt</kbd>+<kbd>E</kbd> | `<% %>` → `<%= %>` → `<%# %>`, and the same for `-` / `=` / comment in Haml, Slim and Pug                        |
| Wrap Selection in ERB / Slim / Pug Output           | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>W</kbd> | <kbd>Cmd</kbd>+<kbd>Alt</kbd>+<kbd>W</kbd> | `foo` → `<%= foo %>`, `= foo` or `#{foo}`                                                                        |
| Extract Selection to Partial                        | none                                        | none                                       | Moves the selection into `_name.html.erb` (or Haml, Slim) and leaves a `render`; in Pug, a file and an `include` |
| Switch to Related File                              | none                                        | none                                       | View ↔ other formats ↔ controller action; ViewComponent class ↔ template ↔ preview                               |

In ERB, <kbd>Ctrl</kbd>+<kbd>/</kbd> (<kbd>Cmd</kbd>+<kbd>/</kbd> on macOS) is bound to the extension's line comment toggle, which follows the file's host language; see [Comment toggling](/alder/guides/file-types/#comment-toggling).

### Toggle a tag

_Toggle ERB Tag_ cycles the tag under the cursor:

```erb
<% user.name %>
<%= user.name %>
<%# user.name %>
```

In Haml it cycles the line prefix (`-`, `=`, `-#`), in Slim the line indicator (`-`, `=`, `==`, `/`), and in Pug the line's marker (`-`, `=`, `!=`, `//-`); see [Pug](#pug).

### Extract a partial

_Extract Selection to Partial_ works the same in ERB, Haml and Slim: the new partial and the `render` call are one edit, so a single undo reverts both. The name must be a Ruby identifier, optionally prefixed by folders (`shared/card`), and the render path is qualified (`render "users/card"`). An existing partial is overwritten only after you confirm.

### Pug

The three editing commands work on Pug (`.pug`, `.jade`, `.jst.pug`) as they do on Slim, kept to what Pug compiles:

- _Toggle Pug Line_ cycles `-` → `=` → `!=` → `//-` → `-` on each selected line. The step to `=` turns code that runs into code whose value is written to the page: `- x++` becomes `= x++`, which still increments `x` and now also writes its value. Code that can't be a JavaScript expression (`- var x = 1`, `- if (a) {`) goes from `-` straight to `//-`, since Pug refuses it after `=`. A `//-` comment goes back to `-` only when it reads as code, an expression or a statement such as `if (a)` or `var x = 1`; a comment in words (`//- TODO fix this`) has nothing to toggle to and stays. A line with lines nested under it is left alone (Pug refuses a block under `=`, and a `//-` comment's nested lines are its text), and so are lines inside `script.`, filters, comments, `-` code blocks and attribute lists over several lines.
- _Wrap Selection in Pug Output_ turns a whole line into `= line` when it has nothing nested and reads as a JavaScript expression (`items.length > 0`, `user.name + "!"`), and a selection in text (after a tag, after `|`, inside `p.` or under a `//` comment, whose nested lines Pug outputs with their `#{}`) into `#{selection}`. A line Pug reads as a tag is left alone, even when it would be an expression too: `user.name` is the tag `user` with the class `name`, and `t('x')` the tag `t` with an attribute. A selection in a tag's name or attributes, inside another `#{}` or `!{}` or in the tag of a `#[]`, on a keyword line, in code, an attribute list, a `//-` comment or a comment's own line, or one that can't be an expression (`Hello world`) is left as it is, with a message. An empty selection inserts `#{}` to type in. Whether text is a JavaScript expression is a guess, as no JavaScript parser runs: brackets and quotes must close, and two words may not stand side by side.
- _Extract Selection to Partial_ moves the selected lines, with every line nested under them (so `script.` keeps its text) and the rest of an attribute list they open, into a new file in the template's folder (`row`, or `users/row` for a subfolder), with the template's extensions: `row.jst.pug` from `list.jst.pug`, `row.pug` from `list.pug`. The lines move to the left edge, keeping their indentation relative to each other, and an `include` replaces them, with the path Pug reads: `include row.jst.pug`, or `include row` for a `.pug` file, since Pug adds `.pug` to a path without an extension. There are no `_` names or locals: an included file shares the template's variables. It refuses a selection that starts inside a text block, inside an attribute list or deeper than it ends, holds `extends`, holds a mixin's `block` without its `mixin`, or would part an `if`, `case` or `each` from its `else`, `when` or `default`.

## Preview and compiled source

| Command                   | Keys (Windows, Linux)                       | Keys (macOS)                               | What it does                                                            |
| ------------------------- | ------------------------------------------- | ------------------------------------------ | ----------------------------------------------------------------------- |
| Preview Rendered HTML     | <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>P</kbd> | <kbd>Cmd</kbd>+<kbd>Alt</kbd>+<kbd>P</kbd> | Renders the template in a side panel, optionally through your Rails app |
| Show Compiled Ruby Source | none                                        | none                                       | The Ruby the template engine generates, for debugging                   |

The preview is controlled by three settings:

- `alder.preview.useRails` renders through the app's `bin/rails runner` when the file is inside a Rails app, so helpers, routes and partials work. It boots the app on every refresh and gives up after 60 seconds.
- `alder.preview.useBundler` prefixes render commands with `bundle exec` when a Gemfile exists in the workspace.
- `alder.preview.refreshOnSave` re-renders the preview whenever the template is saved.

`alder.slim.compileForRails` makes _Show Compiled Ruby Source_ for Slim run `slimrb --rails`, as Rails compiles it.

## Converters

| Command                 | What it does                      |
| ----------------------- | --------------------------------- |
| Convert ERB/HTML → Haml | Converts the current file to Haml |
| Convert Haml/Slim → ERB | Converts the current file to ERB  |
| Convert ERB/Haml → Slim | Converts the current file to Slim |

## Language server

_Restart Language Server_ restarts the language server. Its log is the _Alder_ channel in the Output panel.

## Ruby gems

Preview and the converters call Ruby gems, installed only if you use them:

| Gem           | Used for                                                           | Setting for the executable |
| ------------- | ------------------------------------------------------------------ | -------------------------- |
| `slim`        | Rendering Slim                                                     | `alder.commands.slimrb`    |
| `haml`        | Rendering Haml                                                     | `alder.commands.haml`      |
| `erubi`       | Previewing and compiling ERB through Erubi (stdlib ERB without it) | `alder.commands.ruby`      |
| `haml_to_erb` | Haml → ERB                                                         | `alder.commands.ruby`      |
| `html2haml`   | ERB → Haml                                                         | `alder.commands.html2haml` |
| `html2slim`   | ERB → Slim                                                         | `alder.commands.erb2slim`  |
| `haml2slim`   | Haml → Slim                                                        | `alder.commands.haml2slim` |

Everything else works without Ruby.
