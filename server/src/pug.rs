//! Pug (pug-rails' `.jst.pug`, plain `.pug`): ports of `pug-lexer` and `pug-parser` that stop at the first error
//! Pug raises, with its code, message, line and column. The JavaScript in templates isn't parsed.

pub mod elements;
pub mod lexer;
pub mod parser;
pub mod refs;

use crate::util::{coded, error, leading_ws, LineIndex, BOM};
use parser::{Alternate, Block, Node, NodeKind};
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
    /// Pug's errors that belong to no other code, and the JavaScript errors Pug itself throws on some templates.
    pub const SYNTAX_ERROR: &str = "Syntax/PugTemplate";
    pub const MISSING_FILE: &str = "Syntax/PugMissingFile";
    pub const EXTENDS_NOT_FIRST: &str = "Syntax/PugExtendsNotFirst";
    pub const UNEXPECTED_BLOCK: &str = "Syntax/PugUnexpectedBlock";
    pub const UNKNOWN_MIXIN: &str = "Syntax/PugUnknownMixin";
    pub const VOID_CHILDREN: &str = "Html/VoidChildren";
}

/// The first error Pug raises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// Pug's code (`PUG:INCONSISTENT_INDENTATION`, ...), or for a template that makes Pug itself fail, the code or
    /// class of the JavaScript error it throws (`CHARACTER_PARSER:MISMATCHED_BRACKET`, `ERR_ASSERTION`, `TypeError`).
    pub code: &'static str,
    pub message: String,
    /// 1-based, as Pug counts after turning `\r\n` and `\r` into `\n`.
    pub line: usize,
    /// 1-based, in UTF-16 code units; where Pug's own error has no position, where its lexer or parser was.
    pub column: usize,
}

/// Pug's first error, with Pug's message, filed under the code of the construct it is about; without one, the content in a
/// void element that Pug refuses to compile and, for a file on disk, the problems with what it includes and extends. Only
/// the codes `reports` takes are looked for, and only the first error in text with a lone `\r`.
pub fn diagnostics(text: &str, path: Option<&Path>, reports: &dyn Fn(&str) -> bool) -> Vec<Diagnostic> {
    match parser::parse(text) {
        Ok(_) if has_lone_cr(text) => vec![],
        Ok(tree) => {
            let mut found: Vec<Diagnostic> = Vec::new();
            if reports(code::VOID_CHILDREN) {
                found.extend(elements::void_children(text, &tree));
            }
            found.extend(path.map(|path| refs::diagnostics(text, &tree, path, reports)).unwrap_or_default());
            found
        }
        Err(found) => {
            let code = code_of(found.code);
            if !reports(code) {
                return vec![];
            }
            vec![coded(error(range(text, &found, code == code::INCONSISTENT_INDENT), found.message), code)]
        }
    }
}

/// Whether `text` has a `\r` not followed by `\n`, a line break for Pug and the editor that `LineIndex` doesn't count, so
/// the positions of what follows it would be off.
pub fn has_lone_cr(text: &str) -> bool {
    text.match_indices('\r').any(|(at, _)| !text[at + 1..].starts_with('\n'))
}

/// Whether Pug accepts both texts and builds the same tree from them, positions aside and without the whitespace that
/// ends an expression Pug wraps (`= x  `, `while x  `), which changes nothing it does. The end of a `-` line counts:
/// Pug joins consecutive ones into one piece of JavaScript, where it may be inside a string.
pub fn same_compilation(before: &str, after: &str) -> bool {
    match (parser::parse(before), parser::parse(after)) {
        (Ok(mut before), Ok(mut after)) => {
            normalize_block(&mut before);
            normalize_block(&mut after);
            before == after
        }
        _ => false,
    }
}

fn normalize_block(block: &mut Block) {
    block.line = 0;
    block.nodes.iter_mut().for_each(normalize_node);
}

