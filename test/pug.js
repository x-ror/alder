import assert from "assert";
import path from "path";
import {
  togglePugMarker,
  togglePugLines,
  inTextBlock,
  looksLikeExpression,
  textStart,
  wrapPug,
  pugExtractBlock,
  pugPartialPath,
  pugIncludePath,
} from "../src/pug.js";

for (const [line, next] of [
  ["- x++", "= x++"],
  ["- if (x)", "//- if (x)"],
  ["  = user.name", "  != user.name"],
  ["!= html", "//- html"],
  ["//- note", "- note"],
  ["//- if (x)", "- if (x)"],
  ["//- var x = 1", "- var x = 1"],
  ["//- TODO fix this", null],
  ["//- do this later", null],
  ["\t-x", "\t= x"],
  ["-", null],
  ["// shown", null],
  ["p= x", null],
  ["| - text", null],
]) {
  assert.strictEqual(togglePugMarker(line), next, line);
}

const template = [
  "- if (x)",
  "  p",
  "- a + 1",
  "script.",
  "  - not code",
  "//- note",
  "  body",
  "= y",
  "p",
  "  = z",
];
assert.deepStrictEqual(togglePugLines(template, new Set(template.keys())), [
  [2, "= a + 1"],
  [7, "!= y"],
  [9, "  != z"],
]);

const nested = [
  "p.",
  "  a",
  ":markdown-it",
  "  b",
  "-",
  "  c",
  "// d",
  "  e",
  'div(class="a b").',
  "  f",
  "div",
  "  g",
  "a(href=x) Read more.",
  "  h",
];
assert.deepStrictEqual(
  nested.map((_, i) => inTextBlock(nested, i)),
  [
    false,
    true,
    false,
    true,
    false,
    true,
    false,
    true,
    false,
    true,
    false,
    false,
    false,
    false,
  ],
);

for (const [code, expression] of [
  ["user.name", true],
  ["t('a b', { x: 1 })", true],
  ["a in b", true],
  ["new Date()", true],
  ["typeof a === 'x'", true],
  ["Hello world", false],
  ["var x = 1", false],
  ["a; b", false],
  ["a = 1", false],
  ["a == 1 && b => c", true],
  ["p { color: red }", false],
  ["", false],
]) {
  assert.strictEqual(looksLikeExpression(code), expression, code);
}

for (const [line, start] of [
  ["p Hello", 2],
  ["  | Hi", 4],
  ["|Hi", 1],
  ['a.btn#x(href="/a b" data-x=(1)) Go', 32],
  ["li: a(href=x) more", 14],
  ["p&attributes(a) b", 16],
  ["#{t} b", 5],
  ["<p>Hi</p>", 0],
  ["p", -1],
  ["p= x", -1],
  ["p.", -1],
  ["each u in users", -1],
  ["+card(a) b", -1],
  ['p(title="x', -1],
]) {
  assert.strictEqual(textStart(line), start, line);
}

const page = [
  "p Hello name",
  "user.name",
  "| Hi there",
  "script.",
  "  var a = b;",
  "each u in users",
  "div",
  "  span",
  "//- note x",
];
const wrapped = (n, start, end) => {
  const w = wrapPug(page, n, start, end);
  return w && `${page[n].slice(0, w.start)}${w.text}${page[n].slice(w.end)}`;
};
assert.strictEqual(wrapped(0, 8, 12), "p Hello #{name}");
assert.strictEqual(wrapped(0, 2, 12), null, "not an expression");
assert.strictEqual(wrapped(0, 0, 1), null, "the tag's name");
assert.strictEqual(wrapped(0, 12, 12), "p Hello name#{}");
assert.strictEqual(wrapped(1, 0, 9), null, "the tag user with class name");
assert.strictEqual(wrapped(2, 5, 10), "| Hi #{there}");
assert.strictEqual(wrapped(2, 0, 10), null, "a whole line of Pug syntax");
assert.strictEqual(wrapped(4, 10, 11), "  var a = #{b};");
assert.strictEqual(wrapped(5, 10, 15), null, "a keyword's code");
assert.strictEqual(wrapped(6, 0, 3), null, "a tag with a block");
assert.strictEqual(wrapped(8, 9, 10), null, "a comment");

