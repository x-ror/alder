// Pure Pug editing helpers; kept free of `vscode` so `test/pug.js` can run them under plain Node.
import path from "path";

import { leadingWhitespace } from "./text.js";
import { extractBlock } from "./partial.js";

// `-` code, `=` and `!=` output, and `//-`, a comment Pug drops; `//` is an HTML comment and stays.
const MARKER_RE = /^([ \t]*)(!=|=|\/\/-|-)[ \t]?(.+)$/;
const NEXT_MARKER = { "-": "=", "=": "!=", "!=": "//-", "//-": "-" };
// Lines whose deeper-indented lines are text that doesn't interpolate: comments, `-` alone (code) and filters.
const RAW_PARENT_RE = /^[ \t]*(\/\/|-[ \t]*$|:[\w-])/;
// A tag ending in `.` (`script.`), whose deeper-indented lines are text with `#{}` in it.
const TEXT_TAG_RE =
  /^[ \t]*(?:[\w:-]+|#\{[^}]*\})?(?:[#.][\w-]+|\([^)]*\)|&attributes\([^)]*\))*\.[ \t]*$/;
// Line starts where a whole-line `= …` would turn Pug's own syntax into code.
const PUG_LINE_START_RE = /^(?:[-=|<+:]|!=|\/\/)/;
// Lines Pug reads as a keyword before it tries a tag.
const KEYWORD_RE =
  /^(?:if|unless|else|each|for|while|case|when|default|mixin|block|append|prepend|include|extends?|doctype|yield)\b/;
// JavaScript words that may stand between two operands, and the statements that are no expression.
const OPERATOR_WORDS = new Set([
  "in",
  "instanceof",
  "typeof",
  "new",
  "void",
  "delete",
  "await",
]);
const STATEMENT_RE =
  /^(?:var|let|const|if|else|for|while|do|switch|function|return|throw|try|catch|class|break|continue)\b/;
// Statements a `//-` comment can hold as code rather than prose: `if (`, `} else {`, `var x =`, `function f(`.
const CODE_STATEMENT_RE =
  /^(?:(?:if|for|while|switch|catch)\s*\(|\}|(?:else|do|try|finally)\s*\{|(?:var|let|const)\s+[\w$]+\s*=|function\b[^(]*\()/;
// What an expression can start with (an operand or a unary operator) and can't end with (a binary operator; `x++` can).
const EXPRESSION_START_RE = /^[\w$([{!+\-~]/;
const OPERATOR_END_RE = /(?:^|[^+-])[-+]$|[*/%=<>&|^!~?:,.]$/;
const MIXIN_BLOCK_RE = /^[ \t]*block[ \t]*$/;
const MIXIN_RE = /^[ \t]*mixin\b/;
// Lines that continue the `if`, `case` or `each` above them, which Pug refuses on their own.
const CONTINUATION_RE = /^[ \t]*(else|when|default)\b/;
const EXTENDS_RE = /^[ \t]*extends?\b/;
const INTERPOLATION_CLOSE = "}";

const isDeeper = (line, indent) =>
  line.trim() !== "" && leadingWhitespace(line).length > indent.length;

/** Whether line `n` has lines nested under it. */
function hasNested(lines, n) {
  const next = lines.slice(n + 1).find((l) => l.trim());
  return next !== undefined && isDeeper(next, leadingWhitespace(lines[n]));
}

/** The line that makes line `n` text: a comment, `-` alone, a filter or a `tag.`; null when line `n` is Pug. */
function textBlockOwner(lines, n) {
  let indent = leadingWhitespace(lines[n]).length;
  for (let i = n - 1; i >= 0 && indent > 0; i--) {
    if (!lines[i].trim()) continue;
    const own = leadingWhitespace(lines[i]).length;
    if (own >= indent) continue;
    if (RAW_PARENT_RE.test(lines[i]) || TEXT_TAG_RE.test(lines[i]))
      return lines[i];
    indent = own;
  }
  return null;
}

const inTextBlock = (lines, n) => textBlockOwner(lines, n) !== null;

/**
 * Whether `code` can be a JavaScript expression, which Pug requires after `=` and in `#{}`: it starts and ends as one, its
 * brackets and quotes close, it has no statement keyword, `;`, assignment or Pug interpolation, and no two operands side by
 * side (`a b`, `a {`). A guess, since Pug asks a JavaScript parser.
 */
function looksLikeExpression(code) {
  const bare = code.trim().replace(/(["'`])(?:\\.|(?!\1)[^\\])*\1/g, "0");
  const shaped =
    EXPRESSION_START_RE.test(bare) &&
    !OPERATOR_END_RE.test(bare) &&
    !/[;"'`]|#[{[]|[\w$)\]]\s*\{|(?:^|[^=!<>])=(?![=>])/.test(bare) &&
    !STATEMENT_RE.test(bare) &&
    balanced(bare);
  if (!shaped) return false;
  for (const [, before, after] of bare.matchAll(/([\w$]+|[)\]])\s+([\w$]+)/g)) {
    if (!OPERATOR_WORDS.has(before) && !OPERATOR_WORDS.has(after)) return false;
  }
  return true;
}

/** Whether every bracket in `code` closes, in order. */
function balanced(code) {
  const open = [];
  for (const c of code) {
    if ("([{".includes(c)) open.push(c);
    else if (")]}".includes(c) && open.pop() !== "([{"[")]}".indexOf(c)])
      return false;
  }
  return open.length === 0;
}

/** The end of the bracketed group that opens at `at`, quotes respected; -1 when it doesn't close on the line. */
function groupEnd(line, at) {
  let depth = 0;
  for (let i = at; i < line.length; i++) {
    const c = line[i];
    if (c === "'" || c === '"' || c === "`") {
      const close = line.indexOf(c, i + 1);
      if (close < 0) return -1;
      i = close;
    } else if (c === "(" || c === "[" || c === "{") depth++;
    else if (c === ")" || c === "]" || c === "}") {
      depth--;
      if (depth === 0) return i + 1;
    }
  }
  return -1;
}

/** The end of the tag head (name, `#id`, `.class`, `(attrs)`) that starts at `i`; `i` when there is none, -1 when it doesn't close. */
function tagHeadEnd(line, i) {
  const name = /^(?:\w(?:[-:\w]*\w)?|#\{[^}]*\})/.exec(line.slice(i));
  if (name) i += name[0].length;
  for (;;) {
    const short = /^[#.][\w-]+/.exec(line.slice(i));
    const group = line.startsWith("(", i)
      ? i
      : line.startsWith("&attributes(", i)
        ? i + "&attributes".length
        : -1;
    if (short) i += short[0].length;
    else if (group >= 0) {
      i = groupEnd(line, group);
      if (i < 0) return -1;
    } else return i;
  }
}

/** The column where a line's inline text starts: after `| `, a tag's head and its space, or at a `<`; -1 when it has none. */
function textStart(line) {
  let i = leadingWhitespace(line).length;
  if (line[i] === "|") return line[i + 1] === " " ? i + 2 : i + 1;
  if (line[i] === "<") return i;
  if (KEYWORD_RE.test(line.slice(i))) return -1;
  for (;;) {
    const head = i;
    i = tagHeadEnd(line, i);
    if (i <= head) return -1;
    if (line.startsWith(": ", i)) {
      i += 2;
      continue;
    }
    return line[i] === " " ? i + 1 : -1;
  }
}

/** Whether Pug reads `content` as a tag with no inline text (`user.name`, `t('x')`, `a: b`) rather than as code. */
function readsAsTag(content) {
  const head = tagHeadEnd(content, 0);
  return head !== 0 && content[head] !== " ";
}

/**
 * Whether columns `start`..`end` reach into a `#{…}` or `!{…}`, or a `#[…]` outside its inline text, where a `#{}` would
 * nest or turn the tag's name into code.
 */
function inInterpolationHead(line, start, end) {
  for (const { index } of line.matchAll(/(?<!\\)(?:[#!]\{|#\[)/g)) {
    const close = groupEnd(line, index + 1);
    const spanEnd = close < 0 ? line.length : close;
    const overlaps =
      start === end
        ? index < start && start < spanEnd
        : start < spanEnd && end > index;
    if (!overlaps) continue;
    if (line[index + 1] !== "[" || close < 0) return true;
    const body = index + 2;
    const head = tagHeadEnd(line.slice(body, close - 1), 0);
    const text = line[body + head] === " " ? body + head + 1 : close;
    if (head <= 0 || start < text || end > close - 1) return true;
  }
  return false;
}

/**
 * Cycle a line's leading `-` → `=` → `!=` → `//-` → `-`; code that can't be an expression skips the output steps, and a
 * comment that reads as neither an expression nor a statement stays a comment.
 */
function togglePugMarker(line) {
  const m = MARKER_RE.exec(line);
  if (!m) return null;
  const [, indent, marker, body] = m;
  const expression = looksLikeExpression(body);
  if (marker === "//-" && !expression && !CODE_STATEMENT_RE.test(body))
    return null;
  const next = marker === "-" && !expression ? "//-" : NEXT_MARKER[marker];
  return `${indent}${next} ${body}`.trimEnd();
}

/**
 * Toggled text for each of the `targets` line numbers, as [line, text] pairs. Lines in text blocks are left alone, and so is
 * a line with nested lines: Pug refuses a block under `=` or `!=`, and a `//-` comment's nested lines are its text.
 */
function togglePugLines(lines, targets) {
  return [...targets]
    .filter((n) => !inTextBlock(lines, n) && !hasNested(lines, n))
    .map((n) => [n, togglePugMarker(lines[n])])
    .filter(([, text]) => text !== null);
}

/**
 * What replaces columns `start`..`end` of line `n`: `{ start, end, text }`. A whole line of content becomes `= content`,
 * a selection in text (after a tag, after `|`, in a `tag.` block) `#{selection}`, an empty one `#{}` to type in. Null where
 * Pug would refuse either or read it differently: Pug syntax or a block after `=`, a line Pug reads as a tag (`user.name`),
 * `#{}` in code, in another interpolation, a keyword line or a tag's head, and a selection that can't be a JavaScript
 * expression.
 */
function wrapPug(lines, n, start, end) {
  const line = lines[n];
  const content = line.trim();
  const first = leadingWhitespace(line).length;
  const wholeLine = content !== "" && line.slice(start, end).trim() === content;
  const [from, to] = wholeLine ? [first, first + content.length] : [start, end];
  const selected = line.slice(from, to);
  const expression = selected === "" || looksLikeExpression(selected);
  const interpolated = {
    start: from,
    end: to,
    text: `#{${selected}${INTERPOLATION_CLOSE}`,
  };
  if (inInterpolationHead(line, from, to)) return null;
  const owner = textBlockOwner(lines, n);
  if (owner !== null)
    return TEXT_TAG_RE.test(owner) && expression ? interpolated : null;
  if (wholeLine) {
    const output =
      !hasNested(lines, n) &&
      !PUG_LINE_START_RE.test(content) &&
      !KEYWORD_RE.test(content) &&
      !readsAsTag(content) &&
      looksLikeExpression(content);
    return output
      ? { start: first, end: first + content.length, text: `= ${content}` }
      : null;
  }
  const text = textStart(line);
  return text >= 0 && start >= text && expression ? interpolated : null;
}

/**
 * The whole lines to move into an included file for `selection`, with the lines nested under them, so a tag keeps its
 * text block; or a reason it can't move: it starts in a text block or deeper than it ends, holds `extends`, or would leave
 * an `else`, `when` or `default` without its `if`, `case` or `each`. Returns `{ range, indent, lines }` as `extractBlock`
 * does, or `{ refused }`, or null when only blank lines are selected.
 */
function pugExtractBlock(lines, selection) {
  const block = extractBlock(lines, selection, true);
  if (!block) return null;
  const { indent } = block;
  let last = block.range.end.line;
  for (let i = last + 1; i < lines.length; i++) {
    if (isDeeper(lines[i], indent)) last = i;
    else if (lines[i].trim()) break;
  }
  const moved = lines.slice(block.range.start.line, last + 1);
  const first = moved.find((l) => l.trim());
  const after = lines.slice(last + 1).find((l) => l.trim());
  if (inTextBlock(lines, lines.indexOf(first, block.range.start.line)))
    return { refused: "The selection starts inside a text block." };
  if (leadingWhitespace(first) !== indent)
    return { refused: "The selection starts deeper than it ends." };
  if (moved.some((l) => EXTENDS_RE.test(l)))
    return { refused: "`extends` must stay the template's first line." };
  if (
    moved.some((l) => MIXIN_BLOCK_RE.test(l)) &&
    !moved.some((l) => MIXIN_RE.test(l))
  )
    return { refused: "A mixin's `block` must stay in the mixin." };
  const continues = (l) =>
    CONTINUATION_RE.test(l) && leadingWhitespace(l) === indent;
  if (continues(first) || (after !== undefined && continues(after)))
    return {
      refused:
        "The selection would split an `if`, `case` or `each` from its `else`, `when` or `default`.",
    };
  return {
    range: {
      start: block.range.start,
      end: { line: last, character: lines[last].length },
    },
    indent,
    lines: moved.map((l) =>
      l.startsWith(indent) ? l.slice(indent.length) : l.trimStart(),
    ),
  };
}

/** The new file for `name` from `file`: in its folder, with its extensions (`list.jst.pug` → `row.jst.pug`). */
function pugPartialPath(file, name) {
  const base = path.basename(file);
  const dot = base.indexOf(".");
  return path.join(
    path.dirname(file),
    `${name}${dot < 0 ? "" : base.slice(dot)}`,
  );
}

/** How `include` names `partial` from `file`: relative, with `/`, and without the `.pug` Pug adds to a path without an extension. */
function pugIncludePath(file, partial) {
  const relative = path
    .relative(path.dirname(file), partial)
    .split(path.sep)
    .join("/");
  const stem = relative.slice(0, -".pug".length);
  return relative.endsWith(".pug") && !path.posix.extname(stem)
    ? stem
    : relative;
}

export {
  INTERPOLATION_CLOSE,
  togglePugMarker,
  togglePugLines,
  inTextBlock,
  looksLikeExpression,
  textStart,
  wrapPug,
  pugExtractBlock,
  pugPartialPath,
  pugIncludePath,
};
