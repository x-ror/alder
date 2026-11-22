//! Pug (pug-rails' `.jst.pug`, plain `.pug`): ports of `pug-lexer` and `pug-parser` that stop at the first error
//! Pug raises, with its code, message, line and column. The JavaScript in templates isn't parsed.

pub mod elements;
pub mod lexer;
pub mod parser;
pub mod refs;

use crate::util::{coded, error, leading_ws, LineIndex, BOM};
use parser::{Alternate, Block, Code, Node, NodeKind};
use std::path::Path;
use tower_lsp_server::ls_types::{Diagnostic, Position, Range};

/// Stable diagnostic codes; `rubyTemplates.pug.disabledDiagnostics` refers to these.
pub mod code {
    pub const INCONSISTENT_INDENT: &str = "Syntax/PugInconsistentIndent";
    pub const UNCLOSED_BRACKET: &str = "Syntax/PugUnclosedBracket";
    pub const INVALID_NAME: &str = "Syntax/PugInvalidName";
    pub const DUPLICATE_ATTRIBUTE: &str = "Syntax/PugDuplicateAttribute";
    pub const MALFORMED_KEYWORD: &str = "Syntax/PugMalformedKeyword";
    pub const ILLEGAL_NESTING: &str = "Syntax/PugIllegalNesting";
    /// Pug's errors that belong to no other code, the JavaScript errors Pug itself throws on some templates, and nesting
    /// deeper than `MAX_DEPTH`.
    pub const SYNTAX_ERROR: &str = "Syntax/PugTemplate";
    pub const MISSING_FILE: &str = "Syntax/PugMissingFile";
    pub const EXTENDS_NOT_FIRST: &str = "Syntax/PugExtendsNotFirst";
    pub const UNEXPECTED_BLOCK: &str = "Syntax/PugUnexpectedBlock";
    pub const UNKNOWN_MIXIN: &str = "Syntax/PugUnknownMixin";
    pub const VOID_CHILDREN: &str = "Html/VoidChildren";
}

/// How many levels Pug's tree may nest, `#[...]` and `else if` each counting as one, before the template is refused
/// rather than followed: the parser and every walk over the tree recurse that deep, the parser with about 5 KB of stack a
/// level in a release build, on threads with `util::THREAD_STACK`. Pug's own walks run out of stack past a few hundred
/// levels of `if` and a thousand or so of tags.
pub(crate) const MAX_DEPTH: usize = 256;
/// The codes of the errors Pug raises, and of the JavaScript errors it throws on some templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    BlockInBufferedCode,
    BlockOutsideMixin,
    BracketMismatch,
    DefaultWithExpression,
    DuplicateAttribute,
    DuplicateId,
    ElseCondition,
    InconsistentIndentation,
    InvalidClassName,
    InvalidId,
    InvalidIndentation,
    InvalidKeyCharacter,
    InvalidToken,
    MalformedEach,
    MalformedEachOfLval,
    MalformedExtends,
    MalformedInclude,
    MixinWithoutBody,
    NoCaseExpression,
    NoEndBracket,
    NoExtendsPath,
    NoIncludePath,
    NoWhenExpression,
    NoWhileExpression,
    RawIncludeBlock,
    UnexpectedText,
    /// character-parser's, which Pug lets through.
    MismatchedBracket,
    /// Node's `assert`.
    Assertion,
    TypeError,
    /// Nesting past `MAX_DEPTH`, which Pug doesn't have.
    NestingLimit,
}