fn normalize_node(node: &mut Node) {
    (node.line, node.column) = (0, 0);
    let trim = |js: &mut String| js.truncate(js.trim_end_matches(lexer::is_js_whitespace).len());
    match &mut node.kind {
        NodeKind::Tag(tag) | NodeKind::MixinCall { tag, .. } => {
            tag.attrs.iter_mut().for_each(|attr| (attr.line, attr.column) = (0, 0));
            tag.attribute_blocks.iter_mut().for_each(|block| (block.line, block.column) = (0, 0));
            normalize_block(&mut tag.block);
        }
        NodeKind::Code(code) => {
            if code.buffer {
                trim(&mut code.val);
            }
            code.block.iter_mut().for_each(normalize_block);
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
            alternate.iter_mut().for_each(normalize_block);
        }
        NodeKind::When { expr, block } => {
            trim(expr);
            block.iter_mut().for_each(normalize_block);
        }
        NodeKind::MixinDefinition { block, .. } | NodeKind::NamedBlock { block, .. } | NodeKind::BlockComment { block, .. } => {
            normalize_block(block)
        }
        NodeKind::Extends { file } => (file.line, file.column) = (0, 0),
        NodeKind::Include { file, block } => {
            (file.line, file.column) = (0, 0);
            normalize_block(block);
        }
        NodeKind::RawInclude { file, filters } => {
            (file.line, file.column) = (0, 0);
            for filter in filters {
                (filter.line, filter.column) = (0, 0);
                filter.attrs.iter_mut().for_each(|attr| (attr.line, attr.column) = (0, 0));
            }
        }
        NodeKind::Filter { attrs, block, .. } => {
            attrs.iter_mut().for_each(|attr| (attr.line, attr.column) = (0, 0));
            normalize_block(block);
        }
        NodeKind::Text { .. } | NodeKind::Comment { .. } | NodeKind::Doctype(_) | NodeKind::MixinBlock | NodeKind::YieldBlock => {}
    }
}

/// Every node under `block`, depth first in document order, as pug-walk visits them.
pub(crate) fn nodes(block: &Block) -> Vec<&Node> {
    let mut out = Vec::new();
    push_block(block, &mut out);
    out
}

fn push_block<'a>(block: &'a Block, out: &mut Vec<&'a Node>) {
    block.nodes.iter().for_each(|node| push_node(node, out));
}

fn push_node<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
    out.push(node);
    children(node).into_iter().for_each(|child| push_node(child, out));
}

/// The nodes right under `node`, in pug-walk's order; an `else if` is one of its `if`'s.
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
            alternate.iter().for_each(|block| out.extend(&block.nodes));
        }
        NodeKind::When { block: Some(block), .. }
        | NodeKind::EachOf { block, .. }
        | NodeKind::While { block, .. }
        | NodeKind::Case { block, .. }
        | NodeKind::NamedBlock { block, .. }
        | NodeKind::MixinDefinition { block, .. }
        | NodeKind::Include { block, .. }
        | NodeKind::Filter { block, .. } => out.extend(&block.nodes),
        NodeKind::Code(code) => code.block.iter().for_each(|block| out.extend(&block.nodes)),
        _ => {}
    }
    out
}

/// The template's tree, or the tree of the lines before its first error.
pub fn parsed(text: &str) -> Option<Block> {
    parsable(text).and_then(|text| parser::parse(text).ok())
}

/// The template, or the lines before its first error when those parse.
pub(crate) fn parsable(text: &str) -> Option<&str> {
    let error = match parser::parse(text) {
        Ok(_) => return Some(text),
        Err(error) => error,
    };
    let before: usize = text.split_inclusive('\n').take(error.line.saturating_sub(1)).map(str::len).sum();
    parser::parse(&text[..before]).is_ok().then(|| &text[..before])
}

