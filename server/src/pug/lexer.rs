//! Port of `pug-lexer` 4.1.0 (Pug 2.0.4) with the parts of `character-parser` 2.2.0 it uses to find where a
//! bracketed expression, attribute value or interpolation ends. Lines and columns are Pug's: 1-based, columns in
//! UTF-16 code units. `is-expression` isn't ported: the JavaScript isn't validated (`PUG:SYNTAX_ERROR`), and where
//! Pug asks it whether an attribute value has ended, `looks_complete` answers.

use super::Error;
use crate::util::{closing_bracket, BOM};
use regex::Regex;
use std::ops::Range;
use std::sync::LazyLock;

const INTERPOLATION_END: &str = "End of line was reached with no closing bracket for interpolation.";

macro_rules! regex {
    ($name:ident, $pattern:literal) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pattern).unwrap());
    };
}

// pug-lexer's regular expressions, with JavaScript's ASCII `\w` and `\b` spelled out.
regex!(COMMENT, r"^//(-)?([^\n]*)");
regex!(TAG, r"^([A-Za-z0-9_](?:[-:A-Za-z0-9_]*[A-Za-z0-9_])?)");
regex!(FILTER, r"^:([A-Za-z0-9_\-]+)");
regex!(DOCTYPE, r"^doctype *([^\n]*)");
regex!(ID, r"^#([A-Za-z0-9_-]+)");
regex!(CLASS, r"^\.([_a-zA-Z0-9\-]*[_a-zA-Z][_a-zA-Z0-9\-]*)");
regex!(CLASS_WITHOUT_LETTER, r"^\.[_a-zA-Z0-9\-]+");
// The name quoted in an invalid id or class message: JavaScript's `.` stops at line terminators, its `[^...]` doesn't.
regex!(INVALID_NAME, r"[^\n\r\u{2028}\u{2029}][^ \t(#.:]*");
regex!(TEXT, r"^(?:\| ?| )([^\n]+)");
regex!(TEXT_SPACE, r"^( )");
regex!(TEXT_PIPE, r"^\|( ?)");
regex!(TEXT_HTML, r"^(<[^\n]*)");
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
regex!(CALL, r"^\+(\s*)(([-A-Za-z0-9_]+)|(#\{))");
regex!(CALL_ARGUMENTS, r"^ *\(");
regex!(CALL_ATTRIBUTES, r"^\s*[-A-Za-z0-9_]+ *=");
regex!(MIXIN, r"^mixin +([-A-Za-z0-9_]+)(?: *\((.*)\))? *");
regex!(CONDITIONAL, r"^(if|unless|else if|else)(?-u:\b)([^\n]*)");
regex!(WHILE, r"^while +([^\n]+)");
regex!(WHILE_WORD, r"^while(?-u:\b)");
regex!(EACH, r"^(?:each|for) +([a-zA-Z_$][A-Za-z0-9_$]*)(?: *, *([a-zA-Z_$][A-Za-z0-9_$]*))? * in *([^\n]+)");
regex!(EACH_WORD, r"^(?:each|for)(?-u:\b)");
regex!(EACH_DASHED, r"^- *(?:each|for) +([a-zA-Z_$][A-Za-z0-9_$]*)(?: *, *([a-zA-Z_$][A-Za-z0-9_$]*))? +in +([^\n]+)");
regex!(CODE, r"^(!?=|-)[ \t]*([^\n]+)");
regex!(BLOCK_CODE, r"^-");
regex!(ATTRIBUTES_BLOCK, r"^&attributes(?-u:\b)");
regex!(SLASH, r"^/");
regex!(COLON, r"^: +");
regex!(STRING_INTERPOLATION, r"(\\)?([#!])\{");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

/// Pug's tokens for `text`, or its first error.
pub fn lex(text: &str) -> Result<Vec<Token>, Error> {
    let text = text.strip_prefix(BOM).unwrap_or(text);
    let mut lexer = Lexer::new(text.replace("\r\n", "\n").replace('\r', "\n"), false, Pos { line: 1, column: 1 });
    lexer.run()?;
    Ok(lexer.tokens)
}

