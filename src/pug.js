// Pure Pug editing helpers; kept free of `vscode` so `test/pug.js` can run them under plain Node.
import path from "path";

import { leadingWhitespace } from "./text.js";
import { extractBlock } from "./partial.js";

// `-` code, `=` and `!=` output, and `//-`, a comment Pug drops; `//` is an HTML comment and stays.
const MARKER_RE = /^([ \t]*)(!=|=|\/\/-|-)[ \t]*(\S.*)$/;
const NEXT_MARKER = { "-": "=", "=": "!=", "!=": "//-", "//-": "-" };
// Lines whose deeper-indented lines are text Pug doesn't interpolate: `//-` (never output), `-` alone (code) and filters.
const RAW_PARENT_RE = /^[ \t]*(\/\/-|-[ \t]*$|:[\w-])/;
// A buffered comment: its deeper-indented lines are output with `#{}` interpolated, unlike the text on its own line.
const COMMENT_PARENT_RE = /^[ \t]*\/\/(?!-)/;
// What follows the tags of a line whose deeper-indented lines are text with `#{}` in it (`script.`, `+card(a).`, `li: p.`).
const TEXT_TAG_END_RE = /^\.[ \t]*$/;
const BLOCK_EXPANSION_RE = /^: +/;
// Line starts where a whole-line `= …` would turn Pug's own syntax into code.
const PUG_LINE_START_RE = /^(?:[-=|<+:]|!=|\/\/)/;
// Lines Pug reads as a keyword before it tries a tag.
const KEYWORD_RE =
  /^(?:if|unless|else|each|for|while|case|when|default|mixin|block|append|prepend|include|extends?|doctype|yield)\b/;
// JavaScript words that may stand between two operands, and the reserved words no expression starts with.
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
  /^(?:var|let|const|if|else|for|while|do|switch|function|return|throw|try|catch|finally|class|break|continue|in|instanceof|with|case|default|debugger|enum|export|import|extends)\b/;
