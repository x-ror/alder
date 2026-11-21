//! Pug (pug-rails' `.jst.pug`, plain `.pug`): ports of `pug-lexer` and `pug-parser` that stop at the first error
//! Pug raises, with its code, message, line and column. The JavaScript in templates isn't parsed.

pub mod lexer;
pub mod parser;
pub mod refs;

use crate::util::{coded, error, leading_ws, LineIndex, BOM};
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

/// Pug's first error, with Pug's message, filed under the code of the construct it is about; without one, for a file on
/// disk, the problems with what it includes and extends.
pub fn diagnostics(text: &str, path: Option<&Path>) -> Vec<Diagnostic> {
    match parser::parse(text) {
        Ok(tree) => path.map(|path| refs::diagnostics(text, &tree, path)).unwrap_or_default(),
        Err(found) => {
            let code = code_of(found.code);
            vec![coded(error(range(text, &found, code == code::INCONSISTENT_INDENT), found.message), code)]
        }
    }
}

/// The template's tree, or the tree of the lines before its first error.
pub fn parsed(text: &str) -> Option<parser::Block> {
    let error = match parser::parse(text) {
        Ok(tree) => return Some(tree),
        Err(error) => error,
    };
    let before: usize = text.split_inclusive('\n').take(error.line.saturating_sub(1)).map(str::len).sum();
    parser::parse(&text[..before]).ok()
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
        | "PUG:MALFORMED_EACH" => code::MALFORMED_KEYWORD,
        "PUG:BLOCK_IN_BUFFERED_CODE" | "PUG:RAW_INCLUDE_BLOCK" => code::ILLEGAL_NESTING,
        _ => code::SYNTAX_ERROR,
    }
}

/// From Pug's position to the end of its line, or with `indentation`, to the end of the line's indentation.
fn range(text: &str, found: &Error, indentation: bool) -> Range {
    let line = found.line.saturating_sub(1);
    let raw = LineIndex::new(text).line(line);
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
        diagnostics(text, None)
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
            ("each x\n  p", at(0, 4, 6, MALFORMED_KEYWORD, "malformed each")),
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
    fn broken_fixture() {
        let message = "Inconsistent indentation. Expecting either 4 or 6 spaces/tabs.";
        assert_eq!(found(include_str!("../../test/broken/bad.pug")), [at(6, 0, 5, code::INCONSISTENT_INDENT, message)]);
    }
}
