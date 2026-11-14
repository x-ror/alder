---
title: File types
description: Which language id each template file gets, and what comment toggling, highlighting, checks and formatting it has.
sidebar:
  order: 5
---

The file name picks the language id, which decides the comment syntax and the `[erb-js]`-style editor settings; the language server picks the checks by the same name.

## ERB and Liquid files

| Files                                                                                                                                               | Language id                      | Comment toggle  | Highlighting      | Extra checks     | Formatting |
| --------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- | --------------- | ----------------- | ---------------- | ---------- |
| `.html.erb`, `.html+phone.erb`, `.turbo_stream.erb`, `.rhtml`, `.html.erubi`, `.html.erubis`, bare `.erb` under `app/views` with a locale or markup | `erb` (also `.erubi`, `.erubis`) | `<%# %>`        | HTML + Ruby       | HTML tags, links | yes        |
| `.js.erb`                                                                                                                                           | `erb-js`                         | `//`, `/* */`   | JavaScript + Ruby | none             | no         |
| `.scss.erb`                                                                                                                                         | `erb-scss`                       | `//`, `/* */`   | SCSS + Ruby       | none             | no         |
| `.css.erb`                                                                                                                                          | `erb-css`                        | `/* */`         | CSS + Ruby        | none             | no         |
| `.json.erb`                                                                                                                                         | `erb-json`                       | `<%# %>`        | JSON + Ruby       | none             | no         |
| `.text.erb`, `.txt.erb`                                                                                                                             | `erb-text`                       | `<%# %>`        | Text + Ruby       | none             | no         |
| Any file you assign to `erb-percent` (see [`erb-percent`](#erb-percent))                                                                            | `erb-percent`                    | `<%# %>`        | Text + Ruby       | none             | no         |
| `.xml.erb`, `.rss.erb`, `.atom.erb`, `.svg.erb`                                                                                                     | `erb-xml`                        | `<!-- -->`      | XML + Ruby        | none             | no         |
| `.md.erb`                                                                                                                                           | `erb-markdown`                   | `<!-- -->`      | Markdown + Ruby   | none             | no         |
| `.rb.erb`                                                                                                                                           | `erb-ruby`                       | `#`             | Ruby              | none             | no         |
| `.yml.erb`, `.yaml.erb`                                                                                                                             | `erb-yaml`                       | `#`             | YAML + Ruby       | none             | no         |
| `.liquid`, `.html.liquid`                                                                                                                           | `liquid`                         | `{% comment %}` | HTML + Liquid     | `Syntax/Liquid*` | no         |

Every ERB row gets the ERB and Ruby checks and, under `app/views`, the `render` checks (`unresolved-render`, `strict-locals`). Only HTML hosts add the HTML ones (unclosed or unbalanced tags, missing `alt`, duplicate `id`), linked tag renaming and attribute completion. Any other bare `.erb` (`nginx.conf.erb`) gets the ERB and Ruby checks only.

Use the language id to scope editor settings to one file type:

```json
{
  "[erb-js]": { "editor.tabSize": 2 }
}
```

## Haml, Slim, Skim, Jbuilder and Builder

| Files                   | Language id |
| ----------------------- | ----------- |
| `.haml`, `.html.haml`   | `haml`      |
| `.slim`, `.html.slim`   | `slim`      |
| `.skim`                 | `skim`      |
| `.jbuilder`, `.builder` | `ruby`      |

`.skim` files use the Slim syntax and the `slim` settings. Jbuilder and Builder views stay Ruby files; they get partial navigation, and their checks are turned off with `rubyTemplates.jbuilder.disabledDiagnostics`.

## Comment toggling

The comment column of the table applies to a line that starts outside a tag. On lines strictly inside a multi-line `<% %>` or `<%= %>` tag (not its `<%` or `%>` line), <kbd>Ctrl</kbd>+<kbd>/</kbd> (<kbd>Cmd</kbd>+<kbd>/</kbd> on macOS) toggles a Ruby `# ` instead; on the `<%` line it comments as usual, and _Toggle ERB Tag_ comments a whole tag.

```erb
<%
  title = @user.name
  subtitle = @user.email
%>
```

On the `subtitle` line, <kbd>Ctrl</kbd>+<kbd>/</kbd> gives `# subtitle = @user.email`, keeping the indentation; on the `<%` line it comments as it would outside a tag.

## `erb-percent`

`erb-percent` is for plain-Ruby ERB with `trim_mode: "%"` (RDoc-style templates, some gems): a line starting with `%` is Ruby and `%%` is a literal `%`. Rails' Erubi has no such lines (`% complete` in a view is text), so no file gets it by default. Opt in with `files.associations`:

```json
{
  "files.associations": { "lib/**/*.erb": "erb-percent" }
}
```

It gets the ERB and Ruby checks with every `%` line compiled as Ruby, so `% if x` … `<% else %>` … `% end` is one block and a syntax error in a `%` line is reported on it. `%` lines get diagnostics and the preview (stdlib ERB, `trim_mode: "%-"`) only: no completion or definition inside them, no formatting, no HTML checks. The command line checker stays Erubi-only.

```erb-percent title="lib/reports/summary.erb"
% if @items.any?
Items: <%= @items.size %>
<% else %>
%% Nothing to report.
% end
```

## Liquid

`liquid` (Shopify themes, Jekyll, the `liquid` gem) has highlighting, comment toggling, bracket and `{{ }}` / `{% %}` auto-closing, indentation, navigation to the files `render` and `include` name, and a check of its tags. None of the Rails lookups or checks apply to it, and it is not formatted.

```liquid title="_includes/card.html.liquid"
{% if page.author %}
  <p class="byline">{{ page.author | upcase }}</p>
{% endif %}
```

Plain `.html` files are not claimed, so Jekyll includes need an association:

```json
{
  "files.associations": { "**/_includes/**/*.html": "liquid" }
}
```

With the association, an include opens as Liquid, with its own navigation and checks.

### Navigation and completion

Go to Definition, Ctrl/Cmd+click links and hover work on the name in `{% render 'card' %}`, `{% include 'card' %}`, `{% include_relative 'card.html' %}` and Jekyll's unquoted `{% include footer.html %}`. Completion offers the names inside the quotes. A dynamic name (`{{ var }}`, a variable) is left alone. Where the name is looked up:

| Tag                                    | Where                                                                                                                                   |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `include` under a Jekyll site          | `_includes/<name>`; the site is the nearest folder above the file with a `_config.yml`                                                  |
| `include_relative`                     | `<name>`, relative to the file                                                                                                          |
| `render`, and `include` without Jekyll | `_<name>.liquid` or `_<name>.html.liquid` in the file's folder, as the `liquid` gem looks it up; `shared/card` is `shared/_card.liquid` |

### Checks

The language server reports, as errors, the tags that stop a template from rendering: a `{{` or `{%` that is never closed (`Syntax/LiquidUnclosedTag`), a block tag (`if`, `unless`, `for`, `case`, `capture`, `comment`, `raw`, `tablerow`, `form`, `paginate`, `highlight`, also on the lines of a `{% liquid %}` tag) without its end tag (`Syntax/LiquidUnclosedBlock`), an end tag with nothing to close (`Syntax/LiquidStrayEndTag`) and an end tag for another block (`Syntax/LiquidMismatchedEndTag`). `{% comment %}` and `{% raw %}` are not read as tags, and tags you register yourself are left alone. `ruby-templates check` reports them too, and `rubyTemplates.liquid.disabledDiagnostics` turns them off. See [Diagnostics](/vscode-ruby-templates/reference/diagnostics/).

If Shopify's Liquid extension also contributes `liquid`, disable one of the two for the workspace: VS Code uses one grammar per language id.