impl ErrorCode {
    /// As Pug spells it: `PUG:INVALID_ID`, `CHARACTER_PARSER:MISMATCHED_BRACKET`, `ERR_ASSERTION`, `TypeError`.
    pub fn as_str(self) -> &'static str {
        use ErrorCode::*;
        match self {
            BlockInBufferedCode => "PUG:BLOCK_IN_BUFFERED_CODE",
            // Pug's code has this spelling.
            BlockOutsideMixin => "PUG:BLOCK_OUTISDE_MIXIN",
            BracketMismatch => "PUG:BRACKET_MISMATCH",
            DefaultWithExpression => "PUG:DEFAULT_WITH_EXPRESSION",
            DuplicateAttribute => "PUG:DUPLICATE_ATTRIBUTE",
            DuplicateId => "PUG:DUPLICATE_ID",
            ElseCondition => "PUG:ELSE_CONDITION",
            InconsistentIndentation => "PUG:INCONSISTENT_INDENTATION",
            InvalidClassName => "PUG:INVALID_CLASS_NAME",
            InvalidId => "PUG:INVALID_ID",
            InvalidIndentation => "PUG:INVALID_INDENTATION",
            InvalidKeyCharacter => "PUG:INVALID_KEY_CHARACTER",
            InvalidToken => "PUG:INVALID_TOKEN",
            MalformedEach => "PUG:MALFORMED_EACH",
            MalformedEachOfLval => "PUG:MALFORMED_EACH_OF_LVAL",
            MalformedExtends => "PUG:MALFORMED_EXTENDS",
            MalformedInclude => "PUG:MALFORMED_INCLUDE",
            MixinWithoutBody => "PUG:MIXIN_WITHOUT_BODY",
            NoCaseExpression => "PUG:NO_CASE_EXPRESSION",
            NoEndBracket => "PUG:NO_END_BRACKET",
            NoExtendsPath => "PUG:NO_EXTENDS_PATH",
            NoIncludePath => "PUG:NO_INCLUDE_PATH",
            NoWhenExpression => "PUG:NO_WHEN_EXPRESSION",
            NoWhileExpression => "PUG:NO_WHILE_EXPRESSION",
            RawIncludeBlock => "PUG:RAW_INCLUDE_BLOCK",
            UnexpectedText => "PUG:UNEXPECTED_TEXT",
            MismatchedBracket => "CHARACTER_PARSER:MISMATCHED_BRACKET",
            Assertion => "ERR_ASSERTION",
            TypeError => "TypeError",
            NestingLimit => "NESTING_LIMIT",
        }
    }

    /// Whether Pug raises it itself, rather than a library or the JavaScript engine throwing it.
    pub fn is_pugs(self) -> bool {
        !matches!(self, ErrorCode::MismatchedBracket | ErrorCode::Assertion | ErrorCode::TypeError | ErrorCode::NestingLimit)
    }
}

/// The first error Pug raises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    /// 1-based, as Pug counts after turning `\r\n` and `\r` into `\n`.
    pub line: usize,
    /// 1-based, in UTF-16 code units; where Pug's own error has no position, where its lexer or parser was.
    pub column: usize,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{} {} ({})", self.line, self.column, self.message, self.code.as_str())
    }
}

impl std::error::Error for Error {}

/// The error for a node, at 1-based `line` and UTF-16 `column`, nested past `MAX_DEPTH`.
fn too_deep(line: usize, column: usize) -> Error {
    let message = format!("Nested more than {MAX_DEPTH} levels deep, which isn't checked");
    Error { code: ErrorCode::NestingLimit, message, line, column }
}

/// Pug's first error, with Pug's message, filed under the code of the construct it is about; without one, the content in a
/// void element that Pug refuses to compile and, for a file on disk, the problems with what it includes and extends. Only
/// the codes `reports` takes are looked for. `parsed` is what `parser::parse` gives for `text`, `index` its lines,
/// `LineIndex::with_lone_cr` as Pug counts them, and `file` the file's path with what knows which templates include it.
pub fn diagnostics(
    text: &str,
    index: &LineIndex,
    parsed: &Result<Block, Error>,
    file: Option<(&Path, &refs::Includers)>,
    reports: impl Fn(&str) -> bool,
) -> Vec<Diagnostic> {
    match parsed {
        Ok(tree) => {
            let mut found: Vec<Diagnostic> = Vec::new();
            if reports(code::VOID_CHILDREN) {
                found.extend(elements::void_children(index, tree));
            }
            found.extend(file.map(|file| refs::diagnostics(text, index, tree, file, &reports)).unwrap_or_default());
            found
        }
        Err(found) => {
            let code = code_of(found.code);
            if !reports(code) {
                return vec![];
            }
            vec![coded(error(range(text, found, code == code::INCONSISTENT_INDENT), found.message.clone()), code)]
        }
    }
}

