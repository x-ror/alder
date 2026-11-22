//! Port of `pug-lexer` 5.0.1 (Pug 3.0.4) with the parts of `character-parser` 2.2.0 it uses to find where a
//! bracketed expression, attribute value or interpolation ends. Lines and columns are Pug's: 1-based, columns in
//! UTF-16 code units. `is-expression` isn't ported: the JavaScript isn't validated (`PUG:SYNTAX_ERROR`), and where
//! Pug asks it whether an attribute value has ended, `Completeness` answers.

use super::{too_deep, Error, ErrorCode, MAX_DEPTH};
use crate::util::{closing_bracket, LineIndex, BOM};
use regex::Regex;
use std::borrow::Cow;
use std::ops::Range;
use std::sync::LazyLock;

const INTERPOLATION_END: &str = "End of line was reached with no closing bracket for interpolation.";

macro_rules! regex {
    ($name:ident, $pattern:literal) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pattern).unwrap());
    };
}

// pug-lexer's regular expressions, with JavaScript's ASCII `\w` and `\b` spelled out, its `\s` as `is_js_whitespace`
// has it, and its `.` as `[^\n\r\u{2028}\u{2029}]`: Rust's `.` stops only at `\n`.
regex!(COMMENT, r"^//(-)?([^\n]*)");
regex!(TAG, r"^([A-Za-z0-9_](?:[-:A-Za-z0-9_]*[A-Za-z0-9_])?)");
regex!(FILTER, r"^:([A-Za-z0-9_\-]+)");
regex!(DOCTYPE, r"^doctype *([^\n]*)");
regex!(ID, r"^#([A-Za-z0-9_-]+)");
regex!(CLASS, r"^\.([_a-zA-Z0-9\-]*[_a-zA-Z][_a-zA-Z0-9\-]*)");
regex!(CLASS_WITHOUT_LETTER, r"^\.[_a-zA-Z0-9\-]+");
// The name quoted in an invalid id or class message.
regex!(INVALID_NAME, r"[^\n\r\u{2028}\u{2029}][^ \t(#.:]*");
regex!(TEXT_SPACE, r"^( )");
regex!(TEXT_PIPE, r"^\|( ?)");
regex!(DOT, r"^\.");
regex!(EXTENDS, r"^extends?");
regex!(EXTENDS_WORD, r"^extends?(?-u:\b)");
regex!(PREPEND, r"^(?:block +)?prepend +([^\n]+)");
regex!(APPEND, r"^(?:block +)?append +([^\n]+)");
regex!(BLOCK, r"^block +([^\n]+)");
regex!(MIXIN_BLOCK, r"^block");
regex!(YIELD, r"^yield");
regex!(INCLUDE, r"^include");
regex!(INCLUDE_WORD, r"^include(?-u:\b)");
regex!(PATH, r"^ ([^\n]+)");
regex!(CASE, r"^case +([^\n]+)");
regex!(CASE_WORD, r"^case(?-u:\b)");
regex!(WHEN, r"^when +([^:\n]+)");
regex!(WHEN_MORE, r":([^:\n]+)");
regex!(WHEN_WORD, r"^when(?-u:\b)");
regex!(DEFAULT, r"^default");
regex!(DEFAULT_WORD, r"^default(?-u:\b)");
regex!(CALL, r"^\+([\s\x{FEFF}--\x{85}]*)(([-A-Za-z0-9_]+)|(#\{))");
regex!(CALL_ARGUMENTS, r"^ *\(");
regex!(CALL_ATTRIBUTES, r"^[\s\x{FEFF}--\x{85}]*[-A-Za-z0-9_]+ *=");
regex!(MIXIN, r"^mixin +([-A-Za-z0-9_]+)(?: *\(([^\n\r\u{2028}\u{2029}]*)\))? *");
regex!(CONDITIONAL, r"^(if|unless|else if|else)(?-u:\b)([^\n]*)");
regex!(WHILE, r"^while +([^\n]+)");
regex!(WHILE_WORD, r"^while(?-u:\b)");
regex!(EACH, r"^(?:each|for) +([a-zA-Z_$][A-Za-z0-9_$]*)(?: *, *([a-zA-Z_$][A-Za-z0-9_$]*))? * in *([^\n]+)");
regex!(EACH_WORD, r"^(?:each|for)(?-u:\b)");
regex!(EACH_DASHED, r"^- *(?:each|for) +([a-zA-Z_$][A-Za-z0-9_$]*)(?: *, *([a-zA-Z_$][A-Za-z0-9_$]*))? +in +([^\n]+)");
regex!(EACH_OF, r"^(?:each|for) ([^\n\r\u{2028}\u{2029}]*?) of *([^\n]+)");
regex!(EACH_OF_VALUE, r"^(?:[a-zA-Z_$][A-Za-z0-9_$]*|\[ *[a-zA-Z_$][A-Za-z0-9_$]* *, *[a-zA-Z_$][A-Za-z0-9_$]* *\])$");
regex!(EACH_OF_DASHED, r"^- *(?:each|for) +([a-zA-Z_$][A-Za-z0-9_$]*)(?: *, *([a-zA-Z_$][A-Za-z0-9_$]*))? +of +([^\n]+)");
regex!(CODE, r"^(!?=|-)[ \t]*([^\n]+)");
regex!(BLOCK_CODE, r"^-");
regex!(ATTRIBUTES_BLOCK, r"^&attributes(?-u:\b)");
regex!(SLASH, r"^/");
regex!(COLON, r"^: +");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pos {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockMode {
    Replace,
    Append,
    Prepend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Tag(String),
    Id(String),
    Class(String),
    Text(String),
    TextHtml(String),
    Comment {
        val: String,
        buffer: bool,
    },
    /// `#{}` (escaped) or `!{}` in text.
    InterpolatedCode {
        val: String,
        must_escape: bool,
    },
    Code {
        val: String,
        must_escape: bool,
        buffer: bool,
    },
    /// A `-` line whose code is the indented text below it.
    BlockCode,
    /// `#{expr}` as a tag name.
    Interpolation(String),
    Filter(String),
    Doctype(String),
    Dot,
    Extends,
    Path(String),
    Block {
        name: String,
        mode: BlockMode,
    },
    MixinBlock,
    Yield,
    Include,
    Mixin {
        name: String,
        args: Option<String>,
    },
    Call {
        name: String,
        args: Option<String>,
    },
    /// `if`, and `unless` as `!(test)`.
    If(String),
    ElseIf(String),
    Else,
    Each {
        val: String,
        key: Option<String>,
        code: String,
    },
    /// `each val of code`: `val` is an identifier or `[key, value]`, as written.
    EachOf {
        val: String,
        code: String,
    },
    While(String),
    Case(String),
    When(String),
    Default,
    StartAttributes,
    /// `val` is `None` for a boolean attribute.
    Attribute {
        name: String,
        val: Option<String>,
        must_escape: bool,
    },
    EndAttributes,
    AndAttributes(String),
    Indent(usize),
    Outdent,
    Newline,
    Eos,
    StartPipelessText,
    EndPipelessText,
    StartPugInterpolation,
    EndPugInterpolation,
    Slash,
    Colon,
}

impl Kind {
    /// The token's `type` in pug-lexer, which Pug's messages name.
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Tag(_) => "tag",
            Kind::Id(_) => "id",
            Kind::Class(_) => "class",
            Kind::Text(_) => "text",
            Kind::TextHtml(_) => "text-html",
            Kind::Comment { .. } => "comment",
            Kind::InterpolatedCode { .. } => "interpolated-code",
            Kind::Code { .. } => "code",
            Kind::BlockCode => "blockcode",
            Kind::Interpolation(_) => "interpolation",
            Kind::Filter(_) => "filter",
            Kind::Doctype(_) => "doctype",
            Kind::Dot => "dot",
            Kind::Extends => "extends",
            Kind::Path(_) => "path",
            Kind::Block { .. } => "block",
            Kind::MixinBlock => "mixin-block",
            Kind::Yield => "yield",
            Kind::Include => "include",
            Kind::Mixin { .. } => "mixin",
            Kind::Call { .. } => "call",
            Kind::If(_) => "if",
            Kind::ElseIf(_) => "else-if",
            Kind::Else => "else",
            Kind::Each { .. } => "each",
            Kind::EachOf { .. } => "eachOf",
            Kind::While(_) => "while",
            Kind::Case(_) => "case",
            Kind::When(_) => "when",
            Kind::Default => "default",
            Kind::StartAttributes => "start-attributes",
            Kind::Attribute { .. } => "attribute",
            Kind::EndAttributes => "end-attributes",
            Kind::AndAttributes(_) => "&attributes",
            Kind::Indent(_) => "indent",
            Kind::Outdent => "outdent",
            Kind::Newline => "newline",
            Kind::Eos => "eos",
            Kind::StartPipelessText => "start-pipeless-text",
            Kind::EndPipelessText => "end-pipeless-text",
            Kind::StartPugInterpolation => "start-pug-interpolation",
            Kind::EndPugInterpolation => "end-pug-interpolation",
            Kind::Slash => "slash",
            Kind::Colon => ":",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    pub start: Pos,
    pub end: Pos,
    /// The bytes from `start` to `end` in the text given to `lex`; for a path, the path without the whitespace Pug trims.
    pub span: Range<usize>,
}

/// Pug's tokens for `text`, or its first error.
pub fn lex(text: &str) -> Result<Vec<Token>, Error> {
    let body = text.strip_prefix(BOM).unwrap_or(text);
    let input = if body.contains('\r') { Cow::Owned(body.replace("\r\n", "\n").replace('\r', "\n")) } else { Cow::Borrowed(body) };
    let mut lexer = Lexer::new(input, Pos { line: 1, column: 1 });
    lexer.run()?;
    let mut offsets = Offsets::new(text);
    let mut tokens = lexer.tokens;
    for token in &mut tokens {
        let start = offsets.of(token.start);
        token.span = match &token.kind {
            // Pug's column is past the spaces after the keyword, not past the rest of the whitespace it trims.
            Kind::Path(path) => {
                let start = start + text[start..].len() - text[start..].trim_start_matches(is_js_whitespace).len();
                start..start + path.len()
            }
            _ => start..offsets.of(token.end).max(start),
        };
    }
    Ok(tokens)
}

/// The byte offsets of Pug's positions in the text it lexed, found from the last one asked when that is earlier on the
/// same line: tokens ask in about document order, and a line can hold thousands.
struct Offsets<'a> {
    index: LineIndex<'a>,
    /// The 0-based line, UTF-16 column and offset of the last position asked.
    last: (usize, usize, usize),
}

