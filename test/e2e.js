// Minimal LSP smoke test against a fake Rails tree: definition, hover, completion, diagnostics, symbols, rename.
import cp from "child_process";
import fs from "fs";
import path from "path";
import os from "os";
import { fileURLToPath, pathToFileURL } from "url";
import { isDeepStrictEqual } from "util";
// CI builds per target (`server/target/<triple>/release`) and passes the binary in.
const bin =
  process.env.ALDER_LS ??
  path.join(
    import.meta.dirname,
    "..",
    "server",
    "target",
    "release",
    "alder-ls",
  );
// Node escapes `~` (8.3 names such as RUNNER~1 on CI); the server, like VS Code, leaves it as is.
const fileUri = (p) => pathToFileURL(p).href.replaceAll("%7E", "~");
// macOS puts tmpdir under the /var symlink; one spelling of each path keeps built and returned URIs equal.
const root = fs.realpathSync(
  fs.mkdtempSync(path.join(os.tmpdir(), "alder-e2e-")),
);
const views = path.join(root, "app", "views");
fs.writeFileSync(
  path.join(root, "Gemfile.lock"),
  "GEM\n  specs:\n    actionview (7.1.3)\n",
);
fs.mkdirSync(path.join(views, "users"), { recursive: true });
fs.mkdirSync(path.join(views, "shared"), { recursive: true });
fs.mkdirSync(path.join(views, "strict"), { recursive: true });
fs.writeFileSync(
  path.join(views, "strict", "_row.html.erb"),
  "<%# locals: (a:, b: 1) %>\n",
);
fs.writeFileSync(
  path.join(views, "strict", "_pick.html.erb"),
  "<%# locals: (html_key:) %>\n",
);
fs.writeFileSync(
  path.join(views, "strict", "_pick.json.erb"),
  "<%# locals: (json_key:) %>\n",
);
for (const name of ["_dual.html.erb", "_dual.html+phone.erb"]) {
  fs.writeFileSync(path.join(views, "strict", name), "<%# locals: (u:) %>\n");
}
fs.writeFileSync(
  path.join(views, "strict", "list.html.erb"),
  '<%= render "strict/row", a: 1 %>\n',
);
fs.mkdirSync(path.join(root, "app", "components"), { recursive: true });
fs.writeFileSync(
  path.join(views, "users", "_card.html.slim"),
  ".card = user.name\n",
);
fs.writeFileSync(
  path.join(views, "shared", "_footer.html.erb"),
  "<footer></footer>\n",
);
fs.mkdirSync(path.join(root, "app", "controllers"), { recursive: true });
fs.writeFileSync(
  path.join(root, "app", "controllers", "application_controller.rb"),
  "class ApplicationController < ActionController::Base\n  before_action :set_current_user\n  before_action :set_admin, only: :show\n\n  def set_current_user\n    @current_user = User.first\n  end\n\n  def set_admin\n    @admin = true\n  end\nend\n",
);
fs.writeFileSync(
  path.join(root, "app", "components", "badge_component.rb"),
  "class BadgeComponent\n  renders_one :icon\n  renders_many :links\nend\n",
);
fs.writeFileSync(path.join(views, "users", "_user.html.erb"), "<p></p>\n");
fs.writeFileSync(
  path.join(views, "users", "_user.json.jbuilder"),
  "json.name user.name\n",
);
fs.mkdirSync(path.join(root, "app", "javascript", "controllers"), {
  recursive: true,
});
fs.writeFileSync(
  path.join(root, "app", "javascript", "controllers", "hello_controller.js"),
  'export default class extends Controller {\n  static targets = ["name"]\n  static values = { greetingText: String }\n  greet() {}\n}\n',
);
fs.mkdirSync(path.join(root, "app", "controllers"), { recursive: true });
fs.mkdirSync(path.join(root, "app", "helpers"), { recursive: true });
fs.mkdirSync(path.join(views, "layouts"), { recursive: true });
fs.writeFileSync(path.join(views, "layouts", "admin.html.erb"), "");
fs.writeFileSync(
  path.join(root, "app", "helpers", "users_helper.rb"),
  "module UsersHelper\n  def format_price(x)\n  end\nend\n",
);
const controllerText =
  'class UsersController < ApplicationController\n  layout "admin"\n\n  def index\n    render :index\n  end\n  before_action :set_menu\n\n  def set_menu\n    @menu = Menu.first\n  end\nend\n';
const controllerFile = path.join(
  root,
  "app",
  "controllers",
  "users_controller.rb",
);
fs.writeFileSync(controllerFile, controllerText);
fs.mkdirSync(path.join(root, "app", "mailers"), { recursive: true });
fs.writeFileSync(
  path.join(root, "app", "mailers", "user_mailer.rb"),
  'class UserMailer\n  def subject = t("users.index.title")\nend\n',
);
fs.mkdirSync(path.join(root, "app", "assets", "images"), { recursive: true });
fs.writeFileSync(path.join(root, "app", "assets", "images", "logo.png"), "");
fs.mkdirSync(path.join(root, "app", "assets", "stylesheets"), {
  recursive: true,
});
fs.writeFileSync(
  path.join(root, "app", "assets", "stylesheets", "admin.css"),
  "",
);
fs.mkdirSync(path.join(root, "db"), { recursive: true });
fs.writeFileSync(
  path.join(root, "db", "schema.rb"),
  'create_table "users", force: :cascade do |t|\n  t.string "name"\n  t.string "nickname"\n  t.string "email"\nend\ncreate_table "reports", force: :cascade do |t|\n  t.string "title", null: false\nend\n',
);
fs.mkdirSync(path.join(root, "app", "models"), { recursive: true });
fs.writeFileSync(
  path.join(root, "app", "models", "report.rb"),
  "class Report < ApplicationRecord\nend\n",
);
fs.writeFileSync(
  path.join(root, "app", "controllers", "reports_controller.rb"),
  "class ReportsController < ApplicationController\n  def index\n    @reports = Report.order(:title)\n  end\nend\n",
);
fs.mkdirSync(path.join(root, "config", "locales"), { recursive: true });
fs.writeFileSync(
  path.join(root, "config", "locales", "en.yml"),
  "en:\n  users:\n    index:\n      title: Users\n",
);
fs.writeFileSync(
  path.join(root, "config", "locales", "uk.yml"),
  "uk:\n  users:\n    index:\n      title: Користувачі\n",
);
fs.writeFileSync(
  path.join(root, "app", "components", "badge_component.html.haml"),
  "%span.badge= content\n",
);
const file = path.join(views, "users", "index.html.erb");
const text =
  '<%= render "card", user: u %>\n<%= render "shared/footer" %>\n<%= render BadgeComponent.new %>\n<%= render "sh\n<%= f.text_\n<%= link_to\n<%= render "shared/missing" %>\n<% content_for :aside do %>\n<nav>\n<%= render "card" %>\n</nav>\n<% end %>\n<%= render template: "us\n<%= link_to "x",\n  link_ %>\n<%= render @users %>\n<div data-controller="hello"></div>\n<%= image_tag "logo.png", alt: "Logo" %>\n<%= t(".title") %>\n';
fs.writeFileSync(file, text);
const uri = fileUri(file);
const hamlFile = path.join(views, "users", "show.html.haml");
const hamlText = [
  "- content_for :aside do",
  '  %nav= render "card"',
  "%section#main",
  '  = render "shared/footer"',
  "  = render BadgeComponent.new",
  "  = link_",
  "  %p see (link_",
  "  %sec",
  "  %a{hr}",
  "  :jav",
  "  &= x",
  '  = render "shared/missing"',
  "  %p text",
  "    %b nested",
  "  :css",
  "    .btn{",
  "  :ruby",
  "    x = link_",
  "",
].join("\n");
fs.writeFileSync(hamlFile, hamlText);
const hamlUri = fileUri(hamlFile);
const slimFile = path.join(views, "users", "show.html.slim");
const slimText =
  'mjml:\n  x\n- else\nbr\n  p\n- form_with model: @u do |f|\n  = f.text_field :x\n= link_to("x"\n';
fs.writeFileSync(slimFile, slimText);
const slimUri = fileUri(slimFile);
const outlineFile = path.join(views, "users", "outline.html.slim");
const outlineText =
  'main#page\n  = content_for :aside do\n    nav = render "card"\n  a(href="/"\n    class="x")\n    | Home\n\njavascript:\n  go();\n';
fs.writeFileSync(outlineFile, outlineText);
const outlineUri = fileUri(outlineFile);
// Unsaved text differs from disk; VS Code sends `+` percent-encoded.
const phoneFile = path.join(views, "users", "edit.html+phone.slim");
fs.writeFileSync(phoneFile, '\n\n= render "card"\n');
const phoneText = '= render "card"\n';
const phoneUri = fileUri(phoneFile).replace("+", "%2B");

const enginePartial = path.join(
  root,
  "engines",
  "blog",
  "app",
  "views",
  "blog",
  "_post.html.erb",
);
fs.mkdirSync(path.dirname(enginePartial), { recursive: true });
fs.writeFileSync(enginePartial, "<article></article>\n");
const engineUri = fileUri(path.join(views, "users", "engine.html.erb"));