/// Whether `text` has a `\r` not followed by `\n`, a line break for Pug and the editor that `LineIndex::new` doesn't count,
/// so the positions it gives for what follows would be off.
pub fn has_lone_cr(text: &str) -> bool {
    text.match_indices('\r').any(|(at, _)| !text[at + 1..].starts_with('\n'))
}

/// Whether Pug accepts both texts and builds the same tree from them, positions aside and without the whitespace that
/// ends an expression Pug wraps (`= x  `, `while x  `), which changes nothing it does. The end of a `-` line counts:
/// Pug joins consecutive ones into one piece of JavaScript, where it may be inside a string.
pub fn same_compilation(before: &str, after: &str) -> bool {
    parser::parse(before).is_ok_and(|tree| compiles_as(tree, after))
}

/// `same_compilation` of the template whose tree is `tree` and `after`.
pub fn compiles_as(mut tree: Block, after: &str) -> bool {
    let Ok(mut after) = parser::parse(after) else { return false };
    normalize_block(&mut tree);
    normalize_block(&mut after);
    tree == after
}

fn normalize_block(block: &mut Block) {
    block.line = 0;
    for node in &mut block.nodes {
        normalize_node(node);
    }
}

fn normalize_node(node: &mut Node) {
    (node.line, node.column, node.span) = (0, 0, 0..0);
    let trim = |js: &mut String| js.truncate(js.trim_end_matches(lexer::is_js_whitespace).len());
    match &mut node.kind {
        NodeKind::Tag(tag) | NodeKind::MixinCall { tag, .. } => {
            for attr in &mut tag.attrs {
                (attr.line, attr.column, attr.span) = (0, 0, 0..0);
            }
            for block in &mut tag.attribute_blocks {
                (block.line, block.column, block.span) = (0, 0, 0..0);
            }
            normalize_block(&mut tag.block);
        }
        NodeKind::Code(code) => {
            if code.buffer {
                trim(&mut code.val);
            }
            if let Some(block) = &mut code.block {
                normalize_block(block);
            }
        }
        NodeKind::Conditional(conditional) => {
            normalize_block(&mut conditional.consequent);
            match &mut conditional.alternate {
                Some(Alternate::ElseIf(node)) => normalize_node(node),
                Some(Alternate::Else(block)) => normalize_block(block),
                None => {}
            }
        }
        NodeKind::While { test: js, block } | NodeKind::Case { expr: js, block } | NodeKind::EachOf { obj: js, block, .. } => {
            trim(js);
            normalize_block(block);
        }
        NodeKind::Each { obj, block, alternate, .. } => {
            trim(obj);
            normalize_block(block);
            if let Some(alternate) = alternate {
                normalize_block(alternate);
            }
        }
        NodeKind::When { expr, block } => {
            trim(expr);
            if let Some(block) = block {
                normalize_block(block);
            }
        }
        NodeKind::MixinDefinition { block, .. } | NodeKind::NamedBlock { block, .. } | NodeKind::BlockComment { block, .. } => {
            normalize_block(block)
        }
        NodeKind::Extends { file } => (file.line, file.column, file.span) = (0, 0, 0..0),
        NodeKind::Include { file, block } => {
            (file.line, file.column, file.span) = (0, 0, 0..0);
            normalize_block(block);
        }
        NodeKind::RawInclude { file, filters } => {
            (file.line, file.column, file.span) = (0, 0, 0..0);
            for filter in filters {
                (filter.line, filter.column, filter.span) = (0, 0, 0..0);
                for attr in &mut filter.attrs {
                    (attr.line, attr.column, attr.span) = (0, 0, 0..0);
                }
            }
        }
        NodeKind::Filter { attrs, block, .. } => {
            for attr in attrs {
                (attr.line, attr.column, attr.span) = (0, 0, 0..0);
            }
            normalize_block(block);
        }
        NodeKind::Text { .. } | NodeKind::Comment { .. } | NodeKind::Doctype(_) | NodeKind::MixinBlock | NodeKind::YieldBlock => {}
    }
}

/// Every node under `block`, depth first in document order, as pug-walk visits them.
pub(crate) fn nodes(block: &Block) -> Vec<&Node> {
    walk(block, |_| true)
}