impl<'a> Offsets<'a> {
    fn new(text: &'a str) -> Self {
        Offsets { index: LineIndex::with_lone_cr(text), last: (0, 1, if text.starts_with(BOM) { BOM.len_utf8() } else { 0 }) }
    }

    /// The offset of Pug's 1-based line and UTF-16 column, which don't count the byte order mark.
    fn of(&mut self, at: Pos) -> usize {
        let line = at.line.saturating_sub(1).min(self.index.line_of(usize::MAX));
        let line_start = self.index.line_start(line);
        let (column, from) = match self.last {
            (last_line, column, from) if last_line == line && column <= at.column => (column, from),
            _ => {
                let bom = if line == 0 && self.index.line(0).starts_with(BOM) { BOM.len_utf8() } else { 0 };
                (1, line_start + bom)
            }
        };
        let rest = &self.index.line(line)[from - line_start..];
        let offset = from + unit_offset(rest, at.column.saturating_sub(column));
        self.last = (line, at.column, offset);
        offset
    }
}

fn units(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The bytes and UTF-16 units of a regular expression's whole match.
fn extent(caps: &regex::Captures) -> (usize, usize) {
    let whole = caps.get(0).map_or("", |m| m.as_str());
    (whole.len(), units(whole))
}

/// Byte length of the first `n` UTF-16 code units of `s`.
pub(crate) fn unit_offset(s: &str, n: usize) -> usize {
    // ASCII, a unit a byte, is checked many bytes at a time: nodes far along a long line ask often.
    let head = &s.as_bytes()[..n.min(s.len())];
    if head.is_ascii() {
        return head.len();
    }
    let mut count = 0;
    for (at, c) in s.char_indices() {
        if count >= n {
            return at;
        }
        count += c.len_utf16();
    }
    s.len()
}

/// pug-lexer's `whitespaceRe`.
fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\n' | '\t')
}

/// JavaScript's whitespace, what its `\s` and `trim` take: Unicode's, with U+FEFF and without U+0085.
pub(crate) fn is_js_whitespace(c: char) -> bool {
    c == BOM || (c.is_whitespace() && c != '\u{85}')
}

pub(crate) fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

fn is_js_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn mismatched_bracket(close: char) -> String {
    format!("Mismatched Bracket: {close}")
}

#[derive(Clone, Copy)]
enum IndentChar {
    Tab,
    Space,
}

struct Scanned {
    start: Pos,
    val: Option<String>,
}

struct AttributeValue {
    val: String,
    must_escape: bool,
}

struct Lexer<'a> {
    input: Cow<'a, str>,
    /// Where the unconsumed input starts in `input`.
    at: usize,
    /// How many `#[...]` this lexer is inside; in one, the input ends at the end of its line and the lexer at its `]`.
    depth: usize,
    /// In `#[...]`, where its line goes on without a line terminator but `\n`, in the input.
    terminated: usize,
    line: usize,
    column: usize,
    /// Open indentation widths, innermost last.
    indent_stack: Vec<usize>,
    /// Tabs or spaces, from the first indented line.
    indent_char: Option<IndentChar>,
    interpolation_allowed: bool,
    tokens: Vec<Token>,
    ended: bool,
}

impl<'a> Lexer<'a> {
    fn new(input: Cow<'a, str>, start: Pos) -> Self {
        Lexer {
            input,
            at: 0,
            depth: 0,
            terminated: 0,
            line: start.line,
            column: start.column,
            indent_stack: vec![0],
            indent_char: None,
            interpolation_allowed: true,
            tokens: Vec::new(),
            ended: false,
        }
    }

    fn run(&mut self) -> Result<(), Error> {
        while !self.ended {
            self.advance()?;
        }
        Ok(())
    }

    fn advance(&mut self) -> Result<(), Error> {
        type Step = fn(&mut Lexer) -> Result<bool, Error>;
        const STEPS: &[Step] = &[
            |l| Ok(l.blank()),
            |l| l.eos(),
            |l| Ok(l.end_interpolation()),
            |l| Ok(l.simple_end_of_line(&YIELD, Kind::Yield)),
            |l| l.doctype(),
            |l| l.interpolation(),
            |l| l.case(),
            |l| l.when(),
            |l| l.default_case(),
            |l| l.extends(),
            |l| Ok(l.named_block(&APPEND, BlockMode::Append)),
            |l| Ok(l.named_block(&PREPEND, BlockMode::Prepend)),
            |l| Ok(l.named_block(&BLOCK, BlockMode::Replace)),
            |l| Ok(l.simple_end_of_line(&MIXIN_BLOCK, Kind::MixinBlock)),
            |l| l.include(),
            |l| Ok(l.mixin()),
            |l| l.call(),
            |l| l.conditional(),
            |l| l.each_of(),
            |l| l.each(),
            |l| l.while_(),
            |l| Ok(l.tag()),
            |l| l.filter(false),
            |l| l.block_code(),
            |l| l.code(),
            |l| l.id(),
            |l| l.dot(),
            |l| l.class_name(),
            |l| l.attrs(),
            |l| l.attributes_block(),
            |l| l.indent(),
            |l| l.text(),
            |l| l.text_html(),
            |l| l.comment(),
            |l| Ok(l.punctuation(&SLASH, Kind::Slash)),
            |l| Ok(l.punctuation(&COLON, Kind::Colon)),
        ];
        for step in STEPS {
            if step(self)? {
                return Ok(());
            }
        }
        Err(self.unexpected_text())
    }

    /// Pug's error for code in `#{...}`, or in `#[...]` after `=`, whose end it can't find.
    fn interpolation_error(&self, failure: Failure) -> Error {
        match failure {
            Failure::EndOfString => self.error(ErrorCode::NoEndBracket, INTERPOLATION_END),
            Failure::Mismatch(c) => self.error(ErrorCode::BracketMismatch, mismatched_bracket(c)),
        }
    }

    fn unexpected_text(&self) -> Error {
        let rest = self.rest();
        self.error(ErrorCode::UnexpectedText, format!("unexpected text \"{}\"", &rest[..unit_offset(rest, 5)]))
    }

    fn rest(&self) -> &str {
        &self.input[self.at..]
    }

    fn consume(&mut self, bytes: usize) {
        self.at += bytes;
    }

    fn pos(&self) -> Pos {
        Pos { line: self.line, column: self.column }
    }

    fn error(&self, code: ErrorCode, message: impl Into<String>) -> Error {
        Error { code, message: message.into(), line: self.line, column: self.column }
    }

    fn push(&mut self, kind: Kind, start: Pos) {
        self.tokens.push(Token { kind, start, end: self.pos(), span: 0..0 });
    }

    fn increment_line(&mut self, lines: usize) {
        self.line += lines;
        if lines > 0 {
            self.column = 1;
        }
    }

    /// Moves the position over `c`, which Pug counts as one column per UTF-16 unit.
    fn step(&mut self, c: char) {
        if c == '\n' {
            self.increment_line(1);
        } else {
            self.column += c.len_utf16();
        }
    }

    fn top(&self) -> usize {
        self.indent_stack.last().copied().unwrap_or(0)
    }

    fn scan(&mut self, re: &Regex) -> Option<Scanned> {
        let (len, whole_units, val) = {
            let caps = re.captures(self.rest())?;
            let whole = caps.get(0)?.as_str();
            (whole.len(), units(whole), caps.get(1).map(|m| m.as_str().to_string()))
        };
        let start = self.pos();
        self.consume(len);
        self.column += whole_units - val.as_deref().map_or(0, units);
        Some(Scanned { start, val })
    }

    /// `scan` for a keyword that must end its line or be followed by `:` (block expansion).
    fn scan_end_of_line(&mut self, re: &Regex) -> Option<Scanned> {
        let (whole, val) = {
            let caps = re.captures(self.rest())?;
            (caps.get(0)?.as_str().to_string(), caps.get(1).map(|m| m.as_str().to_string()))
        };
        let spaces = whole.len() - whole.trim_start_matches(' ').len();
        self.column += spaces;
        let after = &self.rest()[whole.len()..];
        let blanks = after.len() - after.trim_start_matches([' ', '\t']).len();
        if after.starts_with(':') {
            self.consume(whole.len());
        } else if after[blanks..].is_empty() || after[blanks..].starts_with('\n') {
            self.consume(whole.len() + blanks);
        } else {
            return None;
        }
        let start = self.pos();
        self.column += units(&whole) - spaces;
        Some(Scanned { start, val })
    }

    /// The byte range, in the unconsumed input, inside the bracket at byte `skip`.
    fn bracket_expression(&mut self, skip: usize) -> Result<Range<usize>, Error> {
        let Some(close) = self.rest()[skip..].chars().next().and_then(closing_bracket) else {
            return Err(self.error(ErrorCode::Assertion, "The start character should be \"(\", \"{\" or \"[\""));
        };
        match parse_until(self.rest(), close, skip + 1) {
            Ok(range) => Ok(range),
            Err((failure, at)) => {
                // Pug looks for line breaks from the bracket on, so one before it counts as a column.
                let after_bracket = &self.rest()[skip..at];
                let lines = after_bracket.matches('\n').count();
                let line_start = after_bracket.rfind('\n').map_or(0, |n| skip + n + 1);
                let columns = units(&self.rest()[line_start..at]);
                self.increment_line(lines);
                self.column += columns;
                Err(match failure {
                    Failure::EndOfString => {
                        self.error(ErrorCode::NoEndBracket, format!("The end of the string reached with no closing bracket {close} found."))
                    }
                    Failure::Mismatch(c) => self.error(ErrorCode::BracketMismatch, mismatched_bracket(c)),
                })
            }
        }
    }

    /// The width of the indentation on the line after the newline that starts the input; the first indented line
    /// decides whether tabs or spaces count.
    fn scan_indentation(&mut self) -> Option<usize> {
        let line = self.rest().strip_prefix('\n')?;
        let count = |c: char| line.len() - line.trim_start_matches(c).len();
        let (tabs, spaces) = (count('\t'), count(' '));
        Some(match self.indent_char {
            Some(IndentChar::Tab) => tabs,
            Some(IndentChar::Space) => spaces,
            None if tabs > 0 => {
                self.indent_char = Some(IndentChar::Tab);
                tabs
            }
            None => {
                if spaces > 0 {
                    self.indent_char = Some(IndentChar::Space);
                }
                spaces
            }
        })
    }