const startServer = () => {
  const proc = cp.spawn(bin);
  let id = 0,
    buf = Buffer.alloc(0);
  const pending = new Map();
  const notes = [];
  const registrations = [];
  proc.stdout.on("data", (d) => {
    buf = Buffer.concat([buf, d]);
    for (;;) {
      const m = buf.toString("utf8").match(/^Content-Length: (\d+)\r\n\r\n/);
      if (!m) return;
      const len = +m[1],
        start = m[0].length;
      if (buf.length < start + len) return;
      const msg = JSON.parse(buf.slice(start, start + len).toString("utf8"));
      buf = buf.slice(start + len);
      if (msg.id !== undefined && pending.has(msg.id)) {
        pending.get(msg.id)(msg);
        pending.delete(msg.id);
      } else if (msg.method === "client/registerCapability") {
        registrations.push(...msg.params.registrations);
        const reply = JSON.stringify({
          jsonrpc: "2.0",
          id: msg.id,
          result: null,
        });
        proc.stdin.write(
          `Content-Length: ${Buffer.byteLength(reply)}\r\n\r\n${reply}`,
        );
      } else notes.push(msg);
    }
  });
  const send = (method, params, isNote) =>
    new Promise((res) => {
      const msg = { jsonrpc: "2.0", method, params };
      if (!isNote) {
        msg.id = ++id;
        pending.set(msg.id, res);
      }
      const s = JSON.stringify(msg);
      proc.stdin.write(`Content-Length: ${Buffer.byteLength(s)}\r\n\r\n${s}`);
      if (isNote) res();
    });
  return { send, notes, registrations };
};
const { send, notes } = startServer();
const rel = (p) => p.replace(fileUri(root), "");
let failures = 0;
const check = (label, actual, expected) => {
  const ok = isDeepStrictEqual(actual, expected);
  if (!ok) failures++;
  console.log(`${ok ? "ok  " : "FAIL"} ${label.padEnd(24)} →`, actual);
  if (!ok) console.log("     expected", expected);
};
(async () => {
  const init = await send("initialize", {
    processId: null,
    rootUri: fileUri(root),
    capabilities: {},
    initializationOptions: {
      slim: {
        extraEngines: ["mjml"],
        disabledDiagnostics: ["Syntax/ExplicitEnd"],
      },
    },
  });
  await send("initialized", {}, true);
  await send(
    "textDocument/didOpen",
    { textDocument: { uri, languageId: "erb", version: 1, text } },
    true,
  );
  const req = (method, u, line, character) =>
    send(method, { textDocument: { uri: u }, position: { line, character } });
  const targets = (r) => (r.result || []).map((l) => rel(l.uri));
  const labels = (r) => (r.result || []).map((i) => i.label);
  const firstLine = (r) => r.result?.contents.value.split("\n")[0] ?? null;
  const tree = (s) =>
    s.children ? `${s.name} [${s.children.map(tree).join(", ")}]` : s.name;
  const component = [
    "/app/components/badge_component.rb",
    "/app/components/badge_component.html.haml",
  ];

  check(
    'definition "card"',
    targets(await req("textDocument/definition", uri, 0, 14)),
    ["/app/views/users/_card.html.slim"],
  );
  check(
    "definition shared/footer",
    targets(await req("textDocument/definition", uri, 1, 16)),
    ["/app/views/shared/_footer.html.erb"],
  );
  check(
    "definition component",
    targets(await req("textDocument/definition", uri, 2, 14)),
    component,
  );
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: engineUri,
        languageId: "erb",
        version: 1,
        text: '<%= render "blog/post" %>\n',
      },
    },
    true,
  );
  check(
    "definition: render in an in-repo engine",
    targets(await req("textDocument/definition", engineUri, 0, 14)),
    ["/engines/blog/app/views/blog/_post.html.erb"],
  );
  const liquidUri = fileUri(path.join(views, "users", "card.liquid"));
  const liquidHelperUri = fileUri(path.join(views, "users", "helper.liquid"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: liquidUri,
        languageId: "liquid",
        version: 1,
        text: '{% render "card" %}\n',
      },
    },
    true,
  );
  check(
    "definition: liquid render is not a partial",
    targets(await req("textDocument/definition", liquidUri, 0, 14)),
    [],
  );
  check(
    "document links: none for a liquid render without a _card.liquid",
    (
      await send("textDocument/documentLink", {
        textDocument: { uri: liquidUri },
      })
    ).result,
    [],
  );
  check(
    "hover: a liquid render without a _card.liquid finds nothing",
    (await req("textDocument/hover", liquidUri, 0, 14)).result.contents.value,
    "**Render** `card`\n\nNo matching file found.",
  );
  const site = path.join(root, "site");
  fs.mkdirSync(path.join(site, "_includes"), { recursive: true });
  fs.writeFileSync(path.join(site, "_config.yml"), "title: x\n");
  fs.writeFileSync(path.join(site, "_includes", "footer.html"), "<footer />\n");
  fs.writeFileSync(path.join(site, "_includes", "nav.html"), "<nav />\n");
  fs.writeFileSync(path.join(site, "_card.liquid"), "<p>{{ card }}</p>\n");
  const pageUri = fileUri(path.join(site, "page.html.liquid"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: pageUri,
        languageId: "liquid",
        version: 1,
        text: "{% render 'card' %}\n{% include footer.html %}\n{% include 'n",
      },
    },
    true,
  );
  check(
    "definition: liquid render finds _card.liquid",
    targets(await req("textDocument/definition", pageUri, 0, 14)),
    ["/site/_card.liquid"],
  );
  check(
    "definition: jekyll include finds _includes/footer.html",
    targets(await req("textDocument/definition", pageUri, 1, 15)),
    ["/site/_includes/footer.html"],
  );
  check(
    "document links: liquid render and include",
    (
      await send("textDocument/documentLink", {
        textDocument: { uri: pageUri },
      })
    ).result.map((l) => rel(l.target)),
    ["/site/_card.liquid", "/site/_includes/footer.html"],
  );
  check(
    "hover: jekyll include lists its file",
    (await req("textDocument/hover", pageUri, 1, 15)).result.contents.value,
    "**Include** `footer.html`\n\n- `_includes/footer.html`",
  );
  const theme = path.join(root, "theme");
  fs.mkdirSync(path.join(theme, "snippets"), { recursive: true });
  fs.mkdirSync(path.join(theme, "layout"), { recursive: true });
  fs.mkdirSync(path.join(theme, "sections"), { recursive: true });
  fs.writeFileSync(path.join(theme, "snippets", "product-card.liquid"), "");
  const sectionUri = fileUri(path.join(theme, "sections", "main.liquid"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: sectionUri,
        languageId: "liquid",
        version: 1,
        text: "{% render 'product-card' %}\n{% render 'p",
      },
    },
    true,
  );
  check(
    "definition: shopify render finds snippets/product-card.liquid",
    targets(await req("textDocument/definition", sectionUri, 0, 14)),
    ["/theme/snippets/product-card.liquid"],
  );
  check(
    "completion: shopify snippet names",
    labels(await req("textDocument/completion", sectionUri, 1, 12)),
    ["product-card"],
  );
  check(
    "completion: jekyll include names",
    labels(await req("textDocument/completion", pageUri, 2, 14)),
    ["nav.html"],
  );
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: liquidHelperUri,
        languageId: "liquid",
        version: 1,
        text: "{{ link_to }}\n",
      },
    },
    true,
  );
  check(
    "hover: none on a helper in liquid",
    (await req("textDocument/hover", liquidHelperUri, 0, 5)).result ?? null,
    null,
  );
  // pug-rails JST: its code is JavaScript, so a `render` or helper name is no Rails one, even under app/views.
  const pugUri = fileUri(path.join(views, "users", "row.jst.pug"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: pugUri,
        languageId: "jade",
        version: 1,
        text: '- render("users/card")\np= link_to("x")\n- render("us',
      },
    },
    true,
  );
  check(
    "definition: none for a render in pug",
    targets(await req("textDocument/definition", pugUri, 0, 12)),
    [],
  );
  check(
    "hover: none for a render or a helper in pug",
    [
      (await req("textDocument/hover", pugUri, 0, 12)).result ?? null,
      (await req("textDocument/hover", pugUri, 1, 5)).result ?? null,
    ],
    [null, null],
  );
  check(
    "completion: no partials in pug",
    labels(await req("textDocument/completion", pugUri, 2, 12)),
    [],
  );
  check(
    "document links: none for a render in pug",
    (
      await send("textDocument/documentLink", {
        textDocument: { uri: pugUri },
      })
    ).result,
    [],
  );
  // pug-rails templates under app/assets/templates: `.pug` is implied, `/` starts from the asset root.
  const templates = path.join(root, "app", "assets", "templates");
  fs.mkdirSync(path.join(templates, "users", "parts"), { recursive: true });
  fs.writeFileSync(path.join(templates, "layout.pug"), "block content\n");
  fs.writeFileSync(path.join(templates, "users", "row.pug"), "li row\n");
  fs.writeFileSync(path.join(templates, "users", "cell.jst.pug"), "td\n");
  const listUri = fileUri(path.join(templates, "users", "list.jst.pug"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: listUri,
        languageId: "jade",
        version: 1,
        text: "extends /layout\nblock content\n  include row\n  include \n  include parts/\n",
      },
    },
    true,
  );
  check(
    "definition: pug include adds .pug",
    targets(await req("textDocument/definition", listUri, 2, 12)),
    ["/app/assets/templates/users/row.pug"],
  );
  check(
    "definition: pug extends from the asset root",
    targets(await req("textDocument/definition", listUri, 0, 10)),
    ["/app/assets/templates/layout.pug"],
  );
  check(
    "document links: pug include and extends",
    (
      await send("textDocument/documentLink", {
        textDocument: { uri: listUri },
      })
    ).result.map((l) => [l.range.start.line, rel(l.target)]),
    [
      [0, "/app/assets/templates/layout.pug"],
      [2, "/app/assets/templates/users/row.pug"],
    ],
  );
  check(
    "hover: pug include names its file",
    (await req("textDocument/hover", listUri, 2, 12)).result.contents.value,
    "**Include** `row`\n\n`app/assets/templates/users/row.pug`",
  );
  check(
    "completion: pug include names as written",
    labels(await req("textDocument/completion", listUri, 3, 10)),
    ["cell.jst.pug", "parts/", "row"],
  );
  check(
    "completion: pug extends from the asset root",
    (await req("textDocument/completion", listUri, 0, 9)).result.map((i) => [
      i.label,
      i.textEdit.range.start.character,
    ]),
    [
      ["layout", 9],
      ["users/", 9],
    ],
  );
  check(
    "completion: pug include in an empty folder",
    labels(await req("textDocument/completion", listUri, 4, 16)),
    [],
  );
  check(
    "hover",
    firstLine(await req("textDocument/hover", uri, 0, 14)),
    "**Partial** `card`",
  );
  check(
    'completion render "sh',
    labels(await req("textDocument/completion", uri, 3, 14)),
    ["shared/footer"],
  );
  check(
    "completion f.text_",
    labels(await req("textDocument/completion", uri, 4, 11)),
    ["text_field", "text_area"],
  );
  check(
    "completion link_to",
    labels(await req("textDocument/completion", uri, 5, 11)),
    ["link_to"],
  );
  const calledHelper = {};
  for (const [lang, text] of Object.entries({
    erb: "<%= link_to user.name, edit_user_path(user) %>\n<%= link_to %>\n",
    haml: '= link_to "Users", users_path\n= link_to\n',
    slim: '= link_to("Users", users_path)\n= link_to\n',
  })) {
    const calledUri = fileUri(path.join(views, "users", `called.html.${lang}`));
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: calledUri, languageId: lang, version: 1, text } },
      true,
    );
    const insert = async (line, character) =>
      (
        await req("textDocument/completion", calledUri, line, character)
      ).result?.find((i) => i.label === "link_to")?.insertText ?? "name";
    const at = text.indexOf("link_to") + "link_to".length;
    calledHelper[lang] = [await insert(0, at - 3), await insert(1, at)];
  }
  const snippet = 'link_to "${1:text}", ${2:path}';
  check("helper completion on a call with arguments", calledHelper, {
    erb: ["name", snippet],
    haml: ["name", snippet],
    slim: ["name", snippet],
  });
  const blockDefinitions = {};
  for (const [lang, text] of Object.entries({
    erb: "<% @users.each do |user| %>\n  <p><%= user.name %></p>\n<% end %>\n",
    haml: "- @users.each do |user|\n  %p= user.name\n",
    slim: "- @users.each do |user|\n  p = user.name\n",
  })) {
    const blockUri = fileUri(path.join(views, "users", `block.html.${lang}`));
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: blockUri, languageId: lang, version: 1, text } },
      true,
    );
    const read = text.split("\n")[1].indexOf("user.name") + 1;
    const r = (await req("textDocument/definition", blockUri, 1, read)).result;
    blockDefinitions[lang] = r
      ? `${r.range.start.line}:${r.range.start.character}-${r.range.end.character}`
      : null;
  }
  const assignedUri = fileUri(path.join(views, "users", "_assigned.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: assignedUri,
        languageId: "erb",
        version: 1,
        text: "<%# locals: (user: nil) %>\n<% user ||= 1 %>\n<% [1].each do |user| %><%= user %><% end %><%= user %>\n",
      },
    },
    true,
  );
  const assignedAt = async (character) => {
    const r = (await req("textDocument/definition", assignedUri, 2, character))
      .result;
    const l = Array.isArray(r) ? r[0] : r;
    return l ? `${l.range.start.line}:${l.range.start.character}` : null;
  };
  check(
    "definition on a strict local the partial assigns",
    [await assignedAt(48), await assignedAt(28)],
    ["0:0", "2:16"],
  );
  const lookupUri = fileUri(path.join(views, "users", "lookup.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: lookupUri,
        languageId: "erb",
        version: 1,
        text: '<%= render partial: "user", formats: [:json] %>\n<%= render "user", formats: [:json] %>\n<%= render partial: "user",\n  formats: [:json] %>\n<%= render formats: [:json], partial: "user" %>\n<%= render partial: "users/ghost",\n  formats: [:json] %>\n<%= render formats: [:json],\n  partial: "user" %>\n',
      },
    },
    true,
  );
  check(
    "render formats: picks the file",
    [
      targets(await req("textDocument/definition", lookupUri, 0, 22)),
      targets(await req("textDocument/definition", lookupUri, 1, 13)),
    ],
    [
      ["/app/views/users/_user.json.jbuilder"],
      ["/app/views/users/_user.html.erb"],
    ],
  );
  check(
    "render formats: on a later line picks the file",
    targets(await req("textDocument/definition", lookupUri, 2, 22)),
    ["/app/views/users/_user.json.jbuilder"],
  );
  check(
    "render formats: before the name picks the file",
    targets(await req("textDocument/definition", lookupUri, 4, 40)),
    ["/app/views/users/_user.json.jbuilder"],
  );
  check(
    "render formats: on a line before the name picks the file",
    targets(await req("textDocument/definition", lookupUri, 8, 14)),
    ["/app/views/users/_user.json.jbuilder"],
  );
  const ghost = {
    start: { line: 5, character: 21 },
    end: { line: 5, character: 32 },
  };
  const ghostActions = await send("textDocument/codeAction", {
    textDocument: { uri: lookupUri },
    range: ghost,
    context: {
      diagnostics: [
        { range: ghost, code: "Rails/MissingPartial", message: "not found" },
      ],
    },
  });
  check(
    "codeAction create partial in the lookup's format",
    ghostActions.result?.map((a) => a.title),
    ["Create partial `users/_ghost.json.erb`"],
  );
  const untitled = {};
  for (const [lang, text] of Object.entries({
    erb: '<div>\n<%= render "card" %>\n',
    haml: '%div\n  = render "card"\n- end\n',
    slim: 'div\n  = render "card"\n  p(class="x"\n',
  })) {
    const u = `untitled:Untitled-${lang}`;
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: u, languageId: lang, version: 1, text } },
      true,
    );
    const at = {
      textDocument: { uri: u },
      position: { line: 1, character: 14 },
    };
    const replies = [
      await send("textDocument/documentSymbol", { textDocument: { uri: u } }),
      await send("textDocument/foldingRange", { textDocument: { uri: u } }),
      await send("textDocument/definition", at),
      await send("textDocument/hover", at),
      await send("textDocument/references", {
        ...at,
        context: { includeDeclaration: true },
      }),
      await send("textDocument/prepareRename", at),
      await send("textDocument/documentLink", { textDocument: { uri: u } }),
    ];
    const published = () =>
      notes
        .filter(
          (n) =>
            n.method === "textDocument/publishDiagnostics" &&
            n.params.uri === u,
        )
        .flatMap((n) => n.params.diagnostics.map((d) => d.code));
    for (let waited = 0; waited < 2000 && !published().length; waited += 50) {
      await new Promise((r) => setTimeout(r, 50));
    }
    untitled[lang] = {
      errors: replies.filter((r) => r.error).length,
      symbols: replies[0].result?.length > 0,
      definition: replies[2].result ?? null,
      diagnostics: published().length > 0,
      unclosed: published().includes("Html/UnclosedElement"),
    };
  }
  const untitledOk = {
    errors: 0,
    symbols: true,
    definition: null,
    diagnostics: true,
    unclosed: false,
  };
  check("untitled documents", untitled, {
    erb: { ...untitledOk, unclosed: true },
    haml: untitledOk,
    slim: untitledOk,
  });
  const erbUntitled = "untitled:Untitled-fmt";
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: erbUntitled,
        languageId: "erb",
        version: 1,
        text: "<div>\n<p>hi</p>\n</div>\n",
      },
    },
    true,
  );
  const untitledEdits =
    (
      await send("textDocument/formatting", {
        textDocument: { uri: erbUntitled },
        options: { tabSize: 2, insertSpaces: true },
      })
    ).result || [];
  check(
    "untitled erb formatting",
    untitledEdits[0]?.newText,
    "<div>\n  <p>hi</p>\n</div>\n",
  );
  const jsUri = "untitled:Untitled-erb-js";
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: jsUri,
        languageId: "erb-js",
        version: 1,
        text: "<div>\n<% end %>\n",
      },
    },
    true,
  );
  const jsCodes = () =>
    notes
      .filter(
        (n) =>
          n.method === "textDocument/publishDiagnostics" &&
          n.params.uri === jsUri,
      )
      .flatMap((n) => n.params.diagnostics.map((d) => d.code));
  // The stray `end` is a non-HTML diagnostic, so its arrival proves the server has published.
  for (let waited = 0; waited < 2000 && !jsCodes().length; waited += 50) {
    await new Promise((r) => setTimeout(r, 50));
  }
  check("untitled erb-js gets no HTML diagnostics", jsCodes(), ["Syntax/Ruby"]);
  // The latest diagnostics as `line:code` for a document opened with `languageId`; empty when none are published.
  const diagnosticsFor = async (uri, languageId, text) => {
    await send(
      "textDocument/didOpen",
      { textDocument: { uri, languageId, version: 1, text } },
      true,
    );
    const published = () =>
      notes.filter(
        (n) =>
          n.method === "textDocument/publishDiagnostics" &&
          n.params.uri === uri,
      );
    for (let waited = 0; waited < 2000 && !published().length; waited += 50) {
      await new Promise((r) => setTimeout(r, 50));
    }
    return (published().at(-1)?.params.diagnostics ?? []).map(
      (d) => `${d.range.start.line}:${d.code}`,
    );
  };
  const mixed = "% if x\n  a\n<% else %>\n  b\n% end\n";
  check(
    "erb-percent: % lines and <% else %> form one block",
    await diagnosticsFor("file:///v/mixed.html.erb", "erb-percent", mixed),
    [],
  );
  check(
    "liquid: an unclosed if is an error with its code",
    await diagnosticsFor(
      "file:///v/card.html.liquid",
      "liquid",
      "<p>{{ a }}</p>\n{% if a %}\n  b\n",
    ),
    ["1:Syntax/LiquidUnclosedBlock"],
  );
  const latestCodes = (uri) =>
    notes
      .filter(
        (n) =>
          n.method === "textDocument/publishDiagnostics" &&
          n.params.uri === uri,
      )
      .at(-1)
      ?.params.diagnostics.map((d) => `${d.range.start.line}:${d.code}`);
  const waitForCodes = async (uri, expected) => {
    for (
      let waited = 0;
      waited < 2000 &&
      JSON.stringify(latestCodes(uri)) !== JSON.stringify(expected);
      waited += 50
    ) {
      await new Promise((r) => setTimeout(r, 50));
    }
    return latestCodes(uri);
  };
  check(
    "pug: no Rails checks on a render",
    await diagnosticsFor(
      fileUri(path.join(views, "users", "list.jst.pug")),
      "jade",
      '- render("users/missing")\nimg(src="x" alt="")\n',
    ),
    [],
  );
  check(
    "pug: a syntax error is reported with its code",
    await diagnosticsFor(
      fileUri(path.join(views, "users", "broken.jst.pug")),
      "jade",
      "ul\n    li\n  li\n",
    ),
    ["2:Syntax/PugInconsistentIndent"],
  );
  fs.writeFileSync(path.join(views, "users", "layout.pug"), "block content\n");
  const pugShowUri = fileUri(path.join(views, "users", "show.jst.pug"));
  check(
    "pug: nothing for a valid template",
    await diagnosticsFor(
      pugShowUri,
      "jade",
      "extends layout\nblock content\n  section#main\n    .card= user.name\nmixin badge(x)\n  span.badge= x\n",
    ),
    [],
  );
  const pugAssets = path.join(root, "app", "assets", "templates", "users");
  const lateUri = fileUri(path.join(pugAssets, "late.jst.pug"));
  check(
    "pug: a missing include and a late extends; no mixin check without it",
    await diagnosticsFor(
      lateUri,
      "jade",
      "p\n  include missing\n  +nope\nextends /layout\n",
    ),
    ["1:Syntax/PugMissingFile", "3:Syntax/PugExtendsNotFirst"],
  );
  const missingPug = path.join(pugAssets, "missing.pug");
  fs.writeFileSync(missingPug, "p\n");
  await send(
    "workspace/didChangeWatchedFiles",
    { changes: [{ uri: fileUri(missingPug), type: 1 }] },
    true,
  );
  const lateAfterCreate = [
    "3:Syntax/PugExtendsNotFirst",
    "2:Syntax/PugUnknownMixin",
  ];
  check(
    "pug: a created include is found, and the mixin call then checked",
    await waitForCodes(lateUri, lateAfterCreate),
    lateAfterCreate,
  );
  fs.unlinkSync(missingPug);
  check(
    "pug: a block the layout lacks",
    await diagnosticsFor(
      fileUri(path.join(pugAssets, "sidebar.jst.pug")),
      "jade",
      "extends /layout\nblock content\n  p\nblock sidebar\n",
    ),
    ["3:Syntax/PugUnexpectedBlock"],
  );
  check(
    "pug: an unknown mixin, with every include in place",
    await diagnosticsFor(
      fileUri(path.join(pugAssets, "mixin-call.jst.pug")),
      "jade",
      "include row\n+nope\n",
    ),
    ["1:Syntax/PugUnknownMixin"],
  );
  const pugHtmlUri = fileUri(path.join(pugAssets, "card.jst.pug"));
  const pugHtml = await diagnosticsFor(
    pugHtmlUri,
    "jade",
    'img(src=user.avatarUrl)\na(href="/x")\n  a(href="/y") y\nimg.logo\n  | Logo\n',
  );
  check("pug: HTML rules on the tags", pugHtml, [
    "3:Html/VoidChildren",
    "0:Html/ImgAlt",
    "3:Html/ImgAlt",
    "2:Html/NestedInteractive",
  ]);
  const altDiagnostic = notes
    .filter(
      (n) =>
        n.method === "textDocument/publishDiagnostics" &&
        n.params.uri === pugHtmlUri,
    )
    .at(-1)
    .params.diagnostics.find(
      (d) => d.code === "Html/ImgAlt" && d.range.start.line === 0,
    );
  check(
    "pug: the alt quick fix goes in the attribute list",
    (
      await send("textDocument/codeAction", {
        textDocument: { uri: pugHtmlUri },
        range: altDiagnostic.range,
        context: { diagnostics: [altDiagnostic] },
      })
    ).result
      .filter((a) => a.kind === "quickfix")
      .map((a) => [
        a.title,
        Object.values(a.edit.changes)[0].map(
          (e) =>
            `${e.range.start.line}:${e.range.start.character} ${e.newText}`,
        ),
      ]),
    [['Add `alt=""`', ['0:22  alt=""']]],
  );
  check(
    "pug documentSymbols",
    (
      (
        await send("textDocument/documentSymbol", {
          textDocument: { uri: pugShowUri },
        })
      ).result || []
    ).map(tree),
    ["block content [section#main [.card]]", "mixin badge [span.badge]"],
  );
  // Nesting that used to overflow the stack: refused past 256 levels, the server living on.
  const deepTags = Array.from(
    { length: 1500 },
    (_, i) => `${"  ".repeat(i)}div#d${i}`,
  ).join("\n");
  const deepUri = fileUri(path.join(pugAssets, "deep.jst.pug"));
  check(
    "pug: nesting too deep is reported",
    [
      ...(await diagnosticsFor(deepUri, "jade", `${deepTags}\n`)),
      ...(await diagnosticsFor(
        fileUri(path.join(pugAssets, "deep-inline.jst.pug")),
        "jade",
        `p ${"#[b ".repeat(1500)}x${"]".repeat(1500)}\n`,
      )),
    ],
    ["256:Syntax/PugTemplate", "0:Syntax/PugTemplate"],
  );
  check(
    "pug: the deep template's outline, links and formatting",
    [
      (
        await send("textDocument/documentSymbol", {
          textDocument: { uri: deepUri },
        })
      ).result.length,
      (
        await send("textDocument/documentLink", {
          textDocument: { uri: deepUri },
        })
      ).result.length,
      (
        (
          await send("textDocument/formatting", {
            textDocument: { uri: deepUri },
            options: { tabSize: 2, insertSpaces: true },
          })
        ).result ?? []
      ).length,
    ],
    [1, 0, 0],
  );
  check(
    "pug: the server answers after the deep template",
    (
      (
        await send("textDocument/documentSymbol", {
          textDocument: { uri: pugShowUri },
        })
      ).result || []
    ).length,
    2,
  );
  check(
    "pug foldingRanges: VS Code's own",
    (
      await send("textDocument/foldingRange", {
        textDocument: { uri: pugShowUri },
      })
    ).result ?? null,
    null,
  );
  check(
    "erb: the same text has a stray else",
    (await diagnosticsFor("file:///v/plain.html.erb", "erb", mixed)).length > 0,
    true,
  );
  check(
    "erb-percent: a syntax error in a % line is reported on it",
    [
      ...new Set(
        await diagnosticsFor(
          "file:///v/bad.html.erb",
          "erb-percent",
          "a\n% x = = 1\nb\n",
        ),
      ),
    ],
    ["1:Syntax/Ruby"],
  );
  const stimulusAttributes = {};
  for (const [lang, text] of Object.entries({
    erb: '<div class="x"\n     data-controller="hello" data-hel',
    haml: '%div(data-controller="hello" data-hel',
    haml_hash: '%div{ data: { controller: "hello", hel',
    slim: 'div data-controller="hello" data-hel',
    erb_tag: '<div data-controller="hello" <%= attrs %> data-hel',
    haml_lines: '%div(data-controller="hello"\n     data-hel',
    haml_rocket: '%div{ "data-controller" => "hello", data',
    slim_lines: 'div(\n  data-controller="hello"\n  data-hel',
  })) {
    const ext = lang.split("_")[0];
    const attrUri = fileUri(
      path.join(views, "users", `stimulus_${lang}.html.${ext}`),
    );
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: attrUri, languageId: ext, version: 1, text } },
      true,
    );
    const lines = text.split("\n");
    const last = lines.length - 1;
    const r = (
      await req("textDocument/completion", attrUri, last, lines[last].length)
    ).result;
    stimulusAttributes[lang] = (r ?? [])
      .map((i) => i.label)
      .filter((l) => l.includes("hello"));
  }
  check(
    "stimulus attributes of the element's controllers",
    stimulusAttributes,
    {
      erb: ["data-hello-target", "data-hello-greeting-text-value"],
      haml: ["data-hello-target", "data-hello-greeting-text-value"],
      haml_hash: ["hello_target", "hello_greeting_text_value"],
      slim: ["data-hello-target", "data-hello-greeting-text-value"],
      erb_tag: ["data-hello-target", "data-hello-greeting-text-value"],
      haml_lines: ["data-hello-target", "data-hello-greeting-text-value"],
      haml_rocket: ["data-hello-target", "data-hello-greeting-text-value"],
      slim_lines: ["data-hello-target", "data-hello-greeting-text-value"],
    },
  );
  const columnChecks = { hover: {}, definition: {}, tokens: {} };
  for (const [lang, text] of Object.entries({
    erb: "<% @reports.each do |report| %>\n  <p><%= report.title %></p>\n<% end %>\n",
    haml: "- @reports.each do |report|\n  %p= report.title\n",
    slim: "- @reports.each do |report|\n  p = report.title\n",
  })) {
    const columnUri = fileUri(
      path.join(views, "reports", `index.html.${lang}`),
    );
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: columnUri, languageId: lang, version: 1, text } },
      true,
    );
    const title = text.split("\n")[1].indexOf("report.title") + 8;
    columnChecks.hover[lang] = (
      await req("textDocument/hover", columnUri, 1, title)
    ).result?.contents.value;
    columnChecks.tokens[lang] = (
      await send("textDocument/semanticTokens/full", {
        textDocument: { uri: columnUri },
      })
    ).result?.data;
    const found = (await req("textDocument/definition", columnUri, 1, title))
      .result;
    columnChecks.definition[lang] = found
      ? `${rel(found.uri)}:${found.range.start.line + 1}`
      : null;
  }
  const reportsHover = "`reports.title` — string, null: false";
  check("hover on a model column", columnChecks.hover, {
    erb: reportsHover,
    haml: reportsHover,
    slim: reportsHover,
  });
  // [line, column, length, type, modifiers] deltas: `report` twice, as a variable (type 1).
  check("semantic tokens for block variables", columnChecks.tokens, {
    erb: [0, 21, 6, 1, 0, 1, 9, 6, 1, 0],
    haml: [0, 20, 6, 1, 0, 1, 6, 6, 1, 0],
    slim: [0, 20, 6, 1, 0, 1, 6, 6, 1, 0],
  });
  const schemaTarget = "/db/schema.rb:7";
  check("definition on a model column", columnChecks.definition, {
    erb: schemaTarget,
    haml: schemaTarget,
    slim: schemaTarget,
  });
  check("definition on a block variable", blockDefinitions, {
    erb: "0:19-23",
    haml: "0:18-22",
    slim: "0:18-22",
  });
  check(
    "definition render @users",
    targets(await req("textDocument/definition", uri, 15, 12)),
    ["/app/views/users/_user.html.erb"],
  );
  check(
    "definition data-controller",
    targets(await req("textDocument/definition", uri, 16, 24)),
    ["/app/javascript/controllers/hello_controller.js"],
  );
  check(
    "definition image_tag",
    targets(await req("textDocument/definition", uri, 17, 18)),
    ["/app/assets/images/logo.png"],
  );
  check(
    'definition t(".title")',
    (await req("textDocument/definition", uri, 18, 9)).result?.map(
      (l) => `${rel(l.uri)}:${l.range.start.line + 1}`,
    ),
    ["/config/locales/en.yml:4", "/config/locales/uk.yml:4"],
  );
  check(
    'hover t(".title")',
    (await req("textDocument/hover", uri, 18, 9)).result?.contents.value
      .split("\n")
      .slice(2)
      .join(" "),
    "- **en**: `Users` - **uk**: `Користувачі`",
  );
  check(
    'completion t(".',
    labels(await req("textDocument/completion", uri, 18, 8)),
    [".title"],
  );
  check(
    'completion template: "us',
    labels(await req("textDocument/completion", uri, 12, 24)),
    ["users/edit", "users/index", "users/outline", "users/show"],
  );
  check(
    "completion 2nd tag line",
    labels(await req("textDocument/completion", uri, 14, 7)),
    ["link_to"],
  );
  check(
    "hover <%=",
    (await req("textDocument/hover", uri, 0, 2)).result?.contents.value,
    "**`<%=`** outputs the value, HTML-escaped unless it is `html_safe`.",
  );
  check(
    "hover link_to",
    (await req("textDocument/hover", uri, 5, 6)).result?.contents.value
      .split("\n")
      .pop(),
    "[API docs](https://api.rubyonrails.org/classes/ActionView/Helpers/UrlHelper.html#method-i-link_to)",
  );
  // Hover right after a multibyte char once killed the server (byte slice inside `’`).
  const quoteUri = fileUri(path.join(views, "users", "quote.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: quoteUri,
        languageId: "erb",
        version: 1,
        text: '<p>Don’t</p>\n<%= link_to "Don’t", x %>\n',
      },
    },
    true,
  );
  check(
    "hover after ’ in text",
    (await req("textDocument/hover", quoteUri, 0, 7)).error ?? null,
    null,
  );
  check(
    "hover after ’ in code",
    (await req("textDocument/hover", quoteUri, 1, 18)).error ?? null,
    null,
  );
  check(
    "hover link_to after ’",
    (await req("textDocument/hover", quoteUri, 1, 6)).result?.contents.value
      .split("\n")
      .pop(),
    "[API docs](https://api.rubyonrails.org/classes/ActionView/Helpers/UrlHelper.html#method-i-link_to)",
  );
  const links = await send("textDocument/documentLink", {
    textDocument: { uri },
  });
  check(
    "documentLinks",
    (links.result || []).map((l) => rel(l.target)),
    [
      "/app/views/users/_card.html.slim",
      "/app/views/shared/_footer.html.erb",
      "/app/views/users/_card.html.slim",
      "/app/views/users/_user.html.erb",
      "/app/javascript/controllers/hello_controller.js",
      "/app/assets/images/logo.png",
    ],
  );
  check(
    "documentLink tooltips from the Rails root",
    (links.result || []).map((l) => l.tooltip).slice(-2),
    [
      "app/javascript/controllers/hello_controller.js",
      "app/assets/images/logo.png",
    ],
  );
  const folds = await send("textDocument/foldingRange", {
    textDocument: { uri },
  });
  check(
    "foldingRanges",
    (folds.result || []).map((f) => `${f.startLine}-${f.endLine}`),
    ["3-5", "7-10", "12-13", "8-9", "13-14"],
  );
  const syms = await send("textDocument/documentSymbol", {
    textDocument: { uri },
  });
  check("documentSymbols", (syms.result || []).map(tree), [
    'render "card"',
    'render "shared/footer"',
    'render "shared/missing"',
    'content_for :aside [nav [render "card"]]',
  ]);
  await new Promise((r) => setTimeout(r, 200));
  const diagnostics = (u) => {
    const diag = notes.find(
      (n) =>
        n.method === "textDocument/publishDiagnostics" && n.params.uri === u,
    );
    return diag
      ? diag.params.diagnostics.map(
          (d) => `${d.range.start.line + 1}: ${d.message}`,
        )
      : "none";
  };
  // Lines 4, 5, 6 and 13 are tags left open for the completion checks, each running on to the next `%>`: Prism's errors
  // for the method Rails builds from the template (`test/oracle/erb.rb` prints the same ones).
  check("diagnostics", diagnostics(uri), [
    "7: unexpected local variable or method, expecting end-of-input",
    "7: unexpected string literal, expecting end-of-input",
    "10: unexpected local variable or method, expecting end-of-input",
    "13: unexpected local variable or method, expecting end-of-input",
    "14: unexpected '<', ignoring it",
    "14: expected a matching `)`",
    "7: Missing partial shared/_missing",
  ]);

  const sources = (u) => [
    ...new Set(
      notes
        .find(
          (n) =>
            n.method === "textDocument/publishDiagnostics" &&
            n.params.uri === u,
        )
        ?.params.diagnostics.map((d) => d.source),
    ),
  ];
  check("diagnostics source", sources(uri), ["alder"]);

  const strictUri = fileUri(path.join(views, "strict", "show.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: strictUri,
        languageId: "erb",
        version: 1,
        text: '<%= render "strict/row" %>\n',
      },
    },
    true,
  );
  await new Promise((r) => setTimeout(r, 400));
  check("strict locals under actionview 7.1", diagnostics(strictUri), [
    "1: missing local: :a",
  ]);
  const strictFixes = async (
    u,
    text,
    at = {
      start: { line: 0, character: 12 },
      end: { line: 0, character: 22 },
    },
    languageId = "erb",
  ) => {
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: u, languageId, version: 1, text } },
      true,
    );
    const { result } = await send("textDocument/codeAction", {
      textDocument: { uri: u },
      range: at,
      context: {
        diagnostics: [{ range: at, code: "Rails/StrictLocals", message: "" }],
      },
    });
    return result
      .filter((a) => a.kind === "quickfix")
      .map((a) => {
        const edits = Object.entries(a.edit.changes).map(([target, [e]]) => {
          return `${rel(target)} ${e.range.start.line}:${e.range.start.character}-${e.range.end.line === e.range.start.line ? "" : `${e.range.end.line}:`}${e.range.end.character} "${e.newText}"`;
        });
        return `${a.title} | ${edits.sort().join(" + ")}`;
      });
  };
  const extraUri = fileUri(path.join(views, "strict", "extra.html.erb"));
  const multiUri = fileUri(path.join(views, "strict", "multi.html.erb"));
  const pipesUri = fileUri(path.join(views, "strict", "pipes.html.haml"));
  check(
    "strict locals quick fixes on a haml pipe multiline",
    await strictFixes(
      pipesUri,
      '= render "strict/row", |\n    c: 2 |\n',
      { start: { line: 0, character: 10 }, end: { line: 0, character: 20 } },
      "haml",
    ),
    [
      'Pass `a` to the render | /app/views/strict/pipes.html.haml 1:8-8 ", a: nil"',
      'Remove `c` from the render | /app/views/strict/pipes.html.haml 0:21-1:8 ""',
      'Declare `c` in the locals of `_row.html.erb` | /app/views/strict/_row.html.erb 0:21-21 ", c: nil"',
    ],
  );
  check(
    "strict locals quick fixes offer an unknown key passed twice once",
    await strictFixes(
      fileUri(path.join(views, "strict", "twice.html.erb")),
      '<%= render "strict/row", c: 2, c: 3 %>\n',
    ),
    [
      'Pass `a` to the render | /app/views/strict/twice.html.erb 0:35-35 ", a: nil"',
      'Remove `c` from the render | /app/views/strict/twice.html.erb 0:23-29 ""',
      'Declare `c` in the locals of `_row.html.erb` | /app/views/strict/_row.html.erb 0:21-21 ", c: nil"',
    ],
  );
  check(
    "strict locals quick fixes on a call continued before its partial",
    await strictFixes(
      fileUri(path.join(views, "strict", "continued.html.erb")),
      '<%= render formats: [:html], locals: { c: 2 },\n  partial: "strict/row" %>\n',
      { start: { line: 1, character: 12 }, end: { line: 1, character: 22 } },
    ),
    [
      'Pass `a` to the render | /app/views/strict/continued.html.erb 0:43-43 ", a: nil"',
      'Remove `c` from the render | /app/views/strict/continued.html.erb 0:39-43 ""',
      'Declare `c` in the locals of `_row.html.erb` | /app/views/strict/_row.html.erb 0:21-21 ", c: nil"',
    ],
  );
  const dualUri = fileUri(path.join(views, "strict", "dual.html.erb"));
  const dualFixes = await strictFixes(
    dualUri,
    '<%= render "strict/dual", z: 1 %>\n',
  );
  await new Promise((r) => setTimeout(r, 400));
  check(
    "strict locals of two variants",
    [diagnostics(dualUri), dualFixes],
    [
      ["1: missing local: :u"],
      [
        'Pass `u` to the render | /app/views/strict/dual.html.erb 0:30-30 ", u: nil"',
        'Remove `z` from the render | /app/views/strict/dual.html.erb 0:24-30 ""',
        'Declare `z` in the locals of `_dual.html.erb` and 1 other | /app/views/strict/_dual.html%2Bphone.erb 0:15-15 ", z: nil" + /app/views/strict/_dual.html.erb 0:15-15 ", z: nil"',
      ],
    ],
  );
  check(
    "strict locals quick fixes",
    [
      await strictFixes(strictUri, '<%= render "strict/row" %>\n'),
      await strictFixes(extraUri, '<%= render "strict/row", a: 1, c: 2 %>\n'),
      await strictFixes(
        multiUri,
        '<p>h\u00e9llo \u65e5\u672c</p>\n<%\n  render "strict/row",\n    c: items\n      .first\n%>\n',
        {
          start: { line: 2, character: 10 },
          end: { line: 2, character: 20 },
        },
      ),
    ],
    [
      [
        'Pass `a` to the render | /app/views/strict/show.html.erb 0:23-23 ", a: nil"',
      ],
      [
        'Remove `c` from the render | /app/views/strict/extra.html.erb 0:29-35 ""',
        'Declare `c` in the locals of `_row.html.erb` | /app/views/strict/_row.html.erb 0:21-21 ", c: nil"',
      ],
      [
        'Pass `a` to the render | /app/views/strict/multi.html.erb 4:12-12 ", a: nil"',
        'Remove `c` from the render | /app/views/strict/multi.html.erb 2:21-4:12 ""',
        'Declare `c` in the locals of `_row.html.erb` | /app/views/strict/_row.html.erb 0:21-21 ", c: nil"',
      ],
    ],
  ); // An unsaved document counts as a caller in later checks.
  await send(
    "textDocument/didClose",
    { textDocument: { uri: extraUri } },
    true,
  );

  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: hamlUri,
        languageId: "haml",
        version: 1,
        text: hamlText,
      },
    },
    true,
  );
  const haml = (method, line, character) =>
    req(method, hamlUri, line, character);
  check(
    'haml definition "card"',
    targets(await haml("textDocument/definition", 1, 17)),
    ["/app/views/users/_card.html.slim"],
  );
  check(
    "haml definition comp.",
    targets(await haml("textDocument/definition", 4, 15)),
    component,
  );
  check(
    "haml hover render",
    firstLine(await haml("textDocument/hover", 3, 16)),
    "**Partial** `shared/footer`",
  );
  check(
    "haml hover &=",
    firstLine(await haml("textDocument/hover", 10, 2)),
    "**Haml** `&=`",
  );
  check(
    "haml completion link_",
    labels(await haml("textDocument/completion", 5, 9)),
    ["link_to"],
  );
  check(
    "haml completion text (",
    labels(await haml("textDocument/completion", 6, 15)),
    [],
  );
  check(
    "haml completion tag",
    labels(await haml("textDocument/completion", 7, 6)),
    ["section"],
  );
  check(
    "haml completion {hr",
    labels(await haml("textDocument/completion", 8, 7)),
    ["href", "hreflang"],
  );
  check(
    "haml completion :jav",
    labels(await haml("textDocument/completion", 9, 6)),
    ["javascript"],
  );
  check(
    "haml completion :css {",
    labels(await haml("textDocument/completion", 15, 9)),
    [],
  );
  check(
    "haml completion :ruby",
    labels(await haml("textDocument/completion", 17, 14)),
    ["link_to"],
  );
  const hamlSyms = await send("textDocument/documentSymbol", {
    textDocument: { uri: hamlUri },
  });
  check("haml documentSymbols", (hamlSyms.result || []).map(tree), [
    'content_for :aside [nav [render "card"]]',
    'section#main [render "shared/footer", render "shared/missing"]',
  ]);
  const hamlFolds = await send("textDocument/foldingRange", {
    textDocument: { uri: hamlUri },
  });
  check(
    "haml foldingRanges",
    (hamlFolds.result || []).map((f) => `${f.startLine}-${f.endLine}`).sort(),
    ["0-1", "12-13", "14-15", "16-17", "2-17"],
  );
  await new Promise((r) => setTimeout(r, 200));
  check("haml diagnostics", diagnostics(hamlUri), [
    "14: Illegal nesting: content can't be both given on the same line as %p and nested within it.",
    "12: Missing partial shared/_missing",
  ]);

  const silentUri = fileUri(path.join(views, "silent", "show.html.haml"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: silentUri,
        languageId: "haml",
        version: 1,
        text: '%p\n  - render "x"\n  = render "y"\n  - binding.irb\n',
      },
    },
    true,
  );
  await new Promise((r) => setTimeout(r, 200));
  check("haml silent render and debugging", diagnostics(silentUri), [
    "2: `render` in `-` outputs nothing; use `=`",
    "4: `binding.irb` is debugging left in the view",
  ]);
  const silent = {
    start: { line: 1, character: 4 },
    end: { line: 1, character: 14 },
  };
  const silentFix = await send("textDocument/codeAction", {
    textDocument: { uri: silentUri },
    range: silent,
    context: {
      diagnostics: [{ range: silent, code: "Rails/SilentRender", message: "" }],
    },
  });
  check(
    "codeAction output a silent render",
    silentFix.result?.map((a) => [
      a.title,
      a.edit.changes[silentUri].map(
        (e) =>
          `${e.range.start.character}-${e.range.end.character} "${e.newText}"`,
      ),
    ]),
    [["Output the result", ['2-3 "="']]],
  );

  const controlFlowUri = fileUri(path.join(views, "silent", "flow.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: controlFlowUri,
        languageId: "erb",
        version: 1,
        text: "<%= if @user %>\n<p>hi</p>\n<% end %>\n",
      },
    },
    true,
  );
  await new Promise((r) => setTimeout(r, 200));
  check("erb control flow in an output tag", diagnostics(controlFlowUri), [
    "1: `if` in an output tag `<%=`: use `<% if … %>`",
  ]);
  const marker = {
    start: { line: 0, character: 2 },
    end: { line: 0, character: 3 },
  };
  const controlFlowFix = await send("textDocument/codeAction", {
    textDocument: { uri: controlFlowUri },
    range: marker,
    context: {
      diagnostics: [
        { range: marker, code: "Rails/OutputControlFlow", message: "" },
      ],
    },
  });
  check(
    "codeAction remove the output marker",
    controlFlowFix.result?.map((a) => [
      a.title,
      a.edit.changes[controlFlowUri].map(
        (e) =>
          `${e.range.start.character}-${e.range.end.character} "${e.newText}"`,
      ),
    ]),
    [["Use a code tag `<%`", ['2-3 ""']]],
  );

  const partialUri = fileUri(path.join(views, "silent", "_card.html.haml"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: partialUri,
        languageId: "haml",
        version: 1,
        text: "-# locals: (user:, size: 1)\n- items.each do |item, index|\n  %p= user.name + @title\n",
      },
    },
    true,
  );
  await new Promise((r) => setTimeout(r, 200));
  check("haml partial variable lints", diagnostics(partialUri), [
    "3: `@title` is read in a partial, so it works only where the controller sets it; pass it as a local",
    "1: `size` is declared in `locals:` but the partial never uses it",
    "2: Block argument `item` is never used; rename it `_item`",
    "2: Block argument `index` is never used; rename it `_index`",
  ]);
  const fixes = async (code, range) => {
    const result = await send("textDocument/codeAction", {
      textDocument: { uri: partialUri },
      range,
      context: { diagnostics: [{ range, code, message: "" }] },
    });
    return result.result?.map((a) => [
      a.title,
      a.edit.changes[partialUri].map(
        (e) =>
          `${e.range.start.character}-${e.range.end.character} "${e.newText}"`,
      ),
    ]);
  };
  check(
    "codeAction remove an unused local",
    await fixes("Rails/UnusedStrictLocal", {
      start: { line: 0, character: 19 },
      end: { line: 0, character: 23 },
    }),
    [["Remove the local", ['17-26 ""']]],
  );
  check(
    "codeAction prefix an unused block argument",
    await fixes("Rails/UnusedBlockArgument", {
      start: { line: 1, character: 23 },
      end: { line: 1, character: 28 },
    }),
    [["Prefix the argument with `_`", ['23-23 "_"']]],
  );

  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: outlineUri,
        languageId: "slim",
        version: 1,
        text: outlineText,
      },
    },
    true,
  );
  const slimSyms = await send("textDocument/documentSymbol", {
    textDocument: { uri: outlineUri },
  });
  check("slim documentSymbols", (slimSyms.result || []).map(tree), [
    'main#page [content_for :aside [nav [render "card"]]]',
    "javascript:",
  ]);
  const slimFolds = await send("textDocument/foldingRange", {
    textDocument: { uri: outlineUri },
  });
  check(
    "slim foldingRanges",
    (slimFolds.result || []).map((f) => `${f.startLine}-${f.endLine}`),
    ["0-5", "1-2", "3-5", "7-8"],
  );
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: slimUri,
        languageId: "slim",
        version: 1,
        text: slimText,
      },
    },
    true,
  );
  check("haml diagnostics source", sources(hamlUri), ["alder"]);

  await new Promise((r) => setTimeout(r, 200));
  const slimDiag = notes.find(
    (n) =>
      n.method === "textDocument/publishDiagnostics" &&
      n.params.uri === slimUri,
  );
  check(
    "slim diagnostics",
    slimDiag
      ? slimDiag.params.diagnostics.map(
          (d) =>
            `${d.range.start.line + 1}:${d.range.start.character + 1} ${d.code}`,
        )
      : [],
    [
      "3:3 Syntax/Ruby",
      "4:1 Syntax/Ruby",
      "5:3 Html/VoidChildren",
      "8:1 Syntax/Ruby",
      "8:1 Syntax/Ruby",
    ],
  );
  check("slim diagnostics source", sources(slimUri), ["alder"]);

  // Haml's Ruby is checked once Haml accepts the template: an error in code, and a `rescue` inside an `if`.
  const hamlRubyUri = fileUri(path.join(views, "users", "ruby.html.haml"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: hamlRubyUri,
        languageId: "haml",
        version: 1,
        text: "%p\n  - x = [1 2]\n%a{href: x} t\n- if x\n  - rescue\n",
      },
    },
    true,
  );
  await new Promise((r) => setTimeout(r, 200));
  const hamlRuby = notes.find(
    (n) =>
      n.method === "textDocument/publishDiagnostics" &&
      n.params.uri === hamlRubyUri,
  );
  check(
    "haml ruby diagnostics",
    hamlRuby.params.diagnostics.map(
      (d) =>
        `${d.range.start.line + 1}:${d.range.start.character + 1} ${d.code}`,
    ),
    [
      "2:11 Syntax/Ruby",
      "5:5 Syntax/Ruby",
      "4:3 Syntax/Ruby",
      "5:3 Syntax/Ruby",
    ],
  );

  const editUri = fileUri(path.join(views, "users", "edit.html.slim"));
  const editText =
    'div\n    p= link_\n    a hr\n    jav\n    == raw x  \n    | text lin\n    p Hi #{link_\n    = render "card"\n';
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: editUri,
        languageId: "slim",
        version: 1,
        text: editText,
      },
    },
    true,
  );
  const expect = (label, ok, got) => {
    if (!ok) failures++;
    console.log(`${ok ? "ok  " : "FAIL"} ${label.padEnd(24)} →`, got);
  };
  const complete = async (line, character) =>
    (
      (
        await send("textDocument/completion", {
          textDocument: { uri: editUri },
          position: { line, character },
        })
      ).result || []
    ).map((i) => i.label);
  const helpers = await complete(1, 11);
  expect(
    "slim completion p= link_",
    helpers.includes("link_to"),
    helpers.slice(0, 3),
  );
  const attrs = await complete(2, 8);
  expect(
    "slim completion a hr",
    attrs.includes("href") && !attrs.includes("link_to"),
    attrs.filter((l) => l.startsWith("hr")),
  );
  const engines = await complete(3, 7);
  expect(
    "slim completion jav",
    engines.includes("javascript:"),
    engines.filter((l) => l.startsWith("jav")),
  );
  const inText = await complete(5, 14);
  expect("slim completion in text", inText.length === 0, inText);
  const openInterpolation = await complete(6, 16);
  expect(
    "slim completion #{link_",
    openInterpolation.includes("link_to"),
    openInterpolation.slice(0, 3),
  );
  const slimDef = (
    (
      await send("textDocument/definition", {
        textDocument: { uri: editUri },
        position: { line: 7, character: 15 },
      })
    ).result || []
  ).map((l) => rel(l.uri));
  expect(
    'slim definition "card"',
    slimDef.length === 1 && slimDef[0].endsWith("_card.html.slim"),
    slimDef,
  );
  const markerHover = await send("textDocument/hover", {
    textDocument: { uri: editUri },
    position: { line: 4, character: 5 },
  });
  const markerDoc = markerHover.result?.contents.value ?? "";
  expect(
    "slim hover ==",
    markerDoc.includes("unescaped output"),
    markerDoc.split("\n")[0],
  );
  const edits =
    (
      await send("textDocument/formatting", {
        textDocument: { uri: editUri },
        options: { tabSize: 2, insertSpaces: true },
      })
    ).result || [];
  const formatted =
    'div\n  p= link_\n  a hr\n  jav\n  == raw x\n  | text lin\n  p Hi #{link_\n  = render "card"\n';
  expect(
    "slim formatting",
    edits.length === 1 && edits[0].newText === formatted,
    JSON.stringify(edits[0]?.newText),
  );
  const tabs = await send("textDocument/formatting", {
    textDocument: { uri: editUri },
    options: { tabSize: 2, insertSpaces: false },
  });
  expect(
    "slim formatting, tabs",
    tabs.result?.length === 1 && tabs.result[0].newText === formatted,
    JSON.stringify(tabs.result?.[0]?.newText),
  );

  // ERB writes a tab per level when the editor says insertSpaces: false; Haml and Slim stay spaces.
  const tabsUri = fileUri(path.join(views, "users", "tabs.html.erb"));
  const tabsText = "<div>\n<p>\nx\n</p>\n</div>\n";
  const tabsOptions = { tabSize: 2, insertSpaces: false };
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: tabsUri,
        languageId: "erb",
        version: 1,
        text: tabsText,
      },
    },
    true,
  );
  const erbTabs = await send("textDocument/formatting", {
    textDocument: { uri: tabsUri },
    options: tabsOptions,
  });
  expect(
    "erb formatting, tabs",
    erbTabs.result?.length === 1 &&
      erbTabs.result[0].newText === "<div>\n\t<p>\n\t\tx\n\t</p>\n</div>\n",
    JSON.stringify(erbTabs.result?.[0]?.newText),
  );
  const erbTabsRange = await send("textDocument/rangeFormatting", {
    textDocument: { uri: tabsUri },
    options: tabsOptions,
    range: {
      start: { line: 2, character: 0 },
      end: { line: 2, character: 0 },
    },
  });
  expect(
    "erb range formatting, tabs",
    erbTabsRange.result?.length === 1 &&
      erbTabsRange.result[0].range.start.line === 2 &&
      erbTabsRange.result[0].newText === "\t\tx",
    JSON.stringify(erbTabsRange.result),
  );

  // Haml through the same formatter: the editor's width, the whole document or the lines of a range.
  const reindentUri = fileUri(path.join(views, "users", "reindent.html.haml"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: reindentUri,
        languageId: "haml",
        version: 1,
        // The formatter drops the empty lines at the end; range formatting still edits the lines above.
        text: "%div\n    %p\n        %b x\n    %i\n\n\n",
      },
    },
    true,
  );
  const hamlOptions = { tabSize: 2, insertSpaces: true };
  const hamlEdits =
    (
      await send("textDocument/formatting", {
        textDocument: { uri: reindentUri },
        options: hamlOptions,
      })
    ).result || [];
  expect(
    "haml formatting",
    hamlEdits[0]?.newText === "%div\n  %p\n    %b x\n  %i\n",
    JSON.stringify(hamlEdits[0]?.newText),
  );
  const hamlTabs = await send("textDocument/formatting", {
    textDocument: { uri: reindentUri },
    options: { tabSize: 2, insertSpaces: false },
  });
  expect(
    "haml formatting, insertSpaces false stays spaces",
    hamlTabs.result?.[0]?.newText === "%div\n  %p\n    %b x\n  %i\n",
    JSON.stringify(hamlTabs.result?.[0]?.newText),
  );
  const rangeEdits =
    (
      await send("textDocument/rangeFormatting", {
        textDocument: { uri: reindentUri },
        options: hamlOptions,
        range: {
          start: { line: 3, character: 0 },
          end: { line: 3, character: 0 },
        },
      })
    ).result || [];
  expect(
    "haml range formatting",
    rangeEdits.length === 1 &&
      rangeEdits[0].range.start.line === 3 &&
      rangeEdits[0].newText === "  %i",
    JSON.stringify(rangeEdits),
  );
  // Pug: the editor's width and style; a text block moves as a block and keeps its trailing spaces.
  const pugFormatUri = fileUri(path.join(views, "users", "reindent.jst.pug"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: pugFormatUri,
        languageId: "jade",
        version: 1,
        text: "ul\n    li hi  \n    = x  \n    script.\n        if (a) {\n          b();  \n        }\n\n\n",
      },
    },
    true,
  );
  const pugFormat = async (insertSpaces) =>
    (
      await send("textDocument/formatting", {
        textDocument: { uri: pugFormatUri },
        options: { tabSize: 2, insertSpaces },
      })
    ).result?.[0]?.newText;
  check(
    "pug formatting",
    await pugFormat(true),
    "ul\n  li hi  \n  = x\n  script.\n    if (a) {\n      b();  \n    }\n",
  );
  check(
    "pug formatting, tabs",
    await pugFormat(false),
    "ul\n\tli hi  \n\t= x\n\tscript.\n\t\tif (a) {\n\t\t  b();  \n\t\t}\n",
  );
  // Without dynamic registration nothing is advertised.
  const staticFormatting = init.result.capabilities.documentFormattingProvider;
  expect(
    "no static formatter",
    staticFormatting === undefined,
    staticFormatting,
  );

  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: phoneUri,
        languageId: "slim",
        version: 1,
        text: phoneText,
      },
    },
    true,
  );
  const missing = {
    start: { line: 6, character: 12 },
    end: { line: 6, character: 26 },
  };
  const actions = await send("textDocument/codeAction", {
    textDocument: { uri },
    range: missing,
    context: {
      diagnostics: [
        { range: missing, code: "Rails/MissingPartial", message: "not found" },
      ],
    },
  });
  check(
    "codeAction create partial",
    actions.result?.map((a) => [
      a.title,
      rel(a.edit.documentChanges[0].uri),
      a.edit.documentChanges[0].kind,
    ]),
    [
      [
        "Create partial `shared/_missing.html.erb`",
        "/app/views/shared/_missing.html.erb",
        "create",
      ],
    ],
  );
  const symbols = await send("workspace/symbol", { query: "shfoot" });
  check(
    "workspace/symbol shfoot",
    symbols.result?.map((s) => `${s.name} ${s.containerName}`),
    ["shared/footer partial"],
  );
  const linked = await req("textDocument/linkedEditingRange", uri, 8, 2);
  check(
    "linkedEditingRange nav",
    linked.result?.ranges.map((r) => `${r.start.line}:${r.start.character}`),
    ["8:1", "10:2"],
  );
  const refs = await send("textDocument/references", {
    textDocument: { uri },
    position: { line: 1, character: 16 },
    context: { includeDeclaration: false },
  });
  check(
    "references shared/footer",
    refs.result?.map((l) => `${rel(l.uri)} ${l.range.start.line}`).sort(),
    ["/app/views/users/index.html.erb 1", "/app/views/users/show.html.haml 3"],
  );
  const keyRefs = await send("textDocument/references", {
    textDocument: { uri },
    position: { line: 18, character: 8 },
    context: { includeDeclaration: true },
  });
  check(
    "references i18n key",
    keyRefs.result?.map((l) => `${rel(l.uri)} ${l.range.start.line}`).sort(),
    [
      "/app/mailers/user_mailer.rb 1",
      "/app/views/users/index.html.erb 18",
      "/config/locales/en.yml 3",
      "/config/locales/uk.yml 3",
    ],
  );
  const controllerUri = fileUri(controllerFile);
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: controllerUri,
        languageId: "ruby",
        version: 1,
        text: controllerText,
      },
    },
    true,
  );
  check(
    "controller action → template",
    targets(await req("textDocument/definition", controllerUri, 3, 7)),
    ["/app/views/users/index.html.erb"],
  );
  const controllerLinks = await send("textDocument/documentLink", {
    textDocument: { uri: controllerUri },
  });
  check(
    "controller links: layout and render",
    controllerLinks.result?.map(
      (l) => `${l.range.start.line} ${rel(l.target)}`,
    ),
    [
      "1 /app/views/layouts/admin.html.erb",
      "4 /app/views/users/index.html.erb",
    ],
  );
  check(
    "controller gets no completion",
    (await req("textDocument/completion", controllerUri, 4, 10)).result,
    null,
  );
  const helperUri = fileUri(path.join(views, "users", "helper.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: helperUri,
        languageId: "erb",
        version: 1,
        text: "<%= format_price(1) %> format_price\n",
      },
    },
    true,
  );
  check(
    "helper definition",
    [
      (await req("textDocument/definition", helperUri, 0, 6)).result?.map(
        (l) => `${rel(l.uri)}:${l.range.start.line}`,
      ),
      (await req("textDocument/definition", helperUri, 0, 26)).result,
    ],
    [["/app/helpers/users_helper.rb:1"], null],
  );
  check(
    "helper hover",
    [
      rel(
        (await req("textDocument/hover", helperUri, 0, 6)).result?.contents
          .value ?? "",
      ),
      (await req("textDocument/hover", helperUri, 0, 26)).result,
    ],
    [
      "```ruby\ndef format_price(x)\n```\n\n[app/helpers/users_helper.rb:2](/app/helpers/users_helper.rb#L2)",
      null,
    ],
  );
  const signatures = {
    erb: ['<%= link_to t(".x", count: 1), user_path(@user), ', "erb"],
    haml: ['= link_to "Users", ', "haml"],
    slim: ["= format_price 1, ", "slim"],
    html: ['<p>link_to "Users", </p>', "erb"],
    comment: ['<%= link_to "x", path,\n  # a, b\n  class: ', "erb"],
    slimLines: ['= link_to "x",\n  path, ', "slim"],
    options: ['<%= render partial: "x", locals: ', "erb"],
    interpolation: ['<%= link_to "#{format_price(1, ', "erb"],
  };
  const signatureAt = async (name) => {
    const [text, languageId] = signatures[name];
    const sigUri = fileUri(
      path.join(views, "users", `sig_${name}.html.${languageId}`),
    );
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: sigUri, languageId, version: 1, text } },
      true,
    );
    const lines = text.split("\n");
    const { result } = await req(
      "textDocument/signatureHelp",
      sigUri,
      lines.length - 1,
      lines.at(-1).length,
    );
    const info = result?.signatures[0];
    return (
      info && [
        info.label,
        result.activeParameter,
        info.documentation?.value.split("\n").pop(),
      ]
    );
  };
  check("signature help: ERB, nested call", await signatureAt("erb"), [
    "link_to(name, options = nil, html_options = nil)",
    2,
    "[API docs](https://api.rubyonrails.org/classes/ActionView/Helpers/UrlHelper.html#method-i-link_to)",
  ]);
  check("signature help: Haml", (await signatureAt("haml"))?.slice(0, 2), [
    "link_to(name, options = nil, html_options = nil)",
    1,
  ]);
  check(
    "signature help: Slim app helper",
    (await signatureAt("slim"))?.slice(0, 2),
    ["def format_price(x)", 1],
  );
  check(
    "signature help: none in HTML text",
    await signatureAt("html"),
    undefined,
  );
  check(
    "signature help: comment line in a tag",
    (await signatureAt("comment"))?.slice(0, 2),
    ["link_to(name, options = nil, html_options = nil)", 2],
  );
  check(
    "signature help: Slim call over lines",
    (await signatureAt("slimLines"))?.slice(0, 2),
    ["link_to(name, options = nil, html_options = nil)", 2],
  );
  check(
    "signature help: keywords are one hash",
    (await signatureAt("options"))?.slice(0, 2),
    ["render(options = {}, locals = {}, &block)", 0],
  );
  check(
    "signature help: open interpolation",
    (await signatureAt("interpolation"))?.slice(0, 2),
    ["def format_price(x)", 1],
  );
  const helperFile = path.join(root, "app", "helpers", "users_helper.rb");
  const helperText = fs.readFileSync(helperFile, "utf8");
  const rewriteHelper = async (text) => {
    fs.writeFileSync(helperFile, text);
    await send(
      "workspace/didChangeWatchedFiles",
      { changes: [{ uri: fileUri(helperFile), type: 2 }] },
      true,
    );
  };
  await rewriteHelper(helperText.replace("(x)", "(x, y)"));
  check(
    "signature help: app helper cache follows the file",
    (await signatureAt("slim"))?.[0],
    "def format_price(x, y)",
  );
  await rewriteHelper(helperText);
  const shadowUri = fileUri(path.join(views, "users", "shadow.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: shadowUri,
        languageId: "erb",
        version: 1,
        text: '<% [1].each do |format_price| %>\n<%= format_price %>\n<% end %>\n<%= "format_price" %>\n',
      },
    },
    true,
  );
  check(
    "no helper hover on a block parameter, its use or in a string",
    [
      (await req("textDocument/hover", shadowUri, 0, 22)).result,
      (await req("textDocument/hover", shadowUri, 1, 6)).result,
      (await req("textDocument/hover", shadowUri, 3, 8)).result,
    ],
    [null, null, null],
  );
  const tagsUri = fileUri(path.join(views, "users", "tags.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: tagsUri,
        languageId: "erb",
        version: 1,
        text: '<%= stylesheet_link_tag "application",\n                        "admin",\n                        "data-turbo-track": "reload" %>\n',
      },
    },
    true,
  );
  check(
    "definition on a stylesheet_link_tag continuation line",
    targets(await req("textDocument/definition", tagsUri, 1, 26)),
    ["/app/assets/stylesheets/admin.css"],
  );
  await send(
    "textDocument/didChange",
    {
      textDocument: { uri: helperUri, version: 2 },
      contentChanges: [{ text: "<%= @m %>\n<%= @menu.name %>\n<%= @tag %>\n" }],
    },
    true,
  );
  const cardUri = fileUri(path.join(views, "users", "_card.html.slim"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: cardUri,
        languageId: "slim",
        version: 1,
        text: "p = user.name\n",
      },
    },
    true,
  );
  const at = (r) =>
    r.result?.map(
      (l) => `${rel(l.uri)}:${l.range.start.line}:${l.range.start.character}`,
    );
  check(
    "ivars and locals",
    [
      (await req("textDocument/completion", helperUri, 0, 6)).result?.map(
        (i) => [i.label, i.textEdit.range.start.character],
      ),
      at(await req("textDocument/definition", helperUri, 1, 6)),
      at(await req("textDocument/definition", cardUri, 0, 5)),
      rel(
        (await req("textDocument/hover", helperUri, 1, 6)).result?.contents
          .value ?? "",
      ),
      (await req("textDocument/hover", helperUri, 2, 6)).result,
    ],
    [
      [["@menu", 4]],
      ["/app/controllers/users_controller.rb:9:0"],
      ["/app/views/users/index.html.erb:0:19"],
      "`@menu`\n\n- [app/controllers/users_controller.rb:10](/app/controllers/users_controller.rb#L10): `@menu = Menu.first`",
      null,
    ],
  );
  const navUri = fileUri(path.join(views, "shared", "_nav.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: navUri,
        languageId: "erb",
        version: 1,
        text: "<%= @current_user %>\n<%= @admin %>\n",
      },
    },
    true,
  );
  check(
    "ivars of ApplicationController in a folder without a controller",
    [
      at(await req("textDocument/definition", navUri, 0, 6)),
      (await req("textDocument/hover", navUri, 1, 6)).result,
    ],
    [["/app/controllers/application_controller.rb:5:0"], null],
  );
  await send(
    "textDocument/didChange",
    {
      textDocument: { uri: helperUri, version: 3 },
      contentChanges: [
        { text: "<%= render BadgeComponent.new do |b| %>\n  <% b.with_l %>\n" },
      ],
    },
    true,
  );
  const badgeUri = fileUri(
    path.join(root, "app", "components", "badge_component.html.haml"),
  );
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: badgeUri,
        languageId: "haml",
        version: 1,
        text: "- if ic\n",
      },
    },
    true,
  );
  check(
    "ViewComponent slots",
    [
      labels(await req("textDocument/completion", helperUri, 1, 13)),
      labels(await req("textDocument/completion", badgeUri, 0, 7)),
    ],
    [
      ["with_link", "with_links"],
      ["icon", "icon?"],
    ],
  );
  await send(
    "alder/routes",
    {
      root,
      routes: [
        {
          name: "users",
          verb: "GET",
          path: "/users(.:format)",
          action: "users#index",
          source: "config/routes.rb:2",
        },
        {
          name: "",
          verb: "POST",
          path: "/users(.:format)",
          action: "users#create",
        },
      ],
    },
    true,
  );
  await send(
    "textDocument/didChange",
    {
      textDocument: { uri: helperUri, version: 4 },
      contentChanges: [
        { text: '<%= link_to "x", users_p %>\n<%= users_path %>\n' },
      ],
    },
    true,
  );
  check(
    "route helpers: completion, hover, definition",
    [
      labels(await req("textDocument/completion", helperUri, 0, 24)),
      firstLine(await req("textDocument/hover", helperUri, 1, 6)),
      (await req("textDocument/definition", helperUri, 1, 6)).result?.map(
        (l) => `${rel(l.uri)}:${l.range.start.line}`,
      ),
    ],
    [["users_path"], "`users_path` → `/users`", ["/config/routes.rb:1"]],
  );
  const unknownRouteUri = fileUri(path.join(views, "users", "links.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: unknownRouteUri,
        languageId: "erb",
        version: 1,
        text: '<%= link_to "x", usrs_path(1) %>\n<%= users_path %>\n<%= asset_path("a.png") %>\n<%= main_app.dash_path %>\n',
      },
    },
    true,
  );
  check(
    "Rails/UnknownRoute: only the undefined helper, and none once the route arrives",
    [
      await waitForCodes(unknownRouteUri, ["0:Rails/UnknownRoute"]),
      await (async () => {
        const sendRoutes = (routes) =>
          send("alder/routes", { root, routes }, true);
        const users = { name: "users", verb: "GET", path: "/users(.:format)" };
        await sendRoutes([users, { name: "usrs", verb: "GET", path: "/usrs" }]);
        const codes = await waitForCodes(unknownRouteUri, []);
        // The routes the later checks expect.
        await sendRoutes([
          { ...users, action: "users#index", source: "config/routes.rb:2" },
          {
            name: "",
            verb: "POST",
            path: "/users(.:format)",
            action: "users#create",
          },
        ]);
        return codes;
      })(),
    ],
    [["0:Rails/UnknownRoute"], []],
  );
  const slimRubyUri = fileUri(path.join(views, "users", "_ruby.html.slim"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: slimRubyUri,
        languageId: "slim",
        version: 1,
        text: '/# locals: (user:)\n= @m\np #{users_p}\n= t("users.index.ti")\na href=us\n= t("users.index.title")\n',
      },
    },
    true,
  );
  check(
    "Slim: ivars, routes, i18n, locals",
    [
      labels(await req("textDocument/completion", slimRubyUri, 1, 4)),
      labels(await req("textDocument/completion", slimRubyUri, 2, 11)),
      labels(await req("textDocument/completion", slimRubyUri, 3, 19)),
      firstLine(await req("textDocument/hover", slimRubyUri, 5, 8)),
      labels(await req("textDocument/completion", slimRubyUri, 4, 9)),
    ],
    [
      ["@menu"],
      ["users_path"],
      ["users.index.title"],
      "`users.index.title`",
      ["user", "users_path", "users_url"],
    ],
  );
  const blockDocs = {
    erb: "<%# locals: (user:) %>\n<% @users.each do |user| %>\n<%= us %>\n<% end %>\n<%= us %>\n",
    slim: "- @users.each do |user|\n  = us\n= us\n",
    haml: "- @users.each do |user|\n  %p= us\n= us\n",
  };
  const blockVariables = {};
  for (const [lang, text] of Object.entries(blockDocs)) {
    const blockUri = fileUri(path.join(views, "users", `_each.html.${lang}`));
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: blockUri, languageId: lang, version: 1, text } },
      true,
    );
    const lines = text.split("\n");
    const at = async (line) =>
      (
        await req(
          "textDocument/completion",
          blockUri,
          line,
          lines[line].indexOf("us") + 2,
        )
      ).result
        ?.filter((i) => i.label === "user")
        .map((i) => i.detail);
    const typed = lines.flatMap((l, i) => (/\bus\b/.test(l) ? [i] : []));
    blockVariables[lang] = [await at(typed[0]), await at(typed[1])];
  }
  check("block variables in scope", blockVariables, {
    erb: [["block variable"], ["local (required)"]],
    slim: [["block variable"], []],
    haml: [["block variable"], []],
  });
  const keyDocs = {
    erb: '<%= render "strict/row", a: 1, ^ %>\n',
    slim: '= render "strict/row",\n    b: 2,\n    ^\n',
    haml: '= render("strict/row", ^, b: 2)\n',
    haml_pipes: '= render "strict/row", b: 2, |\n  ^ |\np x\n',
  };
  const renderKeys = {};
  for (const [lang, marked] of Object.entries(keyDocs)) {
    const ext = lang.split("_")[0];
    const keysUri = fileUri(
      path.join(views, "strict", `keys_${lang}.html.${ext}`),
    );
    const text = marked.replace("^", "");
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: keysUri, languageId: ext, version: 1, text } },
      true,
    );
    const before = marked.slice(0, marked.indexOf("^")).split("\n");
    const r = await req(
      "textDocument/completion",
      keysUri,
      before.length - 1,
      before.at(-1).length,
    );
    renderKeys[lang] = r.result?.map((i) => `${i.insertText} (${i.detail})`);
  }
  check("render locals keys", renderKeys, {
    erb: ["b:  (local = 1)"],
    slim: ["a:  (local (required))"],
    haml: ["a:  (local (required))"],
    haml_pipes: ["a:  (local (required))"],
  });
  const pickUri = fileUri(path.join(views, "strict", "keys_pick.html.erb"));
  const pickText =
    '<%= render partial: "strict/pick", formats: [:json],\n  locals: { } %>\n';
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: pickUri,
        languageId: "erb",
        version: 1,
        text: pickText,
      },
    },
    true,
  );
  check(
    "render locals keys follow formats:",
    (await req("textDocument/completion", pickUri, 1, 12)).result?.map(
      (i) => i.label,
    ),
    ["json_key"],
  );
  const rowUri = fileUri(path.join(views, "strict", "_row.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: rowUri,
        languageId: "erb",
        version: 1,
        text: "<%# locals: (a:, b: 1) %>\n<%= a %> <%= b %> <%= link_to %>\n<% [1].each do |a| %><%= a %><% end %><%= f(b: 1) %>\n",
      },
    },
    true,
  );
  const hoverText = async (character) =>
    (await req("textDocument/hover", rowUri, 1, character)).result?.contents
      .value ?? null;
  check(
    "partial local hover",
    [
      await hoverText(4),
      await hoverText(13),
      (await hoverText(23))?.split("\n")[0],
      (await req("textDocument/hover", rowUri, 2, 25)).result,
      (await req("textDocument/hover", rowUri, 2, 44)).result,
    ],
    [
      "`a`: local, required\n\nPassed by 1 `render` call",
      "`b`: local, default `1`\n\nNo `render` passes it",
      "```ruby",
      null,
      null,
    ],
  );
  const prepare = async (line, character) =>
    (await req("textDocument/prepareRename", rowUri, line, character)).result;
  const localRename = await send("textDocument/rename", {
    textDocument: { uri: rowUri },
    position: { line: 1, character: 4 },
    newName: "item",
  });
  const refusedName = async (newName) =>
    (
      await send("textDocument/rename", {
        textDocument: { uri: rowUri },
        position: { line: 1, character: 4 },
        newName,
      })
    ).error?.code ?? null;
  const flat = (changes) =>
    Object.entries(changes ?? {})
      .flatMap(([u, es]) =>
        es.map(
          (e) =>
            `${rel(u)} ${e.range.start.line}:${e.range.start.character}-${e.range.end.character} ${e.newText}`,
        ),
      )
      .sort();
  const localRefs = (
    await send("textDocument/references", {
      textDocument: { uri: rowUri },
      position: { line: 1, character: 4 },
      context: { includeDeclaration: true },
    })
  ).result
    .map(
      (l) => `${rel(l.uri)} ${l.range.start.line}:${l.range.start.character}`,
    )
    .sort();
  check(
    "rename and references of a partial local",
    [
      await prepare(1, 4),
      await prepare(0, 13),
      await prepare(1, 23),
      await prepare(2, 25),
      flat(localRename.result?.changes),
      localRefs,
      await refusedName("class"),
      await refusedName("link_to"),
      await refusedName("b"),
    ],
    [
      { start: { line: 1, character: 4 }, end: { line: 1, character: 5 } },
      { start: { line: 0, character: 13 }, end: { line: 0, character: 14 } },
      null,
      null,
      [
        "/app/views/strict/_row.html.erb 0:13-14 item",
        "/app/views/strict/_row.html.erb 1:4-5 item",
        "/app/views/strict/list.html.erb 0:25-26 item",
      ],
      [
        "/app/views/strict/_row.html.erb 0:13",
        "/app/views/strict/_row.html.erb 1:4",
        "/app/views/strict/list.html.erb 0:25",
      ],
      -32602,
      -32602,
      -32602,
    ],
  );
  const i18nDocs = {
    slim: '= t(".title")\n= t(".ti")\n',
    haml: '= t(".title")\n%p= t(".ti")\n',
  };
  const i18nResults = {};
  for (const [lang, text] of Object.entries(i18nDocs)) {
    const i18nUri = fileUri(path.join(views, "users", `index.html.${lang}`));
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: i18nUri, languageId: lang, version: 1, text } },
      true,
    );
    const typed = text.split("\n")[1].length;
    i18nResults[lang] = [
      firstLine(await req("textDocument/hover", i18nUri, 0, 8)),
      (await req("textDocument/definition", i18nUri, 0, 8)).result?.map(
        (l) => `${rel(l.uri)}:${l.range.start.line}`,
      ),
      labels(await req("textDocument/completion", i18nUri, 1, typed - 2)),
    ];
  }
  const i18nExpected = [
    "`users.index.title`",
    ["/config/locales/en.yml:3", "/config/locales/uk.yml:3"],
    [".title"],
  ];
  check("lazy i18n keys in Slim and Haml", i18nResults, {
    slim: i18nExpected,
    haml: i18nExpected,
  });
  const streamUri = fileUri(
    path.join(views, "users", "create.turbo_stream.erb"),
  );
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: streamUri,
        languageId: "erb",
        version: 1,
        text: "<%= turbo_stream.rep %>\n",
      },
    },
    true,
  );
  check(
    "turbo_stream. completion",
    labels(await req("textDocument/completion", streamUri, 0, 20)),
    ["replace", "replace_all"],
  );
  await send(
    "textDocument/didChange",
    {
      textDocument: { uri: streamUri, version: 2 },
      contentChanges: [{ text: '<div data-action="click->he">\n' }],
    },
    true,
  );
  check(
    "stimulus completion",
    labels(await req("textDocument/completion", streamUri, 0, 27)),
    ["hello"],
  );
  await send(
    "textDocument/didChange",
    {
      textDocument: { uri: streamUri, version: 3 },
      contentChanges: [
        { text: '<p>render "users/card"</p> <%= render "users/card" %>\n' },
      ],
    },
    true,
  );
  const streamLinks = await send("textDocument/documentLink", {
    textDocument: { uri: streamUri },
  });
  check(
    "documentLink skips prose",
    streamLinks.result?.map((l) => l.range.start.character),
    [39],
  );
  await send(
    "textDocument/didChange",
    {
      textDocument: { uri: streamUri, version: 4 },
      contentChanges: [
        {
          text: "<%= form_with model: @user do |f| %>\n  <%= f.text_field :n\n",
        },
      ],
    },
    true,
  );
  check(
    "schema attribute completion",
    labels(await req("textDocument/completion", streamUri, 1, 21)),
    ["name", "nickname"],
  );
  await send(
    "textDocument/didChange",
    {
      textDocument: { uri: streamUri, version: 5 },
      contentChanges: [
        { text: "<p>Please fill in the form.</p>\n<p>Clear the cache</p>\n" },
      ],
    },
    true,
  );
  check(
    "no form builder in prose",
    labels(await req("textDocument/completion", streamUri, 0, 26)),
    [],
  );
  check(
    "no helper hover in prose",
    (await req("textDocument/hover", streamUri, 1, 16)).result ?? null,
    null,
  );
  await send(
    "textDocument/didChange",
    {
      textDocument: { uri: streamUri, version: 6 },
      contentChanges: [{ text: "<section>\n  <butt\n  <a hre\n  </\n" }],
    },
    true,
  );
  const html = async (line, character) =>
    (
      await req("textDocument/completion", streamUri, line, character)
    ).result?.map((i) => i.insertText ?? i.label);
  check("html element completion", await html(1, 7), ["button"]);
  check("html attribute completion", await html(2, 8), [
    'href="$1"',
    'hreflang="$1"',
  ]);
  check("html close completion", await html(3, 4), ["section>"]);
  await send(
    "textDocument/didChange",
    {
      textDocument: { uri: streamUri, version: 7 },
      contentChanges: [{ text: "<p><%=foo%></p>\n<%= ok %>\n" }],
    },
    true,
  );
  const spacing = (only) =>
    send("textDocument/codeAction", {
      textDocument: { uri: streamUri },
      range: {
        start: { line: 0, character: 5 },
        end: { line: 0, character: 5 },
      },
      context: { diagnostics: [], only },
    });
  const spacingActions = (r) =>
    r.result?.map((a) => [
      a.title,
      a.kind,
      a.edit.changes[streamUri].map((e) => e.newText),
    ]);
  check(
    "codeAction ERB tag spacing",
    spacingActions(await spacing(undefined)),
    [["Add spaces inside this ERB tag", "refactor.rewrite", ["<%= foo %>"]]],
  );
  check(
    "source action ERB spacing",
    spacingActions(await spacing(["source"]))?.filter(([, kind]) =>
      kind.startsWith("source"),
    ),
    [["Add spaces inside every ERB tag", "source.erbSpacing", ["<%= foo %>"]]],
  );
  await send(
    "textDocument/didChange",
    {
      textDocument: { uri: streamUri, version: 8 },
      contentChanges: [{ text: "<%= render Bad\n" }],
    },
    true,
  );
  const components = await req("textDocument/completion", streamUri, 0, 14);
  check(
    "component completion",
    components.result?.map((i) => [i.label, i.textEdit.newText]),
    [["BadgeComponent", "BadgeComponent.new($1)"]],
  );
  await send(
    "textDocument/didChange",
    {
      textDocument: { uri: streamUri, version: 9 },
      contentChanges: [
        { text: '<b data-action="hello#">\n<i data-hello-target="">\n' },
      ],
    },
    true,
  );
  check(
    "stimulus action and target completion",
    [
      labels(await req("textDocument/completion", streamUri, 0, 22)),
      labels(await req("textDocument/completion", streamUri, 1, 22)),
    ],
    [["greet"], ["name"]],
  );
  const jbuilderUri = fileUri(path.join(views, "users", "index.json.jbuilder"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: jbuilderUri,
        languageId: "ruby",
        version: 1,
        text: 'json.array! @users, partial: "users/user", as: :user\njson.partial! "users/user", user: @user\njson.tags @tags, label: "#tag", partial: "users/user"\n',
      },
    },
    true,
  );
  check(
    "jbuilder array! partial:",
    targets(await req("textDocument/definition", jbuilderUri, 0, 33)),
    ["/app/views/users/_user.json.jbuilder"],
  );
  check(
    "jbuilder partial!",
    targets(await req("textDocument/definition", jbuilderUri, 1, 18)),
    ["/app/views/users/_user.json.jbuilder"],
  );
  check(
    "jbuilder partial: after a # in a string",
    targets(await req("textDocument/definition", jbuilderUri, 2, 45)),
    ["/app/views/users/_user.json.jbuilder"],
  );
  check(
    "jbuilder completion",
    labels(
      await send("textDocument/completion", {
        textDocument: { uri: jbuilderUri },
        position: { line: 1, character: 21 },
      }),
    ),
    ["users/user"],
  );
  const card = path.join(views, "users", "_card.html.slim");
  const renamed = await send("workspace/willRenameFiles", {
    files: [
      {
        oldUri: fileUri(card),
        newUri: fileUri(card.replace("_card", "_tile")),
      },
    ],
  });
  const renameEdits = Object.entries(renamed.result?.changes ?? {})
    .flatMap(([u, es]) =>
      es.map(
        (e) =>
          `${rel(decodeURIComponent(u))} ${e.range.start.line}:${e.range.start.character} ${e.newText}`,
      ),
    )
    .sort();
  check("willRenameFiles _card", renameEdits, [
    "/app/views/users/edit.html+phone.slim 0:10 tile",
    // Open and never saved: renamed in the buffer too.
    "/app/views/users/edit.html.slim 7:14 tile",
    "/app/views/users/index.html.erb 0:12 tile",
    "/app/views/users/index.html.erb 9:12 tile",
    "/app/views/users/outline.html.slim 2:18 tile",
    "/app/views/users/show.html.haml 1:16 tile",
  ]);
  // Each Slim file read for a rename uses the extra_engines of its own folder's .alder.toml, else the setting (mjml).
  const engineViews = path.join(root, "slimapp", "app", "views");
  const engineBody = 'mjml:\n  = render "users/tile"\n';
  for (const dir of ["users", "mjml", "plain"]) {
    fs.mkdirSync(path.join(engineViews, dir), { recursive: true });
  }
  fs.writeFileSync(path.join(engineViews, "users", "_tile.html.slim"), "p\n");
  fs.writeFileSync(path.join(engineViews, "plain", "a.html.slim"), engineBody);
  fs.writeFileSync(path.join(engineViews, "mjml", "a.html.slim"), engineBody);
  fs.writeFileSync(
    path.join(engineViews, "plain", ".alder.toml"),
    "[slim]\nextra_engines = []\n",
  );
  const tile = path.join(engineViews, "users", "_tile.html.slim");
  const engineRenamed = await send("workspace/willRenameFiles", {
    files: [
      {
        oldUri: fileUri(tile),
        newUri: fileUri(tile.replace("_tile", "_chip")),
      },
    ],
  });
  check(
    "willRenameFiles reads each Slim file with its own extra_engines",
    Object.keys(engineRenamed.result?.changes ?? {}).map((u) =>
      rel(decodeURIComponent(u)),
    ),
    ["/slimapp/app/views/plain/a.html.slim"],
  );
  // A changed .ruby-version changes the grammar Ruby is parsed with, so open documents are checked again.
  const versionedUri = fileUri(path.join(views, "users", "versioned.html.erb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: versionedUri,
        languageId: "erb",
        version: 1,
        text: "<% x = a\n  && b %>\n",
      },
    },
    true,
  );
  const publishedFor = (u) =>
    notes.filter(
      (n) =>
        n.method === "textDocument/publishDiagnostics" && n.params.uri === u,
    );
  await new Promise((r) => setTimeout(r, 200));
  check(
    "no .ruby-version",
    publishedFor(versionedUri).at(-1)?.params.diagnostics.length,
    0,
  );
  const rubyVersionFile = path.join(root, ".ruby-version");
  fs.writeFileSync(rubyVersionFile, "3.3.0\n");
  await send(
    "workspace/didChangeWatchedFiles",
    { changes: [{ uri: fileUri(rubyVersionFile), type: 1 }] },
    true,
  );
  await new Promise((r) => setTimeout(r, 200));
  check(
    ".ruby-version 3.3.0",
    publishedFor(versionedUri)
      .at(-1)
      ?.params.diagnostics.map((d) => d.message),
    ["unexpected '&&', ignoring it"],
  );
  // Diagnostics run off the server task: edits typed meanwhile supersede them, and only the latest version publishes.
  for (const version of [2, 3, 4]) {
    await send(
      "textDocument/didChange",
      {
        textDocument: { uri: versionedUri, version },
        contentChanges: [{ text: `<%= f(${version},,) %>\n` }],
      },
      true,
    );
  }
  await new Promise((r) => setTimeout(r, 400));
  const lastPublished = publishedFor(versionedUri).at(-1)?.params;
  check(
    "diagnostics for the latest edit only",
    [
      lastPublished?.version,
      publishedFor(versionedUri).some((n) => [2, 3].includes(n.params.version)),
    ],
    [4, false],
  );
  // Two apps in one workspace: every lookup resolves against the app the file belongs to.
  const multiRoot = fs.realpathSync(
    fs.mkdtempSync(path.join(os.tmpdir(), "alder-multi-")),
  );
  const multi = startServer();
  const apps = ["a", "b"].map((name) => {
    const dir = path.join(multiRoot, `app_${name}`);
    const route = name === "a" ? "users" : "posts";
    fs.mkdirSync(path.join(dir, "app/views/shared"), { recursive: true });
    fs.mkdirSync(path.join(dir, "app/views/pages"), { recursive: true });
    fs.mkdirSync(path.join(dir, "config/locales"), { recursive: true });
    fs.writeFileSync(
      path.join(dir, "Gemfile.lock"),
      "GEM\n  specs:\n    actionview (7.1.3)\n",
    );
    fs.writeFileSync(
      path.join(dir, "app/views/shared/_card.html.erb"),
      `<p>${name}</p>\n`,
    );
    fs.writeFileSync(
      path.join(dir, "config/locales/en.yml"),
      `en:\n  greet:\n    hello: "Hi ${name}"\n    only_${name}: "x"\n`,
    );
    const page = fileUri(path.join(dir, "app/views/pages/show.html.erb"));
    const text = `<%= render "shared/card" %>\n<%= t("greet.hello") %>\n<%= ${route}_p %>\n<%= ${route}_path %>\n<%= t("greet.on") %>\n`;
    return { name, dir, route, page, text };
  });
  await multi.send("initialize", {
    processId: null,
    rootUri: fileUri(apps[0].dir),
    workspaceFolders: apps.map(({ name, dir }) => ({
      name,
      uri: fileUri(dir),
    })),
    capabilities: {},
  });
  await multi.send("initialized", {}, true);
  for (const { dir, route, page, text } of apps) {
    const routes = [
      {
        name: route,
        verb: "GET",
        path: `/${route}(.:format)`,
        action: `${route}#index`,
        source: "config/routes.rb:2",
      },
    ];
    await multi.send("alder/routes", { root: dir, routes }, true);
    await multi.send(
      "textDocument/didOpen",
      { textDocument: { uri: page, languageId: "erb", version: 1, text } },
      true,
    );
  }
  const ask = (method, u, line, character) =>
    multi.send(method, {
      textDocument: { uri: u },
      position: { line, character },
    });
  const where = (r) =>
    (r.result || []).map(
      (l) =>
        `${path.relative(multiRoot, fileURLToPath(l.uri)).replaceAll(path.sep, "/")}:${l.range.start.line}`,
    );
  const lookups = [];
  for (const { name, route, page, text } of apps) {
    lookups.push([
      where(await ask("textDocument/definition", page, 0, 14)),
      where(await ask("textDocument/definition", page, 1, 10)),
      (
        await ask("textDocument/hover", page, 1, 10)
      ).result?.contents.value.includes(`Hi ${name}`),
      (
        await ask(
          "textDocument/completion",
          page,
          2,
          text.split("\n")[2].length - 3,
        )
      ).result?.map((i) => i.label),
      (
        await ask("textDocument/hover", page, 3, 6)
      ).result?.contents.value.split("\n")[0],
      where(await ask("textDocument/definition", page, 3, 6)),
      (await ask("textDocument/completion", page, 4, 15)).result?.map(
        (i) => i.label,
      ),
    ]);
  }
  check("two apps in one workspace", lookups, [
    [
      ["app_a/app/views/shared/_card.html.erb:0"],
      ["app_a/config/locales/en.yml:2"],
      true,
      ["users_path"],
      "`users_path` → `/users`",
      ["app_a/config/routes.rb:1"],
      ["greet.only_a"],
    ],
    [
      ["app_b/app/views/shared/_card.html.erb:0"],
      ["app_b/config/locales/en.yml:2"],
      true,
      ["posts_path"],
      "`posts_path` → `/posts`",
      ["app_b/config/routes.rb:1"],
      ["greet.only_b"],
    ],
  ]);
  await multi.send("shutdown", {});
  await multi.send("exit", {}, true);
  fs.rmSync(multiRoot, { recursive: true, force: true });
  // Per-language switches from the client's initializationOptions: no diagnostics, no formatting edits.
  const off = startServer();
  await off.send("initialize", {
    processId: null,
    rootUri: fileUri(root),
    capabilities: {},
    initializationOptions: {
      erb: { diagnostics: { enabled: false }, format: { enabled: false } },
      haml: { format: { enabled: false } },
      pug: { format: { enabled: false } },
    },
  });
  await off.send("initialized", {}, true);
  const open = (u, languageId, text) =>
    off.send(
      "textDocument/didOpen",
      { textDocument: { uri: u, languageId, version: 1, text } },
      true,
    );
  await open(uri, "erb", text);
  await open(reindentUri, "haml", "%div\n    %p\n");
  await open(hamlUri, "haml", hamlText);
  await open(editUri, "slim", "div\n    p x\n");
  await new Promise((r) => setTimeout(r, 300));
  const diagnosticsOf = (u) =>
    off.notes
      .filter(
        (n) =>
          n.method === "textDocument/publishDiagnostics" && n.params.uri === u,
      )
      .at(-1)?.params.diagnostics.length;
  check("erb diagnostics off", diagnosticsOf(uri) ?? 0, 0);
  expect(
    "haml diagnostics stay on",
    diagnosticsOf(hamlUri) > 0,
    diagnosticsOf(hamlUri),
  );
  const formatResult = async (u) =>
    (
      await off.send("textDocument/formatting", {
        textDocument: { uri: u },
        options: { tabSize: 2, insertSpaces: true },
      })
    ).result;
  check("haml format off", await formatResult(reindentUri), null);
  await open(pugFormatUri, "jade", "ul\n    li\n");
  check("pug format off", await formatResult(pugFormatUri), null);
  const rangeOff = await off.send("textDocument/rangeFormatting", {
    textDocument: { uri: reindentUri },
    options: { tabSize: 2, insertSpaces: true },
    range: { start: { line: 1, character: 0 }, end: { line: 1, character: 0 } },
  });
  check("haml range format off", rangeOff.result, null);
  expect(
    "slim format stays on",
    (await formatResult(editUri))?.[0]?.newText === "div\n  p x\n",
  );
  await off.send("shutdown", {});
  await off.send("exit", {}, true);
  // `erb.format.alignComments` moves a comment's lines with the line it opens on.
  const aligned = startServer();
  await aligned.send("initialize", {
    processId: null,
    rootUri: fileUri(root),
    capabilities: {},
    initializationOptions: { erb: { format: { alignComments: true } } },
  });
  await aligned.send("initialized", {}, true);
  const commentUri = fileUri(path.join(views, "users", "comment.html.erb"));
  await aligned.send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: commentUri,
        languageId: "erb",
        version: 1,
        text: "<div>\n<%# a\n  b %>\n</div>\n",
      },
    },
    true,
  );
  const alignedEdits = (
    await aligned.send("textDocument/formatting", {
      textDocument: { uri: commentUri },
      options: { tabSize: 2, insertSpaces: true },
    })
  ).result;
  check(
    "erb comments aligned",
    alignedEdits?.[0]?.newText,
    "<div>\n  <%# a\n    b %>\n</div>\n",
  );
  const alignedRange = (
    await aligned.send("textDocument/rangeFormatting", {
      textDocument: { uri: commentUri },
      options: { tabSize: 2, insertSpaces: true },
      range: {
        start: { line: 2, character: 0 },
        end: { line: 2, character: 0 },
      },
    })
  ).result;
  check(
    "erb comments kept in a range without their first line",
    alignedRange ?? [],
    [],
  );
  // Layout/EmptyLines removes lines in Format Document, never in Format Selection.
  const blankDir = path.join(views, "blank");
  fs.mkdirSync(blankDir);
  fs.writeFileSync(
    path.join(blankDir, ".alder.toml"),
    "[Layout.EmptyLines]\nenabled = true\nmax = 0\n",
  );
  const blankUri = fileUri(path.join(blankDir, "a.html.erb"));
  await aligned.send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: blankUri,
        languageId: "erb",
        version: 1,
        text: "<div>\n<p>a</p>\n\n\n<p>b</p>\n\n</div>\n",
      },
    },
    true,
  );
  const blankOptions = { tabSize: 2, insertSpaces: true };
  check(
    "empty lines cut in the document",
    (
      await aligned.send("textDocument/formatting", {
        textDocument: { uri: blankUri },
        options: blankOptions,
      })
    ).result?.[0]?.newText,
    "<div>\n  <p>a</p>\n  <p>b</p>\n</div>\n",
  );
  const blankRange = (
    await aligned.send("textDocument/rangeFormatting", {
      textDocument: { uri: blankUri },
      options: blankOptions,
      range: {
        start: { line: 0, character: 0 },
        end: { line: 6, character: 0 },
      },
    })
  ).result;
  check(
    "empty lines kept in a range",
    blankRange?.map((e) => [e.range.start.line, e.range.end.line, e.newText]),
    [
      [1, 1, "  <p>a</p>"],
      [4, 4, "  <p>b</p>"],
    ],
  );
  // Layout/ElementContentOnOwnLine adds lines: in Format Document, never in Format Selection.
  const contentDir = path.join(views, "content");
  fs.mkdirSync(contentDir);
  fs.writeFileSync(
    path.join(contentDir, ".alder.toml"),
    "[Layout.ElementContentOnOwnLine]\nenabled = true\n",
  );
  const contentUri = fileUri(path.join(contentDir, "a.html.erb"));
  await aligned.send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: contentUri,
        languageId: "erb",
        version: 1,
        text: "<div>\n<div><%= a %></div>\n</div>\n",
      },
    },
    true,
  );
  check(
    "content moved in the document",
    (
      await aligned.send("textDocument/formatting", {
        textDocument: { uri: contentUri },
        options: blankOptions,
      })
    ).result?.[0]?.newText,
    "<div>\n  <div>\n    <%= a %>\n  </div>\n</div>\n",
  );
  const contentRange = (
    await aligned.send("textDocument/rangeFormatting", {
      textDocument: { uri: contentUri },
      options: blankOptions,
      range: {
        start: { line: 0, character: 0 },
        end: { line: 3, character: 0 },
      },
    })
  ).result;
  check(
    "content kept on its line in a range",
    contentRange?.map((e) => [e.range.start.line, e.range.end.line, e.newText]),
    [[1, 1, "  <div><%= a %></div>"]],
  );
  // Layout/AttributesPerLine adds lines too: in Format Document, never in Format Selection.
  const perLineDir = path.join(views, "perline");
  fs.mkdirSync(perLineDir);
  fs.writeFileSync(
    path.join(perLineDir, ".alder.toml"),
    "[Layout.AttributesPerLine]\nenabled = true\nmax_line_length = 30\n",
  );
  const perLineUri = fileUri(path.join(perLineDir, "a.html.erb"));
  await aligned.send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: perLineUri,
        languageId: "erb",
        version: 1,
        text: '<div>\n<p class="a" id="b" title="c">x</p>\n</div>\n',
      },
    },
    true,
  );
  check(
    "attributes split in the document",
    (
      await aligned.send("textDocument/formatting", {
        textDocument: { uri: perLineUri },
        options: blankOptions,
      })
    ).result?.[0]?.newText,
    '<div>\n  <p\n    class="a"\n    id="b"\n    title="c">x</p>\n</div>\n',
  );
  const perLineRange = (
    await aligned.send("textDocument/rangeFormatting", {
      textDocument: { uri: perLineUri },
      options: blankOptions,
      range: {
        start: { line: 0, character: 0 },
        end: { line: 3, character: 0 },
      },
    })
  ).result;
  check(
    "attributes kept on their line in a range",
    perLineRange?.map((e) => [e.range.start.line, e.range.end.line, e.newText]),
    [[1, 1, '  <p class="a" id="b" title="c">x</p>']],
  );
  // The in-line rules keep every line, so Format Selection applies them to the lines it covers.
  const inlineDir = path.join(views, "inline");
  fs.mkdirSync(inlineDir);
  fs.writeFileSync(
    path.join(inlineDir, ".alder.toml"),
    [
      "Layout.SpaceInsideErbTag",
      "Layout.SpaceInHtmlTag",
      "Style.ErbCommentSyntax",
      "Layout.SpaceAfterScriptMarker",
    ]
      .map((rule) => `[${rule}]\nenabled = true\n`)
      .join(""),
  );
  const inlineTexts = {
    "a.html.erb": [
      '<% # a %>\n<p  id = "b" ><%=b%></p>\n<p  id = "c" ><%=c%></p>\n',
      [[1, '<p id="b"><%= b %></p>']],
    ],
    "a.html.haml": ["=a\n=b\n=c\n", [[1, "= b"]]],
    "a.html.slim": ["p=a\np=b\np=c\n", [[1, "p= b"]]],
  };
  for (const [name, [text, lines]] of Object.entries(inlineTexts)) {
    const uri = fileUri(path.join(inlineDir, name));
    await aligned.send(
      "textDocument/didOpen",
      {
        textDocument: {
          uri,
          languageId: name.split(".").pop(),
          version: 1,
          text,
        },
      },
      true,
    );
    const edits = (
      await aligned.send("textDocument/rangeFormatting", {
        textDocument: { uri },
        options: blankOptions,
        range: {
          start: { line: 1, character: 0 },
          end: { line: 1, character: 3 },
        },
      })
    ).result;
    check(
      `${name}: a range gets the in-line rules on its lines only`,
      edits?.map((e) => [e.range.start.line, e.newText]),
      lines,
    );
  }

  // The rules around bodies and branches add and remove lines: not in Format Selection.
  const aroundDir = path.join(views, "around");
  fs.mkdirSync(aroundDir);
  const aroundToml = path.join(aroundDir, ".alder.toml");
  const aroundOpen = async (name, text) => {
    const uri = fileUri(path.join(aroundDir, name));
    await aligned.send(
      "textDocument/didOpen",
      {
        textDocument: {
          uri,
          languageId: name.split(".").pop(),
          version: 1,
          text,
        },
      },
      true,
    );
    return uri;
  };
  const aroundTexts = {
    "a.html.haml": "%div\n    %p x\n\n    %i y\n\n",
    "a.html.slim": "div\n    p x\n\n    i y\n\n",
    "a.html.erb":
      "<div>\n<% if a %>\n\n<p>x</p>\n\n<% else %>\n<p>y</p>\n<% end %>\n\n</div>\n",
  };
  for (const style of ["empty_lines", "no_empty_lines"]) {
    fs.writeFileSync(
      aroundToml,
      ["BlockBody", "ElementBody", "Branches"]
        .map(
          (rule) =>
            `[Layout.EmptyLinesAround${rule}]\nenabled = true\nstyle = "${style}"\n`,
        )
        .join(""),
    );
    for (const [name, text] of Object.entries(aroundTexts)) {
      const uri = await aroundOpen(`${style}-${name}`, text);
      const lines = text.split("\n").length;
      const edits = (
        await aligned.send("textDocument/rangeFormatting", {
          textDocument: { uri },
          options: blankOptions,
          range: {
            start: { line: 0, character: 0 },
            end: { line: lines, character: 0 },
          },
        })
      ).result;
      // Each edit replaces the text of one line; applied, the file keeps its lines and its content.
      const result = text.split("\n");
      for (const e of edits ?? []) {
        result.splice(
          e.range.start.line,
          e.range.end.line - e.range.start.line + 1,
          ...e.newText.split("\n"),
        );
      }
      check(
        `${style} ${name}: a range keeps the lines and the content`,
        [result.length, result.join("\n").replace(/\s/g, "")],
        [lines, text.replace(/\s/g, "")],
      );
    }
  }

  // .alder.toml: rules and switches per folder, and its own problems.
  const rulesRoot = fs.realpathSync(
    fs.mkdtempSync(path.join(os.tmpdir(), "alder-rules-")),
  );
  fs.mkdirSync(path.join(rulesRoot, "sub"));
  const rulesToml = path.join(rulesRoot, ".alder.toml");
  const quiet =
    "[Html.UnclosedElement]\nenabled = false\n[erb]\nformat.enabled = false\n";
  fs.writeFileSync(rulesToml, quiet);
  fs.writeFileSync(
    path.join(rulesRoot, "sub", ".alder.toml"),
    "[Html.UnclosedElement]\nenabled = true\n[erb]\nformat.enabled = true\n",
  );
  const unclosed = "<div>\n<%= x %>\n";
  const rulesServer = startServer();
  await rulesServer.send("initialize", {
    processId: null,
    rootUri: fileUri(rulesRoot),
    capabilities: {},
  });
  await rulesServer.send("initialized", {}, true);
  const rulesOpen = (name, text) =>
    rulesServer.send(
      "textDocument/didOpen",
      {
        textDocument: {
          uri: fileUri(path.join(rulesRoot, name)),
          languageId: "erb",
          version: 1,
          text,
        },
      },
      true,
    );
  const rulesPublished = (u) =>
    rulesServer.notes
      .filter(
        (n) =>
          n.method === "textDocument/publishDiagnostics" && n.params.uri === u,
      )
      .at(-1)?.params.diagnostics;
  const until = async (label, ready) => {
    for (let i = 0; i < 60; i++) {
      if (ready()) return;
      await new Promise((r) => setTimeout(r, 50));
    }
    failures++;
    console.log(`FAIL timed out waiting for ${label}`);
  };
  await rulesOpen("a.html.erb", unclosed);
  await rulesOpen("sub/b.html.erb", unclosed);
  await until(
    "the first diagnostics",
    () =>
      rulesPublished(fileUri(path.join(rulesRoot, "a.html.erb"))) &&
      rulesPublished(fileUri(path.join(rulesRoot, "sub/b.html.erb"))),
  );
  const rulesCodes = (name) =>
    rulesPublished(fileUri(path.join(rulesRoot, name)))?.map((d) => d.code);
  check("toml: rule off", rulesCodes("a.html.erb"), []);
  check("toml: nearest file wins", rulesCodes("sub/b.html.erb"), [
    "Html/UnclosedElement",
  ]);
  const reindent = "<div>\n<p>a</p>\n</div>\n";
  const rulesFormat = async (name, text) =>
    (
      await rulesServer.send("textDocument/formatting", {
        textDocument: { uri: fileUri(path.join(rulesRoot, name)) },
        options: { tabSize: 2, insertSpaces: true },
      })
    ).result;
  await rulesOpen("c.html.erb", reindent);
  await rulesOpen("sub/d.html.erb", reindent);
  check("toml: format off", await rulesFormat("c.html.erb"), null);
  check(
    "toml: format on in the subfolder",
    (await rulesFormat("sub/d.html.erb"))?.[0]?.newText,
    "<div>\n  <p>a</p>\n</div>\n",
  );
  fs.writeFileSync(rulesToml, "[Html.UnclosedElement]\nenabeld = false\n");
  await rulesServer.send(
    "workspace/didChangeWatchedFiles",
    { changes: [{ uri: fileUri(rulesToml), type: 2 }] },
    true,
  );
  const tomlUri = fileUri(rulesToml);
  await until(
    "the toml problems",
    () =>
      rulesPublished(tomlUri)?.length && rulesCodes("a.html.erb")?.length === 1,
  );
  check(
    "toml: problems are published on the file",
    rulesPublished(tomlUri)?.map((d) => [d.range.start.line, d.message]),
    [
      [
        1,
        "unknown key `enabeld` in [Html.UnclosedElement]; it takes enabled, include, exclude",
      ],
    ],
  );
  check(
    "toml: problems have the rule as code",
    rulesPublished(tomlUri)?.map((d) => d.code),
    ["Config/Invalid"],
  );
  check(
    "toml: problems have the tool as source",
    rulesPublished(tomlUri)?.map((d) => d.source),
    ["alder"],
  );
  check(
    "toml: problems are logged",
    rulesServer.notes.some(
      (n) =>
        n.method === "window/logMessage" &&
        n.params.message.includes("unknown key `enabeld`"),
    ),
    true,
  );
  check("toml: rule back on after the edit", rulesCodes("a.html.erb"), [
    "Html/UnclosedElement",
  ]);
  fs.writeFileSync(rulesToml, quiet);
  await rulesServer.send(
    "workspace/didChangeWatchedFiles",
    { changes: [{ uri: tomlUri, type: 2 }] },
    true,
  );
  await until(
    "the toml problems to clear",
    () =>
      rulesPublished(tomlUri)?.length === 0 &&
      rulesCodes("a.html.erb")?.length === 0,
  );
  check("toml: problems cleared when fixed", rulesPublished(tomlUri), []);
  check("toml: rule off again", rulesCodes("a.html.erb"), []);
  await rulesServer.send("shutdown", {});
  await rulesServer.send("exit", {}, true);
  fs.rmSync(rulesRoot, { recursive: true, force: true });
  // VS Code percent-encodes `@` and `+` and the server must answer with URIs that decode to the same files.
  const oddDir = path.join(views, "my app @1+2");
  fs.mkdirSync(oddDir, { recursive: true });
  fs.writeFileSync(path.join(oddDir, "_note.html.erb"), "<i></i>\n");
  const oddText = '<%= render "note" %>\n';
  fs.writeFileSync(path.join(oddDir, "show.html.erb"), oddText);
  const oddUri = fileUri(root) + "/app/views/my%20app%20%401%2B2/show.html.erb";
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: oddUri,
        languageId: "erb",
        version: 1,
        text: oddText,
      },
    },
    true,
  );
  check(
    "definition in a dir with space, @ and +",
    (await req("textDocument/definition", oddUri, 0, 14)).result?.map((l) =>
      fs.realpathSync(fileURLToPath(l.uri)),
    ),
    [path.join(oddDir, "_note.html.erb")],
  );
  // Quick fixes of the rules that know the answer, from the diagnostics the server publishes: their `data` comes back.
  const fixRoot = fs.realpathSync(
    fs.mkdtempSync(path.join(os.tmpdir(), "alder-fixes-")),
  );
  const helloController =
    'import { Controller } from "@hotwired/stimulus"\n\nexport default class extends Controller {\n  static targets = ["name"]\n\n  greet() {\n  }\n}\n';
  const manifestLines = [
    'import { application } from "./application"',
    "",
    'import HelloController from "./hello_controller"',
    'application.register("hello", HelloController)',
    "",
  ];
  const fixFiles = {
    "Gemfile.lock": "GEM\n  specs:\n    actionview (7.1.3)\n",
    "config/application.rb": "",
    "config/locales/en.yml": "en:\n  users:\n    index:\n      title: Users\n",
    "app/javascript/controllers/hello_controller.js": helloController,
    "app/javascript/controllers/index.js": manifestLines.join("\n"),
  };
  for (const [name, content] of Object.entries(fixFiles)) {
    fs.mkdirSync(path.dirname(path.join(fixRoot, name)), { recursive: true });
    fs.writeFileSync(path.join(fixRoot, name), content);
  }
  const fixViewText = [
    '<%= link_to "x", usrs_path(1) %>',
    '<%= t(".titel") %>',
    '<%= t(".heading") %>',
    '<div data-controller="helo"></div>',
    '<div data-controller="hello" data-hello-target="nme" data-action="click->hello#gret"></div>',
    '<div data-controller="hello" data-hello-target="output" data-action="click->hello#rotate"></div>',
    '<img src="a.png">',
    '<%= image_tag "a.png" %>',
    "<div data-turbo-permanent></div>",
    "",
  ].join("\n");
  const fixView = path.join(fixRoot, "app/views/users/index.html.erb");
  fs.mkdirSync(path.dirname(fixView), { recursive: true });
  fs.writeFileSync(fixView, fixViewText);
  const fixViewUri = fileUri(fixView);
  const inFixRoot = (uri) =>
    path.relative(fixRoot, fileURLToPath(uri)).replaceAll(path.sep, "/");
  const fixServer = async (capabilities, initializationOptions) => {
    const srv = startServer();
    await srv.send("initialize", {
      processId: null,
      rootUri: fileUri(fixRoot),
      capabilities,
      initializationOptions,
    });
    await srv.send("initialized", {}, true);
    const routes = ["users", "user"].map((name) => ({
      name,
      verb: "GET",
      path: `/${name}`,
    }));
    await srv.send("alder/routes", { root: fixRoot, routes }, true);
    const text = fixViewText;
    const didOpen = {
      textDocument: { uri: fixViewUri, languageId: "erb", version: 1, text },
    };
    await srv.send("textDocument/didOpen", didOpen, true);
    const published = () =>
      srv.notes
        .filter(
          (n) =>
            n.method === "textDocument/publishDiagnostics" &&
            n.params.uri === fixViewUri,
        )
        .at(-1)?.params.diagnostics ?? [];
    for (
      let waited = 0;
      waited < 3000 && published().length < 11;
      waited += 50
    ) {
      await new Promise((r) => setTimeout(r, 50));
    }
    // The quick fixes for the `nth` diagnostic with `code` on `line`, counted from the left.
    const actions = async (code, line, nth = 0) => {
      const found = published()
        .filter((d) => d.code === code && d.range.start.line === line)
        .sort((a, b) => a.range.start.character - b.range.start.character);
      if (!found[nth]) return `no ${code} on line ${line}`;
      const { result } = await srv.send("textDocument/codeAction", {
        textDocument: { uri: fixViewUri },
        range: found[nth].range,
        context: { diagnostics: [found[nth]] },
      });
      return result.filter((a) => a.kind === "quickfix");
    };
    return { srv, actions, published, flagged: published().length };
  };
  const spelled = (e) =>
    `${e.range.start.line}:${e.range.start.character}-${e.range.end.line}:${e.range.end.character} ${e.newText}`;
  // Title, preferred, and the edits per file of each action that only changes existing files.
  const edited = (list) =>
    list.map((a) => [
      a.title,
      a.isPreferred,
      Object.fromEntries(
        Object.entries(a.edit.changes).map(([uri, edits]) => [
          inFixRoot(uri),
          edits.map(spelled),
        ]),
      ),
    ]);
  const view = "app/views/users/index.html.erb";
  const hello = "app/javascript/controllers/hello_controller.js";
  const canCreate = {
    workspace: {
      workspaceEdit: {
        documentChanges: true,
        resourceOperations: ["create", "rename", "delete"],
      },
    },
  };
  const withFiles = await fixServer(canCreate, { syncsFixTargets: true });
  check("quick fixes: the diagnostics come with data", withFiles.flagged, 11);
  check(
    "quick fix: replace a route helper",
    edited(await withFiles.actions("Rails/UnknownRoute", 0)),
    [["Replace with `users_path`", true, { [view]: ["0:17-0:26 users_path"] }]],
  );
  const titleEnd = "      title: Users".length;
  check(
    "quick fixes: a missing key is replaced or added to the default locale's file",
    [
      edited(await withFiles.actions("Rails/MissingTranslation", 1)),
      edited(await withFiles.actions("Rails/MissingTranslation", 2)),
    ],
    [
      [
        ["Use `.title`", true, { [view]: ['1:6-1:14 ".title"'] }],
        [
          "Add `users.index.titel` to config/locales/en.yml",
          false,
          {
            "config/locales/en.yml": [
              `3:${titleEnd}-3:${titleEnd} \n      titel: ""`,
            ],
          },
        ],
      ],
      [
        [
          "Add `users.index.heading` to config/locales/en.yml",
          false,
          {
            "config/locales/en.yml": [
              `3:${titleEnd}-3:${titleEnd} \n      heading: ""`,
            ],
          },
        ],
      ],
    ],
  );
  const created = (
    await withFiles.actions("Stimulus/UnknownController", 3)
  ).find((a) => a.title.startsWith("Create"));
  const importEnd = manifestLines[2].length;
  const registerEnd = manifestLines[3].length;
  check(
    "quick fix: create a controller and list it in the manifest",
    [
      created?.title,
      created?.isPreferred,
      created?.edit.documentChanges.map((change) =>
        change.kind === "create"
          ? `create ${inFixRoot(change.uri)} ${JSON.stringify(change.options)}`
          : `${inFixRoot(change.textDocument.uri)} ${change.textDocument.version} ${change.edits.map(spelled).join(" | ")}`,
      ),
    ],
    [
      "Create app/javascript/controllers/helo_controller.js",
      false,
      [
        'create app/javascript/controllers/helo_controller.js {"ignoreIfExists":false,"overwrite":false}',
        'app/javascript/controllers/helo_controller.js null 0:0-0:0 import { Controller } from "@hotwired/stimulus"\n\nexport default class extends Controller {\n}\n',
        `app/javascript/controllers/index.js null 2:${importEnd}-2:${importEnd} \nimport HeloController from "./helo_controller" | 3:${registerEnd}-3:${registerEnd} \napplication.register("helo", HeloController)`,
      ],
    ],
  );
  check(
    "quick fixes: names Stimulus lints suggest",
    [
      (await withFiles.actions("Stimulus/UnknownController", 3)).filter((a) =>
        a.title.startsWith("Use"),
      ),
      await withFiles.actions("Stimulus/UnknownTargetOrAction", 4, 0),
      await withFiles.actions("Stimulus/UnknownTargetOrAction", 4, 1),
    ].map((list) => (typeof list === "string" ? list : edited(list))),
    [
      [["Use `hello`", true, { [view]: ["3:22-3:26 hello"] }]],
      [
        ["Use `name`", false, { [view]: ["4:48-4:51 name"] }],
        [
          `Add \`nme\` to static targets in ${hello}`,
          false,
          { [hello]: ['3:26-3:26 , "nme"'] },
        ],
      ],
      [
        ["Use `greet`", true, { [view]: ["4:79-4:83 greet"] }],
        [
          `Add method \`gret()\` to ${hello}`,
          false,
          { [hello]: ["7:0-7:0 \n  gret() {\n  }\n"] },
        ],
      ],
    ],
  );
  check(
    "quick fixes: a target and a method are added to the controller",
    [
      edited(await withFiles.actions("Stimulus/UnknownTargetOrAction", 5, 0)),
      edited(await withFiles.actions("Stimulus/UnknownTargetOrAction", 5, 1)),
    ],
    [
      [
        [
          `Add \`output\` to static targets in ${hello}`,
          false,
          { [hello]: ['3:26-3:26 , "output"'] },
        ],
      ],
      [
        [
          `Add method \`rotate()\` to ${hello}`,
          false,
          { [hello]: ["7:0-7:0 \n  rotate() {\n  }\n"] },
        ],
      ],
    ],
  );
  check(
    "quick fixes: alt and id are added",
    [
      await withFiles.actions("Html/ImgAlt", 6),
      await withFiles.actions("Html/ImgAlt", 7),
      await withFiles.actions("Turbo/PermanentRequiresId", 8),
    ].map(edited),
    [
      [['Add `alt=""`', false, { [view]: ['6:4-6:4  alt=""'] }]],
      [['Add `alt: ""`', false, { [view]: ['7:21-7:21 , alt: ""'] }]],
      [["Add `id`", false, { [view]: ['8:4-8:4  id=""'] }]],
    ],
  );
  // An open locale file's unsaved text is what the key is added to, until it is closed.
  const enUri = fileUri(path.join(fixRoot, "config/locales/en.yml"));
  const headingEdit = async () =>
    edited(await withFiles.actions("Rails/MissingTranslation", 2))[0][2][
      "config/locales/en.yml"
    ];
  await withFiles.srv.send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: enUri,
        languageId: "yaml",
        version: 1,
        text: fs.readFileSync(fileURLToPath(enUri), "utf8"),
      },
    },
    true,
  );
  await withFiles.srv.send(
    "textDocument/didChange",
    {
      textDocument: { uri: enUri, version: 2 },
      contentChanges: [
        {
          range: {
            start: { line: 0, character: 0 },
            end: { line: 0, character: 0 },
          },
          text: "# unsaved\n",
        },
      ],
    },
    true,
  );
  const unsavedEdit = await headingEdit();
  await withFiles.srv.send(
    "textDocument/didClose",
    { textDocument: { uri: enUri } },
    true,
  );
  check(
    "quick fix: a key goes into the open locale file's unsaved text, then the saved one once closed",
    [unsavedEdit, await headingEdit()],
    [
      [`4:${titleEnd}-4:${titleEnd} \n      heading: ""`],
      [`3:${titleEnd}-3:${titleEnd} \n      heading: ""`],
    ],
  );
  // The unsaved controller is what the template is checked against.
  const helloUri = fileUri(path.join(fixRoot, hello));
  const insert = (line, character, text) => ({
    range: { start: { line, character }, end: { line, character } },
    text,
  });
  await withFiles.srv.send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: helloUri,
        languageId: "javascript",
        version: 1,
        text: helloController,
      },
    },
    true,
  );
  // What the client holds when it asks for fixes before the server's lint of the edit arrives.
  const staleDiagnostics = withFiles
    .published()
    .filter(
      (d) =>
        d.code === "Stimulus/UnknownTargetOrAction" && d.range.start.line === 5,
    );
  await withFiles.srv.send(
    "textDocument/didChange",
    {
      textDocument: { uri: helloUri, version: 2 },
      contentChanges: [
        insert(7, 0, "\n  rotate() {\n  }\n"),
        insert(3, 26, ', "output"'),
      ],
    },
    true,
  );
  const typedFixes = [];
  for (const diagnostic of staleDiagnostics) {
    const { result } = await withFiles.srv.send("textDocument/codeAction", {
      textDocument: { uri: fixViewUri },
      range: diagnostic.range,
      context: { diagnostics: [diagnostic] },
    });
    typedFixes.push(result.filter((a) => a.kind === "quickfix"));
  }
  check(
    "quick fixes: no target or method the unsaved controller already has, for diagnostics older than the edit",
    [staleDiagnostics.length, typedFixes],
    [2, [[], []]],
  );
  // The template's warnings on line 5, once the server has published `count` of them.
  const unknownMembers = async (count) => {
    const found = () =>
      withFiles
        .published()
        .filter(
          (d) =>
            d.code === "Stimulus/UnknownTargetOrAction" &&
            d.range.start.line === 5,
        ).length;
    for (let waited = 0; waited < 3000 && found() !== count; waited += 50) {
      await new Promise((r) => setTimeout(r, 50));
    }
    return found();
  };
  const typed = await unknownMembers(0);
  await withFiles.srv.send(
    "textDocument/didClose",
    { textDocument: { uri: helloUri } },
    true,
  );
  check(
    "stimulus lint: the unsaved controller's members count, the saved file's once it closes",
    [typed, await unknownMembers(2)],
    [0, 2],
  );
  await withFiles.srv.send("shutdown", {});
  await withFiles.srv.send("exit", {}, true);
  // A client that declares no sync of fix targets is trusted with a kind of file once it has opened one of that kind.
  const unsynced = await fixServer(canCreate);
  const titles = async (code, line, nth) =>
    (await unsynced.actions(code, line, nth)).map((a) => a.title);
  const openFile = (uri, languageId, text) =>
    unsynced.srv.send(
      "textDocument/didOpen",
      { textDocument: { uri, languageId, version: 1, text } },
      true,
    );
  const fixesNow = async () => [
    await titles("Rails/MissingTranslation", 2),
    await titles("Stimulus/UnknownController", 3),
    await titles("Stimulus/UnknownTargetOrAction", 4, 0),
  ];
  check(
    "quick fixes: no edit of an existing file from a client that may not sync it",
    await fixesNow(),
    [[], ["Use `hello`"], ["Use `name`"]],
  );
  await openFile(
    fileUri(path.join(fixRoot, "config/locales/fr.yml")),
    "yaml",
    "fr:\n",
  );
  check(
    "quick fixes: a YAML didOpen shows the client syncs locale files",
    await fixesNow(),
    [
      ["Add `users.index.heading` to config/locales/en.yml"],
      ["Use `hello`"],
      ["Use `name`"],
    ],
  );
  await openFile(
    fileUri(path.join(fixRoot, "app/javascript/application.js")),
    "javascript",
    "",
  );
  check(
    "quick fixes: a script didOpen shows the client syncs controllers",
    await fixesNow(),
    [
      ["Add `users.index.heading` to config/locales/en.yml"],
      ["Use `hello`", "Create app/javascript/controllers/helo_controller.js"],
      ["Use `name`", `Add \`nme\` to static targets in ${hello}`],
    ],
  );
  await unsynced.srv.send("shutdown", {});
  await unsynced.srv.send("exit", {}, true);
  // Creating files edits nothing that exists, so it needs no sync.
  fs.rmSync(path.join(fixRoot, "app/javascript/controllers/index.js"));
  const creating = await fixServer(canCreate);
  check(
    "quick fixes: files are created for a client that may not sync",
    (await creating.actions("Stimulus/UnknownController", 3)).map(
      (a) => a.title,
    ),
    ["Use `hello`", "Create app/javascript/controllers/helo_controller.js"],
  );
  await creating.srv.send("shutdown", {});
  await creating.srv.send("exit", {}, true);
  // A client that can't create files is offered no file to create.
  const withoutFiles = await fixServer({});
  check(
    "quick fix: no controller file for a client that cannot create files",
    (await withoutFiles.actions("Stimulus/UnknownController", 3)).map(
      (a) => a.title,
    ),
    ["Use `hello`"],
  );
  await withoutFiles.srv.send("shutdown", {});
  await withoutFiles.srv.send("exit", {}, true);
  fs.rmSync(fixRoot, { recursive: true, force: true });
  // A client may send any request for a file it opened that is no template, or one it never opened.
  const modelUri = fileUri(path.join(root, "app", "models", "report.rb"));
  await send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: modelUri,
        languageId: "ruby",
        version: 1,
        text: "class Report < ApplicationRecord\nend\n",
      },
    },
    true,
  );
  // One set of params for every request below; each reads its own fields.
  const anywhere = {
    position: { line: 0, character: 6 },
    range: {
      start: { line: 0, character: 0 },
      end: { line: 1, character: 0 },
    },
    options: { tabSize: 2, insertSpaces: true },
    context: {
      diagnostics: [],
      includeDeclaration: true,
      triggerKind: 1,
      isRetrigger: false,
    },
    newName: "x",
  };
  const answers = async (u, methods) =>
    Promise.all(
      methods.map(async (method) => {
        const { result, error } = await send(method, {
          textDocument: { uri: u },
          ...anywhere,
        });
        return `${method} ${JSON.stringify(error ?? result)}`;
      }),
    );
  const everyRequest = [
    "textDocument/definition",
    "textDocument/hover",
    "textDocument/references",
    "textDocument/completion",
    "textDocument/signatureHelp",
    "textDocument/prepareRename",
    "textDocument/rename",
    "textDocument/linkedEditingRange",
    "textDocument/documentLink",
    "textDocument/documentSymbol",
    "textDocument/foldingRange",
    "textDocument/semanticTokens/full",
    "textDocument/codeAction",
    "textDocument/formatting",
    "textDocument/rangeFormatting",
  ];
  const nothing = (methods) => methods.map((method) => `${method} null`);
  check(
    "untracked: an open model and a file never opened get nothing",
    [
      await answers(modelUri, everyRequest),
      await answers(fileUri(path.join(root, "app", "models", "x.rb")), [
        "textDocument/definition",
        "textDocument/hover",
        "textDocument/references",
        "textDocument/documentLink",
      ]),
    ],
    [
      nothing(everyRequest),
      nothing([
        "textDocument/definition",
        "textDocument/hover",
        "textDocument/references",
        "textDocument/documentLink",
      ]),
    ],
  );
  // Zed opts YAML, JavaScript and TypeScript in for the fix targets: they are synced, and nothing else is done for them.
  const optedIn = [
    ["config/locales/en.yml", "yaml", "en:\n  a: b\n"],
    [
      "app/javascript/controllers/x_controller.js",
      "javascript",
      "export default class {}\n",
    ],
    [
      "app/javascript/controllers/y_controller.ts",
      "typescript",
      "export default class {}\n",
    ],
  ];
  for (const [name, languageId, body] of optedIn) {
    const u = fileUri(path.join(root, name));
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: u, languageId, version: 1, text: body } },
      true,
    );
    check(
      `untracked: an open ${languageId} file gets nothing`,
      [
        await answers(u, everyRequest),
        notes.filter(
          (n) =>
            n.method === "textDocument/publishDiagnostics" &&
            n.params.uri === u,
        ).length,
      ],
      [nothing(everyRequest), 0],
    );
  }
  const partialJbuilderUri = fileUri(
    path.join(views, "users", "_user.json.jbuilder"),
  );
  const percentUri = fileUri(path.join(views, "users", "percent.html.erb"));
  for (const [u, languageId, body] of [
    [partialJbuilderUri, "ruby", "json.name user.name\n"],
    [percentUri, "erb-percent", "% if x\n<div>\n<p>a</p>\n</div>\n% end\n"],
  ]) {
    await send(
      "textDocument/didOpen",
      { textDocument: { uri: u, languageId, version: 1, text: body } },
      true,
    );
  }
  const formatting = [
    "textDocument/formatting",
    "textDocument/rangeFormatting",
  ];
  check(
    "formatting: nothing for Liquid, Jbuilder and erb-percent",
    [
      await answers(liquidUri, formatting),
      await answers(partialJbuilderUri, formatting),
      await answers(percentUri, formatting),
    ],
    [nothing(formatting), nothing(formatting), nothing(formatting)],
  );
  // `initialized` is a notification: the registrations arrive after it, and "ready" is logged after the last.
  const registered = async (srv) => {
    const ready = () =>
      srv.notes.some(
        (n) =>
          n.method === "window/logMessage" &&
          n.params.message.endsWith(" ready"),
      );
    for (let i = 0; i < 80 && !ready(); i++) {
      await new Promise((r) => setTimeout(r, 25));
    }
    return srv.registrations;
  };
  const watching = startServer();
  await watching.send("initialize", {
    processId: null,
    rootUri: fileUri(root),
    capabilities: {
      workspace: { didChangeWatchedFiles: { dynamicRegistration: true } },
      textDocument: { synchronization: { dynamicRegistration: true } },
    },
  });
  await watching.send("initialized", {}, true);
  const watched = await registered(watching);
  const globs = watched[0]?.registerOptions.watchers.map((w) => w.globPattern);
  check(
    "watchers and fix target sync are registered by the server",
    [
      watched.map((r) => r.method),
      [
        "**/app/views/**",
        "**/config/locales/**",
        "**/config/routes.rb",
        "**/app/javascript/**",
        "**/Gemfile.lock",
        "**/.alder.toml",
        "**/app",
      ].filter((glob) => !globs?.includes(glob)),
      watched[1]?.registerOptions.documentSelector[0],
      watched[2]?.registerOptions.syncKind,
    ],
    [
      [
        "workspace/didChangeWatchedFiles",
        "textDocument/didOpen",
        "textDocument/didChange",
        "textDocument/didClose",
      ],
      [],
      { scheme: "file", language: "yaml", pattern: "**/config/locales/**" },
      2,
    ],
  );
  const laterUri = fileUri(path.join(views, "users", "watched.html.erb"));
  await watching.send(
    "textDocument/didOpen",
    {
      textDocument: {
        uri: laterUri,
        languageId: "erb",
        version: 1,
        text: '<%= render "users/later" %>\n',
      },
    },
    true,
  );
  const laterCodes = () =>
    watching.notes
      .filter(
        (n) =>
          n.method === "textDocument/publishDiagnostics" &&
          n.params.uri === laterUri,
      )
      .at(-1)
      ?.params.diagnostics.map((d) => d.code);
  const waitForLater = async (done) => {
    for (let i = 0; i < 80 && !done(laterCodes()); i++) {
      await new Promise((r) => setTimeout(r, 25));
    }
    return laterCodes();
  };
  const unresolved = await waitForLater((codes) => codes?.length);
  const laterPartial = path.join(views, "users", "_later.html.erb");
  fs.writeFileSync(laterPartial, "<p></p>\n");
  await watching.send(
    "workspace/didChangeWatchedFiles",
    { changes: [{ uri: fileUri(laterPartial), type: 1 }] },
    true,
  );
  check(
    "a watched-file event lets a created partial resolve",
    [unresolved, await waitForLater((codes) => codes?.length === 0)],
    [["Rails/MissingPartial"], []],
  );
  fs.unlinkSync(laterPartial);
  await watching.send("shutdown", {});
  await watching.send("exit", {}, true);
  const dynamic = startServer();
  await dynamic.send("initialize", {
    processId: null,
    rootUri: fileUri(root),
    capabilities: {
      textDocument: {
        formatting: { dynamicRegistration: true },
        rangeFormatting: { dynamicRegistration: true },
      },
    },
  });
  await dynamic.send("initialized", {}, true);
  const registeredLanguages = Object.fromEntries(
    (await registered(dynamic)).map((r) => [
      r.method,
      r.registerOptions.documentSelector?.map((s) => s.language),
    ]),
  );
  // Neither watchers nor syncing for a client that takes no such registration.
  check("dynamic formatting registration only", registeredLanguages, {
    "textDocument/formatting": ["erb", "haml", "slim", "skim", "jade"],
    "textDocument/rangeFormatting": ["erb", "haml"],
  });
  await dynamic.send("shutdown", {});
  await dynamic.send("exit", {}, true);
  await aligned.send("shutdown", {});
  await aligned.send("exit", {}, true);
  await send("shutdown", {});
  await send("exit", {}, true);
  fs.rmSync(root, { recursive: true, force: true });
  console.log(failures ? `${failures} check(s) failed` : "all checks passed");
  process.exit(failures ? 1 : 0);
})();
// The suite takes about 6 s locally; CI's Windows and macOS runners are slower.
const SUITE_TIMEOUT_MS = 20_000;
setTimeout(() => {
  console.error("timeout");
  process.exit(1);
}, SUITE_TIMEOUT_MS);