/// `nodes`, without what is under the nodes `enter` refuses.
pub(crate) fn walk(block: &Block, enter: impl Fn(&Node) -> bool) -> Vec<&Node> {
    let mut out = Vec::new();
    let mut pending: Vec<&Node> = block.nodes.iter().rev().collect();
    while let Some(node) = pending.pop() {
        out.push(node);
        if enter(node) {
            pending.extend(children(node).into_iter().rev());
        }
    }
    out
}

/// The nodes right under `node`, in pug-walk's order; an `else if` is one of its `if`'s, and the `#[...]` tags in a
/// `//` comment's text are the comment's, which pug-code-gen compiles.
pub(crate) fn children(node: &Node) -> Vec<&Node> {
    let mut out: Vec<&Node> = Vec::new();
    match &node.kind {
        NodeKind::Tag(tag) | NodeKind::MixinCall { tag, .. } => out.extend(&tag.block.nodes),
        NodeKind::Conditional(conditional) => {
            out.extend(&conditional.consequent.nodes);
            match &conditional.alternate {
                Some(Alternate::ElseIf(node)) => out.push(node),
                Some(Alternate::Else(block)) => out.extend(&block.nodes),
                None => {}
            }
        }
        NodeKind::Each { block, alternate, .. } => {
            out.extend(&block.nodes);
            if let Some(block) = alternate {
                out.extend(&block.nodes);
            }
        }
        NodeKind::When { block: Some(block), .. }
        | NodeKind::EachOf { block, .. }
        | NodeKind::While { block, .. }
        | NodeKind::Case { block, .. }
        | NodeKind::NamedBlock { block, .. }
        | NodeKind::MixinDefinition { block, .. }
        | NodeKind::Include { block, .. }
        | NodeKind::Filter { block, .. }
        | NodeKind::BlockComment { block, .. }
        | NodeKind::Code(Code { block: Some(block), .. }) => out.extend(&block.nodes),
        NodeKind::When { block: None, .. }
        | NodeKind::Code(Code { block: None, .. })
        | NodeKind::Text { .. }
        | NodeKind::MixinBlock
        | NodeKind::YieldBlock
        | NodeKind::Extends { .. }
        | NodeKind::RawInclude { .. }
        | NodeKind::Comment { .. }
        | NodeKind::Doctype(_) => {}
    }
    out
}

/// The template's tree, or the tree of the lines before its first error.
pub fn parsed(text: &str) -> Option<Block> {
    parsable(text).map(|(_, tree)| tree)
}

/// The tokens and tree of the template, or of the lines before its first error when those parse.
pub(crate) fn parsable(text: &str) -> Option<(Vec<lexer::Token>, Block)> {
    let read = |text: &str| -> Result<(Vec<lexer::Token>, Block), Error> {
        let tokens = lexer::lex(text)?;
        let tree = parser::parse_tokens(tokens.clone())?;
        Ok((tokens, tree))
    };
    let error = match read(text) {
        Ok(read) => return Some(read),
        Err(error) => error,
    };
    let before: usize = text.split_inclusive('\n').take(error.line.saturating_sub(1)).map(str::len).sum();
    read(&text[..before]).ok()
}

fn code_of(pug_code: ErrorCode) -> &'static str {
    use ErrorCode::*;
    match pug_code {
        InconsistentIndentation | InvalidIndentation => code::INCONSISTENT_INDENT,
        NoEndBracket | BracketMismatch | MismatchedBracket => code::UNCLOSED_BRACKET,
        InvalidId | InvalidClassName => code::INVALID_NAME,
        DuplicateId | DuplicateAttribute => code::DUPLICATE_ATTRIBUTE,
        NoExtendsPath
        | MalformedExtends
        | NoIncludePath
        | MalformedInclude
        | NoCaseExpression
        | NoWhenExpression
        | DefaultWithExpression
        | ElseCondition
        | NoWhileExpression
        | MalformedEach
        | MalformedEachOfLval => code::MALFORMED_KEYWORD,
        BlockInBufferedCode | RawIncludeBlock => code::ILLEGAL_NESTING,
        BlockOutsideMixin | InvalidKeyCharacter | InvalidToken | MixinWithoutBody | UnexpectedText | Assertion | TypeError
        | NestingLimit => code::SYNTAX_ERROR,
    }
}

