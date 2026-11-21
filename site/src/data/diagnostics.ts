// Every diagnostic code (rule name) the language server reports, with a template that triggers it and the message
// `ruby-templates check` prints for that template.
import fs from "node:fs";
import path from "node:path";

export type Language = "erb" | "haml" | "slim" | "jbuilder" | "liquid" | "pug";

export const LANGUAGE_NAMES: Record<Language, string> = {
  erb: "ERB",
  haml: "Haml",
  slim: "Slim",
  jbuilder: "Jbuilder",
  liquid: "Liquid",
  pug: "Pug",
};

export interface Diagnostic {
  /** The rule name, `Department/Name`: the diagnostic's code. */
  code: string;
  languages: Language[];
  summary: string;
  /** When it fires; backticks quote code, as in the server's messages. */
  explanation: string;
  example: { file: string; lang: string; code: string };
  message: string;
  line: number;
  column: number;
}

/** The id of a code's section on the diagnostics page: `Html/ImgAlt` is `html-imgalt`. */
export const codeAnchor = (code: string) =>
  code.toLowerCase().replace("/", "-");

export const diagnostics: Diagnostic[] = [
  {
    code: "Syntax/UnclosedErbTag",
    languages: ["erb"],
    summary: "A `<%` with no `%>` after it.",
    explanation:
      "Erubi only compiles a tag it can close, so a `<%` without `%>` stays in the output as text, and the Ruby is printed instead of run. In HTML, `<` followed by `%` starts no tag (HTML 13.2.5.6). Reported only where the HTML tokenizer is between tags, not inside attribute values, comments or `<script>` and `<style>` bodies, and only in ERB with an HTML host (`.html.erb` and the like).",
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<p><%= @user.name %></p>
<p><%= @user.email</p>
`,
    },
    message:
      "invalid-first-character-of-tag-name: `<%` without `%>` is text, and `<` followed by `%` starts no tag (HTML 13.2.5.6)",
    line: 2,
    column: 4,
  },
  {
    code: "Html/UnexpectedCloseTag",
    languages: ["erb"],
    summary: "An end tag with no open element of that name.",
    explanation:
      'The end tag closes nothing, so the browser drops it. Also reported for the end tag of a void element ("End tag `</br>` for a void element") and for a `</p>` after a block element such as `<div>` already closed the paragraph. A stray end tag in a partial is not reported, since a partial may close what another one opened. ERB with an HTML host only; checking stops after 20 tag problems in one file.',
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<div class="profile">
  <p><%= @user.name %></p>
</div>
</div>
`,
    },
    message:
      "Unexpected end tag `</div>`: no open element of that name (HTML 13.2.6.4.7)",
    line: 4,
    column: 1,
  },
  {
    code: "Html/MismatchedCloseTag",
    languages: ["erb"],
    summary: "An end tag that doesn't close the innermost open element.",
    explanation:
      "The message names the element that is still open and its line. When the end tag crosses an open element (`<div><span></div>`), the browser could recover in more than one way, so checking stops there instead of guessing. Reported in partials too. ERB with an HTML host only.",
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<div class="profile">
  <span><%= @user.name %></div>
`,
    },
    message:
      "End tag `</div>` doesn't close the open `<span>` (line 2) (HTML 13.2.6.4.7)",
    line: 2,
    column: 26,
  },
  {
    code: "Html/UnclosedElement",
    languages: ["erb"],
    summary: "An element whose end tag never comes.",
    explanation:
      "Reported at the start tag when the file ends with the element still open. Elements whose end tag HTML lets you omit (`li`, `p`, `td`, `option`, …) and void elements are never reported. Partials are exempt, because a `_header` partial may open a `<div>` that `_footer` closes, and so is a file that ends inside an ERB block. ERB with an HTML host only.",
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<section class="users">
  <h2>Users</h2>
`,
    },
    message: "`<section>` is never closed (HTML 13.2.6.4.7)",
    line: 1,
    column: 1,
  },
  {
    code: "Html/ImgAlt",
    languages: ["erb", "haml", "slim", "pug"],
    summary: "An image without alt text.",
    explanation:
      'Fires for an `<img>` (`%img` in Haml, `img` in Slim and Pug) with no `alt` attribute and for an `image_tag` call whose options have no `alt:`; that one reads "`image_tag` without `alt:`". In ERB the call must be in an output tag. Not reported for an `<img>` with ERB where attributes go (`<img <%= attrs %>>`), a Haml `**attrs`, a Slim `*attrs` or a Pug `&attributes(…)`, an `image_tag` given a `**splat`, or `generator-unable-to-provide-required-alt`, since each may supply the text. An empty `alt=""` counts as alt text. Templates that output HTML only: not `.js.erb`, `.text.erb` or `.xml.haml`.',
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<img src="<%= @user.avatar_url %>">
`,
    },
    message:
      "`<img>` without `alt`: the alt attribute must be specified (HTML 4.8.4.4.1)",
    line: 1,
    column: 1,
  },
  {
    code: "Html/DuplicateId",
    languages: ["erb", "haml", "slim", "pug"],
    summary: "The same static id twice in one template.",
    explanation:
      "Reported at the second occurrence, naming the line of the first. Only literal values count: an id holding ERB or interpolation is left alone, and so is an element with both `#id` and `id:` in Haml (it gets one joined id). Two ids in different branches of a conditional don't clash, and content inside `<template>` isn't in the document yet. In Haml and Slim, ids are also left alone when one of them sits in a conditional, a loop or a block that the other is not in. Templates that output HTML only.",
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<div id="profile">
  <%= @user.name %>
</div>
<div id="profile"></div>
`,
    },
    message:
      "Duplicate id `profile` (first on line 1): the value must be unique amongst all the IDs in the element's tree (HTML 3.2.6)",
    line: 4,
    column: 10,
  },
  {
    code: "Html/DuplicateAttribute",
    languages: ["erb", "haml", "slim"],
    summary: "An attribute given twice on one tag.",
    explanation:
      "Reported at the second one, for names written out in ERB start tags, Haml tags (the `(…)` list and the `{…}` hash) and Slim tags, and in the literal options of Rails tag helpers. In ERB markup the page gets both, so the second is reported; in Haml and the helpers a repeated key renders once, so the value that is dropped is reported. `class` and `id` are left alone in Haml, Slim and the helpers, which merge them. An ERB start tag with ERB inside is left alone. Slim rejects a repeated attribute itself, which is reported as `Syntax/Template`. Templates that output HTML only.",
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<div class="card" class="wide">
  <%= @user.name %>
</div>
`,
    },
    message:
      "Duplicate attribute `class` on `<div>`: a start tag must not repeat an attribute (HTML 13.1.2.3)",
    line: 1,
    column: 19,
  },
  {
    code: "Html/AnchorHref",
    languages: ["erb", "haml", "slim", "pug"],
    summary:
      "An `<a>` without an `href`, which is not a link and cannot be reached with the keyboard.",
    explanation:
      "Reported on the tag name. `link_to` always has an `href` and is not checked. An `<a>` with a `name`, a `tabindex`, an `id` and no text of its own, or an attribute ending in `:href` (`:href`, `x-bind:href`), and one inside an `<svg>`, is left alone, as is one whose attributes may come from code that can't be read (ERB inside the start tag, `**attrs`, `*attrs`), is left alone. Templates that output HTML only.",
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<a class="btn" data-action="click->menu#open">Menu</a>
`,
    },
    message:
      "`<a>` without `href` is not a link and the keyboard can't reach it; use a `<button>` for an action",
    line: 1,
    column: 2,
  },
  {
    code: "Html/NestedInteractive",
    languages: ["erb", "haml", "slim", "pug"],
    summary:
      "An interactive element inside another: a link in a link, a form in a form.",
    explanation:
      "Reported on the inner element's tag name or helper name. A link or button (also `link_to`, `tag.a`, `button_tag` with a block) can't hold another link, a button, an `<input>` that isn't hidden, a `<select>`, `<textarea>`, `<label>`, `<iframe>`, `<details>`, controls of `<audio>` and `<video>`, or an `<img usemap>` (HTML 4.5.1); a form can't hold a form, and `button_to` renders one (HTML 4.10.3). Only nesting the template writes is reported: partials rendered inside the element, a `yield`, elements started and ended in different branches of an `if`, and anything in an `<svg>` or `<template>` are left alone, and so are a `<label>` around its `<input>` and a hidden input. Templates that output HTML only.",
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<%= link_to user_path(user) do %>
  <%= link_to "Edit", edit_user_path(user) %>
<% end %>
`,
    },
    message:
      "`<a>` inside `<a>`: interactive content can't be nested (HTML 4.5.1)",
    line: 2,
    column: 7,
  },
  {
    code: "A11y/IframeTitle",
    languages: ["erb", "haml", "slim", "pug"],
    summary:
      "An `<iframe>` without a `title`, which screen readers use to name it.",
    explanation:
      "Reported on the tag name, also for `tag.iframe` and `content_tag :iframe`. An `aria-label` or `aria-labelledby` (also in an `aria: { label: … }` hash) names the frame as well. Not reported where attributes may come from code that can't be read. Templates that output HTML only.",
    example: {
      file: "app/views/videos/show.html.erb",
      lang: "erb",
      code: `<iframe src="https://www.youtube.com/embed/<%= @video.youtube_id %>"></iframe>
`,
    },
    message:
      "`<iframe>` without `title`: screen readers announce it to name the frame (WCAG 4.1.2)",
    line: 1,
    column: 2,
  },
  {
    code: "A11y/GenericLinkText",
    languages: ["erb", "haml", "slim", "pug"],
    summary:
      'Link text that says nothing out of context: "click here", "here", "read more".',
    explanation:
      'Reported on the text, for an `<a>` that holds only text and for a `link_to`, `tag.a` or `content_tag :a` given a string literal. The texts are "click here", "here", "read more", "learn more", "more", "more info", "more information", "link", "this link" and "click", in any case and ignoring punctuation. A link with an `aria-label`, `aria-labelledby` or `aria-describedby`, or with ERB, interpolation or other elements inside, is left alone. Templates that output HTML only.',
    example: {
      file: "app/views/pricing/show.html.erb",
      lang: "erb",
      code: `<p>Our new pricing is live. <%= link_to "Click here", pricing_path %></p>
`,
    },
    message:
      "Link text `Click here` says nothing out of context; name where the link goes (WCAG 2.4.9, F84)",
    line: 1,
    column: 42,
  },
  {
    code: "Syntax/InconsistentIndent",
    languages: ["haml"],
    summary: "Indentation Haml can't follow.",
    explanation:
      'Haml\'s own indentation errors: a width other than the one the rest of the document uses, tabs mixed with spaces ("Indentation can\'t use both tabs and spaces."), or a line more than one level deeper than the one before ("The line was indented 2 levels deeper than the previous line."). Haml stops at its first error, so only that one is shown, and the Ruby isn\'t checked until it is fixed.',
    example: {
      file: "app/views/users/index.html.haml",
      lang: "haml",
      code: `%section
  %h2 Users
  %ul
   %li= @user.name
`,
    },
    message:
      "Inconsistent indentation: 3 spaces used for indentation, but the rest of the document was indented using 2 spaces.",
    line: 4,
    column: 1,
  },
  {
    code: "Syntax/TopLevelIndent",
    languages: ["haml"],
    summary: "The first line is indented.",
    explanation:
      "Haml measures every level from the first line, which must start at column 1.",
    example: {
      file: "app/views/users/show.html.haml",
      lang: "haml",
      code: `  %p= @user.name
`,
    },
    message: "Indenting at the beginning of the document is illegal.",
    line: 1,
    column: 1,
  },
  {
    code: "Syntax/IllegalNesting",
    languages: ["haml"],
    summary: "Lines nested under something that can't hold them.",
    explanation:
      'Haml rejects lines nested under a tag that already has content on its own line, under plain text, and under a self-closing tag such as `%br/`; each has its own "Illegal nesting" message. Nesting under `!!!` has its own code, `Syntax/DoctypeChildren`.',
    example: {
      file: "app/views/users/show.html.haml",
      lang: "haml",
      code: `%p= @user.name
  %span Admin
`,
    },
    message:
      "Illegal nesting: content can't be both given on the same line as %p and nested within it.",
    line: 2,
    column: 3,
  },
  {
    code: "Syntax/DoctypeChildren",
    languages: ["haml", "slim"],
    summary: "Lines nested under the doctype.",
    explanation:
      'The doctype line (`!!!` in Haml, `doctype` in Slim) takes no children; the `%html` or `html` that follows belongs at the same level. Slim reports it as "Unexpected indentation".',
    example: {
      file: "app/views/layouts/application.html.haml",
      lang: "haml",
      code: `!!! 5
  %html
`,
    },
    message: "Illegal nesting: nesting within a header command is illegal.",
    line: 2,
    column: 3,
  },
  {
    code: "Syntax/UnmatchedBranch",
    languages: ["haml"],
    summary:
      "An `else`, `elsif` or `when` that doesn't line up with its block.",
    explanation:
      'In Haml a branch keyword must stand at the indentation of the `- if` or `- case` it continues. One at another level is reported with the expected and actual levels; one with no open block before it reads "Got "else" with no preceding "if"". The first `when` may sit one level deeper than its `case`.',
    example: {
      file: "app/views/users/index.html.haml",
      lang: "haml",
      code: `- if @users.any?
  = render @users
  - else
    %p No users
`,
    },
    message: '"else" is indented at wrong level: expected 0, but was at 1.',
    line: 3,
    column: 3,
  },
  {
    code: "Syntax/ExplicitEnd",
    languages: ["haml", "slim"],
    summary: "A `- end` line.",
    explanation:
      'Haml and Slim close a Ruby block by un-indenting, and both refuse an explicit `- end`. Slim\'s message is shown here; Haml\'s starts with "You don\'t need to use "- end" in Haml." and goes on with an example. Skim templates, whose code is CoffeeScript, are not checked for it.',
    example: {
      file: "app/views/users/show.html.slim",
      lang: "slim",
      code: `- if @user.admin?
  p Admin
- end
`,
    },
    message: "Explicit end statements are forbidden",
    line: 3,
    column: 1,
  },
  {
    code: "Syntax/IllegalElement",
    languages: ["haml"],
    summary: "A `.` or `#` with no class or id name after it.",
    explanation:
      "Haml needs a name after each `.` and `#` in a tag (`%p.lead`, `#main`).",
    example: {
      file: "app/views/users/show.html.haml",
      lang: "haml",
      code: `%p. Hello
`,
    },
    message: "Illegal element: classes and ids must have values.",
    line: 1,
    column: 1,
  },
  {
    code: "Syntax/UnknownFilter",
    languages: ["haml"],
    summary: "A filter Haml doesn't have.",
    explanation:
      "The name after `:` isn't one of Haml's filters (`:javascript`, `:css`, `:markdown`, `:plain`, `:ruby`, …), usually a typo. Haml raises this when it compiles the template. A name that isn't a word at all is a `Syntax/Template` (\"Invalid filter name\").",
    example: {
      file: "app/views/users/show.html.haml",
      lang: "haml",
      code: `:mardown
  # Users
`,
    },
    message: "FilterCompiler for 'mardown' was not found",
    line: 1,
    column: 1,
  },
  {
    code: "Syntax/SelfClosingContent",
    languages: ["haml"],
    summary: "Content after a tag closed with `/`.",
    explanation:
      "A tag ending in `/` renders as `<br />` and can hold nothing on its own line.",
    example: {
      file: "app/views/users/show.html.haml",
      lang: "haml",
      code: `%br/ Hello
`,
    },
    message: "Self-closing tags can't have content.",
    line: 1,
    column: 1,
  },
  {
    code: "Syntax/UnclosedAttribute",
    languages: ["haml", "slim"],
    summary: "An attribute list or value that is never closed.",
    explanation:
      'Slim reports the bracket it expected ("Expected closing delimiter )", or `]` or `}`), or "Unexpected end of file" for a quote left open. Haml reports "Unbalanced brackets." for a `{` or `(` attribute list that doesn\'t close, and "Invalid attribute list" for one it can\'t read.',
    example: {
      file: "app/views/users/show.html.slim",
      lang: "slim",
      code: `a href=(user_path(@user) Profile
`,
    },
    message: "Expected closing delimiter )",
    line: 1,
    column: 33,
  },
  {
    code: "Syntax/Template",
    languages: ["haml", "slim"],
    summary: "Any other error the Haml or Slim parser raises.",
    explanation:
      'The parser errors that have no code of their own. In Haml: "Invalid tag", "Invalid filter name" and "There\'s no Ruby code for = to evaluate." In Slim: "Unknown line indicator", "Illegal shortcut", "Invalid empty attribute", "Expected attribute", "Expected tag" for a block expansion with nothing after it (`ul:`), "Unexpected text after closed tag", "Multiple id attributes specified" (for any repeated attribute) and "Invalid doctype".',
    example: {
      file: "app/views/users/show.html.haml",
      lang: "haml",
      code: `%p=
`,
    },
    message: "There's no Ruby code for = to evaluate.",
    line: 1,
    column: 1,
  },
  {
    code: "Html/VoidChildren",
    languages: ["haml", "slim", "pug"],
    summary: "Content inside a void element such as `img` or `input`.",
    explanation:
      "Void elements have no end tag, so the content would end up after them. Haml reports content on the tag's line or nested under it. Slim reports lines nested under a void element, and \"Unexpected indentation\" under a tag closed with `/`. In Slim it applies to HTML templates only, not to `.xml.slim` and other formats. Pug refuses to compile a void element, or a tag closed with `/`, that holds anything but blank text; that is reported as an error with Pug's own message, at the first such tag. A tag closed with `/` counts whatever its name, `#{…}` too; a void element doesn't after a `doctype xml`, until a `doctype` with another name.",
    example: {
      file: "app/views/users/show.html.slim",
      lang: "slim",
      code: `img src="/logo.png"
  | Logo
`,
    },
    message: "`img` is a void element and can't have children",
    line: 2,
    column: 3,
  },
  {
    code: "Syntax/BadDedent",
    languages: ["slim"],
    summary: "A line un-indented to a level no open line has.",
    explanation:
      "When a line moves left, Slim needs it to land on the indentation of a line still open above it. Here `li Two` stops between `ul` and `li One`.",
    example: {
      file: "app/views/users/index.html.slim",
      lang: "slim",
      code: `section
  ul
    li One
   li Two
`,
    },
    message: "Malformed indentation",
    line: 4,
    column: 4,
  },
  {
    code: "Syntax/UnknownEngine",
    languages: ["slim"],
    summary: "An embedded engine Slim doesn't know.",
    explanation:
      'A lone `name:` line opens an embedded engine. Slim\'s own are `ruby`, `javascript`, `css`, `sass`, `scss`, `coffee`, `markdown`, `textile`, `rdoc` and `asciidoc`; for any other Slim raises "Expected tag". Engines your app registers (such as `mjml`) go in `rubyTemplates.slim.extraEngines`.',
    example: {
      file: "app/views/users/show.html.slim",
      lang: "slim",
      code: `markdwn:
  # Users
`,
    },
    message: "Expected tag",
    line: 1,
    column: 9,
  },
  {
    code: "Syntax/LiquidUnclosedTag",
    languages: ["liquid"],
    summary: "A `{{` or `{%` with no `}}` or `%}` after it.",
    explanation:
      "Liquid reads the rest of the template as part of the output or tag, so it fails to render (\"Variable '{{' was not properly terminated\", \"Tag '{%' was not properly terminated\"). Checking stops at the first one, since what follows is inside it. Not reported inside `{% raw %}` and `{% comment %}`, whose content is not parsed.",
    example: {
      file: "_includes/card.html",
      lang: "liquid",
      code: `<h2>{{ page.title</h2>
<p>{{ page.excerpt</p>
`,
    },
    message: "`{{` is never closed",
    line: 1,
    column: 5,
  },
  {
    code: "Syntax/LiquidUnclosedBlock",
    languages: ["liquid"],
    summary: "A block tag such as `if` or `for` with no end tag.",
    explanation:
      "Liquid raises \"'if' tag was never closed\" and renders nothing. Reported on the opening tag, for `if`, `unless`, `for`, `case`, `capture`, `comment`, `raw`, `tablerow`, `form`, `paginate` and `highlight`, also inside a multi-line `{% liquid %}` tag. A block left open by an end tag for an outer block is reported here too. Tags the app registers itself are not checked.",
    example: {
      file: "_includes/card.html",
      lang: "liquid",
      code: `{% if page.author %}
  <p>{{ page.author }}</p>
<p>{{ page.date }}</p>
`,
    },
    message: "`if` is never closed; add `endif`",
    line: 1,
    column: 1,
  },
  {
    code: "Syntax/LiquidStrayEndTag",
    languages: ["liquid"],
    summary: "An end tag with no open block to close.",
    explanation:
      "Liquid raises \"Unknown tag 'endif'\": the tag closes nothing. Often the opening tag was deleted or sits in another file; Liquid reads each template on its own.",
    example: {
      file: "_includes/card.html",
      lang: "liquid",
      code: `<p>{{ page.author }}</p>
{% endif %}
`,
    },
    message: "`endif` has no open `if` tag to close",
    line: 2,
    column: 1,
  },
  {
    code: "Syntax/LiquidMismatchedEndTag",
    languages: ["liquid"],
    summary: "An end tag that doesn't match the innermost open block.",
    explanation:
      "Liquid raises \"'endfor' is not a valid delimiter for if tags. use endif\". When the end tag belongs to an outer block, the blocks inside it are reported as `Syntax/LiquidUnclosedBlock` instead; otherwise the end tag is taken for a typo and closes the innermost block.",
    example: {
      file: "_includes/card.html",
      lang: "liquid",
      code: `{% if page.tags %}
  <p>{{ page.tags }}</p>
{% endfor %}
`,
    },
    message: "`endfor` doesn't close the open `if` tag; Liquid expects `endif`",
    line: 3,
    column: 1,
  },
  {
    code: "Syntax/PugInconsistentIndent",
    languages: ["pug"],
    summary: "Indentation Pug can't follow, or tabs and spaces mixed.",
    explanation:
      'Pug raises `PUG:INCONSISTENT_INDENTATION` for a line un-indented to a level no open line has, and `PUG:INVALID_INDENTATION` ("Invalid indentation, you can use tabs or spaces but not both") for a line indented with the other character than the lines before it. The template doesn\'t compile. Like every Pug error, only the first one is reported, at the line and column Pug gives: Pug stops there.',
    example: {
      file: "app/assets/templates/users/list.jst.pug",
      lang: "pug",
      code: `ul.users
    li= user.name
  li= user.email
`,
    },
    message: "Inconsistent indentation. Expecting either 0 or 4 spaces/tabs.",
    line: 3,
    column: 1,
  },
  {
    code: "Syntax/PugUnclosedBracket",
    languages: ["pug"],
    summary:
      "An attribute list, interpolation or expression whose bracket is never closed, or closed by the wrong one.",
    explanation:
      "Pug raises `PUG:NO_END_BRACKET` when an attribute list `(…)`, the arguments of a mixin call or of `&attributes(…)`, or a `#{…}` or `#[…]` interpolation runs to the end of the line or the file, and `PUG:BRACKET_MISMATCH` when a bracket inside it is closed by another kind. The JavaScript inside is not otherwise checked.",
    example: {
      file: "app/assets/templates/users/nav.jst.pug",
      lang: "pug",
      code: `li(class={active: user.active) Profile
`,
    },
    message: "Mismatched Bracket: )",
    line: 1,
    column: 30,
  },
  {
    code: "Syntax/PugInvalidName",
    languages: ["pug"],
    summary: "A `#id` or `.class` that isn't a valid name.",
    explanation:
      'Pug raises `PUG:INVALID_ID` or `PUG:INVALID_CLASS_NAME`: an id may hold letters, digits, `_` and `-`, and a class name the same with at least one letter or `_`. Use an attribute for any other value: `div(class="6")`.',
    example: {
      file: "app/assets/templates/users/row.jst.pug",
      lang: "pug",
      code: `.grid
  .col.6 Name
`,
    },
    message: "Class names must contain at least one letter or underscore.",
    line: 2,
    column: 7,
  },
  {
    code: "Syntax/PugDuplicateAttribute",
    languages: ["pug"],
    summary: "An attribute given twice on one tag, `id` included.",
    explanation:
      "Pug raises `PUG:DUPLICATE_ATTRIBUTE`, or `PUG:DUPLICATE_ID` for a `#id` shorthand after an `id` attribute; the template doesn't compile. `class` is the exception: Pug joins several classes.",
    example: {
      file: "app/assets/templates/users/form.jst.pug",
      lang: "pug",
      code: `input#email(type="email" id="user-email")
`,
    },
    message: 'Duplicate attribute "id" is not allowed.',
    line: 1,
    column: 26,
  },
  {
    code: "Syntax/PugMalformedKeyword",
    languages: ["pug"],
    summary:
      "A keyword such as `each`, `case` or `include` without the expression or path it needs, or with one it can't take.",
    explanation:
      "Pug raises `PUG:MALFORMED_EACH` (`each user from users`, or `- each` with a dash), `PUG:MALFORMED_EACH_OF_LVAL` (an `each … of` whose variable is neither a name nor a `[key, value]` pair; Pug tries `each … of` first, so `each offer in offers` is one, as ` of` starts `offers`), `PUG:NO_CASE_EXPRESSION`, `PUG:NO_WHEN_EXPRESSION`, `PUG:NO_WHILE_EXPRESSION`, `PUG:NO_INCLUDE_PATH`, `PUG:NO_EXTENDS_PATH`, `PUG:MALFORMED_INCLUDE`, `PUG:MALFORMED_EXTENDS`, `PUG:DEFAULT_WITH_EXPRESSION` and `PUG:ELSE_CONDITION` (`else x`, where `else if x` was meant).",
    example: {
      file: "app/assets/templates/users/list.jst.pug",
      lang: "pug",
      code: `each user from users
  li= user.name
`,
    },
    message:
      "This `each` has a syntax error. `each` statements should be of the form: `each VARIABLE_NAME of JS_EXPRESSION`",
    line: 1,
    column: 5,
  },
  {
    code: "Syntax/PugIllegalNesting",
    languages: ["pug"],
    summary:
      "Lines nested under buffered code or a raw include, which can't hold them.",
    explanation:
      "Pug raises `PUG:BLOCK_IN_BUFFERED_CODE` for lines indented under a `= code` or `!= code` line, and `PUG:RAW_INCLUDE_BLOCK` for lines under an `include` of a file that isn't Pug. Unbuffered `- code` may hold a block.",
    example: {
      file: "app/assets/templates/users/list.jst.pug",
      lang: "pug",
      code: `ul
  = user.name
    li admin
`,
    },
    message: "Buffered code cannot have a block attached to it",
    line: 3,
    column: 1,
  },
  {
    code: "Syntax/PugTemplate",
    languages: ["pug"],
    summary: "Any other error the Pug parser raises.",
    explanation:
      "Pug's lexer and parser errors that belong to no other code, such as `PUG:UNEXPECTED_TEXT`, `PUG:INVALID_TOKEN`, `PUG:MIXIN_WITHOUT_BODY` and `PUG:BLOCK_OUTISDE_MIXIN` (an anonymous `block` outside a mixin), with Pug's message. A few templates make Pug itself throw a JavaScript error (a `#` with nothing after it, a lone `.` under a tag); that error's message is reported. The JavaScript in `- code`, `#{}` and attribute values is not checked, so Pug's `PUG:SYNTAX_ERROR` is never reported.",
    example: {
      file: "app/assets/templates/users/badge.jst.pug",
      lang: "pug",
      code: `mixin badge
p Hello
`,
    },
    message: "Mixin badge declared without body",
    line: 1,
    column: 1,
  },
  {
    code: "Syntax/PugMissingFile",
    languages: ["pug"],
    summary: "An `include` or `extends` of a file that doesn't exist.",
    explanation:
      "Pug reads the file when it compiles the template and fails when it isn't there. A path without an extension gets `.pug`, so `include row` never finds pug-rails' `row.jst.pug`: write `include row.jst.pug`. A relative path starts from the template's folder; an absolute one (`/shared/nav`) from `app/assets/templates`, else `app/assets/javascripts`, standing in for the `basedir` option, and isn't checked when neither exists. Here `users/row.jst.pug` and `layout.pug` exist.",
    example: {
      file: "app/assets/templates/users/show.jst.pug",
      lang: "pug",
      code: `extends ../layout
block content
  include row
`,
    },
    message: "Missing file app/assets/templates/users/row.pug",
    line: 3,
    column: 11,
  },
  {
    code: "Syntax/PugExtendsNotFirst",
    languages: ["pug"],
    summary:
      "An `extends` that isn't the template's first statement, or a second one.",
    explanation:
      "pug-linker raises `PUG:EXTENDS_NOT_FIRST` with this message. Blank lines and `//-` comments may come before `extends`, since Pug drops them; a `doctype`, a `//` comment (it is output) or a `mixin` may not. The layout's `doctype` belongs in the layout.",
    example: {
      file: "app/assets/templates/users/show.jst.pug",
      lang: "pug",
      code: `doctype html
extends ../layout
`,
    },
    message:
      'Declaration of template inheritance ("extends") should be the first thing in the file. There can only be one extends statement per file.',
    line: 2,
    column: 1,
  },
  {
    code: "Syntax/PugUnexpectedBlock",
    languages: ["pug"],
    summary: "A `block` of an extending template that its layouts don't have.",
    explanation:
      "pug-linker raises `PUG:UNEXPECTED_BLOCK` for a block at the top level of an extending template, `append` and `prepend` included, whose name isn't in its layout once linked: no layout up the `extends` chain has it (with the files they include), a layout that replaces a block drops the blocks that block held, and a block the template nests in another counts only inside one the layout has. Blocks of a file the template includes at its top level count as its own and are reported on the `include` line. Not reported when a file on the way is missing, has a syntax error or includes itself. Here `layout.pug` has `block content` only.",
    example: {
      file: "app/assets/templates/users/index.jst.pug",
      lang: "pug",
      code: `extends ../layout
block content
  ul.users
block sidebar
  p Filters
`,
    },
    message: "Unexpected block sidebar",
    line: 4,
    column: 1,
  },
  {
    code: "Syntax/PugUnknownMixin",
    languages: ["pug"],
    summary:
      "A `+mixin` call with no definition in the template, the files it includes or its layouts.",
    explanation:
      "Pug compiles the call and fails when the template renders (`pug_mixins.usr is not a function`). A template that another one includes or extends may call that one's mixins, so the call is only reported in a template inside an `app/` folder that no Pug file in it pulls in, and when every file it includes and extends is there and parses. A `+#{name}` call is never reported. Here `mixins.pug` defines `mixin user(user)`.",
    example: {
      file: "app/assets/templates/users/list.jst.pug",
      lang: "pug",
      code: `include ../mixins
ul
  each user in users
    +usr(user)
`,
    },
    message:
      "Mixin `usr` is not defined here, in an included file or in a layout",
    line: 4,
    column: 5,
  },
  {
    code: "Syntax/Ruby",
    languages: ["erb", "haml", "slim", "jbuilder"],
    summary: "Ruby in the template that doesn't parse.",
    explanation:
      "The template is compiled the way Erubi, Haml or Slim compile it, and the result is parsed with Prism for the project's Ruby version; the message is Prism's. Haml and Slim code is checked only once the template itself parses. Also covers the `locals:` comment of strict locals (Rails 7.1 and later), for instance a local without a colon: `# locals: (user)` gives \"`user` set as non-keyword argument for … Locals can only be set as keyword arguments.\" In Jbuilder and Builder views only that comment is checked, and in Skim templates nothing, since their code is CoffeeScript.",
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<ul>
  <% @users.each do |user| %>
    <li><%= user.name %></li>
</ul>
`,
    },
    message:
      "unexpected end-of-input, assuming it is closing the parent top level context",
    line: 4,
    column: 1,
  },
  {
    code: "Rails/MissingPartial",
    languages: ["erb", "haml", "slim", "jbuilder"],
    summary: "A `render` of a partial or template that isn't in app/views.",
    explanation:
      'Checks literal names in `render "users/card"`, `render partial:`, `render template:` and Jbuilder\'s `json.partial!`, in a Rails app (`actionview` in `Gemfile.lock`) and in templates under app/views. A name with a folder is checked when that folder exists in app/views; a bare name (`render "form"`) only in an action template, not a partial, whose controller chain is in the app, and the message then lists each folder Rails searches. Folders of view-shipping gems such as `devise/` are skipped. The message is Rails\' "Missing partial" or "Missing template", without the formats and view paths Rails adds.',
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<%= render "users/profile", user: @user %>
`,
    },
    message: "Missing partial users/_profile",
    line: 1,
    column: 13,
  },
  {
    code: "Rails/StrictLocals",
    languages: ["erb", "haml", "slim", "jbuilder"],
    summary:
      "A `render` whose locals don't match the partial's `locals:` comment.",
    explanation:
      'With Rails 7.1 or later in `Gemfile.lock`, a literal render that resolves to exactly one partial is compared with that partial\'s strict locals. Here `_card.html.erb` starts with `<%# locals: (user:) -%>`. The messages are Rails\': "missing local", "missing keywords", "unknown local", "unknown keywords", and "no locals accepted" for a partial declaring `locals: ()`. From Rails 7.2 they end with the partial\'s path. The `layout:` of a render isn\'t checked, since it gets the call\'s locals.',
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<%= render "users/card", person: @user %>
`,
    },
    message: "missing local: :user for app/views/users/_card.html.erb",
    line: 1,
    column: 13,
  },
  {
    code: "Rails/SilentRender",
    languages: ["erb", "haml", "slim"],
    summary: "A `render` in a tag that outputs nothing.",
    explanation:
      "A `<% %>` tag in ERB, and a `-` line in Haml and Slim, run Ruby and print nothing, so the partial `render` returns is thrown away. Reported for a `render` that is a whole statement, also in its block form. Not reported when the result is assigned or passed on. Rails projects only, in templates under app/views. The quick fix changes the tag to `<%=` (`=` in Haml and Slim) when the call is alone in its tag or line.",
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<% render "users/card", user: @user %>
`,
    },
    message: "`render` in `<% %>` outputs nothing; use `<%=`",
    line: 1,
    column: 4,
  },
  {
    code: "Rails/SilentHelper",
    languages: ["erb", "haml", "slim"],
    summary:
      "A helper that returns markup, called in a tag that outputs nothing.",
    explanation:
      "`link_to`, `image_tag`, `content_tag`, `form_with` and the other helpers on the list return markup and do nothing else, so calling one in a `<% %>` tag (a `-` line in Haml and Slim) has no effect. Helpers with side effects (`content_for`, `provide`), your own helpers and calls on a receiver are not reported. Rails projects only, in templates under app/views. The quick fix changes the tag to `<%=` (`=` in Haml and Slim) when the call is alone in its tag or line.",
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<% link_to "Home", root_path %>
`,
    },
    message: "`link_to` returns markup that `<% %>` throws away; use `<%=`",
    line: 1,
    column: 4,
  },
  {
    code: "Rails/DebugOutput",
    languages: ["erb", "haml", "slim"],
    summary: "Debugging left in a view.",
    explanation:
      "`p`, `pp`, `puts` and `debug` with arguments, `byebug`, `debugger`, `binding.irb` and `binding.pry`, as a statement or as the whole of an output tag. A local variable named `p`, a method on a receiver, a call used as an argument and a call under an `if` or `unless` that tests `Rails.env` are not reported. Rails projects only, in templates under app/views.",
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<%= debug @user %>
<p><%= @user.name %></p>
`,
    },
    message: "`debug` is debugging left in the view",
    line: 1,
    column: 5,
  },
  {
    code: "Rails/InstanceVariableInPartial",
    languages: ["erb", "haml", "slim"],
    summary: "An instance variable read inside a partial.",
    explanation:
      'A partial that reads `@user` works only for the controllers that set it. Pass it as a local instead (`render "users/card", user: @user`). Reported once per variable and file. Not reported when the partial assigns the variable itself, for `defined?(@user)`, for ActionView\'s own variables, and in partials under app/views/layouts. Reported as information, so `ruby-templates check` prints it without failing. Rails projects only, in templates under app/views.',
    example: {
      file: "app/views/users/_card.html.erb",
      lang: "erb",
      code: `<p><%= @user.name %></p>
`,
    },
    message:
      "`@user` is read in a partial, so it works only where the controller sets it; pass it as a local",
    line: 1,
    column: 8,
  },
  {
    code: "Rails/UnusedStrictLocal",
    languages: ["erb", "haml", "slim"],
    summary: "A local in the `locals:` comment that the partial never uses.",
    explanation:
      "Reads, assignments, `local_assigns[:name]`, `local_assigns.fetch(:name)`, `binding.local_variable_get(:name)`, `defined?(name)` and the default of another local count as use. Nothing is reported when the template passes `local_assigns` on as a whole or calls `binding`, `local_variables` or `eval`. The quick fix removes the local from the comment. Rails 7.1 or newer, in Rails projects, in templates under app/views.",
    example: {
      file: "app/views/users/_card.html.erb",
      lang: "erb",
      code: `<%# locals: (user:, compact: false) %>
<p><%= user.name %></p>
`,
    },
    message: "`compact` is declared in `locals:` but the partial never uses it",
    line: 1,
    column: 21,
  },
  {
    code: "Rails/UnusedBlockArgument",
    languages: ["erb", "haml", "slim"],
    summary: "A block argument the block never uses.",
    explanation:
      "Looks at plain and optional block parameters, and at the whole block, which spans every tag between `do |…|` and `end`. Names starting with `_`, `it` and numbered parameters, destructured, `*rest`, keyword and `&block` parameters, and blocks that call `binding`, `local_variables` or `eval` are not reported. The quick fix prefixes the name with `_`. Rails projects only, in templates under app/views.",
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<% @users.each do |user, index| %>
  <li><%= user.name %></li>
<% end %>
`,
    },
    message: "Block argument `index` is never used; rename it `_index`",
    line: 1,
    column: 26,
  },
  {
    code: "Rails/RenderInLoop",
    languages: ["erb", "haml", "slim"],
    summary: "A partial rendered once per item inside a loop.",
    explanation:
      'Reported for `collection.each do |item|` and `each_with_index` whose whole body is `render "partial", name: item`, with a string literal for the partial. `render partial: "partial", collection: collection, as: :name` renders the same markup in one call. Loops that do anything else, other locals and other iterators are not reported. Rails projects only, in templates under app/views.',
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<% @users.each do |user| %>
  <%= render "users/card", user: user %>
<% end %>
`,
    },
    message:
      'Partial "users/card" is rendered once per item of a loop; use `render partial: "users/card", collection: @users, as: :user`',
    line: 2,
    column: 7,
  },
  {
    code: "Rails/OutputControlFlow",
    languages: ["erb"],
    summary: "Control flow in an output tag.",
    explanation:
      "`<%= if @user %>` is a syntax error, or prints a loop's value (`<%= for x in y do %>`): an output tag holds an expression, so a tag that opens `if`, `unless`, `while`, `until`, `case`, `for` or `begin` without closing it, or that starts with `else`, `elsif`, `when`, `end`, `rescue` or `ensure`, needs `<%` instead. Reported on the `=`, with a quick fix that removes it; the Ruby syntax errors it would cause are not shown, the template's other errors are. A complete expression such as `<%= if a then b else c end %>` is not reported.",
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<%= if @user %>
  <p>hi</p>
<% end %>
`,
    },
    message: "`if` in an output tag `<%=`: use `<% if … %>`",
    line: 1,
    column: 3,
  },
  {
    code: "Rails/UnsafeOutput",
    languages: ["erb", "haml", "slim"],
    summary: "Output that skips Rails' HTML escaping.",
    explanation:
      '`raw x`, `x.html_safe` and `safe_concat x` are reported on the method name, and the unescaped output markers `<%==` (ERB), `!=` and `!~` (Haml) and `==` (Slim) on the marker. `raw x.to_json` in a `<script>` is the usual idiom and is not reported. Not reported when the value is certainly safe: a literal whose interpolations are all safe (`"Hi #{h(name)}"`), `yield`, `content_for`, a `t` call with a literal key ending in `_html` or `.html`, or a call to `sanitize`, `safe_join`, `strip_tags`, `json_escape`, `j`, `escape_javascript`, `h`, `html_escape`, `link_to`, `content_tag`, `render`, `capture`, `truncate` (unless `escape: false`) and the like, or an escaper on `ERB::Util`, `CGI` or `Rack::Utils`. Not checked in text templates. Rails projects only, in templates under app/views.',
    example: {
      file: "app/views/users/show.html.erb",
      lang: "erb",
      code: `<%= raw @user.bio %>
`,
    },
    message:
      "`raw` outputs `@user.bio` without escaping; use `sanitize` if it holds user HTML",
    line: 1,
    column: 5,
  },
  {
    code: "Rails/MissingTranslation",
    languages: ["erb", "haml", "slim"],
    summary: "A lazy translation key with no entry in `config/locales`.",
    explanation:
      'A literal lazy key (`t(".title")`, `translate`, with or without parentheses) is turned into its full key as Rails does (`users/_form` gives `users.form.title`, a layout keeps its folder) and must be in the default locale of the YAML files under config/locales: `config.i18n.default_locale`, `en` without one. The closest key of the view within two edits is offered. Absolute keys are not checked, since gems supply keys such as `date.formats` and `helpers.submit.create`; nor is a key with `default:`, `scope:` or `locale:`, a key with interpolation, or one in a view named for a gem in the Gemfile.lock. Nothing is reported without a config/application.rb or a locale file for the default locale, or when translations may come from elsewhere: Ruby locale files, `I18n.load_path`, `I18n.backend`, a gem that keeps them in a store. With `config.i18n.fallbacks` a key in any locale counts. Rails projects only, in templates under app/views.',
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<h1><%= t(".titel") %></h1>
`,
    },
    message:
      "Translation `users.index.titel` is missing in the `en` locale; did you mean `.title`?",
    line: 1,
    column: 11,
  },
  {
    code: "Rails/UnknownRoute",
    languages: ["erb", "haml", "slim"],
    summary: "A `*_path` or `*_url` helper that the app's routes don't define.",
    explanation:
      "A `*_path` or `*_url` call without a receiver that no route of the app defines but that is within two edits of one (`usr_path` for `user_path`) is reported, with that route offered; a name with no route near it may come from a gem, a concern or `lib`, which can't be seen, so it is left alone. The routes are read with `bin/rails runner` once the editor has loaded them (setting `rubyTemplates.routes.enabled`), so this is reported in the editor and not by `ruby-templates check`. Not reported: a call on a receiver (`main_app.users_path`, an engine's `admin.users_path`), Rails' asset and polymorphic helpers, `rails_*`, `turbo_*`, `action_mailbox_*`, `omniauth_*`, `vite_*` and `asset_pack_*` helpers, Devise's own helpers (`session_path`, `edit_password_url`, …) when the Gemfile.lock has `devise`, a view in a folder named for a gem (`spree/`, `rails_admin/`), a helper the app defines in app/helpers or names with a symbol (`helper_method`, `delegate`, `define_method`), a call under `defined?`, a route declared with `direct`, and a local variable or block parameter. A bare name (`item_path` with `resources :items`) could be a local the caller passes, so it is reported only when called with arguments or when the template's strict locals don't list it. Rails projects only, in templates under app/views.",
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<%= link_to @user.name, usr_path(@user) %>
`,
    },
    message:
      "`usr_path` is not a route helper of this app; did you mean `user_path`?",
    line: 1,
    column: 25,
  },
  {
    code: "Turbo/PermanentRequiresId",
    languages: ["erb", "haml", "slim"],
    summary: "An element with `data-turbo-permanent` and no `id`.",
    explanation:
      "Turbo keeps a permanent element across visits by matching its `id` on the next page, so without one the attribute does nothing. Read from ERB start tags, Haml and Slim tags, and the options of `tag.div`, `content_tag`, `link_to` and the form and button helpers; any value counts. An `id` of any kind (`#id` in Haml and Slim, an object reference such as `%div[@user]`) satisfies it. Nothing is reported where attributes may come from code that can't be read: ERB inside the start tag, a Haml `**attrs` or `data: attrs`, a Slim `*attrs` or `data=` hash, a helper's `**options`, or a value that isn't a literal. Rails projects only, in templates under app/views.",
    example: {
      file: "app/views/layouts/application.html.erb",
      lang: "erb",
      code: `<div data-turbo-permanent>
  <%= render 'shared/player' %>
</div>
`,
    },
    message:
      "`data-turbo-permanent` needs an `id`: Turbo matches the element across pages by it",
    line: 1,
    column: 6,
  },
  {
    code: "Stimulus/UnknownController",
    languages: ["erb", "haml", "slim"],
    summary: "A `data-controller` name that no Stimulus controller has.",
    explanation:
      'Each name in a static `data-controller` (also `data: { controller: "…" }` in Haml and in the options of `tag`, `content_tag`, `link_to`, `button_to` and the form helpers) must be a controller file in app/javascript/controllers (`search_controller.js`, `admin/users_controller.js` for `admin--users`; `-controller`, `.ts`, `.jsx`, `.tsx` and `.mjs` too) or a name a script in app/javascript registers with `application.register("name", …)`. The closest controller within two edits is offered. Nothing is reported when the app has no app/javascript/controllers folder, loads controllers from another folder (`eagerLoadControllersFrom("components", …)`, a context or glob outside the controllers folder, an importmap pin), registers one under a name the code computes, and not for a name with ERB or interpolation. Rails projects only, in templates under app/views.',
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<div data-controller="serch">
  <%= text_field_tag :q %>
</div>
`,
    },
    message:
      "No Stimulus controller `serch` in app/javascript/controllers; did you mean `search`?",
    line: 1,
    column: 23,
  },
  {
    code: "Stimulus/UnknownTargetOrAction",
    languages: ["erb", "haml", "slim"],
    summary:
      "A `data-*-target` or `data-action` method that its Stimulus controller doesn't have.",
    explanation:
      'Static `data-<controller>-target` names are checked against the controller\'s `static targets = ["…"]`, and the method of each `data-action` descriptor against the members its class defines (also `data: { search_target: "…", action: "…" }` in Haml and in the options of the Rails helpers). The closest name within two edits is offered. Only a controller that is one file in app/javascript/controllers and can be read with confidence is checked: a class that extends something other than `Controller` (a base controller is followed when imported by a relative path), computed member names, decorators, mixins and a `static targets` that is not a plain list skip it. Rails projects only, in templates under app/views.',
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<div data-controller="search">
  <input data-search-target="querry">
</div>
`,
    },
    message:
      "`search` controller has no target `querry`; did you mean `query`?",
    line: 2,
    column: 30,
  },
  {
    code: "Style/ErbTrimMode",
    languages: ["erb"],
    summary: "Both `%>` and `-%>` in one template.",
    explanation:
      "Every closer of the kind the template uses less is reported, `-%>` when there are as many of each. Comments (`<%#`) and literal `<%%` tags are not counted, nor are closers with more on their line. Off by default: turn it on with `enabled = true` under `[Style.ErbTrimMode]` in .ruby-templates.toml.",
    example: {
      file: "app/views/users/index.html.erb",
      lang: "erb",
      code: `<% @users.each do |user| -%>
  <li><%= user.name %></li>
<% end %>
`,
    },
    message: "`-%>` here, but most closers in this template are `%>`",
    line: 1,
    column: 26,
  },
  {
    code: "Layout/LineLength",
    languages: ["erb", "haml", "slim", "pug"],
    summary: "A line longer than the limit.",
    explanation:
      "Every line over `max` characters (100 unless set) is reported from the first character past the limit. Characters are counted, a tab is one, trailing whitespace is not counted, and a line that cannot be made shorter is skipped: one with no space after its indentation, such as a long URL, and the `# locals:` comment of strict locals. Off by default: turn it on with `enabled = true` under `[Layout.LineLength]` in .ruby-templates.toml, where `max` sets the limit.",
    example: {
      file: "app/views/announcements/show.html.erb",
      lang: "erb",
      code: `<p class="lead text-muted"><%= link_to "Read the full announcement", announcement_path(@announcement), class: "link" %></p>
`,
    },
    message: "line is 123 characters long; the limit is 100",
    line: 1,
    column: 101,
  },
];