// Statements a `//-` comment can hold as code rather than prose: `if (`, `} else {`, `var x =`, `function f(`.
const CODE_STATEMENT_RE =
  /^(?:(?:if|for|while|switch|catch)\s*\(|\}|(?:else|do|try|finally)\s*\{|(?:var|let|const)\s+(?:[\w$]+|\{[^}]*\}|\[[^\]]*\])\s*=|function\b[^(]*\()/;
// What an expression can start with (an operand or a unary operator) and can't end with (a binary operator; `x++` can).
const EXPRESSION_START_RE = /^[\w$([{!+\-~]/;
const OPERATOR_END_RE =
  /(?:^|[^+-])[-+]$|[*/%=<>&|^!~?:,.]$|(?<![\w$.])(?:in|instanceof|typeof|new|void|delete)$/;
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

/** The text block that `head`, whose tags end at `end`, opens: `{ indent, kind }`, or null when its deeper lines are Pug. */
function textBlockOf(head, end) {
  const indent = leadingWhitespace(head).length;
  if (RAW_PARENT_RE.test(head)) return { indent, kind: "raw" };
  if (COMMENT_PARENT_RE.test(head) || TEXT_TAG_END_RE.test(head.slice(end)))
    return { indent, kind: "text" };
  return null;
}

/**
 * What Pug reads each line as: "pug"; "text" (with `#{}`) or "raw" under a line that makes its deeper lines text; or
 * "attributes" from the line after one that leaves an attribute list open up to the line that closes it.
 */
function lineKinds(lines) {
  const kinds = [];
  let block = null;
  let openHead = null;
  for (const line of lines) {
    if (openHead !== null) {
      kinds.push("attributes");
      openHead += `\n${line}`;
      const end = lineTagsEnd(openHead);
      if (end >= 0) {
        block = textBlockOf(openHead, end);
        openHead = null;
      }
    } else if (
      block &&
      (!line.trim() || leadingWhitespace(line).length > block.indent)
    ) {
      kinds.push(block.kind);
    } else {
      kinds.push("pug");
      const end = lineTagsEnd(line);
      block = end < 0 ? null : textBlockOf(line, end);
      if (end < 0) openHead = line;
    }
  }
  return kinds;
}

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
      i++;
      while (i < line.length && line[i] !== c) i += line[i] === "\\" ? 2 : 1;
      if (i >= line.length) return -1;
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

/** The end of the tag heads from `i`, block expansion (`li: a`) included; `i` when there is none, -1 when one doesn't close. */
function tagsEnd(line, i) {
  for (;;) {
    const head = tagHeadEnd(line, i);
    if (head <= i) return head;
    const expansion = BLOCK_EXPANSION_RE.exec(line.slice(head));
    if (!expansion) return head;
    i = head + expansion[0].length;
  }
}

/** The end of the tags a line starts with, a mixin call's included; -1 when an attribute list stays open. */
function lineTagsEnd(line) {
  const i = leadingWhitespace(line).length;
  if (KEYWORD_RE.test(line.slice(i))) return i;
  return tagsEnd(line, line[i] === "+" ? i + 1 : i);
}

/** The column where a line's inline text starts: after `| `, a tag's head and its space, or at a `<`; -1 when it has none. */
function textStart(line) {
  const first = leadingWhitespace(line).length;
  if (line[first] === "|")
    return line[first + 1] === " " ? first + 2 : first + 1;
  if (line[first] === "<") return first;
  if (KEYWORD_RE.test(line.slice(first))) return -1;
  const end = tagsEnd(line, first);
  return end > first && line[end] === " " ? end + 1 : -1;
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
  const kinds = lineKinds(lines);
  return [...targets]
    .filter((n) => kinds[n] === "pug" && !hasNested(lines, n))
    .map((n) => [n, togglePugMarker(lines[n])])
    .filter(([, text]) => text !== null);
}

/**
 * What replaces columns `start`..`end` of line `n`: `{ start, end, text }`. A whole line of content becomes `= content`,
 * a selection in text (after a tag, after `|`, in a `tag.` or `//` block) `#{selection}`, an empty one `#{}` to type in. Null where
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
  const kind = lineKinds(lines)[n];
  if (kind !== "pug")
    return kind === "text" && expression ? interpolated : null;
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
 * text block, and the rest of an attribute list a moved line opens; or a reason it can't move: it starts in a text block,
 * an attribute list or deeper than it ends, holds `extends` or a mixin's `block` without its `mixin`, or would leave an
 * `else`, `when` or `default` without its `if`, `case` or `each`. Returns `{ range, indent, lines }` as `extractBlock`
 * does, or `{ refused }`, or null when only blank lines are selected.
 */
function pugExtractBlock(lines, selection) {
  const block = extractBlock(lines, selection, true);
  if (!block) return null;
  const { indent } = block;
  const kinds = lineKinds(lines);
  const start = block.range.start.line;
  let last = block.range.end.line;
  for (let i = last + 1; i < lines.length; i++) {
    if (kinds[i] === "attributes" || isDeeper(lines[i], indent)) last = i;
    else if (lines[i].trim()) break;
  }
  const moved = lines.slice(start, last + 1);
  const firstLine = moved.findIndex((l) => l.trim()) + start;
  const first = lines[firstLine];
  const after = lines.slice(last + 1).find((l) => l.trim());
  if (kinds[firstLine] === "attributes")
    return { refused: "The selection starts inside an attribute list." };
  if (kinds[firstLine] !== "pug")
    return { refused: "The selection starts inside a text block." };
  if (leadingWhitespace(first) !== indent)
    return { refused: "The selection starts deeper than it ends." };
  const pugLines = moved
    .map((_, i) => i + start)
    .filter((n) => kinds[n] === "pug");
  if (pugLines.some((n) => EXTENDS_RE.test(lines[n])))
    return { refused: "`extends` must stay the template's first line." };
  const outsideMixin = (n) =>
    MIXIN_BLOCK_RE.test(lines[n]) && !underMixin(lines, kinds, start, n);
  if (pugLines.some(outsideMixin))
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

/** Whether Pug line `n` is nested under a `mixin` line from line `from` on. */
function underMixin(lines, kinds, from, n) {
  let indent = leadingWhitespace(lines[n]).length;
  for (let i = n - 1; i >= from && indent > 0; i--) {
    if (kinds[i] !== "pug" || !lines[i].trim()) continue;
    const own = leadingWhitespace(lines[i]).length;
    if (own >= indent) continue;
    if (MIXIN_RE.test(lines[i])) return true;
    indent = own;
  }
  return false;
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
  lineKinds,
  looksLikeExpression,
  textStart,
  wrapPug,
  pugExtractBlock,
  pugPartialPath,
  pugIncludePath,
};