    fn line_indentation(&self, line: &str) -> usize {
        let c = match self.indent_char {
            Some(IndentChar::Tab) => '\t',
            _ => ' ',
        };
        line.len() - line.trim_start_matches(c).len()
    }

    fn eos(&mut self) -> Result<bool, Error> {
        if !self.rest().is_empty() {
            return Ok(false);
        }
        if self.depth > 0 {
            return Err(self.error(ErrorCode::NoEndBracket, INTERPOLATION_END));
        }
        let open = self.indent_stack.iter().rev().take_while(|&&width| width != 0).count();
        for _ in 0..open {
            self.push(Kind::Outdent, self.pos());
        }
        self.push(Kind::Eos, self.pos());
        self.ended = true;
        Ok(true)
    }

    fn blank(&mut self) -> bool {
        let Some(line) = self.rest().strip_prefix('\n') else {
            return false;
        };
        if !line.trim_start_matches([' ', '\t']).starts_with('\n') {
            return false;
        }
        let len = 1 + line.len() - line.trim_start_matches([' ', '\t']).len();
        self.consume(len);
        self.increment_line(1);
        true
    }

    fn comment(&mut self) -> Result<bool, Error> {
        let Some(((len, width), buffer, val)) =
            COMMENT.captures(self.rest()).map(|caps| (extent(&caps), caps.get(1).is_none(), caps[2].to_string()))
        else {
            return Ok(false);
        };
        self.consume(len);
        let start = self.pos();
        self.interpolation_allowed = buffer;
        self.column += width;
        self.push(Kind::Comment { val, buffer }, start);
        self.pipeless_text()?;
        Ok(true)
    }

    fn interpolation(&mut self) -> Result<bool, Error> {
        if !self.rest().starts_with("#{") {
            return Ok(false);
        }
        let range = self.bracket_expression(1)?;
        let src = self.rest()[range.clone()].to_string();
        self.consume(range.end + 1);
        let start = self.pos();
        self.column += 2;
        let last = src.rsplit('\n').next().unwrap_or_default();
        self.increment_line(src.matches('\n').count());
        self.column += units(last) + 1;
        self.push(Kind::Interpolation(src), start);
        Ok(true)
    }

    fn tag(&mut self) -> bool {
        let Some(name) = TAG.captures(self.rest()).map(|caps| caps[1].to_string()) else {
            return false;
        };
        self.consume(name.len());
        let start = self.pos();
        self.column += units(&name);
        self.push(Kind::Tag(name), start);
        true
    }

    fn filter(&mut self, in_include: bool) -> Result<bool, Error> {
        let Some(Scanned { start, val }) = self.scan(&FILTER) else {
            return Ok(false);
        };
        let name = val.unwrap_or_default();
        self.column += units(&name);
        self.push(Kind::Filter(name), start);
        self.attrs()?;
        if !in_include {
            self.interpolation_allowed = false;
            self.pipeless_text()?;
        }
        Ok(true)
    }

    fn doctype(&mut self) -> Result<bool, Error> {
        let Some(Scanned { start, val }) = self.scan_end_of_line(&DOCTYPE) else {
            return Ok(false);
        };
        self.push(Kind::Doctype(val.unwrap_or_default()), start);
        Ok(true)
    }

    fn simple_end_of_line(&mut self, re: &Regex, kind: Kind) -> bool {
        let Some(Scanned { start, .. }) = self.scan_end_of_line(re) else {
            return false;
        };
        self.push(kind, start);
        true
    }

    fn punctuation(&mut self, re: &Regex, kind: Kind) -> bool {
        let Some(Scanned { start, .. }) = self.scan(re) else {
            return false;
        };
        self.push(kind, start);
        true
    }

    /// The text of an invalid id or class name error, after the `#` or `.`.
    fn invalid_name(&self, code: ErrorCode, message: impl Fn(&str) -> String) -> Error {
        match INVALID_NAME.find(&self.rest()[1..]) {
            Some(name) => self.error(code, message(name.as_str())),
            // Pug's own lexer throws on the regular expression's missing match here.
            None => self.error(ErrorCode::TypeError, "Cannot read properties of null (reading '0')"),
        }
    }

    fn id(&mut self) -> Result<bool, Error> {
        if let Some(Scanned { start, val }) = self.scan(&ID) {
            let name = val.unwrap_or_default();
            self.column += units(&name);
            self.push(Kind::Id(name), start);
            return Ok(true);
        }
        if !self.rest().starts_with('#') {
            return Ok(false);
        }
        Err(self.invalid_name(ErrorCode::InvalidId, |name| format!("\"{name}\" is not a valid ID.")))
    }

    fn class_name(&mut self) -> Result<bool, Error> {
        if let Some(Scanned { start, val }) = self.scan(&CLASS) {
            let name = val.unwrap_or_default();
            self.column += units(&name);
            self.push(Kind::Class(name), start);
            return Ok(true);
        }
        if CLASS_WITHOUT_LETTER.is_match(self.rest()) {
            return Err(self.error(ErrorCode::InvalidClassName, "Class names must contain at least one letter or underscore."));
        }
        if !self.rest().starts_with('.') {
            return Ok(false);
        }
        Err(self.invalid_name(ErrorCode::InvalidClassName, |name| {
            format!(
                "\"{name}\" is not a valid class name.  Class names can only contain \"_\", \"-\", a-z and 0-9, and must contain at least one of \"_\", or a-z"
            )
        }))
    }

    fn end_interpolation(&mut self) -> bool {
        if self.depth == 0 || !self.rest().starts_with(']') {
            return false;
        }
        self.consume(1);
        self.ended = true;
        true
    }

    /// The text in the `value` bytes of the input, with its `#{}`, `!{}` and `#[]` interpolations, and in `#[...]` the `]`
    /// that ends it. `prefix` is text that goes before it, and `escaped` the backslashes dropped from it, which take columns.
    fn add_text(&mut self, make: fn(String) -> Kind, mut value: Range<usize>, mut prefix: String, mut escaped: usize) -> Result<(), Error> {
        #[derive(Clone, Copy)]
        enum Next {
            EscapedTag,
            Tag,
            Interpolation { escaped: bool },
        }
        let (allowed, text_end) = (self.interpolation_allowed, value.end);
        // A `#{` or `!{` interpolates only when no line terminator but `\n` follows it; `#[...]` has its line's.
        let terminated = if self.depth > 0 && text_end == self.input.len() {
            self.terminated.max(value.start)
        } else {
            let terminator = self.input[value.clone()].char_indices().rev().find(|&(_, c)| matches!(c, '\r' | '\u{2028}' | '\u{2029}'));
            terminator.map_or(value.start, |(at, c)| value.start + at + c.len_utf8())
        };
        let (mut escaped_tags, mut tags, mut interpolations, mut ends) =
            (Ahead::default(), Ahead::default(), Ahead::default(), Ahead::default());
        loop {
            let input: &str = &self.input;
            let from = value.start;
            let search = |pattern: &'static str| move |from: usize, end: usize| input[from..end].find(pattern).map(|at| from + at);
            let end = (self.depth > 0).then(|| ends.find(from, text_end, search("]"))).flatten();
            // Each is looked for where it can be, before the `]` (none of them can hold one); the first one found wins.
            let before = end.unwrap_or(text_end);
            let candidates = [
                allowed.then(|| escaped_tags.find(from, before, search("\\#["))).flatten().map(|at| (at, Next::EscapedTag)),
                allowed.then(|| tags.find(from, before, search("#["))).flatten().map(|at| (at, Next::Tag)),
                allowed
                    .then(|| interpolations.find(from, before, |from, end| interpolation_at(input, from.max(terminated), end)))
                    .flatten()
                    .map(|at| {
                        if at > from && input.as_bytes()[at - 1] == b'\\' {
                            (at - 1, Next::Interpolation { escaped: true })
                        } else {
                            (at, Next::Interpolation { escaped: false })
                        }
                    }),
            ];
            let first = candidates.into_iter().flatten().min_by_key(|&(at, _)| at);
            if let Some(end) = end.filter(|_| first.is_none()) {
                if !prefix.is_empty() || end > from {
                    self.add_text(make, from..end, prefix, 0)?;
                }
                self.ended = true;
                // In `#[...]` the text runs to the end of the input, which goes on past the `]`.
                self.at = end + 1;
                return Ok(());
            }
            let Some((at, next)) = first else {
                // pug-lexer returns early on an empty text only with a prefix given, which no call does, so "" is a token.
                prefix.push_str(&input[from..text_end]);
                self.push_text(make, prefix, escaped);
                return Ok(());
            };
            prefix.push_str(&input[from..at]);
            match next {
                Next::EscapedTag => {
                    prefix.push_str("#[");
                    escaped += 1;
                    value.start = at + "\\#[".len();
                }
                Next::Interpolation { escaped: true } => {
                    prefix.push_str(&input[at + 1..at + "\\#{".len()]);
                    escaped += 1;
                    value.start = at + "\\#{".len();
                }
                Next::Tag => {
                    self.push_text(make, std::mem::take(&mut prefix), escaped);
                    escaped = 0;
                    value.start = self.tag_interpolation(at + "#[".len()..text_end, terminated)?;
                }
                Next::Interpolation { escaped: false } => {
                    let must_escape = input[at..].starts_with('#');
                    if !prefix.is_empty() {
                        self.push_text(make, std::mem::take(&mut prefix), escaped);
                    }
                    escaped = 0;
                    match self.string_interpolation(at + "#{".len()..text_end, must_escape)? {
                        Some(after) => value.start = after,
                        None => return Ok(()),
                    }
                }
            }
        }
    }

    /// Text that took `escaped` more columns than it has, for the backslashes dropped from it.
    fn push_text(&mut self, make: fn(String) -> Kind, text: String, escaped: usize) {
        let at = self.pos();
        self.column += units(&text) + escaped;
        self.push(make(text), at);
    }

    /// The `#[...]` whose inside starts the `inside` bytes of the input, the rest of a line that goes on without a line
    /// terminator but `\n` from `terminated`: lexed by a lexer of its own that stops at the `]`. Where the input goes on after it.
    fn tag_interpolation(&mut self, inside: Range<usize>, terminated: usize) -> Result<usize, Error> {
        let at = self.pos();
        if self.depth == MAX_DEPTH {
            return Err(too_deep(at.line, at.column));
        }
        self.column += 2;
        self.push(Kind::StartPugInterpolation, at);
        let mut child = Lexer::new(Cow::Borrowed(&self.input[inside.clone()]), self.pos());
        child.depth = self.depth + 1;
        child.terminated = terminated.saturating_sub(inside.start);
        child.tokens = std::mem::take(&mut self.tokens);
        let lexed = child.run();
        let Lexer { tokens, column, at: after, .. } = child;
        self.tokens = tokens;
        if let Err(error) = lexed {
            if !error.code.is_pugs() {
                return Err(error);
            }
            self.column = error.column;
            return Err(self.error(error.code, error.message));
        }
        self.column = column;
        let at = self.pos();
        self.column += 1;
        self.push(Kind::EndPugInterpolation, at);
        Ok(inside.start + after)
    }