const wrapLine = (line, start, end) => {
  const w = wrapPug([line], 0, start, end);
  return w && `${line.slice(0, w.start)}${w.text}${line.slice(w.end)}`;
};
for (const [line, start, end, result] of [
  ["p hello #{name} x", 10, 14, null],
  ["p hello #{name} x", 12, 12, null],
  ["p hello !{name} x", 10, 14, null],
  ["p hello #{name} x", 8, 8, "p hello #{}#{name} x"],
  ["p hello #{name} x", 16, 17, "p hello #{name} #{x}"],
  ["p a #[strong b c] d", 13, 14, "p a #[strong #{b} c] d"],
  ["p a #[strong b c] d", 6, 12, null],
  ["p a #[strong b c] d", 15, 18, null],
  ["p a #[br] d", 6, 6, null],
  ["p a #[em #{b}] d", 11, 12, null],
]) {
  assert.strictEqual(wrapLine(line, start, end), result, `${line} ${start}`);
}
for (const [line, result] of [
  ["div", null],
  ["a(b)", null],
  ["foo.bar", null],
  ["t('x')", null],
  ["a: b", null],
  ["p!= x", null],
  ["mixin", null],
  ["case", null],
  ['user.name + "!"', '= user.name + "!"'],
  ["items.length > 0", "= items.length > 0"],
  ["!done", "= !done"],
  ["[1, 2].length", "= [1, 2].length"],
]) {
  assert.strictEqual(wrapLine(line, 0, line.length), result, line);
}

const sel = (sl, sc, el, ec) => ({
  start: { line: sl, character: sc },
  end: { line: el, character: ec },
});
const list = [
  "ul",
  "  li.a",
  "    | one",
  "  li.b",
  "  script.",
  "    var x = 1;",
  "",
  "  li.c",
];
assert.deepStrictEqual(pugExtractBlock(list, sel(1, 2, 1, 6)), {
  range: { start: { line: 1, character: 0 }, end: { line: 2, character: 9 } },
  indent: "  ",
  lines: ["li.a", "  | one"],
});
assert.deepStrictEqual(pugExtractBlock(list, sel(3, 0, 4, 0)).lines, ["li.b"]);
assert.deepStrictEqual(pugExtractBlock(list, sel(3, 0, 4, 9)).lines, [
  "li.b",
  "script.",
  "  var x = 1;",
]);
assert.strictEqual(pugExtractBlock(list, sel(6, 0, 6, 0)), null);
for (const [lines, selection] of [
  [list, sel(5, 4, 5, 8)],
  [["div", "    p", "  span"], sel(1, 0, 2, 6)],
  [["extends layout", "block content"], sel(0, 0, 1, 5)],
  [["if x", "  p", "else", "  q"], sel(0, 0, 1, 3)],
  [["if x", "  p", "else", "  q"], sel(2, 0, 3, 3)],
  [["case x", "  when 1", "    p", "  when 2"], sel(1, 0, 2, 5)],
]) {
  assert.ok(pugExtractBlock(lines, selection).refused, JSON.stringify(lines));
}

const users = path.join(path.sep, "app", "assets", "templates", "users");
for (const [file, name, partial, include] of [
  ["list.jst.pug", "row", "row.jst.pug", "row.jst.pug"],
  ["list.pug", "parts/row", path.join("parts", "row.pug"), "parts/row"],
  ["list.jade", "row", "row.jade", "row.jade"],
]) {
  const target = pugPartialPath(path.join(users, file), name);
  assert.strictEqual(target, path.join(users, partial), file);
  assert.strictEqual(pugIncludePath(path.join(users, file), target), include);
}

console.log("pug ok");