/// The byte offset of Pug's 1-based line and UTF-16 column, which don't count the byte order mark.
pub fn offset(index: &LineIndex, line: usize, column: usize) -> usize {
    let line = line.saturating_sub(1).min(index.line_of(usize::MAX));
    let raw = index.line(line);
    let bom = if line == 0 && raw.starts_with(BOM) { BOM.len_utf8() } else { 0 };
    index.line_start(line) + bom + lexer::unit_offset(&raw[bom..], column.saturating_sub(1))
}

fn code_of(pug_code: &str) -> &'static str {
    match pug_code {
        "PUG:INCONSISTENT_INDENTATION" | "PUG:INVALID_INDENTATION" => code::INCONSISTENT_INDENT,
        "PUG:NO_END_BRACKET" | "PUG:BRACKET_MISMATCH" | "CHARACTER_PARSER:MISMATCHED_BRACKET" => code::UNCLOSED_BRACKET,
        "PUG:INVALID_ID" | "PUG:INVALID_CLASS_NAME" => code::INVALID_NAME,
        "PUG:DUPLICATE_ID" | "PUG:DUPLICATE_ATTRIBUTE" => code::DUPLICATE_ATTRIBUTE,
        "PUG:NO_EXTENDS_PATH"
        | "PUG:MALFORMED_EXTENDS"
        | "PUG:NO_INCLUDE_PATH"
        | "PUG:MALFORMED_INCLUDE"
        | "PUG:NO_CASE_EXPRESSION"
        | "PUG:NO_WHEN_EXPRESSION"
        | "PUG:DEFAULT_WITH_EXPRESSION"
        | "PUG:ELSE_CONDITION"
        | "PUG:NO_WHILE_EXPRESSION"
        | "PUG:MALFORMED_EACH"
        | "PUG:MALFORMED_EACH_OF_LVAL" => code::MALFORMED_KEYWORD,
        "PUG:BLOCK_IN_BUFFERED_CODE" | "PUG:RAW_INCLUDE_BLOCK" => code::ILLEGAL_NESTING,
        _ => code::SYNTAX_ERROR,
    }
}

/// From Pug's position to the end of its line, or with `indentation`, to the end of the line's indentation. Lines are Pug's,
/// which a lone `\r` ends as the editor's do.
fn range(text: &str, found: &Error, indentation: bool) -> Range {
    let line = found.line.saturating_sub(1);
    let lines = text.split('\n').flat_map(|line| line.strip_suffix('\r').unwrap_or(line).split('\r'));
    let raw = lines.clone().nth(line).unwrap_or_default();
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
        diagnostics(text, None, &|_| true)
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
    fn a_lone_cr_ends_a_line_for_errors_and_leaves_out_the_rest() {
        let message = "Inconsistent indentation. Expecting either 0 or 4 spaces/tabs.";
        assert_eq!(found("div\r    p\r  a\r"), [at(2, 0, 2, code::INCONSISTENT_INDENT, message)]);
        assert_eq!(
            found("p #[b\rq"),
            [at(0, 5, 5, code::UNCLOSED_BRACKET, "End of line was reached with no closing bracket for interpolation.")]
        );
        assert!(found("img\r  p\r").is_empty());
        assert!(found("img\n  p\n").len() == 1);
        assert!(has_lone_cr("a\rb") && has_lone_cr("a\r") && !has_lone_cr("a\r\nb\n"));
    }

    #[test]
    fn checks_whose_rule_is_off_are_not_run() {
        assert!(diagnostics("img\n  p\n", None, &|code| code != code::VOID_CHILDREN).is_empty());
        assert!(diagnostics("div\n    p\n  a\n", None, &|code| code != code::INCONSISTENT_INDENT).is_empty());
    }

    #[test]
    fn broken_fixture() {
        let message = "Inconsistent indentation. Expecting either 4 or 6 spaces/tabs.";
        assert_eq!(found(include_str!("../../test/broken/bad.pug")), [at(6, 0, 5, code::INCONSISTENT_INDENT, message)]);
    }
}