    /// The code of a `#{` or `!{` whose brace ended before the `rest` bytes of the input; where the text after its `}` starts,
    /// unless the line ends there.
    fn string_interpolation(&mut self, rest: Range<usize>, must_escape: bool) -> Result<Option<usize>, Error> {
        let at = self.pos();
        self.column += 2;
        let code = &self.input[rest.clone()];
        let range = match parse_until(code, '}', 0) {
            Ok(range) => range,
            Err((failure, index)) => {
                self.column += units(&code[..index]);
                return Err(self.interpolation_error(failure));
            }
        };
        let val = code[range.clone()].to_string();
        let after = range.end + 1;
        self.column += units(&code[..after]);
        self.push(Kind::InterpolatedCode { val, must_escape }, at);
        Ok((after < rest.len()).then_some(rest.start + after))
    }

    /// pug-lexer's `text`: `| text`, `|text` or ` text` (`TEXT`), or a lone `|`, `| ` or space, which is empty text.
    fn text(&mut self) -> Result<bool, Error> {
        let rest = self.rest();
        let goes_on = |at: usize| rest[at..].starts_with(|c| c != '\n');
        // As `TEXT` matches it: past `| ` unless the line ends there, then past `|`, or past a space.
        let mark = match rest.strip_prefix('|') {
            Some(after) if after.starts_with(' ') && goes_on(2) => Some(2),
            Some(_) => goes_on(1).then_some(1),
            None => (rest.starts_with(' ') && goes_on(1)).then_some(1),
        };
        if let Some(mark) = mark {
            let (start, line) = (self.at + mark, self.line_end());
            self.column += mark;
            self.consume(line - self.at);
            self.add_text(Kind::Text, start..line, String::new(), 0)?;
            return Ok(true);
        }
        let Some(Scanned { val, .. }) = self.scan(&TEXT_SPACE).or_else(|| self.scan(&TEXT_PIPE)) else {
            return Ok(false);
        };
        let len = val.map_or(0, |val| val.len());
        self.add_text(Kind::Text, self.at - len..self.at, String::new(), 0)?;
        Ok(true)
    }

    fn text_html(&mut self) -> Result<bool, Error> {
        if !self.rest().starts_with('<') {
            return Ok(false);
        }
        let (start, line) = (self.at, self.line_end());
        self.consume(line - start);
        self.add_text(Kind::TextHtml, start..line, String::new(), 0)?;
        Ok(true)
    }

    /// Where the line the unconsumed input starts on ends; the input of `#[...]` is the rest of one line.
    fn line_end(&self) -> usize {
        let rest = self.rest();
        self.at + if self.depth > 0 { rest.len() } else { rest.find('\n').unwrap_or(rest.len()) }
    }

    fn dot(&mut self) -> Result<bool, Error> {
        if !self.simple_end_of_line(&DOT, Kind::Dot) {
            return Ok(false);
        }
        self.pipeless_text()?;
        Ok(true)
    }

    fn extends(&mut self) -> Result<bool, Error> {
        let keyword = EXTENDS.find(self.rest()).map(|m| m.len());
        let followed = keyword.is_some_and(|len| matches!(self.rest()[len..].chars().next(), None | Some(' ' | '\n')));
        if followed {
            let Some(Scanned { start, .. }) = self.scan(&EXTENDS) else {
                return Ok(false);
            };
            self.push(Kind::Extends, start);
            if !self.path() {
                return Err(self.error(ErrorCode::NoExtendsPath, "missing path for extends"));
            }
            return Ok(true);
        }
        if self.scan(&EXTENDS_WORD).is_some() {
            return Err(self.error(ErrorCode::MalformedExtends, "malformed extends"));
        }
        Ok(false)
    }

    /// `block name`, `block append name`/`append name`, `block prepend name`/`prepend name`. A `//` comment after
    /// the name is left in the input, cut where Pug cuts it.
    fn named_block(&mut self, re: &Regex, mode: BlockMode) -> bool {
        let Some((whole, captured)) = re.captures(self.rest()).map(|caps| (caps[0].to_string(), caps[1].to_string())) else {
            return false;
        };
        let mut name = js_trim(&captured);
        let mut comment = "";
        if let Some(at) = name.find("//") {
            comment = &name[at..];
            name = js_trim(&name[..at]);
        }
        if name.is_empty() {
            return false;
        }
        let kept_units = units(&whole) - units(comment);
        let kept = unit_offset(&whole, kept_units);
        let len = whole[..kept].trim_end_matches(is_space).len();
        let start = self.pos();
        self.column += units(&whole[..len]);
        self.push(Kind::Block { name: name.to_string(), mode }, start);
        self.consume(kept);
        self.column += kept_units - units(&whole[..len]);
        true
    }

    fn include(&mut self) -> Result<bool, Error> {
        let keyword = INCLUDE.find(self.rest()).map(|m| m.len());
        let followed = keyword.is_some_and(|len| matches!(self.rest()[len..].chars().next(), None | Some(':' | ' ' | '\n')));
        if followed {
            let Some(Scanned { start, .. }) = self.scan(&INCLUDE) else {
                return Ok(false);
            };
            self.push(Kind::Include, start);
            while self.filter(true)? {}
            if !self.path() {
                if self.rest().starts_with(|c| c != ' ' && c != '\n') {
                    return Err(self.unexpected_text());
                }
                return Err(self.error(ErrorCode::NoIncludePath, "missing path for include"));
            }
            return Ok(true);
        }
        if self.scan(&INCLUDE_WORD).is_some() {
            return Err(self.error(ErrorCode::MalformedInclude, "malformed include"));
        }
        Ok(false)
    }

    fn path(&mut self) -> bool {
        let Some(Scanned { start, val }) = self.scan_end_of_line(&PATH) else {
            return false;
        };
        let path = js_trim(&val.unwrap_or_default()).to_string();
        if path.is_empty() {
            return false;
        }
        self.push(Kind::Path(path), start);
        true
    }

    fn case(&mut self) -> Result<bool, Error> {
        if let Some(Scanned { start, val }) = self.scan_end_of_line(&CASE) {
            self.push(Kind::Case(val.unwrap_or_default()), start);
            return Ok(true);
        }
        if self.scan(&CASE_WORD).is_some() {
            return Err(self.error(ErrorCode::NoCaseExpression, "missing expression for case"));
        }
        Ok(false)
    }

    /// `when expr`; a `:` inside a string or brackets doesn't end the expression.
    fn when(&mut self) -> Result<bool, Error> {
        let Some(Scanned { start, val }) = self.scan_end_of_line(&WHEN) else {
            if self.scan(&WHEN_WORD).is_some() {
                return Err(self.error(ErrorCode::NoWhenExpression, "missing expression for when"));
            }
            return Ok(false);
        };
        let mut expr = val.unwrap_or_default();
        let mut state = CharState::default();
        let mut read = 0;
        loop {
            for c in expr[read..].chars() {
                state.parse_char(c).map_err(|close| self.crash_on_mismatch(close))?;
            }
            read = expr.len();
            if !state.is_nesting() {
                break;
            }
            // Pug finds the next `:` anywhere ahead but consumes from where it is.
            let Some(more) = WHEN_MORE.find(self.rest()).map(|m| m.as_str().to_string()) else {
                break;
            };
            expr.push_str(&more);
            let len = unit_offset(self.rest(), units(&more));
            self.consume(len);
            self.column += units(&more);
        }
        self.push(Kind::When(expr), start);
        Ok(true)
    }

    fn default_case(&mut self) -> Result<bool, Error> {
        if self.simple_end_of_line(&DEFAULT, Kind::Default) {
            return Ok(true);
        }
        if self.scan(&DEFAULT_WORD).is_some() {
            return Err(self.error(ErrorCode::DefaultWithExpression, "default should not have an expression"));
        }
        Ok(false)
    }

    /// `+name` or `+#{expr}`, then `(args)` unless the parentheses hold attributes.
    fn call(&mut self) -> Result<bool, Error> {
        let Some(((len, width), spaces, name)) =
            CALL.captures(self.rest()).map(|caps| (extent(&caps), caps[1].len(), caps.get(3).map(|m| m.as_str().to_string())))
        else {
            return Ok(false);
        };
        let (start, name) = match name {
            Some(name) => {
                self.consume(len);
                let start = self.pos();
                self.column += width;
                (start, name)
            }
            None => {
                let range = self.bracket_expression(1 + spaces + 1)?;
                let name = format!("#{{{}}}", &self.rest()[range.clone()]);
                let width = units(&self.rest()[..range.end + 1]);
                self.consume(range.end + 1);
                let start = self.pos();
                self.column += width;
                (start, name)
            }
        };
        let mut args = None;
        if let Some(open) = CALL_ARGUMENTS.find(self.rest()).map(|m| m.len() - 1) {
            let range = self.bracket_expression(open)?;
            let src = self.rest()[range.clone()].to_string();
            if !CALL_ATTRIBUTES.is_match(&src) {
                self.column += 1;
                self.consume(range.end + 1);
                // Pug counts the arguments and the `)`, not the spaces before the `(`.
                for c in src.chars() {
                    self.step(c);
                }
                self.column += 1;
                args = Some(src);
            }
        }
        self.push(Kind::Call { name, args }, start);
        Ok(true)
    }

    fn mixin(&mut self) -> bool {
        let Some(((len, width), name, args)) =
            MIXIN.captures(self.rest()).map(|caps| (extent(&caps), caps[1].to_string(), caps.get(2).map(|m| m.as_str().to_string())))
        else {
            return false;
        };
        self.consume(len);
        let start = self.pos();
        self.column += width;
        self.push(Kind::Mixin { name, args: args.filter(|a| !a.is_empty()) }, start);
        true
    }