/// From Pug's position to the end of its line, or with `indentation`, to the end of the line's indentation. Lines are Pug's,
/// which a lone `\r` ends as the editor's do.
fn range(text: &str, found: &Error, indentation: bool) -> Range {
    let line = found.line.saturating_sub(1);
    let mut lines = text.split('\n').flat_map(|line| line.strip_suffix('\r').unwrap_or(line).split('\r'));
    let raw = lines.nth(line).unwrap_or_default();
    // Pug drops the byte order mark; the editor counts it.
    let bom = if line == 0 && raw.starts_with(BOM) { BOM.len_utf16() } else { 0 };
    let start = bom + found.column.saturating_sub(1);
    let end = if indentation { leading_ws(raw).len() } else { raw.trim_end().len() };
    let end = raw[..end].encode_utf16().count().max(start);
    Range::new(Position::new(line as u32, start as u32), Position::new(line as u32, end as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (line, start, end, code, message) of the diagnostic for `text`, 0-based as the editor gets them.
    fn found(text: &str) -> Vec<(u32, u32, u32, String, String)> {
        diagnostics(text, &LineIndex::with_lone_cr(text), &parser::parse(text), None, |_| true)
            .into_iter()
            .map(|d| {
                let code = crate::util::code(&d).unwrap_or_default().to_string();
                (d.range.start.line, d.range.start.character, d.range.end.character, code, d.message)
            })
            .collect()
    }

    fn at(line: u32, start: u32, end: u32, code: &str, message: &str) -> (u32, u32, u32, String, String) {
        (line, start, end, code.to_string(), message.to_string())
    }

    #[test]
    fn errors_print_with_pugs_code() {
        let error = parser::parse("p#!x").unwrap_err();
        assert_eq!(error.to_string(), "1:2 \"!x\" is not a valid ID. (PUG:INVALID_ID)");
    }

    #[test]
    fn valid_templates_have_none() {
        assert!(found("extends layout\nblock content\n  p= user.name\n").is_empty());
    }

    #[test]
    fn errors_start_at_pugs_position_and_run_to_the_end_of_the_line() {
        use code::*;
        let malformed_each =
            "This `each` has a syntax error. `each` statements should be of the form: `each VARIABLE_NAME of JS_EXPRESSION`".to_string();
        let each_of_value = "The value variable for each must either be a valid identifier (e.g. `item`) or a pair of identifiers in square brackets (e.g. `[key, value]`).";
        let cases = [
            ("div\n    p\n  a", at(2, 0, 2, INCONSISTENT_INDENT, "Inconsistent indentation. Expecting either 0 or 4 spaces/tabs.")),
            ("div\n\tp\n  a", at(2, 0, 2, INCONSISTENT_INDENT, "Invalid indentation, you can use tabs or spaces but not both")),
            ("p #[b", at(0, 5, 5, UNCLOSED_BRACKET, "End of line was reached with no closing bracket for interpolation.")),
            ("p(a=\"{\"=})  ", at(0, 8, 10, UNCLOSED_BRACKET, "Mismatched Bracket: }")),
            // Pug's own error has no position: where its lexer stopped.
            ("case x\n  when a)", at(1, 9, 9, UNCLOSED_BRACKET, "Mismatched Bracket: )")),
            ("p#!x", at(0, 1, 4, INVALID_NAME, "\"!x\" is not a valid ID.")),
            ("p.1", at(0, 1, 3, INVALID_NAME, "Class names must contain at least one letter or underscore.")),
            ("p(id=\"b\")#a", at(0, 9, 11, DUPLICATE_ATTRIBUTE, "Duplicate attribute \"id\" is not allowed.")),
            ("case", at(0, 4, 4, MALFORMED_KEYWORD, "missing expression for case")),
            ("each x\n  p", at(0, 4, 6, MALFORMED_KEYWORD, &malformed_each)),
            ("each item in offers\n  p", at(0, 19, 19, MALFORMED_KEYWORD, each_of_value)),
            ("= x\n  p", at(1, 0, 3, ILLEGAL_NESTING, "Buffered code cannot have a block attached to it")),
            ("p\n  ]", at(1, 2, 3, SYNTAX_ERROR, "unexpected text \"]\"")),
            ("mixin foo", at(0, 0, 9, SYNTAX_ERROR, "Mixin foo declared without body")),
            ("div\n  .", at(1, 2, 3, SYNTAX_ERROR, "Cannot read properties of undefined (reading 'type')")),
        ];
        for (text, expected) in cases {
            assert_eq!(found(text), [expected], "{text:?}");
        }
    }

    #[test]
    fn columns_count_utf16_units_and_the_byte_order_mark() {
        assert_eq!(
            found("p(title=\"😀\")#a(id=\"b\")"),
            [at(0, 16, 23, code::DUPLICATE_ATTRIBUTE, "Duplicate attribute \"id\" is not allowed.")]
        );
        assert_eq!(found("\u{feff}p#!x"), [at(0, 2, 5, code::INVALID_NAME, "\"!x\" is not a valid ID.")]);
        assert_eq!(found("div\r\n    p\r\n  a\r\n")[0].0, 2);
    }

    #[test]
    fn a_lone_cr_ends_a_line() {
        let message = "Inconsistent indentation. Expecting either 0 or 4 spaces/tabs.";
        assert_eq!(found("div\r    p\r  a\r"), [at(2, 0, 2, code::INCONSISTENT_INDENT, message)]);
        assert_eq!(
            found("p #[b\rq"),
            [at(0, 5, 5, code::UNCLOSED_BRACKET, "End of line was reached with no closing bracket for interpolation.")]
        );
        let void = "img is a self closing element: <img/> but contains nested content.";
        assert_eq!(found("p\r\nimg\r  p\r"), [at(1, 0, 3, code::VOID_CHILDREN, void)]);
        assert!(has_lone_cr("a\rb") && has_lone_cr("a\r") && !has_lone_cr("a\r\nb\n"));
    }

    #[test]
    fn checks_whose_rule_is_off_are_not_run() {
        let without = |text: &str, code: &str| diagnostics(text, &LineIndex::new(text), &parser::parse(text), None, |c| c != code);
        assert!(without("img\n  p\n", code::VOID_CHILDREN).is_empty());
        assert!(without("div\n    p\n  a\n", code::INCONSISTENT_INDENT).is_empty());
    }

    #[test]
    fn broken_fixture() {
        let message = "Inconsistent indentation. Expecting either 4 or 6 spaces/tabs.";
        assert_eq!(found(include_str!("../../test/broken/bad.pug")), [at(6, 0, 5, code::INCONSISTENT_INDENT, message)]);
    }

    /// Templates whose tree is `levels` deep, by the construct that nests, and where the node `MAX_DEPTH + 1` levels deep
    /// starts.
    fn nested(levels: usize) -> Vec<(&'static str, String, (usize, usize))> {
        let indent = |depth: usize| "  ".repeat(depth);
        let lines = |line: &dyn Fn(usize) -> String| (0..levels).map(line).collect::<Vec<_>>().join("\n") + "\n";
        let inline = |levels: usize| format!("{}x{}", "#[b ".repeat(levels), "]".repeat(levels));
        // A `case` and its `when` are a level each, and the deepest node is a tag in a `when`.
        let mut cases = if levels % 2 == 0 { vec!["div".to_string()] } else { Vec::new() };
        while cases.len() + 1 < levels {
            cases.extend([format!("{}case x", indent(cases.len())), format!("{}when 1", indent(cases.len() + 1))]);
        }
        cases.push(format!("{}p", indent(cases.len())));
        let (deepest, tag_column) = (MAX_DEPTH + 1, 2 * MAX_DEPTH + 1);
        vec![
            ("tags", lines(&|i| format!("{}div", indent(i))), (deepest, tag_column)),
            ("conditionals", lines(&|i| format!("{}if x", indent(i))), (deepest, tag_column)),
            (
                "loops",
                lines(&|i| if i + 1 < levels { format!("{}each x in y", indent(i)) } else { format!("{}p", indent(i)) }),
                (deepest, tag_column),
            ),
            ("cases", cases.join("\n") + "\n", (deepest, tag_column)),
            ("mixin calls", format!("mixin m\n  block\n{}", lines(&|i| format!("{}+m", indent(i)))), (deepest + 2, tag_column)),
            ("block expansion", vec!["a"; levels].join(": ") + "\n", (1, 3 * MAX_DEPTH + 1)),
            ("else if", format!("if a\n{}", "else if b\n".repeat(levels - 1)), (deepest, 1)),
            ("filters", ":a".repeat(levels) + "\n", (1, tag_column)),
            // `p` and then each `#[b ` is a level; the tag in it starts past its `#[`.
            ("inline tags", format!("p {}\n", inline(levels - 1)), (1, 4 * MAX_DEPTH + 1)),
            ("a comment's inline tags", format!("//\n  {}\n", inline(levels - 1)), (2, 4 * MAX_DEPTH + 1)),
            // Joined into one text node, but parsed a block inside another.
            ("lines of HTML", (0..=levels).map(|i| format!("{}<i>", indent(i))).collect::<Vec<_>>().join("\n") + "\n", (deepest + 1, 1)),
        ]
    }

    /// The check, the HTML lints, the outline and the formatter, which all walk the tree, on a thread with the server's
    /// stack; the check's findings.
    fn every_walk(text: String) -> Vec<(u32, u32, u32, String, String)> {
        let walk = move || {
            let root = std::env::temp_dir().join(format!("rt-pug-depth-{}", std::process::id()));
            crate::util::write_files(&root, &[("layout.pug", "block content\n")]);
            // What an extending template has outside its blocks is linked too.
            let page = root.join("page.pug");
            let extending = format!("extends layout\n{text}");
            std::fs::write(&page, &extending).unwrap();
            let includers = refs::Includers::default();
            diagnostics(&extending, &LineIndex::with_lone_cr(&extending), &parser::parse(&extending), Some((&page, &includers)), |_| true);
            let _ = std::fs::remove_dir_all(&root);
            if let Ok(tree) = parser::parse(&text) {
                let markup = elements::tags(&text, &tree);
                let ancestry = elements::ancestry(&tree);
                crate::element_lints::diagnostics(crate::lang::TemplateLang::Pug, &text, &markup, &[], Some(&ancestry));
                crate::nested_interactive::diagnostics(crate::lang::TemplateLang::Pug, &text, &markup, &[], Some(&ancestry));
                crate::outline::pug::document_symbols(&text);
                assert!(same_compilation(&text, &text));
                let tokens = lexer::lex(&text).unwrap();
                crate::format::pug::format(&text, &tokens, &crate::format::Settings::default());
            }
            found(&text)
        };
        std::thread::Builder::new().stack_size(crate::util::THREAD_STACK).spawn(walk).unwrap().join().unwrap()
    }

    #[test]
    fn nesting_up_to_the_limit_is_followed_and_past_it_refused() {
        for (construct, text, _) in nested(MAX_DEPTH) {
            assert!(every_walk(text).iter().all(|found| found.3 != code::SYNTAX_ERROR), "{construct}");
        }
        let message = format!("Nested more than {MAX_DEPTH} levels deep, which isn't checked");
        // The lexer, which reads the whole template first, stops at the `#[` past the limit.
        let past_lexer = format!("p {}x{}\n", "#[b ".repeat(MAX_DEPTH + 1), "]".repeat(MAX_DEPTH + 1));
        let error = lexer::lex(&past_lexer).unwrap_err();
        assert_eq!((error.code, error.line, error.column), (ErrorCode::NestingLimit, 1, 4 * MAX_DEPTH + 3));
        for (construct, text, (line, column)) in nested(MAX_DEPTH + 1) {
            let found = every_walk(text);
            let (line, column) = ((line - 1) as u32, (column - 1) as u32);
            assert_eq!(
                (found[0].0, found[0].1, found[0].3.as_str(), found[0].4.as_str()),
                (line, column, code::SYNTAX_ERROR, message.as_str()),
                "{construct}"
            );
        }
    }

    #[test]
    fn a_long_line_of_interpolations_is_read_without_recursing() {
        const LONG: usize = 20_000;
        for text in ["p #[b x] ".repeat(LONG), "p \\#{".repeat(LONG), "p \\#[x ".repeat(LONG), "p #{a}".repeat(LONG)] {
            assert!(found(&(text.clone() + "\n")).is_empty(), "{}", &text[..20]);
        }
    }
}
