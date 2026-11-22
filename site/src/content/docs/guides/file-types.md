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

`.skim` files use the Slim syntax and the `slim` settings. Jbuilder and Builder views stay Ruby files; they get partial navigation, and their checks are turned off with `alder.jbuilder.disabledDiagnostics`.

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

| Tag                                       | Where                                                                                                                                      |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `include` under a Jekyll site             | `_includes/<name>`; the site is the nearest folder above the file with a `_config.yml` (or `.yaml`, `.toml`)                               |
| `include_relative`                        | `<name>`, relative to the file                                                                                                             |
| `render` and `include` in a Shopify theme | `snippets/<name>.liquid`; the theme is the nearest folder above the file with `snippets/` and `layout/` (or `config/settings_schema.json`) |
| `render`, and `include` without Jekyll    | `_<name>.liquid` or `_<name>.html.liquid` in the file's folder, as the `liquid` gem looks it up; `shared/card` is `shared/_card.liquid`    |

### Checks

The language server reports, as errors, the tags that stop a template from rendering: a `{{` or `{%` that is never closed (`Syntax/LiquidUnclosedTag`; Jekyll front matter, `{% raw %}`, `{% comment %}` and `{% doc %}` are not read), a block tag (`if`, `unless`, `for`, `case`, `capture`, `comment`, `raw`, `tablerow`, `form`, `paginate`, `highlight`, also on the lines of a `{% liquid %}` tag) without its end tag (`Syntax/LiquidUnclosedBlock`), an end tag with nothing to close (`Syntax/LiquidStrayEndTag`) and an end tag for another block (`Syntax/LiquidMismatchedEndTag`). `{% comment %}` and `{% raw %}` are not read as tags, and tags you register yourself are left alone. `alder check` reports them too, and `alder.liquid.disabledDiagnostics` turns them off. See [Diagnostics](/alder/reference/diagnostics/).

If Shopify's Liquid extension also contributes `liquid`, disable one of the two for the workspace: VS Code uses one grammar per language id.

## Pug

`.pug` and `.jade` files, including `pug-rails`' `.jst.pug` templates (compiled by Sprockets into JST functions, usually under `app/assets/templates`), keep VS Code's own `jade` language: its highlighting, comment toggling, folding and Emmet. The extension adds Pug snippets, syntax errors, the HTML rules, an outline, navigation through `include` and `extends`, whitespace-only formatting and the editing commands. Pug code is JavaScript, so none of the Rails lookups or checks apply to it.

```pug title="app/assets/templates/users/list.jst.pug"
ul.users
  each user in users
    li(class={admin: user.admin})= user.name
```

### Checks