    fn conditional(&mut self) -> Result<bool, Error> {
        let Some(((len, width), keyword, js)) =
            CONDITIONAL.captures(self.rest()).map(|caps| (extent(&caps), caps[1].to_string(), js_trim(&caps[2]).to_string()))
        else {
            return Ok(false);
        };
        self.consume(len);
        let start = self.pos();
        self.column += width - units(&js);
        let kind = match keyword.as_str() {
            "if" => Kind::If(js.clone()),
            "unless" => Kind::If(format!("!({js})")),
            "else if" => Kind::ElseIf(js.clone()),
            _ if !js.is_empty() => {
                return Err(self.error(ErrorCode::ElseCondition, "`else` cannot have a condition, perhaps you meant `else if`"));
            }
            _ => Kind::Else,
        };
        self.column += units(&js);
        self.push(kind, start);
        Ok(true)
    }

    fn while_(&mut self) -> Result<bool, Error> {
        if let Some(((len, width), test)) = WHILE.captures(self.rest()).map(|caps| (extent(&caps), caps[1].to_string())) {
            self.consume(len);
            let start = self.pos();
            self.column += width;
            self.push(Kind::While(test), start);
            return Ok(true);
        }
        if self.scan(&WHILE_WORD).is_some() {
            return Err(self.error(ErrorCode::NoWhileExpression, "missing expression for while"));
        }
        Ok(false)
    }

    fn each(&mut self) -> Result<bool, Error> {
        let captured = EACH
            .captures(self.rest())
            .map(|caps| (extent(&caps), caps[1].to_string(), caps.get(2).map(|m| m.as_str().to_string()), caps[3].to_string()));
        if let Some(((len, width), val, key, code)) = captured {
            self.consume(len);
            let start = self.pos();
            self.column += width;
            self.push(Kind::Each { val, key, code }, start);
            return Ok(true);
        }
        let keyword = if self.rest().starts_with("each") { "each" } else { "for" };
        if self.scan(&EACH_WORD).is_some() {
            let message = format!(
                "This `{keyword}` has a syntax error. `{keyword}` statements should be of the form: `{keyword} VARIABLE_NAME of JS_EXPRESSION`"
            );
            return Err(self.error(ErrorCode::MalformedEach, message));
        }
        if EACH_DASHED.is_match(self.rest()) {
            return Err(self.error(
                ErrorCode::MalformedEach,
                "Pug each and for should no longer be prefixed with a dash (\"-\"). They are pug keywords and not part of JavaScript.",
            ));
        }
        Ok(false)
    }

    /// `each item of list` and `for [key, value] of map`, tried before `each`: `each x in offers` is one too, with a value
    /// Pug refuses.
    fn each_of(&mut self) -> Result<bool, Error> {
        let captured = EACH_OF.captures(self.rest()).map(|caps| (extent(&caps), caps[1].to_string(), caps[2].to_string()));
        if let Some(((len, width), val, code)) = captured {
            self.consume(len);
            let start = self.pos();
            self.column += width;
            if !EACH_OF_VALUE.is_match(js_trim(&val)) {
                return Err(self.error(
                    ErrorCode::MalformedEachOfLval,
                    "The value variable for each must either be a valid identifier (e.g. `item`) or a pair of identifiers in square brackets (e.g. `[key, value]`).",
                ));
            }
            self.push(Kind::EachOf { val, code }, start);
            return Ok(true);
        }
        if EACH_OF_DASHED.is_match(self.rest()) {
            return Err(self.error(
                ErrorCode::MalformedEach,
                "Pug each and for should not be prefixed with a dash (\"-\"). They are pug keywords and not part of JavaScript.",
            ));
        }
        Ok(false)
    }

    /// `= code`, `!= code` and `- code`; inside `#[...]` the code ends at the `]`.
    fn code(&mut self) -> Result<bool, Error> {
        let Some(((len, width), flags, mut code)) =
            CODE.captures(self.rest()).map(|caps| (extent(&caps), caps[1].to_string(), caps[2].to_string()))
        else {
            return Ok(false);
        };
        let lead = width - units(&code);
        let mut shortened = 0;
        if self.depth > 0 {
            match parse_until(&code, ']', 0) {
                Ok(range) => {
                    shortened = code.len() - range.end;
                    code.truncate(range.end);
                }
                Err((failure, index)) => {
                    self.column += lead + units(&code[..index]);
                    return Err(self.interpolation_error(failure));
                }
            }
        }
        self.consume(len - shortened);
        let start = self.pos();
        let must_escape = flags.starts_with('=');
        let buffer = must_escape || flags.ends_with('=');
        self.column += lead + units(&code);
        self.push(Kind::Code { val: code, must_escape, buffer }, start);
        Ok(true)
    }

    fn block_code(&mut self) -> Result<bool, Error> {
        if !self.simple_end_of_line(&BLOCK_CODE, Kind::BlockCode) {
            return Ok(false);
        }
        self.interpolation_allowed = false;
        self.pipeless_text()?;
        Ok(true)
    }

    /// One attribute of `chars`, the inside of `(...)`, and the comma after it; returns the characters it used.
    fn attribute(&mut self, chars: &[char]) -> Result<usize, Error> {
        let mut i = chars.iter().take_while(|&&c| is_space(c)).count();
        for &c in &chars[..i] {
            self.step(c);
        }
        if i == chars.len() {
            return Ok(i);
        }
        let start = self.pos();
        let quote = matches!(chars[i], '\'' | '"').then_some(chars[i]);
        if quote.is_some() {
            self.column += 1;
            i += 1;
        }
        let mut name = String::new();
        while i < chars.len() {
            let c = chars[i];
            match quote {
                Some(q) if c == q => {
                    self.column += 1;
                    i += 1;
                    break;
                }
                None if is_space(c) || matches!(c, '!' | '=' | ',') => break,
                _ => {}
            }
            name.push(c);
            self.step(c);
            i += 1;
        }
        let (value, used) = self.attribute_value(&chars[i..])?;
        i += used;
        let (val, must_escape) = match value {
            Some(AttributeValue { val, must_escape }) if !val.is_empty() => (Some(val), must_escape),
            _ => (None, true),
        };
        self.push(Kind::Attribute { name, val, must_escape }, start);
        while i < chars.len() && is_space(chars[i]) {
            self.step(chars[i]);
            i += 1;
        }
        if chars.get(i) == Some(&',') {
            self.column += 1;
            i += 1;
        }
        Ok(i)
    }

    /// `=value` or `!=value` after an attribute name, and the characters it used.
    fn attribute_value(&mut self, chars: &[char]) -> Result<(Option<AttributeValue>, usize), Error> {
        let unexpected =
            |c: Option<&char>| format!("Unexpected character {} expected `=`", c.map_or("undefined".to_string(), char::to_string));
        let mut must_escape = true;
        let mut at = self.pos();
        let mut i = 0;
        while i < chars.len() && is_space(chars[i]) {
            at = step_pos(at, chars[i]);
            i += 1;
        }
        if i == chars.len() {
            return Ok((None, 0));
        }
        if chars[i] == '!' {
            must_escape = false;
            at.column += 1;
            i += 1;
            if chars.get(i) != Some(&'=') {
                return Err(self.error(ErrorCode::InvalidKeyCharacter, unexpected(chars.get(i))));
            }
        }
        if chars[i] != '=' {
            if i == 0 && !is_space(chars[0]) && chars[0] != ',' {
                return Err(self.error(ErrorCode::InvalidKeyCharacter, unexpected(chars.first())));
            }
            return Ok((None, 0));
        }
        self.line = at.line;
        self.column = at.column + 1;
        i += 1;
        while i < chars.len() && is_space(chars[i]) {
            self.step(chars[i]);
            i += 1;
        }
        let mut at = self.pos();
        let mut state = CharState::default();
        let start = i;
        let mut complete = Completeness::default();
        // Where the first character after the spaces at `i` is, once looked for.
        let mut next = 0;
        while i < chars.len() {
            let c = chars[i];
            if !state.is_nesting() {
                if is_space(c) {
                    if next <= i {
                        next = chars[i..].iter().position(|&c| !is_space(c)).map_or(chars.len(), |n| i + n);
                    }
                    let Some(&following) = chars.get(next) else {
                        break;
                    };
                    let spread = chars.get(next..next + 3) == Some(&['.', '.', '.'][..]);
                    let continues = is_punctuator(Some(following)) && !matches!(following, '\'' | '"' | ':') && !spread;
                    if !continues && complete.check(&chars[start..i]) {
                        break;
                    }
                }
                if c == ',' && complete.check(&chars[start..i]) {
                    break;
                }
            }
            if let Err(close) = state.parse_char(c) {
                return Err(self.crash_on_mismatch(close));
            }
            at = step_pos(at, c);
            i += 1;
        }
        self.line = at.line;
        self.column = at.column;
        Ok((Some(AttributeValue { val: chars[start..i].iter().collect(), must_escape }), i))
    }

    /// character-parser throws a SyntaxError without a Pug code; Pug passes it on.
    fn crash_on_mismatch(&self, close: char) -> Error {
        self.error(ErrorCode::MismatchedBracket, mismatched_bracket(close))
    }

    fn attrs(&mut self) -> Result<bool, Error> {
        if !self.rest().starts_with('(') {
            return Ok(false);
        }
        let start = self.pos();
        let range = self.bracket_expression(0)?;
        let chars: Vec<char> = self.rest()[range.clone()].chars().collect();
        self.column += 1;
        self.push(Kind::StartAttributes, start);
        self.consume(range.end + 1);
        let mut rest = &chars[..];
        while !rest.is_empty() {
            let used = self.attribute(rest)?;
            rest = &rest[used..];
        }
        let start = self.pos();
        self.column += 1;
        self.push(Kind::EndAttributes, start);
        Ok(true)
    }

    fn attributes_block(&mut self) -> Result<bool, Error> {
        let Some(len) = ATTRIBUTES_BLOCK.find(self.rest()).map(|m| m.len()) else {
            return Ok(false);
        };
        self.consume(len);
        let start = self.pos();
        self.column += len;
        let range = self.bracket_expression(0)?;
        let width = units(&self.rest()[..range.end + 1]);
        let val = self.rest()[range.clone()].to_string();
        self.consume(range.end + 1);
        self.column += width;
        self.push(Kind::AndAttributes(val), start);
        Ok(true)
    }