/** The codes each language's `disabledDiagnostics` setting in `package.json` accepts. */
function settingCodes(): Map<string, string[]> {
  // Builds run from site/; this module is bundled elsewhere, so its own path says nothing.
  const manifest = JSON.parse(
    fs.readFileSync(path.resolve(process.cwd(), "..", "package.json"), "utf8"),
  );
  const properties: Record<string, { items?: { enum?: string[] } }> =
    manifest.contributes.configuration.properties;
  return new Map(
    Object.entries(properties).flatMap(([key, value]) => {
      const lang = key.match(
        /^rubyTemplates\.(\w+)\.disabledDiagnostics$/,
      )?.[1];
      return lang ? [[lang, value.items?.enum ?? []]] : [];
    }),
  );
}

/** Fails the build unless the entries cover exactly the codes the settings accept, language by language. */
function checkCoverage(entries: Diagnostic[], settings: Map<string, string[]>) {
  const setting = (lang: string) => `rubyTemplates.${lang}.disabledDiagnostics`;
  const missing = [...settings].flatMap(([lang, codes]) =>
    codes
      .filter(
        (code) =>
          !entries.some(
            (d) => d.code === code && d.languages.includes(lang as Language),
          ),
      )
      .map(
        (code) =>
          `no entry for \`${code}\` in ${LANGUAGE_NAMES[lang as Language] ?? lang} (${setting(lang)})`,
      ),
  );
  const stale = entries.flatMap((d) =>
    d.languages
      .filter((lang) => !settings.get(lang)?.includes(d.code))
      .map(
        (lang) =>
          `entry \`${d.code}\` lists ${LANGUAGE_NAMES[lang]}, but ${setting(lang)} has no such code`,
      ),
  );
  const problems = [...missing, ...stale];
  if (problems.length) {
    throw new Error(
      `site/src/data/diagnostics.ts is out of step with package.json:\n  ${problems.join("\n  ")}`,
    );
  }
}

checkCoverage(diagnostics, settingCodes());