The language server reports the first error Pug 3.0.4, the current release, raises (what `pug-ruby`'s default system compiler runs with the `pug` npm package installed, so what `pug-rails` compiles with), with Pug's message, at Pug's line and column: inconsistent indentation (`Syntax/PugInconsistentIndent`), a bracket never closed or closed by the wrong one (`Syntax/PugUnclosedBracket`), an invalid `#id` or `.class` (`Syntax/PugInvalidName`), an attribute given twice (`Syntax/PugDuplicateAttribute`), a keyword such as `each` or `include` without what it needs (`Syntax/PugMalformedKeyword`; Pug reads `each … of` before `each … in`, so it also refuses `each offer in offers`, where ` of` starts `offers`), lines nested under buffered code or a raw include (`Syntax/PugIllegalNesting`), and any other Pug error (`Syntax/PugTemplate`). Pug stops at its first error, and so does the check. The JavaScript in `- code`, `#{}` and attribute values is not checked.

In a template without a syntax error, the files it includes and extends are checked too (see [Include and extends](#include-and-extends) for how paths resolve): a file that doesn't exist (`Syntax/PugMissingFile`), an `extends` after another statement or a second one (`Syntax/PugExtendsNotFirst`; blank lines and `//-` comments may come first), a top-level `block` of an extending template, or of a file it includes at its top level, that its layout doesn't have once linked as Pug links it (`Syntax/PugUnexpectedBlock`), and, as a warning, a `+call` of a mixin that neither the template, the files it includes nor its layouts define (`Syntax/PugUnknownMixin`). The last two are left out when a file on the way is missing or doesn't parse, and the mixin check also for a template that another Pug file in the app includes or extends, since it may call that file's mixins. These follow the files on disk and run again when a Pug file is saved, created or deleted.

The HTML rules that need no Ruby run on the tags of a template Pug accepts: `Html/ImgAlt` (with the quick fix that adds `alt=""`), `Html/DuplicateId`, `Html/AnchorHref`, `Html/NestedInteractive`, `A11y/IframeTitle`, `A11y/GenericLinkText`, and `Layout/LineLength` when turned on. `Html/VoidChildren` is an error here, with Pug's message: Pug refuses to compile a void element such as `img` that holds anything but blank text. Attribute values are JavaScript, so only a string literal (`id="main"`, `href='/users'`) is a value the rules read; any other value counts as set to something unknown, and `&attributes(...)` may set any attribute, so the rules about a missing one stay silent. Tags written inside text as `#[a(href=x) here]` count; a `#{}` tag name, a `+mixin` call, the content of `script.` and other text blocks, and raw HTML lines (`<img src="x">`) are not read. A `doctype xml` template isn't HTML, so none of these run on it. `Html/DuplicateAttribute` doesn't run on Pug, since Pug refuses a repeated attribute itself (`Syntax/PugDuplicateAttribute`), and the `Rails/*`, `Turbo/*` and `Stimulus/*` rules stay off.

`alder check` reports all of these too, and `alder.pug.disabledDiagnostics` turns them off, as do the rule tables of a `.alder.toml`. See [Diagnostics](/alder/reference/diagnostics/).

### Include and extends

Go to Definition, Ctrl/Cmd+click links and hover work on the path after `include`, `include:filter` and `extends`, and completion offers the folders and files of the folder typed so far. Under `pug-rails`, `include` and `extends` compile only with `pug-ruby`'s system compiler (`Pug.use :system`, its default): the compilers the gem ships, picked with `Pug.use "2.0.3"`, can't read files. Paths resolve as Pug resolves them:

- `.pug` is added to a path without an extension: `include row` reads `row.pug`. A `pug-rails` template named `row.jst.pug` has to be included as `include row.jst.pug`, and completion offers it that way. A path with another extension (`include notes.md`) is read as it is.
- A relative path starts from the template's folder.
- An absolute path (`extends /layout`) starts from `app/assets/templates`, or `app/assets/javascripts` when that doesn't exist, standing in for the `basedir` option Pug needs for it (`pug-rails` sets none; set `config.pug.basedir` to the same folder). Without either folder, an absolute path isn't followed.

```pug title="app/assets/templates/users/show.jst.pug"
extends /layout
block content
  include row.jst.pug
  include:markdown-it notes.md
```

### Formatting

_Format Document_, format on save and `alder format` re-indent a Pug template one level per nesting, with the editor's tab size, in spaces or tabs; text blocks (`script.`, `style.`, comments, filters, `-` code blocks) move with their line and keep their own indentation; trailing whitespace goes where Pug doesn't output it, and the file ends with one line break. A template Pug would reject, or one whose Pug tree would change, is left as it is. _Format Selection_ is not offered. `alder.pug.format.enabled` (or `format.enabled` under `[pug]` in a `.alder.toml`) turns it off. See [Formatting](/alder/guides/formatting/#pug).

### Editing commands

_Toggle Pug Line_ (<kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>E</kbd>), _Wrap Selection in Pug Output_ (<kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>W</kbd>) and _Extract Selection to Partial_, which writes an `include`, work as they do for Slim; see [Commands](/alder/guides/commands/#pug).

### Outline

The outline lists tags with a `#id` or `.class` (`section#main.wide`, `.card`), a literal `id` (`form(id="signup")` is `form#signup`) or a landmark name (`header`, `nav`, `main`, `section`, `aside`, `footer`, `form`), `block`s and `mixin` definitions, nested by indentation. With a syntax error it shows the lines above the error.