    fn indent(&mut self) -> Result<bool, Error> {
        let Some(indents) = self.scan_indentation() else {
            return Ok(false);
        };
        self.increment_line(1);
        self.consume(indents + 1);
        if self.rest().starts_with([' ', '\t']) {
            return Err(self.error(ErrorCode::InvalidIndentation, "Invalid indentation, you can use tabs or spaces but not both"));
        }
        if self.rest().starts_with('\n') {
            self.interpolation_allowed = true;
            return Ok(true);
        }
        let top = self.top();
        if indents < top {
            let mut outdents = 0;
            while let [.., below, top] = self.indent_stack[..] {
                if top <= indents {
                    break;
                }
                if below < indents {
                    let message = format!("Inconsistent indentation. Expecting either {below} or {top} spaces/tabs.");
                    return Err(self.error(ErrorCode::InconsistentIndentation, message));
                }
                outdents += 1;
                self.indent_stack.pop();
            }
            for _ in 0..outdents {
                self.column = 1;
                let start = self.pos();
                self.column = self.top() + 1;
                self.push(Kind::Outdent, start);
            }
        } else if indents > 0 && indents != top {
            let start = self.pos();
            self.column = 1 + indents;
            self.push(Kind::Indent(indents), start);
            self.indent_stack.push(indents);
        } else {
            let start = self.pos();
            self.column = 1 + top.min(indents);
            self.push(Kind::Newline, start);
        }
        self.interpolation_allowed = true;
        Ok(true)
    }

    /// The lines indented deeper than the current block after `.`, `//`, `-` or a filter, as text.
    fn pipeless_text(&mut self) -> Result<bool, Error> {
        while self.blank() {}
        let Some(first) = self.scan_indentation().filter(|&n| n > self.top()) else {
            return Ok(false);
        };
        let (len, indents, lines) = self.text_block_lines(first);
        // Each line without the block's indentation, and whether it had all of it (a blank line may not).
        let mut lines: Vec<(Range<usize>, bool)> = lines
            .into_iter()
            .map(|(line, width)| (line.start + unit_offset(&self.input[line.clone()], indents)..line.end, width >= indents))
            .collect();
        self.push(Kind::StartPipelessText, self.pos());
        self.consume(len);
        if self.rest().is_empty() {
            while lines.last().is_some_and(|(line, _)| line.is_empty()) {
                lines.pop();
            }
        }
        for (i, (line, indented)) in lines.into_iter().enumerate() {
            self.increment_line(1);
            let newline = (i != 0).then(|| self.pos());
            if indented {
                self.column += indents;
            }
            if let Some(start) = newline {
                self.push(Kind::Newline, start);
            }
            self.add_text(Kind::Text, line, String::new(), 0)?;
        }
        self.push(Kind::EndPipelessText, self.pos());
        Ok(true)
    }

    /// The bytes a text block whose first line is `first` deep takes, the indentation Pug reads it with and its lines as
    /// byte ranges of the input, each with its indentation's width. A line less indented than the first but inside the
    /// enclosing block makes Pug start over with that width, which the lines before it also have.
    fn text_block_lines(&self, first: usize) -> (usize, usize, Vec<(Range<usize>, usize)>) {
        let rest = self.rest();
        let mut indents = first;
        let mut lines = Vec::new();
        let mut at = 0;
        while let Some(after) = rest.get(at + 1..) {
            let line = &after[..after.find('\n').unwrap_or(after.len())];
            let width = self.line_indentation(line);
            if width < indents && !line.chars().all(is_js_whitespace) {
                if width <= self.top() {
                    break;
                }
                indents = width;
            }
            let start = self.at + at + 1;
            lines.push((start..start + line.len(), width));
            at += line.len() + 1;
            if at >= rest.len() {
                break;
            }
        }
        (at, indents, lines)
    }
}

/// A search through text the lexer goes on reading, from ever later bytes: what it found and how far it looked are kept, so
/// each part of the text is searched once.
#[derive(Default)]
struct Ahead {
    found: Option<usize>,
    /// Where the last search stopped: at what it found, or at the end it was given.
    until: usize,
}

impl Ahead {
    /// The first match in the bytes `from..end`; `search(start, end)` finds it in a part of them.
    fn find(&mut self, from: usize, end: usize, search: impl FnOnce(usize, usize) -> Option<usize>) -> Option<usize> {
        let start = match self.found {
            Some(at) if at >= from => return Some(at).filter(|&at| at < end),
            Some(_) => from,
            None => from.max(self.until),
        };
        if start >= end {
            return None;
        }
        self.found = search(start, end);
        self.until = self.found.unwrap_or(end);
        self.found
    }
}

/// Where the first `#{` or `!{` in `input[from..end]` starts.
fn interpolation_at(input: &str, mut from: usize, end: usize) -> Option<usize> {
    loop {
        let at = from + input.get(from..end)?.find(['#', '!'])?;
        if input[at + 1..end].starts_with('{') {
            return Some(at);
        }
        from = at + 1;
    }
}