fn units(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Byte length of the first `n` UTF-16 code units of `s`.
pub(crate) fn unit_offset(s: &str, n: usize) -> usize {
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

struct Lexer {
    input: String,
    /// Where the unconsumed input starts in `input`.
    at: usize,
    /// Lexing the inside of `#[...]`, which ends at its `]`.
    interpolated: bool,
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

impl Lexer {
    fn new(input: String, interpolated: bool, start: Pos) -> Self {
        Lexer {
            input,
            at: 0,
            interpolated,
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
            Lexer::eos,
            |l| Ok(l.end_interpolation()),
            |l| Ok(l.simple_end_of_line(&YIELD, Kind::Yield)),
            Lexer::doctype,
            Lexer::interpolation,
            Lexer::case,
            Lexer::when,
            Lexer::default_case,
            Lexer::extends,
            |l| Ok(l.named_block(&APPEND, BlockMode::Append)),
            |l| Ok(l.named_block(&PREPEND, BlockMode::Prepend)),
            |l| Ok(l.named_block(&BLOCK, BlockMode::Replace)),
            |l| Ok(l.simple_end_of_line(&MIXIN_BLOCK, Kind::MixinBlock)),
            Lexer::include,
            |l| Ok(l.mixin()),
            Lexer::call,
            Lexer::conditional,
            Lexer::each,
            Lexer::while_,
            |l| Ok(l.tag()),
            |l| l.filter(false),
            Lexer::block_code,
            Lexer::code,
            Lexer::id,
            Lexer::dot,
            Lexer::class_name,
            Lexer::attrs,
            Lexer::attributes_block,
            Lexer::indent,
            Lexer::text,
            Lexer::text_html,
            Lexer::comment,
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
            Failure::EndOfString => self.error("PUG:NO_END_BRACKET", INTERPOLATION_END),
            Failure::Mismatch(c) => self.error("PUG:BRACKET_MISMATCH", mismatched_bracket(c)),
        }
    }

    fn unexpected_text(&self) -> Error {
        let rest = self.rest();
        self.error("PUG:UNEXPECTED_TEXT", format!("unexpected text \"{}\"", &rest[..unit_offset(rest, 5)]))
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

    fn error(&self, code: &'static str, message: impl Into<String>) -> Error {
        Error { code, message: message.into(), line: self.line, column: self.column }
    }

    fn push(&mut self, kind: Kind, start: Pos) {
        self.tokens.push(Token { kind, start, end: self.pos() });
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

    /// Moves the position to the end of `text`, which starts at the current position.
    fn step_over(&mut self, text: &str) {
        match text.rfind('\n') {
            Some(last) => {
                self.increment_line(text.matches('\n').count());
                self.column += units(&text[last + 1..]);
            }
            None => self.column += units(text),
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
            return Err(self.error("ERR_ASSERTION", "The start character should be \"(\", \"{\" or \"[\""));
        };
        match parse_until(self.rest(), close, skip + 1) {
            Ok(range) => Ok(range),
            Err((failure, at)) => {
                let before = self.rest()[..at].to_string();
                self.step_over(&before);
                Err(match failure {
                    Failure::EndOfString => {
                        self.error("PUG:NO_END_BRACKET", format!("The end of the string reached with no closing bracket {close} found."))
                    }
                    Failure::Mismatch(c) => self.error("PUG:BRACKET_MISMATCH", mismatched_bracket(c)),
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
        if self.interpolated {
            return Err(self.error("PUG:NO_END_BRACKET", INTERPOLATION_END));
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
        let Some((whole, buffer, val)) =
            COMMENT.captures(self.rest()).map(|caps| (caps[0].to_string(), caps.get(1).is_none(), caps[2].to_string()))
        else {
            return Ok(false);
        };
        self.consume(whole.len());
        let start = self.pos();
        self.interpolation_allowed = buffer;
        self.column += units(&whole);
        self.push(Kind::Comment { val, buffer }, start);
        self.pipeless_text(None)?;
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
            self.pipeless_text(None)?;
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
    fn invalid_name(&self, code: &'static str, message: impl Fn(&str) -> String) -> Error {
        match INVALID_NAME.find(&self.rest()[1..]) {
            Some(name) => self.error(code, message(name.as_str())),
            // Pug's own lexer throws on the regular expression's missing match here.
            None => self.error("TypeError", "Cannot read properties of null (reading '0')"),
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
        Err(self.invalid_name("PUG:INVALID_ID", |name| format!("\"{name}\" is not a valid ID.")))
    }

    fn class_name(&mut self) -> Result<bool, Error> {
        if let Some(Scanned { start, val }) = self.scan(&CLASS) {
            let name = val.unwrap_or_default();
            self.column += units(&name);
            self.push(Kind::Class(name), start);
            return Ok(true);
        }
        if CLASS_WITHOUT_LETTER.is_match(self.rest()) {
            return Err(self.error("PUG:INVALID_CLASS_NAME", "Class names must contain at least one letter or underscore."));
        }
        if !self.rest().starts_with('.') {
            return Ok(false);
        }
        Err(self.invalid_name("PUG:INVALID_CLASS_NAME", |name| {
            format!(
                "\"{name}\" is not a valid class name.  Class names can only contain \"_\", \"-\", a-z and 0-9, and must contain at least one of \"_\", or a-z"
            )
        }))
    }

    fn end_interpolation(&mut self) -> bool {
        if !self.interpolated || !self.rest().starts_with(']') {
            return false;
        }
        self.consume(1);
        self.ended = true;
        true
    }

    /// Text with its `#{}`, `!{}` and `#[]` interpolations, and in `#[...]` the `]` that ends it.
    fn add_text(&mut self, make: fn(String) -> Kind, value: &str, prefix: &str, escaped: usize) -> Result<(), Error> {
        // pug-lexer returns early on an empty text only with a prefix given, which no call does, so "" is a token.
        const NONE: usize = usize::MAX;
        let allowed = self.interpolation_allowed;
        let end = if self.interpolated { value.find(']') } else { None }.unwrap_or(NONE);
        let start = if allowed { value.find("#[") } else { None }.unwrap_or(NONE);
        let escaped_start = if allowed { value.find("\\#[") } else { None }.unwrap_or(NONE);
        let string_interpolation = STRING_INTERPOLATION.captures(value);
        let string_at = match &string_interpolation {
            Some(caps) if allowed => caps.get(0).map_or(NONE, |m| m.start()),
            _ => NONE,
        };

        if escaped_start != NONE && escaped_start < end && escaped_start < start && escaped_start < string_at {
            let prefix = format!("{prefix}{}#[", &value[..escaped_start]);
            return self.add_text(make, &value[escaped_start + 3..], &prefix, escaped + 1);
        }
        if start != NONE && start < end && start < escaped_start && start < string_at {
            return self.add_tag_interpolation(make, value, prefix, escaped, start);
        }
        if end != NONE && end < start && end < escaped_start && end < string_at {
            if !prefix.is_empty() || end > 0 {
                self.add_text(make, &value[..end], prefix, 0)?;
            }
            self.ended = true;
            self.input = format!("{}{}", &value[end + 1..], self.rest());
            self.at = 0;
            return Ok(());
        }
        if let Some(caps) = string_interpolation.filter(|_| string_at != NONE) {
            if caps.get(1).is_some() {
                // Pug 2 writes an escaped `\!{` back as `#{` too.
                let prefix = format!("{prefix}{}#{{", &value[..string_at]);
                return self.add_text(make, &value[string_at + 3..], &prefix, escaped + 1);
            }
            let before = format!("{prefix}{}", &value[..string_at]);
            if !before.is_empty() {
                let at = self.pos();
                self.column += units(&before) + escaped;
                self.push(make(before), at);
            }
            let must_escape = &caps[2] == "#";
            let rest = &value[caps.get(0).map_or(0, |m| m.end())..];
            return self.add_string_interpolation(make, rest, must_escape);
        }
        let value = format!("{prefix}{value}");
        let at = self.pos();
        self.column += units(&value) + escaped;
        self.push(make(value), at);
        Ok(())
    }

    /// `#[tag ...]` at byte `start` of `value`: lexed by a lexer of its own that stops at the `]`.
    fn add_tag_interpolation(
        &mut self,
        make: fn(String) -> Kind,
        value: &str,
        prefix: &str,
        escaped: usize,
        start: usize,
    ) -> Result<(), Error> {
        let before = format!("{prefix}{}", &value[..start]);
        let at = self.pos();
        self.column += units(prefix) + units(&value[..start]) + escaped;
        self.push(make(before), at);
        let at = self.pos();
        self.column += 2;
        self.push(Kind::StartPugInterpolation, at);
        let mut child = Lexer::new(value[start + 2..].to_string(), true, self.pos());
        if let Err(error) = child.run() {
            if !error.code.starts_with("PUG:") {
                return Err(error);
            }
            self.column = error.column;
            return Err(self.error(error.code, error.message));
        }
        self.column = child.column;
        self.tokens.append(&mut child.tokens);
        let at = self.pos();
        self.column += 1;
        self.push(Kind::EndPugInterpolation, at);
        self.add_text(make, child.rest(), "", 0)
    }

    /// The code of a `#{` or `!{` whose brace ended before `rest`, then the text after its `}`.
    fn add_string_interpolation(&mut self, make: fn(String) -> Kind, rest: &str, must_escape: bool) -> Result<(), Error> {
        let at = self.pos();
        self.column += 2;
        let range = match parse_until(rest, '}', 0) {
            Ok(range) => range,
            Err((failure, index)) => {
                self.column += units(&rest[..index]);
                return Err(self.interpolation_error(failure));
            }
        };
        let val = rest[range.clone()].to_string();
        let after = range.end + 1;
        if after < rest.len() {
            self.column += units(&rest[..after]);
            self.push(Kind::InterpolatedCode { val, must_escape }, at);
            return self.add_text(make, &rest[after..], "", 0);
        }
        self.column += units(rest);
        self.push(Kind::InterpolatedCode { val, must_escape }, at);
        Ok(())
    }

    fn text(&mut self) -> Result<bool, Error> {
        let Some(Scanned { val, .. }) = self.scan(&TEXT).or_else(|| self.scan(&TEXT_SPACE)).or_else(|| self.scan(&TEXT_PIPE)) else {
            return Ok(false);
        };
        self.add_text(Kind::Text, &val.unwrap_or_default(), "", 0)?;
        Ok(true)
    }

    fn text_html(&mut self) -> Result<bool, Error> {
        let Some(Scanned { val, .. }) = self.scan(&TEXT_HTML) else {
            return Ok(false);
        };
        self.add_text(Kind::TextHtml, &val.unwrap_or_default(), "", 0)?;
        Ok(true)
    }

    fn dot(&mut self) -> Result<bool, Error> {
        if !self.simple_end_of_line(&DOT, Kind::Dot) {
            return Ok(false);
        }
        self.pipeless_text(None)?;
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
                return Err(self.error("PUG:NO_EXTENDS_PATH", "missing path for extends"));
            }
            return Ok(true);
        }
        if self.scan(&EXTENDS_WORD).is_some() {
            return Err(self.error("PUG:MALFORMED_EXTENDS", "malformed extends"));
        }
        Ok(false)
    }

    /// `block name`, `block append name`/`append name`, `block prepend name`/`prepend name`. A `//` comment after
    /// the name is left in the input, cut where Pug cuts it.
    fn named_block(&mut self, re: &Regex, mode: BlockMode) -> bool {
        let Some((whole, captured)) = re.captures(self.rest()).map(|caps| (caps[0].to_string(), caps[1].to_string())) else {
            return false;
        };
        let mut name = captured.trim();
        let mut comment = "";
        if let Some(at) = name.find("//") {
            comment = &name[at..];
            name = name[..at].trim();
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
                return Err(self.error("PUG:NO_INCLUDE_PATH", "missing path for include"));
            }
            return Ok(true);
        }
        if self.scan(&INCLUDE_WORD).is_some() {
            return Err(self.error("PUG:MALFORMED_INCLUDE", "malformed include"));
        }
        Ok(false)
    }

    fn path(&mut self) -> bool {
        let Some(Scanned { start, val }) = self.scan_end_of_line(&PATH) else {
            return false;
        };
        let path = val.unwrap_or_default().trim().to_string();
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
            return Err(self.error("PUG:NO_CASE_EXPRESSION", "missing expression for case"));
        }
        Ok(false)
    }

    /// `when expr`; a `:` inside a string or brackets doesn't end the expression.
    fn when(&mut self) -> Result<bool, Error> {
        let Some(Scanned { start, val }) = self.scan_end_of_line(&WHEN) else {
            if self.scan(&WHEN_WORD).is_some() {
                return Err(self.error("PUG:NO_WHEN_EXPRESSION", "missing expression for when"));
            }
            return Ok(false);
        };
        let mut expr = val.unwrap_or_default();
        while self.char_state(&expr)?.is_nesting() {
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
            return Err(self.error("PUG:DEFAULT_WITH_EXPRESSION", "default should not have an expression"));
        }
        Ok(false)
    }

    /// `+name` or `+#{expr}`, then `(args)` unless the parentheses hold attributes.
    fn call(&mut self) -> Result<bool, Error> {
        let Some((whole, spaces, name)) =
            CALL.captures(self.rest()).map(|caps| (caps[0].to_string(), caps[1].len(), caps.get(3).map(|m| m.as_str().to_string())))
        else {
            return Ok(false);
        };
        let (start, name) = match name {
            Some(name) => {
                self.consume(whole.len());
                let start = self.pos();
                self.column += units(&whole);
                (start, name)
            }
            None => {
                let range = self.bracket_expression(1 + spaces + 1)?;
                let consumed = self.rest()[..range.end + 1].to_string();
                let expr = &self.rest()[range];
                let name = format!("#{{{expr}}}");
                self.consume(consumed.len());
                let start = self.pos();
                self.column += units(&consumed);
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
                src.chars().for_each(|c| self.step(c));
                self.column += 1;
                args = Some(src);
            }
        }
        self.push(Kind::Call { name, args }, start);
        Ok(true)
    }

    fn mixin(&mut self) -> bool {
        let Some((whole, name, args)) =
            MIXIN.captures(self.rest()).map(|caps| (caps[0].to_string(), caps[1].to_string(), caps.get(2).map(|m| m.as_str().to_string())))
        else {
            return false;
        };
        self.consume(whole.len());
        let start = self.pos();
        self.column += units(&whole);
        self.push(Kind::Mixin { name, args: args.filter(|a| !a.is_empty()) }, start);
        true
    }

    fn conditional(&mut self) -> Result<bool, Error> {
        let Some((whole, keyword, js)) =
            CONDITIONAL.captures(self.rest()).map(|caps| (caps[0].to_string(), caps[1].to_string(), caps[2].trim().to_string()))
        else {
            return Ok(false);
        };
        self.consume(whole.len());
        let start = self.pos();
        self.column += units(&whole) - units(&js);
        let kind = match keyword.as_str() {
            "if" => Kind::If(js.clone()),
            "unless" => Kind::If(format!("!({js})")),
            "else if" => Kind::ElseIf(js.clone()),
            _ if !js.is_empty() => {
                return Err(self.error("PUG:ELSE_CONDITION", "`else` cannot have a condition, perhaps you meant `else if`"));
            }
            _ => Kind::Else,
        };
        self.column += units(&js);
        self.push(kind, start);
        Ok(true)
    }

    fn while_(&mut self) -> Result<bool, Error> {
        if let Some((whole, test)) = WHILE.captures(self.rest()).map(|caps| (caps[0].to_string(), caps[1].to_string())) {
            self.consume(whole.len());
            let start = self.pos();
            self.column += units(&whole);
            self.push(Kind::While(test), start);
            return Ok(true);
        }
        if self.scan(&WHILE_WORD).is_some() {
            return Err(self.error("PUG:NO_WHILE_EXPRESSION", "missing expression for while"));
        }
        Ok(false)
    }

    fn each(&mut self) -> Result<bool, Error> {
        let captured = EACH
            .captures(self.rest())
            .map(|caps| (caps[0].to_string(), caps[1].to_string(), caps.get(2).map(|m| m.as_str().to_string()), caps[3].to_string()));
        if let Some((whole, val, key, code)) = captured {
            self.consume(whole.len());
            let start = self.pos();
            self.column += units(&whole);
            self.push(Kind::Each { val, key, code }, start);
            return Ok(true);
        }
        if self.scan(&EACH_WORD).is_some() {
            return Err(self.error("PUG:MALFORMED_EACH", "malformed each"));
        }
        if EACH_DASHED.is_match(self.rest()) {
            return Err(self.error(
                "PUG:MALFORMED_EACH",
                "Pug each and for should no longer be prefixed with a dash (\"-\"). They are pug keywords and not part of JavaScript.",
            ));
        }
        Ok(false)
    }

    /// `= code`, `!= code` and `- code`; inside `#[...]` the code ends at the `]`.
    fn code(&mut self) -> Result<bool, Error> {
        let Some((whole, flags, mut code)) =
            CODE.captures(self.rest()).map(|caps| (caps[0].to_string(), caps[1].to_string(), caps[2].to_string()))
        else {
            return Ok(false);
        };
        let lead = units(&whole) - units(&code);
        let mut shortened = 0;
        if self.interpolated {
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
        self.consume(whole.len() - shortened);
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
        self.pipeless_text(None)?;
        Ok(true)
    }

    /// One attribute of `chars`, the inside of `(...)`, and the comma after it; returns the characters it used.
    fn attribute(&mut self, chars: &[char]) -> Result<usize, Error> {
        let mut i = chars.iter().take_while(|&&c| is_space(c)).count();
        chars[..i].iter().for_each(|&c| self.step(c));
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
                return Err(self.error("PUG:INVALID_KEY_CHARACTER", unexpected(chars.get(i))));
            }
        }
        if chars[i] != '=' {
            if i == 0 && !is_space(chars[0]) && chars[0] != ',' {
                return Err(self.error("PUG:INVALID_KEY_CHARACTER", unexpected(chars.first())));
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
        let mut val = String::new();
        while i < chars.len() {
            let c = chars[i];
            if !state.is_nesting() {
                if is_space(c) {
                    let Some(next) = chars[i..].iter().position(|&c| !is_space(c)).map(|n| i + n) else {
                        break;
                    };
                    let following = chars[next];
                    let spread = chars.get(next..next + 3) == Some(&['.', '.', '.'][..]);
                    let continues = is_punctuator(Some(following)) && !matches!(following, '\'' | '"' | ':') && !spread;
                    if !continues && looks_complete(&val) {
                        break;
                    }
                }
                if c == ',' && looks_complete(&val) {
                    break;
                }
            }
            if let Err(close) = state.parse_char(c) {
                return Err(self.crash_on_mismatch(close));
            }
            val.push(c);
            at = step_pos(at, c);
            i += 1;
        }
        self.line = at.line;
        self.column = at.column;
        Ok((Some(AttributeValue { val, must_escape }), i))
    }

    /// character-parser throws a SyntaxError without a Pug code; Pug passes it on.
    fn crash_on_mismatch(&self, close: char) -> Error {
        self.error("CHARACTER_PARSER:MISMATCHED_BRACKET", mismatched_bracket(close))
    }

    fn char_state(&self, code: &str) -> Result<CharState, Error> {
        let mut state = CharState::default();
        for c in code.chars() {
            state.parse_char(c).map_err(|close| self.crash_on_mismatch(close))?;
        }
        Ok(state)
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
        let consumed = self.rest()[..range.end + 1].to_string();
        let val = self.rest()[range].to_string();
        self.consume(consumed.len());
        self.column += units(&consumed);
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
            return Err(self.error("PUG:INVALID_INDENTATION", "Invalid indentation, you can use tabs or spaces but not both"));
        }
        if self.rest().starts_with('\n') {
            self.interpolation_allowed = true;
            return Ok(true);
        }
        let top = self.top();
        if indents < top {
            let mut outdents = 0;
            while self.top() > indents {
                let below = self.indent_stack[self.indent_stack.len() - 2];
                if below < indents {
                    let message = format!("Inconsistent indentation. Expecting either {below} or {} spaces/tabs.", self.top());
                    return Err(self.error("PUG:INCONSISTENT_INDENTATION", message));
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

    /// The lines indented deeper than the current block after `.`, `//`, `-` or a filter, as text; `indents` is
    /// the width found when a less indented line made Pug start over.
    fn pipeless_text(&mut self, indents: Option<usize>) -> Result<bool, Error> {
        while self.blank() {}
        let captured = self.scan_indentation();
        let Some(indents) = indents.filter(|&n| n > 0).or(captured).filter(|&n| n > self.top()) else {
            return Ok(false);
        };
        let (len, mut lines) = match self.text_block_lines(indents) {
            Ok(block) => block,
            Err(width) => return self.pipeless_text(Some(width)),
        };
        self.push(Kind::StartPipelessText, self.pos());
        self.consume(len);
        if self.rest().is_empty() {
            while lines.last().is_some_and(|(line, _)| line.is_empty()) {
                lines.pop();
            }
        }
        for (i, (line, indented)) in lines.iter().enumerate() {
            self.increment_line(1);
            let newline = (i != 0).then(|| self.pos());
            if *indented {
                self.column += indents;
            }
            if let Some(start) = newline {
                self.push(Kind::Newline, start);
            }
            self.add_text(Kind::Text, line, "", 0)?;
        }
        self.push(Kind::EndPipelessText, self.pos());
        Ok(true)
    }

    /// The bytes a text block `indents` deep takes and its lines, each without that indentation and with whether
    /// it had all of it (a blank line may not); or the width to start over with when a line less indented than the
    /// first is still inside the enclosing block.
    fn text_block_lines(&self, indents: usize) -> Result<(usize, Vec<(String, bool)>), usize> {
        let rest = self.rest();
        let mut lines = Vec::new();
        let mut at = 0;
        while let Some(after) = rest.get(at + 1..) {
            let line = &after[..after.find('\n').unwrap_or(after.len())];
            let width = self.line_indentation(line);
            if width < indents && !line.trim().is_empty() {
                if width > self.top() {
                    return Err(width);
                }
                break;
            }
            at += line.len() + 1;
            lines.push((line[unit_offset(line, indents)..].to_string(), width >= indents));
            if at >= rest.len() {
                break;
            }
        }
        Ok((at, lines))
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
        let mut before = self.history.iter().rev().copied().skip_while(|c| c.is_whitespace()).peekable();
        match before.peek().copied() {
            Some(')') => false,
            Some('}') => true,
            next if is_punctuator(next) => true,
            _ => {
                let word: String = before.take_while(|&c| c.is_ascii_alphanumeric() || c == '_').collect();
                is_keyword(&word.chars().rev().collect::<String>())
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Failure {
    EndOfString,
    Mismatch(char),
}

/// character-parser's `parseUntil`: the bytes from `start` to the first `delimiter` outside brackets, strings and
/// comments, or the failure and the byte where it happened.
fn parse_until(src: &str, delimiter: char, start: usize) -> Result<Range<usize>, (Failure, usize)> {
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

fn is_keyword(word: &str) -> bool {
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
    KEYWORDS.contains(&word)
}

/// Stands in for `is-expression` where Pug only asks whether an attribute value is complete before a space or a
/// comma: balanced, without an open `?` of a conditional, and not ending in an operator or an operator keyword.
fn looks_complete(code: &str) -> bool {
    let code = code.trim();
    if code.is_empty() {
        return false;
    }
    let chars: Vec<char> = code.chars().collect();
    let mut state = CharState::default();
    let mut open_conditionals = 0i32;
    for (i, &c) in chars.iter().enumerate() {
        if !state.is_nesting() {
            let optional = matches!(chars.get(i + 1), Some('.' | '?')) || (i > 0 && chars[i - 1] == '?');
            match c {
                '?' if !optional => open_conditionals += 1,
                ':' => open_conditionals -= 1,
                _ => {}
            }
        }
        if state.parse_char(c).is_err() {
            return false;
        }
    }
    if state.is_nesting() || open_conditionals > 0 {
        return false;
    }
    if code.ends_with("++") || code.ends_with("--") {
        return true;
    }
    if code.ends_with(|c: char| is_punctuator(Some(c)) && !matches!(c, ')' | ']' | '}')) {
        return false;
    }
    let before_word = code.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    let word = &code[before_word.len()..];
    let operand_keyword = is_keyword(word) && word != "this" && word != "super";
    !operand_keyword || before_word.ends_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// pug-lexer 4.1.0's tokens for the same template.
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

    #[test]
    fn attribute_values_end_where_is_expression_would_say() {
        for complete in ["a", "'x'", "f(a, b)", "a ? b : c", "a.new", "x++", "this", "[1, 2]", "`t ${a}`"] {
            assert!(looks_complete(complete), "{complete}");
        }
        for incomplete in ["", "a +", "a ?", "a ? b", "new", "typeof", "(a", "'x", "() =>", "a.", "a ? b :"] {
            assert!(!looks_complete(incomplete), "{incomplete}");
        }
    }

    #[test]
    fn columns_count_utf16_units() {
        let error = lex("p 😀 #{a").unwrap_err();
        assert_eq!((error.line, error.column), (1, 9));
    }
}