fn step_pos(mut at: Pos, c: char) -> Pos {
    if c == '\n' {
        at.line += 1;
        at.column = 1;
    } else {
        at.column += c.len_utf16();
    }
    at
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nesting {
    /// An open bracket, by the character that closes it.
    Bracket(char),
    LineComment,
    BlockComment,
    SingleQuote,
    DoubleQuote,
    TemplateQuote,
    Regexp,
}

/// character-parser's `State`.
#[derive(Default)]
struct CharState {
    stack: Vec<Nesting>,
    regexp_start: bool,
    escaped: bool,
    has_dollar: bool,
    /// The characters seen outside comments, latest last.
    history: Vec<char>,
    last_char: Option<char>,
}

impl CharState {
    fn current(&self) -> Option<Nesting> {
        self.stack.last().copied()
    }

    fn is_comment(&self) -> bool {
        matches!(self.current(), Some(Nesting::LineComment | Nesting::BlockComment))
    }

    fn is_nesting(&self) -> bool {
        !self.stack.is_empty()
    }

    /// character-parser's `parseChar`; a closing bracket that doesn't match is returned as the error.
    fn parse_char(&mut self, c: char) -> Result<(), char> {
        let was_comment = self.is_comment();
        let last = self.history.last().copied();
        if self.regexp_start {
            if c == '/' || c == '*' {
                self.stack.pop();
            }
            self.regexp_start = false;
        }
        match self.current() {
            Some(Nesting::LineComment) => {
                if c == '\n' {
                    self.stack.pop();
                }
            }
            Some(Nesting::BlockComment) => {
                if self.last_char == Some('*') && c == '/' {
                    self.stack.pop();
                }
            }
            Some(Nesting::SingleQuote) => self.quoted(c, '\''),
            Some(Nesting::DoubleQuote) => self.quoted(c, '"'),
            Some(Nesting::Regexp) => self.quoted(c, '/'),
            Some(Nesting::TemplateQuote) => {
                if c == '`' && !self.escaped {
                    self.stack.pop();
                    self.has_dollar = false;
                } else if c == '\\' && !self.escaped {
                    self.escaped = true;
                    self.has_dollar = false;
                } else if c == '$' && !self.escaped {
                    self.has_dollar = true;
                } else if c == '{' && self.has_dollar {
                    self.stack.push(Nesting::Bracket('}'));
                } else {
                    self.escaped = false;
                    self.has_dollar = false;
                }
            }
            None | Some(Nesting::Bracket(_)) => {
                if let Some(close) = closing_bracket(c) {
                    self.stack.push(Nesting::Bracket(close));
                } else if matches!(c, ')' | ']' | '}') {
                    if self.current() != Some(Nesting::Bracket(c)) {
                        return Err(c);
                    }
                    self.stack.pop();
                } else if last == Some('/') && (c == '/' || c == '*') {
                    self.history.pop();
                    self.stack.push(if c == '/' { Nesting::LineComment } else { Nesting::BlockComment });
                } else if c == '/' && self.regexp_can_start() {
                    self.stack.push(Nesting::Regexp);
                    // A `*` or `/` next makes it a comment after all.
                    self.regexp_start = true;
                } else if c == '\'' {
                    self.stack.push(Nesting::SingleQuote);
                } else if c == '"' {
                    self.stack.push(Nesting::DoubleQuote);
                } else if c == '`' {
                    self.stack.push(Nesting::TemplateQuote);
                }
            }
        }
        if !self.is_comment() && !was_comment {
            self.history.push(c);
        }
        self.last_char = Some(c);
        Ok(())
    }

    fn quoted(&mut self, c: char, quote: char) {
        if c == quote && !self.escaped {
            self.stack.pop();
        } else {
            self.escaped = c == '\\' && !self.escaped;
        }
    }

    /// character-parser's `isRegexp`: whether a `/` here starts a regular expression rather than dividing.
    fn regexp_can_start(&self) -> bool {
        let end = self.history.iter().rposition(|&c| !is_js_whitespace(c)).map_or(0, |at| at + 1);
        let before = &self.history[..end];
        match before.last().copied() {
            Some(')') => false,
            Some('}') => true,
            last if is_punctuator(last) => true,
            _ => {
                let word = before.iter().rposition(|&c| !(c.is_ascii_alphanumeric() || c == '_')).map_or(0, |at| at + 1);
                is_keyword(&before[word..])
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    EndOfString,
    Mismatch(char),
}

/// character-parser's `parseUntil`: the bytes from `start` to the first `delimiter` outside brackets, strings and
/// comments, or the failure and the byte where it happened.
pub(crate) fn parse_until(src: &str, delimiter: char, start: usize) -> Result<Range<usize>, (Failure, usize)> {
    let mut state = CharState::default();
    for (at, c) in src[start..].char_indices() {
        let at = start + at;
        if !state.is_nesting() && c == delimiter {
            return Ok(start..at);
        }
        state.parse_char(c).map_err(|close| (Failure::Mismatch(close), at))?;
    }
    Err((Failure::EndOfString, src.len()))
}

fn is_punctuator(c: Option<char>) -> bool {
    c.is_none_or(|c| ".();,{}[]:?~%&*+-/<>^|!=".contains(c))
}

fn is_keyword(word: &[char]) -> bool {
    const KEYWORDS: &[&str] = &[
        "if",
        "in",
        "do",
        "var",
        "for",
        "new",
        "try",
        "let",
        "this",
        "else",
        "case",
        "void",
        "with",
        "enum",
        "while",
        "break",
        "catch",
        "throw",
        "const",
        "yield",
        "class",
        "super",
        "return",
        "typeof",
        "delete",
        "switch",
        "export",
        "import",
        "default",
        "finally",
        "extends",
        "function",
        "continue",
        "debugger",
        "package",
        "private",
        "interface",
        "instanceof",
        "implements",
        "protected",
        "public",
        "static",
    ];
    KEYWORDS.iter().any(|keyword| keyword.chars().eq(word.iter().copied()))
}

/// Stands in for `is-expression` (acorn 7, ES2019, not strict) where Pug only asks whether an attribute value is complete
/// before a space or a comma: balanced, without an open `?` of a conditional (`?.` and `??` are ES2020), a `class` or
/// `function` with its body, no statement keyword, and ending in an operand or a postfix `++`/`--`. Pug asks again as
/// the value grows, so the tokens that more characters can't change are read once.
#[derive(Default)]
struct Completeness {
    /// Where the characters start that the next check reads again.
    unread: usize,
    /// A line break comes between the last token read and `unread`.
    newline: bool,
    /// The tokens before `unread`.
    read: Tokens,
}

impl Completeness {
    /// Whether `code`, which the code of the checks before starts, looks like a complete expression.
    fn check(&mut self, code: &[char]) -> bool {
        if self.read.refused {
            return false;
        }
        // The tokens past those read for good, on a copy of what those left.
        let mut tail: Option<Tokens> = None;
        let (mut at, mut newline) = (self.unread, self.newline);
        loop {
            let previous = tail.as_ref().unwrap_or(&self.read).last[1].map(|token| token.kind);
            let (start, kind, end) = match js_token(code, at, previous, &mut newline) {
                Ok(token) => token,
                Err(Stop::End) => {
                    if tail.is_none() {
                        (self.unread, self.newline) = (code.len(), newline);
                    }
                    break;
                }
                Err(Stop::Unfinished) => return false,
                Err(Stop::Never) => {
                    self.read.refused |= tail.is_none();
                    return false;
                }
            };
            let token = JsToken { kind, newline_before: newline };
            newline = false;
            // A token is decided by its characters and at most the two after it.
            if tail.is_none() && end + 2 <= code.len() {
                self.read.add(token);
                (self.unread, self.newline) = (end, false);
            } else {
                if tail.is_none() {
                    (self.unread, self.newline) = (start, token.newline_before);
                }
                tail.get_or_insert_with(|| self.read.clone()).add(token);
            }
            at = end;
        }
        tail.as_ref().unwrap_or(&self.read).complete()
    }
}

/// What `Completeness` keeps of the tokens it read.
#[derive(Default, Clone)]
struct Tokens {
    brackets: Vec<char>,
    /// The bracket depths of the `class` and `function` words whose body hasn't closed.
    bodies: Vec<usize>,
    open_conditionals: i32,
    /// The last two tokens, the last one last.
    last: [Option<JsToken>; 2],
    /// A token was read that no code after it makes an expression of.
    refused: bool,
}

impl Tokens {
    fn add(&mut self, token: JsToken) {
        match token.kind {
            Js::Open(close) => self.brackets.push(close),
            Js::Close(c) => {
                self.refused |= self.brackets.pop() != Some(c);
                let depth = self.brackets.len();
                self.bodies.retain(|&at| at <= depth);
                if c == '}' && self.bodies.last() == Some(&depth) {
                    self.bodies.pop();
                }
            }
            Js::Question if self.brackets.is_empty() => self.open_conditionals += 1,
            Js::Colon if self.brackets.is_empty() => self.open_conditionals -= 1,
            Js::Spread if self.brackets.is_empty() => self.refused = true,
            Js::Reserved(Reserved::Body) => self.bodies.push(self.brackets.len()),
            // Inside brackets a statement keyword may be in a function's body or an object's key.
            Js::Reserved(Reserved::Statement) if self.brackets.is_empty() => self.refused = true,
            _ => {}
        }
        self.last = [self.last[1], Some(token)];
    }

    fn complete(&self) -> bool {
        if self.refused || !self.brackets.is_empty() || !self.bodies.is_empty() || self.open_conditionals > 0 {
            return false;
        }
        match self.last {
            [Some(before), Some(last)] if last.kind == Js::Update => {
                !last.newline_before && matches!(before.kind, Js::Name | Js::Close(')' | ']'))
            }
            [_, Some(last)] => last.kind.ends_operand(),
            [_, None] => false,
        }
    }
}

/// What a reserved word is in an expression. `let`, `yield`, `static` and the other words only strict mode reserves are
/// identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reserved {
    Value,
    Operator,
    /// `class` and `function`, complete with their `{...}`.
    Body,
    /// Never in an expression, but as a property name or key.
    Statement,
}

fn reserved(word: &str) -> Option<Reserved> {
    Some(match word {
        "this" | "super" | "null" | "true" | "false" => Reserved::Value,
        "new" | "typeof" | "void" | "delete" | "in" | "instanceof" | "extends" => Reserved::Operator,
        "class" | "function" => Reserved::Body,
        "break" | "case" | "catch" | "const" | "continue" | "debugger" | "default" | "do" | "else" | "enum" | "export" | "finally"
        | "for" | "if" | "import" | "return" | "switch" | "throw" | "try" | "var" | "while" | "with" => Reserved::Statement,
        _ => return None,
    })
}

/// A JavaScript token, as far as `Completeness` tells them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Js {
    /// An identifier, or any word after `.`.
    Name,
    Reserved(Reserved),
    /// A number, string, template literal or regular expression.
    Literal,
    /// An opening bracket, by the character that closes it.
    Open(char),
    Close(char),
    /// `++` or `--`.
    Update,
    Spread,
    Dot,
    Question,
    Colon,
    Operator,
}

impl Js {
    fn ends_operand(self) -> bool {
        matches!(self, Js::Name | Js::Literal | Js::Close(_) | Js::Reserved(Reserved::Value))
    }
}

#[derive(Debug, Clone, Copy)]
struct JsToken {
    kind: Js,
    newline_before: bool,
}

/// Why `js_token` found no token.
enum Stop {
    /// Only whitespace and comments are left.
    End,
    /// The code ends in a string, template literal, regular expression or comment, which more code may close.
    Unfinished,
    /// Where acorn stops whatever follows: a line comment (`is-expression` refuses them), a line break in a string or
    /// regular expression, a bracket mismatch in a template literal, or a character no token starts with.
    Never,
}

/// The first token of `chars` from `i` on, past whitespace and comments: where it starts, what it is and where it ends.
/// `previous` is the token before; `newline` is set when a line break comes before this one.
fn js_token(chars: &[char], mut i: usize, previous: Option<Js>, newline: &mut bool) -> Result<(usize, Js, usize), Stop> {
    loop {
        let Some(&c) = chars.get(i) else { return Err(Stop::End) };
        let start = i;
        let next = chars.get(i + 1).copied();
        i += 1;
        let kind = match c {
            _ if is_js_line_terminator(c) => {
                *newline = true;
                continue;
            }
            _ if is_js_whitespace(c) => continue,
            '/' if next == Some('/') => return Err(Stop::Never),
            '/' if next == Some('*') => {
                let body = &chars[start + 2..];
                let len = body.windows(2).position(|pair| pair == ['*', '/']).ok_or(Stop::Unfinished)?;
                *newline |= body[..len].iter().any(|&c| is_js_line_terminator(c));
                i = start + 2 + len + 2;
                continue;
            }
            '/' if previous.is_none_or(|p| !p.ends_operand() && p != Js::Update) => {
                i = regexp_end(chars, start)?;
                Js::Literal
            }
            '\'' | '"' => {
                i = string_end(chars, start)?;
                Js::Literal
            }
            '`' => {
                i = template_end(chars, start)?;
                Js::Literal
            }
            '.' if next.is_some_and(|n| n.is_ascii_digit()) => {
                i = number_end(chars, start);
                Js::Literal
            }
            '0'..='9' => {
                i = number_end(chars, start);
                Js::Literal
            }
            '.' if chars.get(i..i + 2) == Some(&['.', '.'][..]) => {
                i += 2;
                Js::Spread
            }
            '.' => Js::Dot,
            '+' | '-' if next == Some(c) => {
                i += 1;
                Js::Update
            }
            '?' => Js::Question,
            ':' => Js::Colon,
            ')' | ']' | '}' => Js::Close(c),
            '(' | '[' | '{' => Js::Open(closing_bracket(c).ok_or(Stop::Never)?),
            _ if is_punctuator(Some(c)) => Js::Operator,
            _ if c.is_alphabetic() || c == '_' || c == '$' => {
                while chars.get(i).is_some_and(|&c| is_identifier_part(c)) {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                match reserved(&word) {
                    Some(kind) if previous != Some(Js::Dot) => Js::Reserved(kind),
                    _ => Js::Name,
                }
            }
            _ => return Err(Stop::Never),
        };
        return Ok((start, kind, i));
    }
}

fn is_identifier_part(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '$' | '\u{200c}' | '\u{200d}')
}

/// The end of the number at `start`: `1`, `1.`, `.5`, `1e-3`, `0x1f`.
fn number_end(chars: &[char], start: usize) -> usize {
    let digits = |mut i: usize| {
        while chars.get(i).is_some_and(char::is_ascii_digit) {
            i += 1;
        }
        i
    };
    if chars[start] == '0' && chars.get(start + 1).is_some_and(|c| matches!(c, 'x' | 'X' | 'o' | 'O' | 'b' | 'B')) {
        return start + 2 + chars[start + 2..].iter().take_while(|c| c.is_ascii_alphanumeric()).count();
    }
    let mut i = digits(start);
    if chars.get(i) == Some(&'.') {
        i = digits(i + 1);
    }
    if chars.get(i).is_some_and(|c| matches!(c, 'e' | 'E')) {
        let sign = usize::from(chars.get(i + 1).is_some_and(|c| matches!(c, '+' | '-')));
        i = digits(i + 1 + sign);
    }
    i
}

/// The end of the string whose quote is at `open`; a line break can't be in it.
fn string_end(chars: &[char], open: usize) -> Result<usize, Stop> {
    let mut i = open + 1;
    loop {
        match *chars.get(i).ok_or(Stop::Unfinished)? {
            '\\' => i += 1,
            '\n' | '\r' => return Err(Stop::Never),
            c if c == chars[open] => return Ok(i + 1),
            _ => {}
        }
        i += 1;
    }
}

/// The end of the template literal at `open`, with what its `${...}` hold.
fn template_end(chars: &[char], open: usize) -> Result<usize, Stop> {
    let mut state = CharState::default();
    for (i, &c) in chars.iter().enumerate().skip(open) {
        state.parse_char(c).map_err(|_| Stop::Never)?;
        if !state.is_nesting() {
            return Ok(i + 1);
        }
    }
    Err(Stop::Unfinished)
}

/// The end of the regular expression at `open`, its flags included.
fn regexp_end(chars: &[char], open: usize) -> Result<usize, Stop> {
    let mut i = open + 1;
    let mut in_class = false;
    loop {
        let c = *chars.get(i).ok_or(Stop::Unfinished)?;
        if is_js_line_terminator(c) {
            return Err(Stop::Never);
        }
        match c {
            '\\' => i += 1,
            '[' => in_class = true,
            ']' => in_class = false,
            '/' if !in_class => break,
            _ => {}
        }
        i += 1;
    }
    i += 1;
    while chars.get(i).is_some_and(|&c| is_identifier_part(c)) {
        i += 1;
    }
    Ok(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn looks_complete(code: &str) -> bool {
        Completeness::default().check(&code.chars().collect::<Vec<_>>())
    }

    /// A token's type, value, start and end as pug-lexer prints them.
    type Printed = (&'static str, String, (usize, usize), (usize, usize));

    fn tokens(text: &str) -> Vec<Printed> {
        lex(text)
            .unwrap()
            .into_iter()
            .map(|t| {
                let val = match &t.kind {
                    Kind::Tag(v) | Kind::Id(v) | Kind::Class(v) | Kind::Text(v) => v.clone(),
                    Kind::InterpolatedCode { val, .. } => val.clone(),
                    Kind::Attribute { val, .. } => val.clone().unwrap_or_else(|| "true".to_string()),
                    Kind::Indent(width) => width.to_string(),
                    _ => String::new(),
                };
                (t.kind.name(), val, (t.start.line, t.start.column), (t.end.line, t.end.column))
            })
            .collect()
    }

    /// pug-lexer 5.0.1's tokens for the same template.
    #[test]
    fn tokens_and_their_positions() {
        let expected = [
            ("tag", "a", (1, 1), (1, 2)),
            ("id", "x", (1, 2), (1, 4)),
            ("class", "y", (1, 4), (1, 6)),
            ("start-attributes", "", (1, 6), (1, 7)),
            ("attribute", "u", (1, 7), (1, 13)),
            ("attribute", "true", (1, 15), (1, 16)),
            ("end-attributes", "", (1, 16), (1, 17)),
            ("text", "hi ", (1, 18), (1, 21)),
            ("interpolated-code", "n", (1, 21), (1, 25)),
            ("indent", "2", (2, 1), (2, 3)),
            ("tag", "span", (2, 3), (2, 7)),
            ("outdent", "", (2, 7), (2, 7)),
            ("eos", "", (2, 7), (2, 7)),
        ];
        let expected: Vec<_> = expected.into_iter().map(|(kind, val, start, end)| (kind, val.to_string(), start, end)).collect();
        assert_eq!(tokens("a#x.y(href=u, b) hi #{n}\n  span"), expected);
    }

    #[test]
    fn line_endings_and_bom_are_normalized() {
        assert_eq!(tokens("\u{feff}p\r\n  b\r\n"), tokens("p\n  b\n"));
        assert_eq!(tokens("p\r  b"), tokens("p\n  b"));
    }

    /// Checked against character-parser 2.2.0's `parseUntil`.
    #[test]
    fn expression_ends_skip_strings_comments_and_regexps() {
        assert_eq!(parse_until("(a, ')', \")\", `)${b}`) x", ')', 1), Ok(1..21));
        assert_eq!(parse_until("(a /* ) */ b) x", ')', 1), Ok(1..12));
        assert_eq!(parse_until("(x = /)/g) y", ')', 1), Ok(1..9));
        // After `)` a `/` divides.
        assert_eq!(parse_until("((a) / 2) y", ')', 1), Ok(1..8));
        assert_eq!(parse_until("(a]", ')', 1), Err((Failure::Mismatch(']'), 2)));
        assert_eq!(parse_until("(a", ')', 1), Err((Failure::EndOfString, 2)));
    }

    /// Checked against is-expression 4.0.0 (acorn 7.4.1).
    #[test]
    fn attribute_values_end_where_is_expression_would_say() {
        let complete = [
            "a",
            "'x'",
            "f(a, b)",
            "a ? b : c",
            "a.new",
            "x++",
            "this",
            "[1, 2]",
            "`t ${a}`",
            "1.",
            "/a/",
            "/a\\/b/",
            "/[/]/g",
            "1 /* c */",
            "public",
            "let",
            "yield",
            "(a)++",
            "a[0] --",
            "class C extends B {}",
            "function f(a) { return a }",
            "a?.5 : 1",
            "o. default",
            "o./*c*/default",
            "πin",
            "cafénew",
            "{default: 1}",
            "x\u{feff}",
        ];
        for code in complete {
            assert!(looks_complete(code), "{code}");
        }
        let incomplete = [
            "",
            "a +",
            "a ?",
            "a ? b",
            "new",
            "typeof",
            "(a",
            "'x",
            "() =>",
            "a.",
            "a ? b :",
            "++",
            "-- ",
            "a +++",
            "a\n++",
            "class C",
            "class C extends B",
            "function f(a)",
            "a?.b",
            "a ?? b",
            "1 // c",
            "/* c */",
            "if",
            "...y",
            "x\u{85}",
            "enum",
        ];
        for code in incomplete {
            assert!(!looks_complete(code), "{code}");
        }
    }

    /// Checked as a value grows a character at a time, as Pug's lexer asks, the answers are those for each prefix alone.
    #[test]
    fn values_read_as_they_grow_are_judged_as_a_whole() {
        let values = [
            "f(a, b) ? c : d ? e : f",
            "a /* x */ + b // c",
            "`a${b}` + 1 ... x",
            "x = /re/g.test(y) / 2",
            "1. + .5e3 - 0x1f",
            "f(a)(b) ++ \n--c",
            "'s\\'q' + \"t\\\"u\" + 'v\nw'",
            "{a: [1, (2)]}.a .. .b",
            "class A extends B { m() { return 1 } } (x)",
            "function () {} () if",
            "o. default + x.if + typeof /a/",
            "πin + cafénew + \u{feff}x\u{2028}y",
            "a ?. b ?? c } ] )",
            "`${`${a}`}` + `x",
        ];
        for value in values {
            let chars: Vec<char> = value.chars().collect();
            let mut growing = Completeness::default();
            for end in 0..=chars.len() {
                let prefix = &chars[..end];
                assert_eq!(growing.check(prefix), Completeness::default().check(prefix), "{:?}", prefix.iter().collect::<String>());
            }
        }
    }

    fn attribute_values(text: &str) -> Vec<String> {
        lex(text).unwrap().into_iter().filter_map(|t| if let Kind::Attribute { val, .. } = t.kind { val } else { None }).collect()
    }

    /// pug-lexer 5.0.1's attribute values for the same templates.
    #[test]
    fn attributes_split_where_pug_splits() {
        let cases = [
            ("a(x=1. y=2)", "1."),
            ("a(x=/a/ y=2)", "/a/"),
            ("a(x=/a\\/b/ y=2)", "/a\\/b/"),
            ("a(x=1 /* c */ y=2)", "1 /* c */"),
            ("a(x=public y=2)", "public"),
            ("a(x=++ i, y=2)", "++ i"),
            ("a(x=-- i, y=2)", "-- i"),
            ("a(x=a +++ b, y=2)", "a +++ b"),
            ("a(x=class C extends B {}, y=2)", "class C extends B {}"),
            ("a(x=a?.5 : 1, y=2)", "a?.5 : 1"),
            ("a(x=o. default, y=2)", "o. default"),
            ("a(x=o./*c*/default, y=2)", "o./*c*/default"),
            ("a(x=πin, y=2)", "πin"),
            ("a(x=cafénew y=2)", "cafénew"),
        ];
        for (text, first) in cases {
            assert_eq!(attribute_values(text), [first, "2"], "{text}");
        }
    }

    #[test]
    fn whitespace_is_javascripts() {
        assert!(is_js_whitespace('\u{feff}') && is_js_whitespace('\u{a0}') && !is_js_whitespace('\u{85}'));
        assert_eq!(js_trim("\u{feff} a\u{85}"), "a\u{85}");
        // character-parser skips it before `typeof`, so the `/` starts a regular expression.
        assert_eq!(attribute_values("a(x=typeof\u{feff}/[)]/)"), ["typeof\u{feff}/[)]/"]);
        assert_eq!(tokens("+\u{feff}foo")[0], ("call", String::new(), (1, 1), (1, 6)));
        assert_eq!(lex("+\u{85}foo").unwrap_err().code, ErrorCode::UnexpectedText);
        assert!(matches!(&lex("+foo(\u{feff}a=1)").unwrap()[0].kind, Kind::Call { args: None, .. }));
        // A line of U+FEFF is blank in a text block.
        let texts: Vec<_> =
            tokens("p.\n  a\n\u{feff}\n  b").into_iter().filter(|t| t.0 == "text").map(|(_, val, start, _)| (val, start)).collect();
        assert_eq!(texts, [("a".to_string(), (2, 3)), (String::new(), (3, 1)), ("b".to_string(), (4, 3))]);
    }

    #[test]
    fn line_terminators_are_javascripts() {
        let texts: Vec<_> = tokens("p #{a} b\u{2028} #{c}").into_iter().map(|(kind, val, ..)| (kind, val)).collect();
        assert_eq!(texts[1..3], [("text", "#{a} b\u{2028} ".to_string()), ("interpolated-code", "c".to_string())]);
        assert!(matches!(&lex("mixin foo(a\u{2028}b)").unwrap()[0].kind, Kind::Mixin { args: None, .. }));
    }

    #[test]
    fn columns_count_utf16_units() {
        let error = lex("p 😀 #{a").unwrap_err();
        assert_eq!((error.line, error.column), (1, 9));
    }

    #[test]
    fn spans_are_bytes_of_the_text_as_given() {
        let text = "\u{feff}p 😀 #[b x]\r\n  a(href=\"/\")\rinclude \u{a0}row \n";
        let spans: Vec<(&str, &str)> = lex(text).unwrap().iter().map(|t| (t.kind.name(), &text[t.span.clone()])).collect();
        let expected = [
            ("tag", "p"),
            ("text", "😀 "),
            ("start-pug-interpolation", "#["),
            ("tag", "b"),
            ("text", "x"),
            ("end-pug-interpolation", "]"),
            ("text", ""),
            ("indent", "  "),
            ("tag", "a"),
            ("start-attributes", "("),
            ("attribute", "href=\"/\""),
            ("end-attributes", ")"),
            ("outdent", ""),
            ("include", "include"),
            ("path", "row"),
            ("newline", ""),
            ("eos", ""),
        ];
        assert_eq!(spans, expected);
    }
}
