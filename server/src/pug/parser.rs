//! Port of `pug-parser` 6.0.0 (Pug 3.0.4), run as Pug runs it: after `pug-strip-comments` 2.0.0 with Pug's defaults,
//! which drop `//-` comments and their bodies, and with `.pug` implied for a path without an extension, as `pug.compile`
//! gives it. The AST has Pug's nodes, each with the line and column of the token that starts it (1-based, columns in
//! UTF-16 code units). Where it differs from Pug's on purpose:
//! - `#[...]` in text is spliced into the text's nodes, and one whose expression gives several nodes (Pug's `Block`) is
//!   spliced flat: there is no `Block` node;
//! - a mixin call without a block has an empty one rather than none;
//! - `FileReference::path` is the path as written; `implied` adds the `.pug`;
//! - a message Pug builds from half of a surrogate pair (`a(foo!😀)`, `]]]]😀`) has the whole character: a Rust string
//!   can't hold half of one.

use super::lexer::{js_trim, lex, BlockMode, Kind, Pos, Token, TokenType};
use super::{too_deep, Error, ErrorCode, MAX_DEPTH};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::ops::Range;

/// pug-parser's `lib/inline-tags.js`.
const INLINE_TAGS: &[&str] = &[
    "a", "abbr", "acronym", "b", "br", "code", "em", "font", "i", "img", "ins", "kbd", "map", "samp", "small", "span", "strong", "sub",
    "sup",
];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Block {
    pub line: usize,
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub line: usize,
    pub column: usize,
    /// The bytes of the token that starts it.
    pub span: Range<usize>,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Tag(Tag),
    /// `html` for a line that starts with `<`.
    Text {
        val: String,
        html: bool,
    },
    Code(Code),
    Conditional(Conditional),
    While {
        test: String,
        block: Block,
    },
    /// `alternate` is the `else` block.
    Each {
        obj: String,
        val: String,
        key: Option<String>,
        block: Block,
        alternate: Option<Block>,
    },
    /// `each val of obj`, which takes no `else`.
    EachOf {
        obj: String,
        val: String,
        block: Block,
    },
    /// Its block holds `When` nodes.
    Case {
        expr: String,
        block: Block,
    },
    /// `expr` is `default` for the default branch; a `when` with no block falls through.
    When {
        expr: String,
        block: Option<Block>,
    },
    MixinDefinition {
        name: String,
        args: Option<String>,
        block: Block,
    },
    /// `+name(args)`: the tag's name is the mixin's, or the JavaScript of `+#{expr}`, interpolated; its block is empty when
    /// none is given.
    MixinCall {
        args: Option<String>,
        tag: Tag,
    },
    /// `block` inside a mixin.
    MixinBlock,
    YieldBlock,
    NamedBlock {
        name: String,
        mode: BlockMode,
        block: Block,
    },
    Extends {
        file: FileReference,
    },
    /// A `.pug` (or `.jade`) file, or a path without an extension, without filters.
    Include {
        file: FileReference,
        block: Block,
    },
    RawInclude {
        file: FileReference,
        filters: Vec<IncludeFilter>,
    },
    Filter {
        name: String,
        attrs: Vec<Attr>,
        block: Block,
    },
    Comment {
        val: String,
        buffer: bool,
    },
    BlockComment {
        val: String,
        buffer: bool,
        block: Block,
    },
    Doctype(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tag {
    /// The JavaScript of `#{expr}` when `interpolated`.
    pub name: String,
    pub interpolated: bool,
    pub attrs: Vec<Attr>,
    /// `&attributes(...)`.
    pub attribute_blocks: Vec<AttributeBlock>,
    pub block: Block,
    pub self_closing: bool,
    /// `tag.`: the block is text.
    pub text_only: bool,
    pub inline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub name: String,
    pub val: AttrValue,
    pub must_escape: bool,
    pub line: usize,
    pub column: usize,
    /// From the name, or the quote before it, to the end of the value; a `#id` or `.class` with its `#` or `.`.
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValue {
    Boolean,
    /// The name of a `#id` or `.class`.
    Shorthand(String),
    Expr(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeBlock {
    pub val: String,
    pub line: usize,
    pub column: usize,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Code {
    pub val: String,
    pub buffer: bool,
    pub must_escape: bool,
    /// In a line of text or after a tag, rather than a line of its own.
    pub inline: bool,
    pub block: Option<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conditional {
    pub test: String,
    pub consequent: Block,
    pub alternate: Option<Alternate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Alternate {
    /// An `else if`, a `Conditional` node.
    ElseIf(Box<Node>),
    Else(Block),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReference {
    pub path: String,
    pub line: usize,
    pub column: usize,
    /// The bytes of `path`.
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeFilter {
    pub name: String,
    pub attrs: Vec<Attr>,
    pub line: usize,
    pub column: usize,
    pub span: Range<usize>,
}

/// Pug's AST for `text`, or the first error its lexer or parser raises.
pub fn parse(text: &str) -> Result<Block, Error> {
    parse_tokens(lex(text)?)
}

/// Pug's AST for the tokens `lex` gave, or the first error its parser raises.
pub fn parse_tokens(tokens: Vec<Token>) -> Result<Block, Error> {
    let tokens = strip_comments(tokens);
    let start = Pos { line: 1, column: 1 };
    let eos = tokens.last().cloned().unwrap_or(Token { kind: Kind::Eos, start, end: start, span: 0..0 });
    Parser { tokens: tokens.into(), eos, in_mixin: 0, depth: 0 }.parse()
}

/// Node's `path.extname(path) != ""`, which on Windows also splits at `\` and skips a drive (`C:`).
pub(crate) fn has_extension(path: &str, windows: bool) -> bool {
    let is_separator = |c: char| c == '/' || (windows && c == '\\');
    let path = match path.as_bytes() {
        [drive, b':', ..] if windows && drive.is_ascii_alphabetic() => &path[2..],
        _ => path,
    };
    let name = path.trim_end_matches(is_separator).rsplit(is_separator).next().unwrap_or_default();
    name != ".." && name.rfind('.').is_some_and(|dot| dot > 0)
}

/// The path Pug reads for `path`: `pug.compile` adds `.pug` when it has no extension, so `include row` never finds
/// pug-rails' `row.jst.pug`.
pub fn implied(path: &str) -> Cow<'_, str> {
    if has_extension(path, cfg!(windows)) {
        Cow::Borrowed(path)
    } else {
        Cow::Owned(format!("{path}.pug"))
    }
}

/// `pug-strip-comments` with `stripUnbuffered` (`//-` and its body go) and without `stripBuffered`.
fn strip_comments(tokens: Vec<Token>) -> Vec<Token> {
    let mut in_comment = false;
    let mut in_pipeless_text = false;
    tokens
        .into_iter()
        .filter(|token| match &token.kind {
            Kind::Comment { buffer, .. } => {
                in_comment = !buffer;
                !in_comment
            }
            Kind::StartPipelessText if in_comment => {
                in_pipeless_text = true;
                false
            }
            Kind::EndPipelessText if in_comment => {
                in_pipeless_text = false;
                in_comment = false;
                false
            }
            Kind::StartPipelessText | Kind::EndPipelessText => true,
            Kind::Text(_) => !in_comment,
            _ if in_pipeless_text => false,
            _ => {
                in_comment = false;
                true
            }
        })
        .collect()
}

/// What `parseExpr` returns: a node, or several in a block that the caller splices in.
enum Expr {
    Node(Node),
    Block(Block),
}

impl Expr {
    fn into_block(self, line: usize) -> Block {
        match self {
            Expr::Node(node) => Block { line, nodes: vec![node] },
            Expr::Block(block) => block,
        }
    }

    fn append_to(self, nodes: &mut Vec<Node>) {
        match self {
            Expr::Node(node) => nodes.push(node),
            Expr::Block(block) => nodes.extend(block.nodes),
        }
    }
}

/// A token without its kind; a node takes its position and bytes from the token that starts it.
struct At {
    start: Pos,
    end: Pos,
    span: Range<usize>,
}

fn split(token: Token) -> (Kind, At) {
    let Token { kind, start, end, span } = token;
    (kind, At { start, end, span })
}

fn joined(kind: Kind, at: At) -> Token {
    Token { kind, start: at.start, end: at.end, span: at.span }
}

fn node(at: At, kind: NodeKind) -> Node {
    Node { line: at.start.line, column: at.start.column, span: at.span, kind }
}

fn text_node(at: At, val: String) -> Node {
    node(at, NodeKind::Text { val, html: false })
}

fn interpolated_code(at: At, val: String, must_escape: bool) -> Node {
    node(at, NodeKind::Code(Code { val, buffer: true, must_escape, inline: true, block: None }))
}

fn empty_block(line: usize) -> Block {
    Block { line, nodes: Vec::new() }
}

fn error_at(start: Pos, code: ErrorCode, message: impl Into<String>) -> Error {
    Error { code, message: message.into(), line: start.line, column: start.column }
}

fn unexpected(expected: TokenType, got: &Token) -> Error {
    let message = format!("expected \"{}\", but got \"{}\"", expected.name(), got.kind.name());
    error_at(got.start, ErrorCode::InvalidToken, message)
}

struct Parser {
    tokens: VecDeque<Token>,
    /// The last token, which `peek` and `advance` return once the others are used up.
    eos: Token,
    in_mixin: usize,
    /// How many levels deep the node being parsed is.
    depth: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        self.tokens.front().unwrap_or(&self.eos)
    }

    fn peek_is(&self, ty: TokenType) -> bool {
        self.peek().kind.ty() == ty
    }

    fn advance(&mut self) -> Token {
        self.tokens.pop_front().unwrap_or_else(|| self.eos.clone())
    }

    /// Puts back a token `advance` gave that what is being parsed doesn't take.
    fn put_back(&mut self, kind: Kind, at: At) {
        self.tokens.push_front(joined(kind, at));
    }

    fn expect(&mut self, ty: TokenType) -> Result<Token, Error> {
        if self.peek_is(ty) {
            return Ok(self.advance());
        }
        Err(unexpected(ty, self.peek()))
    }

    fn accept(&mut self, ty: TokenType) -> Option<Token> {
        self.peek_is(ty).then(|| self.advance())
    }

    fn parse(mut self) -> Result<Block, Error> {
        let mut block = empty_block(0);
        while !self.peek_is(TokenType::Eos) {
            match self.peek().kind {
                Kind::Newline => {
                    self.advance();
                }
                Kind::TextHtml(_) => {
                    let nodes = self.parse_text_html()?;
                    block.nodes.extend(nodes);
                }
                _ => {
                    if let Some(expr) = self.parse_expr()? {
                        expr.append_to(&mut block.nodes);
                    }
                }
            }
        }
        Ok(block)
    }

    /// Goes a level deeper in the tree, which the node starting at `start` starts; refused past `MAX_DEPTH`.
    fn descend(&mut self, start: Pos) -> Result<(), Error> {
        if self.depth == MAX_DEPTH {
            return Err(too_deep(start.line, start.column));
        }
        self.depth += 1;
        Ok(())
    }

    /// `parse`, a level deeper in the tree, for the node starting at `start`.
    fn nested_at<T>(&mut self, start: Pos, parse: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error> {
        self.descend(start)?;
        let parsed = parse(self);
        self.depth -= 1;
        parsed
    }

    /// `parse`, a level deeper in the tree, for the node the next token starts.
    fn nested<T>(&mut self, parse: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error> {
        self.nested_at(self.peek().start, parse)
    }

    /// `parseExpr` where Pug reads the result's `type`: nothing there (a `.` without text) makes Pug itself throw.
    fn parse_required_expr(&mut self) -> Result<Expr, Error> {
        let Pos { line, column } = self.peek().start;
        self.parse_expr()?.ok_or_else(|| {
            let message = "Cannot read properties of undefined (reading 'type')".to_string();
            Error { code: ErrorCode::TypeError, message, line, column }
        })
    }

    fn parse_expr(&mut self) -> Result<Option<Expr>, Error> {
        self.nested(Self::expr)
    }

    fn expr(&mut self) -> Result<Option<Expr>, Error> {
        let one = |node: Node| Ok(Some(Expr::Node(node)));
        match self.peek().kind {
            Kind::Text(_) | Kind::InterpolatedCode { .. } | Kind::StartPugInterpolation => return self.parse_text(true),
            Kind::TextHtml(_) => {
                let line = self.peek().start.line;
                return Ok(Some(Expr::Block(Block { line, nodes: self.parse_text_html()? })));
            }
            Kind::Id(_) | Kind::Class(_) => {
                let token = self.peek();
                let div = Token { kind: Kind::Tag("div".to_string()), start: token.start, end: token.end, span: token.span.clone() };
                self.tokens.push_front(div);
            }
            _ => {}
        }
        let (kind, at) = split(self.advance());
        match kind {
            Kind::Tag(name) => one(self.parse_tag(at, name)?),
            Kind::Mixin { name, args } => one(self.parse_mixin(at, name, args)?),
            Kind::Block { name, mode } => one(self.parse_named_block(at, name, mode)?),
            Kind::MixinBlock => one(self.parse_mixin_block(at)?),
            Kind::Case(expr) => one(self.parse_case(at, expr)?),
            Kind::Extends => one(self.parse_extends(at)?),
            Kind::Include => one(self.parse_include(at)?),
            Kind::Doctype(val) => one(node(at, NodeKind::Doctype(val))),
            Kind::Filter(name) => one(self.parse_filter(at, name)?),
            Kind::Comment { val, buffer } => one(self.parse_comment(at, val, buffer)?),
            Kind::Dot => Ok(self.parse_text_block()?.map(Expr::Block)),
            Kind::Each { val, key, code } => one(self.parse_each(at, val, key, code)?),
            Kind::EachOf { val, code } => one(self.parse_each_of(at, val, code)?),
            Kind::Code { val, must_escape, buffer } => {
                one(self.parse_code(at, Code { val, buffer, must_escape, inline: false, block: None })?)
            }
            Kind::BlockCode => one(self.parse_block_code(at)?),
            Kind::If(test) => one(self.parse_conditional(at, test)?),
            Kind::While(test) => one(self.parse_while(at, test)?),
            Kind::Call { name, args } => one(self.parse_call(at, name, args)?),
            Kind::Interpolation(expr) => one(self.parse_interpolation(at, expr)?),
            Kind::Yield => one(node(at, NodeKind::YieldBlock)),
            other => Err(error_at(at.start, ErrorCode::InvalidToken, format!("unexpected token \"{}\"", other.name()))),
        }
    }

    /// Text and the code and tags interpolated in it, over several lines when `block` (after `|`).
    fn parse_text(&mut self, block: bool) -> Result<Option<Expr>, Error> {
        let line = self.peek().start.line;
        let mut nodes = Vec::new();
        loop {
            let (kind, at) = split(self.advance());
            match kind {
                Kind::Text(val) => nodes.push(text_node(at, val)),
                Kind::InterpolatedCode { val, must_escape } => nodes.push(interpolated_code(at, val, must_escape)),
                Kind::Newline if block => {
                    if matches!(self.peek().kind, Kind::Text(_) | Kind::InterpolatedCode { .. }) {
                        nodes.push(text_node(at, "\n".to_string()));
                    }
                }
                Kind::StartPugInterpolation => {
                    if let Some(expr) = self.parse_expr()? {
                        expr.append_to(&mut nodes);
                    }
                    self.expect(TokenType::EndPugInterpolation)?;
                }
                kind => {
                    self.put_back(kind, at);
                    break;
                }
            }
        }
        Ok(match nodes.len() {
            0 => None,
            1 => nodes.pop().map(Expr::Node),
            _ => Some(Expr::Block(Block { line, nodes })),
        })
    }

    /// Lines starting with `<`: consecutive ones, and those nested in them, join into one text node.
    fn parse_text_html(&mut self) -> Result<Vec<Node>, Error> {
        let mut nodes: Vec<Node> = Vec::new();
        let mut current: Option<usize> = None;
        let add = |nodes: &mut Vec<Node>, current: &mut Option<usize>, node: Node| match (&node.kind, *current) {
            (NodeKind::Text { val, html: true }, Some(at)) => {
                if let NodeKind::Text { val: joined, .. } = &mut nodes[at].kind {
                    joined.push('\n');
                    joined.push_str(val);
                }
            }
            (NodeKind::Text { html: true, .. }, None) => {
                nodes.push(node);
                *current = Some(nodes.len() - 1);
            }
            _ => {
                *current = None;
                nodes.push(node);
            }
        };
        loop {
            if matches!(self.peek().kind, Kind::Indent(_)) {
                for nested in self.nested(Self::block)?.nodes {
                    add(&mut nodes, &mut current, nested);
                }
                continue;
            }
            let (kind, at) = split(self.advance());
            match kind {
                Kind::TextHtml(val) => add(&mut nodes, &mut current, node(at, NodeKind::Text { val, html: true })),
                Kind::Code { val, must_escape, buffer } => {
                    current = None;
                    nodes.push(self.parse_code(at, Code { val, buffer, must_escape, inline: true, block: None })?);
                }
                Kind::Newline => {}
                kind => {
                    self.put_back(kind, at);
                    return Ok(nodes);
                }
            }
        }
    }

    /// `: expr` on the same line, or an indented block.
    fn parse_block_expansion(&mut self) -> Result<Block, Error> {
        match self.accept(TokenType::Colon) {
            Some(colon) => Ok(self.parse_required_expr()?.into_block(colon.start.line)),
            None => self.block(),
        }
    }

    fn parse_case(&mut self, at: At, expr: String) -> Result<Node, Error> {
        let mut block = empty_block(at.start.line + 1);
        self.expect(TokenType::Indent)?;
        while !self.peek_is(TokenType::Outdent) {
            let (kind, when) = split(self.advance());
            match kind {
                Kind::Comment { .. } | Kind::Newline => {}
                // A `when` is a node of its own, a level below the `case`.
                Kind::When(expr) => block.nodes.push(self.nested_at(when.start, |parser| parser.parse_when(when, expr))?),
                Kind::Default => block.nodes.push(self.nested_at(when.start, |parser| {
                    let kind = NodeKind::When { expr: "default".to_string(), block: Some(parser.parse_block_expansion()?) };
                    Ok(node(when, kind))
                })?),
                other => {
                    let message = format!("Unexpected token \"{}\", expected \"when\", \"default\" or \"newline\"", other.name());
                    return Err(error_at(when.start, ErrorCode::InvalidToken, message));
                }
            }
        }
        self.expect(TokenType::Outdent)?;
        Ok(node(at, NodeKind::Case { expr, block }))
    }

    fn parse_when(&mut self, at: At, expr: String) -> Result<Node, Error> {
        let block = if self.peek_is(TokenType::Newline) { None } else { Some(self.parse_block_expansion()?) };
        Ok(node(at, NodeKind::When { expr, block }))
    }

    /// The code of a `code` token, with the block a `-` line has under it.
    fn parse_code(&mut self, at: At, mut code: Code) -> Result<Node, Error> {
        if !code.inline && self.peek_is(TokenType::Indent) {
            if code.buffer {
                let message = "Buffered code cannot have a block attached to it";
                return Err(error_at(self.peek().start, ErrorCode::BlockInBufferedCode, message));
            }
            code.block = Some(self.block()?);
        }
        Ok(node(at, NodeKind::Code(code)))
    }

    fn parse_conditional(&mut self, at: At, test: String) -> Result<Node, Error> {
        let consequent = self.optional_block(at.start.line)?;
        let mut else_ifs = Vec::new();
        let mut alternate = None;
        // Each `else if` is a level deeper in the tree than the one before.
        let depth = self.depth;
        loop {
            let (kind, next) = split(self.advance());
            match kind {
                Kind::Newline => {}
                Kind::ElseIf(test) => {
                    self.descend(next.start)?;
                    let consequent = self.optional_block(next.start.line)?;
                    else_ifs.push((next, test, consequent));
                }
                Kind::Else => {
                    if self.peek_is(TokenType::Indent) {
                        alternate = Some(Alternate::Else(self.block()?));
                    }
                    break;
                }
                kind => {
                    self.put_back(kind, next);
                    break;
                }
            }
        }
        self.depth = depth;
        for (else_if, test, consequent) in else_ifs.into_iter().rev() {
            let conditional = Conditional { test, consequent, alternate };
            alternate = Some(Alternate::ElseIf(Box::new(node(else_if, NodeKind::Conditional(conditional)))));
        }
        Ok(node(at, NodeKind::Conditional(Conditional { test, consequent, alternate })))
    }

    /// The indented block that follows, or an empty one at `line`.
    fn optional_block(&mut self, line: usize) -> Result<Block, Error> {
        if self.peek_is(TokenType::Indent) {
            self.block()
        } else {
            Ok(empty_block(line))
        }
    }

    fn parse_while(&mut self, at: At, test: String) -> Result<Node, Error> {
        let block = self.optional_block(at.start.line)?;
        Ok(node(at, NodeKind::While { test, block }))
    }

    /// `-` alone on a line, with its code in the indented text below.
    fn parse_block_code(&mut self, at: At) -> Result<Node, Error> {
        let mut val = String::new();
        if self.accept(TokenType::StartPipelessText).is_some() {
            while !self.peek_is(TokenType::EndPipelessText) {
                let (kind, line) = split(self.advance());
                match kind {
                    Kind::Text(text) => val.push_str(&text),
                    Kind::Newline => val.push('\n'),
                    other => {
                        let message = format!("Unexpected token type: {}", other.name());
                        return Err(error_at(line.start, ErrorCode::InvalidToken, message));
                    }
                }
            }
            self.advance();
        }
        let code = Code { val, buffer: false, must_escape: false, inline: false, block: None };
        Ok(node(at, NodeKind::Code(code)))
    }

    fn parse_comment(&mut self, at: At, val: String, buffer: bool) -> Result<Node, Error> {
        Ok(match self.parse_text_block()? {
            Some(block) => node(at, NodeKind::BlockComment { val, buffer, block }),
            None => node(at, NodeKind::Comment { val, buffer }),
        })
    }

    /// `(...)` after a filter, when there is one.
    fn filter_attrs(&mut self) -> Result<Vec<Attr>, Error> {
        match self.accept(TokenType::StartAttributes) {
            Some(_) => self.attrs(None),
            None => Ok(Vec::new()),
        }
    }

    fn parse_filter(&mut self, at: At, name: String) -> Result<Node, Error> {
        let attrs = self.filter_attrs()?;
        let (kind, next) = split(self.advance());
        let block = match kind {
            Kind::Text(val) => Block { line: next.start.line, nodes: vec![text_node(next, val)] },
            kind => {
                let nested = matches!(kind, Kind::Filter(_));
                self.put_back(kind, next);
                if nested {
                    self.parse_required_expr()?.into_block(at.start.line)
                } else {
                    self.parse_text_block()?.unwrap_or_else(|| empty_block(at.start.line))
                }
            }
        };
        Ok(node(at, NodeKind::Filter { name, attrs, block }))
    }

    fn parse_each(&mut self, at: At, val: String, key: Option<String>, obj: String) -> Result<Node, Error> {
        let block = self.block()?;
        let alternate = match self.accept(TokenType::Else) {
            Some(_) => Some(self.block()?),
            None => None,
        };
        Ok(node(at, NodeKind::Each { obj, val, key, block, alternate }))
    }

    fn parse_each_of(&mut self, at: At, val: String, obj: String) -> Result<Node, Error> {
        let block = self.block()?;
        Ok(node(at, NodeKind::EachOf { obj, val, block }))
    }

    fn file_reference(&mut self) -> Result<FileReference, Error> {
        let token = self.advance();
        match token.kind {
            Kind::Path(path) => Ok(FileReference { path, line: token.start.line, column: token.start.column, span: token.span }),
            _ => Err(unexpected(TokenType::Path, &token)),
        }
    }

    fn parse_extends(&mut self, at: At) -> Result<Node, Error> {
        let file = self.file_reference()?;
        Ok(node(at, NodeKind::Extends { file }))
    }

    fn parse_named_block(&mut self, at: At, name: String, mode: BlockMode) -> Result<Node, Error> {
        let block = self.optional_block(at.start.line)?;
        Ok(node(at, NodeKind::NamedBlock { name: js_trim(&name).to_string(), mode, block }))
    }

    fn parse_mixin_block(&mut self, at: At) -> Result<Node, Error> {
        if self.in_mixin == 0 {
            let message = "Anonymous blocks are not allowed unless they are part of a mixin.";
            return Err(error_at(at.start, ErrorCode::BlockOutsideMixin, message));
        }
        Ok(node(at, NodeKind::MixinBlock))
    }

    fn parse_include(&mut self, at: At) -> Result<Node, Error> {
        let mut filters = Vec::new();
        loop {
            let (kind, filter) = split(self.advance());
            let Kind::Filter(name) = kind else {
                self.put_back(kind, filter);
                break;
            };
            let attrs = self.filter_attrs()?;
            let (line, column) = (filter.start.line, filter.start.column);
            filters.push(IncludeFilter { name, attrs, line, column, span: filter.span });
        }
        let file = self.file_reference()?;
        let read = implied(&file.path);
        if filters.is_empty() && (read.ends_with(".pug") || read.ends_with(".jade")) {
            let block = self.optional_block(at.start.line)?;
            return Ok(node(at, NodeKind::Include { file, block }));
        }
        if self.peek_is(TokenType::Indent) {
            return Err(error_at(self.peek().start, ErrorCode::RawIncludeBlock, "Raw inclusion cannot contain a block"));
        }
        Ok(node(at, NodeKind::RawInclude { file, filters }))
    }

    fn parse_call(&mut self, at: At, name: String, args: Option<String>) -> Result<Node, Error> {
        let (name, interpolated) = match name.strip_prefix("#{").and_then(|expr| expr.strip_suffix('}')) {
            Some(expr) => (expr.to_string(), true),
            None => (name, false),
        };
        let mut tag = Tag { name, interpolated, block: empty_block(at.start.line), ..Tag::default() };
        self.tag(&mut tag, at.start.line, false)?;
        Ok(node(at, NodeKind::MixinCall { args, tag }))
    }

    fn parse_mixin(&mut self, at: At, name: String, args: Option<String>) -> Result<Node, Error> {
        if !self.peek_is(TokenType::Indent) {
            return Err(error_at(at.start, ErrorCode::MixinWithoutBody, format!("Mixin {name} declared without body")));
        }
        self.in_mixin += 1;
        let block = self.block()?;
        self.in_mixin -= 1;
        Ok(node(at, NodeKind::MixinDefinition { name, args, block }))
    }

    /// The text lines after `.`, `//` or a filter.
    fn parse_text_block(&mut self) -> Result<Option<Block>, Error> {
        let Some(start) = self.accept(TokenType::StartPipelessText) else {
            return Ok(None);
        };
        let mut block = empty_block(start.start.line);
        while !self.peek_is(TokenType::EndPipelessText) {
            let (kind, at) = split(self.advance());
            match kind {
                Kind::Text(val) => block.nodes.push(text_node(at, val)),
                Kind::Newline => block.nodes.push(text_node(at, "\n".to_string())),
                Kind::StartPugInterpolation => {
                    if let Some(expr) = self.parse_expr()? {
                        expr.append_to(&mut block.nodes);
                    }
                    self.expect(TokenType::EndPugInterpolation)?;
                }
                Kind::InterpolatedCode { val, must_escape } => block.nodes.push(interpolated_code(at, val, must_escape)),
                other => {
                    let message = format!("Unexpected token type: {}", other.name());
                    return Err(error_at(at.start, ErrorCode::InvalidToken, message));
                }
            }
        }
        self.advance();
        Ok(Some(block))
    }

    /// An indented block, up to its outdent.
    fn block(&mut self) -> Result<Block, Error> {
        let token = self.expect(TokenType::Indent)?;
        let mut block = empty_block(token.start.line);
        while !self.peek_is(TokenType::Outdent) {
            match self.peek().kind {
                Kind::Newline => {
                    self.advance();
                }
                Kind::TextHtml(_) => {
                    let nodes = self.parse_text_html()?;
                    block.nodes.extend(nodes);
                }
                _ => self.parse_required_expr()?.append_to(&mut block.nodes),
            }
        }
        self.expect(TokenType::Outdent)?;
        Ok(block)
    }

    fn parse_interpolation(&mut self, at: At, expr: String) -> Result<Node, Error> {
        let mut tag = Tag { name: expr, interpolated: true, block: empty_block(at.start.line), ..Tag::default() };
        self.tag(&mut tag, at.start.line, true)?;
        Ok(node(at, NodeKind::Tag(tag)))
    }

    fn parse_tag(&mut self, at: At, name: String) -> Result<Node, Error> {
        let inline = INLINE_TAGS.contains(&name.as_str());
        let mut tag = Tag { name, inline, block: empty_block(at.start.line), ..Tag::default() };
        self.tag(&mut tag, at.start.line, true)?;
        Ok(node(at, NodeKind::Tag(tag)))
    }

    /// What follows a tag, interpolated tag or mixin call on line `line`: `#id`, `.class`, `(attributes)` and
    /// `&attributes(...)`, then `.`, text, `= code`, `: expr` or `/`, then the indented block.
    fn tag(&mut self, tag: &mut Tag, line: usize, self_closing_allowed: bool) -> Result<(), Error> {
        let mut names = Vec::new();
        let shorthand = |name: &str, val: String, at: At| Attr {
            name: name.to_string(),
            val: AttrValue::Shorthand(val),
            must_escape: false,
            line: at.start.line,
            column: at.start.column,
            span: at.span,
        };
        loop {
            let (kind, at) = split(self.advance());
            match kind {
                Kind::Id(val) => {
                    if names.iter().any(|n| n == "id") {
                        return Err(error_at(at.start, ErrorCode::DuplicateId, "Duplicate attribute \"id\" is not allowed."));
                    }
                    names.push("id".to_string());
                    tag.attrs.push(shorthand("id", val, at));
                }
                Kind::Class(val) => tag.attrs.push(shorthand("class", val, at)),
                Kind::StartAttributes => {
                    let attrs = self.attrs(Some(&mut names))?;
                    tag.attrs.extend(attrs);
                }
                Kind::AndAttributes(val) => {
                    let (line, column) = (at.start.line, at.start.column);
                    tag.attribute_blocks.push(AttributeBlock { val, line, column, span: at.span });
                }
                kind => {
                    self.put_back(kind, at);
                    break;
                }
            }
        }
        if self.accept(TokenType::Dot).is_some() {
            tag.text_only = true;
        }
        match self.peek().kind {
            Kind::Text(_) | Kind::InterpolatedCode { .. } => {
                if let Some(text) = self.parse_text(false)? {
                    text.append_to(&mut tag.block.nodes);
                }
            }
            Kind::Newline | Kind::Indent(_) | Kind::Outdent | Kind::Eos | Kind::StartPipelessText | Kind::EndPugInterpolation => {}
            _ => {
                let (kind, at) = split(self.advance());
                match kind {
                    Kind::Code { val, must_escape, buffer } => {
                        let code = self.parse_code(at, Code { val, buffer, must_escape, inline: true, block: None })?;
                        tag.block.nodes.push(code);
                    }
                    Kind::Colon => tag.block = self.parse_required_expr()?.into_block(line),
                    Kind::Slash if self_closing_allowed => tag.self_closing = true,
                    other => {
                        let slash = if self_closing_allowed { ", `slash`" } else { "" };
                        let message = format!(
                            "Unexpected token `{}` expected `text`, `interpolated-code`, `code`, `:`{slash}, `newline` or `eos`",
                            other.name()
                        );
                        return Err(error_at(at.start, ErrorCode::InvalidToken, message));
                    }
                }
            }
        }
        while self.accept(TokenType::Newline).is_some() {}
        if tag.text_only {
            tag.block = self.parse_text_block()?.unwrap_or_else(|| empty_block(line));
        } else if self.peek_is(TokenType::Indent) {
            let block = self.block()?;
            tag.block.nodes.extend(block.nodes);
        }
        Ok(())
    }

    /// The attributes after `(`, and the `)`; with `names`, the attribute names already on the tag, a repeated one other
    /// than `class` is an error.
    fn attrs(&mut self, mut names: Option<&mut Vec<String>>) -> Result<Vec<Attr>, Error> {
        let mut attrs = Vec::new();
        loop {
            let (kind, at) = split(self.advance());
            let Kind::Attribute { name, val, must_escape } = kind else {
                self.put_back(kind, at);
                break;
            };
            if name != "class" {
                if let Some(names) = names.as_deref_mut() {
                    if names.contains(&name) {
                        let message = format!("Duplicate attribute \"{name}\" is not allowed.");
                        return Err(error_at(at.start, ErrorCode::DuplicateAttribute, message));
                    }
                    names.push(name.clone());
                }
            }
            let val = val.map_or(AttrValue::Boolean, AttrValue::Expr);
            attrs.push(Attr { name, val, must_escape, line: at.start.line, column: at.start.column, span: at.span });
        }
        self.expect(TokenType::EndAttributes)?;
        Ok(attrs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn json(s: &str) -> String {
        serde_json::to_string(s).unwrap_or_default()
    }

    fn attr(a: &Attr) -> String {
        let val = match &a.val {
            AttrValue::Boolean => "true".to_string(),
            AttrValue::Shorthand(name) => format!("'{name}'"),
            AttrValue::Expr(val) => val.clone(),
        };
        format!("{}{}{val}@{}:{}", a.name, if a.must_escape { "=" } else { "!=" }, a.line, a.column)
    }

    fn attr_list(attrs: &[Attr], blocks: &[AttributeBlock]) -> String {
        let blocks = blocks.iter().map(|b| format!("&({})@{}:{}", b.val, b.line, b.column));
        attrs.iter().map(attr).chain(blocks).collect::<Vec<_>>().join(" ")
    }

    /// The outline `test/oracle/pug.js` writes from Pug's AST.
    fn outline(block: &Block, out: &mut Vec<String>) {
        block.nodes.iter().for_each(|node| outline_node(node, out));
    }

    fn outline_node(node: &Node, out: &mut Vec<String>) {
        let at = format!("@{}:{}", node.line, node.column);
        let file = |f: &FileReference| format!("{}@{}:{}", implied(&f.path), f.line, f.column);
        match &node.kind {
            NodeKind::NamedBlock { name, mode, block } => {
                let mode = match mode {
                    BlockMode::Replace => "replace",
                    BlockMode::Append => "append",
                    BlockMode::Prepend => "prepend",
                };
                out.push(format!("block {mode} {name}{at}"));
                outline(block, out);
            }
            NodeKind::Tag(tag) => {
                let name = if tag.interpolated { format!("#{{{}}}", tag.name) } else { tag.name.clone() };
                let slash = if tag.self_closing { "/" } else { "" };
                let dot = if tag.text_only { "." } else { "" };
                out.push(format!("{name}{slash}{dot} ({}){at}", attr_list(&tag.attrs, &tag.attribute_blocks)));
                outline(&tag.block, out);
            }
            NodeKind::MixinCall { args, tag } => {
                let args = args.as_deref().unwrap_or("");
                let name = if tag.interpolated { format!("#{{{}}}", tag.name) } else { tag.name.clone() };
                out.push(format!("+{name}({args}) ({}){at}", attr_list(&tag.attrs, &tag.attribute_blocks)));
                outline(&tag.block, out);
            }
            NodeKind::MixinDefinition { name, args, block } => {
                out.push(format!("mixin {name}({}){at}", args.as_deref().unwrap_or("")));
                outline(block, out);
            }
            NodeKind::Text { val, html } => out.push(format!("text{} {}{at}", if *html { " html" } else { "" }, json(val))),
            NodeKind::Code(code) => {
                let flag = match (code.buffer, code.must_escape) {
                    (false, _) => "-",
                    (true, true) => "=",
                    (true, false) => "!=",
                };
                out.push(format!("code{flag}{} {}{at}", if code.inline { " inline" } else { "" }, json(&code.val)));
                if let Some(block) = &code.block {
                    outline(block, out);
                }
            }
            NodeKind::Conditional(conditional) => {
                out.push(format!("if {}{at}", conditional.test));
                outline(&conditional.consequent, out);
                match &conditional.alternate {
                    Some(Alternate::ElseIf(node)) => outline_node(node, out),
                    Some(Alternate::Else(block)) => {
                        out.push(format!("else@{}", block.line));
                        outline(block, out);
                    }
                    None => {}
                }
            }
            NodeKind::While { test, block } => {
                out.push(format!("while {test}{at}"));
                outline(block, out);
            }
            NodeKind::Each { obj, val, key, block, alternate } => {
                out.push(format!("each {val},{} in {obj}{at}", key.as_deref().unwrap_or("")));
                outline(block, out);
                if let Some(alternate) = alternate {
                    out.push(format!("else@{}", alternate.line));
                    outline(alternate, out);
                }
            }
            NodeKind::EachOf { obj, val, block } => {
                out.push(format!("each {val} of {obj}{at}"));
                outline(block, out);
            }
            NodeKind::Case { expr, block } => {
                out.push(format!("case {expr}{at}"));
                outline(block, out);
            }
            NodeKind::When { expr, block } => {
                out.push(format!("when {expr}{at}"));
                if let Some(block) = block {
                    outline(block, out);
                }
            }
            NodeKind::MixinBlock => out.push(format!("mixin-block{at}")),
            NodeKind::YieldBlock => out.push(format!("yield{at}")),
            NodeKind::Extends { file: f } => out.push(format!("extends {}{at}", file(f))),
            NodeKind::Include { file: f, block } => {
                out.push(format!("include {}{at}", file(f)));
                outline(block, out);
            }
            NodeKind::RawInclude { file: f, filters } => {
                let filters: String =
                    filters.iter().map(|x| format!(":{}({})@{}:{}", x.name, attr_list(&x.attrs, &[]), x.line, x.column)).collect();
                out.push(format!("include{filters} {}{at}", file(f)));
            }
            NodeKind::Filter { name, attrs, block } => {
                out.push(format!(":{name}({}){at}", attr_list(attrs, &[])));
                outline(block, out);
            }
            NodeKind::Comment { val, buffer } => out.push(format!("//{} {}{at}", if *buffer { "" } else { "-" }, json(val))),
            NodeKind::BlockComment { val, buffer, block } => {
                out.push(format!("//{} {}{at}", if *buffer { "" } else { "-" }, json(val)));
                outline(block, out);
            }
            NodeKind::Doctype(val) => out.push(format!("doctype {val}{at}")),
        }
    }

    /// How ours differs from what the oracle recorded for `case` (`{source, error, outline}`), if it does.
    fn disagreement(case: &Value) -> Option<String> {
        let source = case["source"].as_str().unwrap_or_default();
        let expected = &case["error"];
        let differs = match (parse(source), expected) {
            // JavaScript isn't validated: past Pug's syntax error (a lexer error) ours goes on, to no error, a parser
            // error, or a later lexer error.
            (Ok(_), _) if expected["code"] == "PUG:SYNTAX_ERROR" => None,
            (Err(error), _) if expected["code"] == "PUG:SYNTAX_ERROR" => {
                let at = |line: &Value, column: &Value| (line.as_u64().unwrap_or(0), column.as_u64().unwrap_or(0));
                let later = (error.line as u64, error.column as u64) > at(&expected["line"], &expected["column"]);
                (lex(source).is_err() && !later).then(|| format!("lexer error before Pug's syntax error {expected}: {error:?}"))
            }
            (Ok(block), Value::Null) => {
                let mut got = Vec::new();
                outline(&block, &mut got);
                let want: Vec<&str> = case["outline"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                (got != want).then(|| format!("outline\n    pug:  {want:?}\n    ours: {got:?}"))
            }
            (Err(error), Value::Object(_)) => {
                let same = expected["code"] == error.code.as_str()
                    && expected["message"] == error.message.as_str()
                    && (expected["line"].is_null() || (expected["line"] == error.line && expected["column"] == error.column));
                (!same).then(|| format!("error\n    pug:  {expected}\n    ours: {error:?}"))
            }
            (Ok(_), _) => Some(format!("pug: {expected}, ours: no error")),
            (Err(error), _) => Some(format!("pug: no error, ours: {error:?}")),
        };
        differs.map(|d| format!("{} {}: {d}", case["label"].as_str().unwrap_or_default(), json(source)))
    }

    fn error_of(text: &str) -> (ErrorCode, String, usize, usize) {
        let error = parse(text).unwrap_err();
        (error.code, error.message, error.line, error.column)
    }

    /// Each code Pug 3.0.4's lexer and parser raise, with its message and position as Pug reports them.
    #[test]
    fn errors_at_pug_positions() {
        let class_message = "\" x\" is not a valid class name.  Class names can only contain \"_\", \"-\", a-z and 0-9, and must contain at least one of \"_\", or a-z";
        let dashed_each =
            "Pug each and for should no longer be prefixed with a dash (\"-\"). They are pug keywords and not part of JavaScript.";
        let dashed_each_of =
            "Pug each and for should not be prefixed with a dash (\"-\"). They are pug keywords and not part of JavaScript.";
        let malformed = |keyword: &str| {
            format!("This `{keyword}` has a syntax error. `{keyword}` statements should be of the form: `{keyword} VARIABLE_NAME of JS_EXPRESSION`")
        };
        let (malformed_each, malformed_for) = (malformed("each"), malformed("for"));
        let each_of_value = "The value variable for each must either be a valid identifier (e.g. `item`) or a pair of identifiers in square brackets (e.g. `[key, value]`).";
        let after_tag = "Unexpected token `slash` expected `text`, `interpolated-code`, `code`, `:`, `newline` or `eos`";
        let cases: &[(&str, ErrorCode, &str, usize, usize)] = &[
            ("div\n    p\n  a", ErrorCode::InconsistentIndentation, "Inconsistent indentation. Expecting either 0 or 4 spaces/tabs.", 3, 1),
            ("div\n\tp\n  a", ErrorCode::InvalidIndentation, "Invalid indentation, you can use tabs or spaces but not both", 3, 1),
            ("p\n  ]", ErrorCode::UnexpectedText, "unexpected text \"]\"", 2, 3),
            ("include:markdown(x)foo", ErrorCode::UnexpectedText, "unexpected text \"foo\"", 1, 20),
            ("p.1", ErrorCode::InvalidClassName, "Class names must contain at least one letter or underscore.", 1, 2),
            ("p. x", ErrorCode::InvalidClassName, class_message, 1, 2),
            ("p#!x", ErrorCode::InvalidId, "\"!x\" is not a valid ID.", 1, 2),
            ("a(href=\n  \"x\"", ErrorCode::NoEndBracket, "The end of the string reached with no closing bracket ) found.", 2, 6),
            ("p #[b", ErrorCode::NoEndBracket, "End of line was reached with no closing bracket for interpolation.", 1, 6),
            ("p(a=\"{\"=})", ErrorCode::BracketMismatch, "Mismatched Bracket: }", 1, 9),
            ("p #[strong #{x]", ErrorCode::BracketMismatch, "Mismatched Bracket: ]", 1, 15),
            ("extends", ErrorCode::NoExtendsPath, "missing path for extends", 1, 8),
            ("extends:x", ErrorCode::MalformedExtends, "malformed extends", 1, 8),
            ("include:markdown", ErrorCode::NoIncludePath, "missing path for include", 1, 17),
            ("include-x", ErrorCode::MalformedInclude, "malformed include", 1, 8),
            ("case", ErrorCode::NoCaseExpression, "missing expression for case", 1, 5),
            ("when", ErrorCode::NoWhenExpression, "missing expression for when", 1, 5),
            ("default x", ErrorCode::DefaultWithExpression, "default should not have an expression", 1, 8),
            ("else x", ErrorCode::ElseCondition, "`else` cannot have a condition, perhaps you meant `else if`", 1, 6),
            ("while", ErrorCode::NoWhileExpression, "missing expression for while", 1, 6),
            ("each x\n  p", ErrorCode::MalformedEach, &malformed_each, 1, 5),
            ("for\n  p", ErrorCode::MalformedEach, &malformed_for, 1, 4),
            ("- for x, i in y", ErrorCode::MalformedEach, dashed_each, 1, 1),
            ("- each x of y", ErrorCode::MalformedEach, dashed_each_of, 1, 1),
            // `each … of` is tried first, and finds ` of` in `offers`.
            ("each offer in offers\n  li= offer", ErrorCode::MalformedEachOfLval, each_of_value, 1, 21),
            ("each k, v of m\n  p", ErrorCode::MalformedEachOfLval, each_of_value, 1, 15),
            ("each 😀 of y\n  p", ErrorCode::MalformedEachOfLval, each_of_value, 1, 13),
            ("each x of y", ErrorCode::InvalidToken, "expected \"indent\", but got \"eos\"", 1, 12),
            ("each x of y\n  p\nelse\n  p", ErrorCode::InvalidToken, "unexpected token \"else\"", 3, 1),
            ("p(a!x)", ErrorCode::InvalidKeyCharacter, "Unexpected character x expected `=`", 1, 4),
            ("p(a!)", ErrorCode::InvalidKeyCharacter, "Unexpected character undefined expected `=`", 1, 4),
            ("case x\n  p", ErrorCode::InvalidToken, "Unexpected token \"tag\", expected \"when\", \"default\" or \"newline\"", 2, 3),
            ("case x", ErrorCode::InvalidToken, "expected \"indent\", but got \"eos\"", 1, 7),
            ("if a\n  p\n// comment\nelse\n  p", ErrorCode::InvalidToken, "unexpected token \"else\"", 4, 1),
            ("+foo/", ErrorCode::InvalidToken, after_tag, 1, 5),
            ("= x\n  p", ErrorCode::BlockInBufferedCode, "Buffered code cannot have a block attached to it", 2, 1),
            ("include a.txt\n  p", ErrorCode::RawIncludeBlock, "Raw inclusion cannot contain a block", 2, 1),
            ("mixin foo", ErrorCode::MixinWithoutBody, "Mixin foo declared without body", 1, 1),
            ("block", ErrorCode::BlockOutsideMixin, "Anonymous blocks are not allowed unless they are part of a mixin.", 1, 1),
            ("p(id=\"b\")#a", ErrorCode::DuplicateId, "Duplicate attribute \"id\" is not allowed.", 1, 10),
            ("p#a(id=\"b\")", ErrorCode::DuplicateAttribute, "Duplicate attribute \"id\" is not allowed.", 1, 5),
            // Pug counts the line break before `#{` as a column.
            ("+\n#{a", ErrorCode::NoEndBracket, "The end of the string reached with no closing bracket } found.", 1, 6),
            ("+\n#{a]", ErrorCode::BracketMismatch, "Mismatched Bracket: ]", 1, 6),
            ("+\n#{a\nb", ErrorCode::NoEndBracket, "The end of the string reached with no closing bracket } found.", 2, 2),
            // JavaScript's `trim` takes U+FEFF and leaves U+0085.
            ("else \u{feff}", ErrorCode::InvalidToken, "unexpected token \"else\"", 1, 1),
            ("else \u{85}", ErrorCode::ElseCondition, "`else` cannot have a condition, perhaps you meant `else if`", 1, 6),
            ("p.\n  a\n\u{85}\n  b", ErrorCode::UnexpectedText, "unexpected text \"\u{85}\n  b\"", 3, 1),
            // JavaScript's `.` doesn't take U+2028, so the parentheses are attributes.
            ("mixin foo(a\u{2028}b)\n  p", ErrorCode::MixinWithoutBody, "Mixin foo declared without body", 1, 1),
        ];
        for &(text, code, message, line, column) in cases {
            assert_eq!(error_of(text), (code, message.to_string(), line, column), "{text:?}");
        }
    }

    /// Where Pug itself throws rather than raising a Pug error, the JavaScript error's code or class and message.
    #[test]
    fn templates_that_make_pug_throw() {
        let cases = [
            ("p#", ErrorCode::TypeError, "Cannot read properties of null (reading '0')"),
            ("div\n  .", ErrorCode::TypeError, "Cannot read properties of undefined (reading 'type')"),
            ("case x\n  when a)", ErrorCode::MismatchedBracket, "Mismatched Bracket: )"),
            ("p&attributes x", ErrorCode::Assertion, "The start character should be \"(\", \"{\" or \"[\""),
        ];
        for (text, code, message) in cases {
            let (got_code, got_message, ..) = error_of(text);
            assert_eq!((got_code, got_message.as_str()), (code, message), "{text:?}");
        }
    }

    #[test]
    fn paths_get_pugs_implied_extension() {
        for (path, read) in [
            ("row", "row.pug"),
            ("row.jst.pug", "row.jst.pug"),
            ("old.jade", "old.jade"),
            ("frag.html", "frag.html"),
            ("../shared/.hidden", "../shared/.hidden.pug"),
            ("sub/", "sub/.pug"),
            ("..", "...pug"),
            ("a.", "a."),
        ] {
            assert_eq!(implied(path), read, "{path}");
        }
    }

    /// Node's `path.posix.extname` and `path.win32.extname`, whatever the platform the test runs on.
    #[test]
    fn extensions_follow_the_platforms_separators() {
        for (path, posix, windows) in [
            ("a.b\\c", true, false),
            ("a\\b.c", true, true),
            ("dir\\.a", true, false),
            ("x\\..", true, false),
            ("a.b\\", true, true),
            ("C:.a", true, false),
            ("C:a.b", true, true),
            ("c:\\x\\y.pug", true, true),
            ("a.b/c", false, false),
            ("dir/.a", false, false),
            ("..a", true, true),
            ("...", true, true),
            (".", false, false),
        ] {
            assert_eq!((has_extension(path, false), has_extension(path, true)), (posix, windows), "{path}");
        }
    }

    #[test]
    fn javascript_is_not_validated() {
        assert!(parse("if\n  p").is_ok());
        assert!(parse("p= )").is_ok());
        assert!(parse("a(x=...rest)").is_ok());
    }

    #[test]
    fn broken_fixture() {
        let message = "Inconsistent indentation. Expecting either 4 or 6 spaces/tabs.".to_string();
        assert_eq!(error_of(include_str!("../../../test/broken/bad.pug")), (ErrorCode::InconsistentIndentation, message, 7, 1));
    }

    #[test]
    fn tags_keep_their_shorthand_id_and_classes() {
        let block = parse("extends layout.pug\nblock content\n  section#main.wide.dark(data-x=1)\n    +card(item)(class=\"c\")").unwrap();
        let [extends, named] = &block.nodes[..] else { panic!("{block:?}") };
        let NodeKind::Extends { file } = &extends.kind else { panic!("{extends:?}") };
        assert_eq!((file.path.as_str(), file.line, file.column), ("layout.pug", 1, 9));
        let NodeKind::NamedBlock { name, mode: BlockMode::Replace, block } = &named.kind else { panic!("{named:?}") };
        assert_eq!(name, "content");
        let NodeKind::Tag(section) = &block.nodes[0].kind else { panic!("{block:?}") };
        let attrs: Vec<(&str, &AttrValue)> = section.attrs.iter().map(|a| (a.name.as_str(), &a.val)).collect();
        assert_eq!(
            attrs,
            [
                ("id", &AttrValue::Shorthand("main".to_string())),
                ("class", &AttrValue::Shorthand("wide".to_string())),
                ("class", &AttrValue::Shorthand("dark".to_string())),
                ("data-x", &AttrValue::Expr("1".to_string())),
            ]
        );
        let call = &section.block.nodes[0];
        let NodeKind::MixinCall { args, tag } = &call.kind else { panic!("{call:?}") };
        assert_eq!((tag.name.as_str(), args.as_deref(), call.line, call.column), ("card", Some("item"), 4, 5));
        assert_eq!(tag.attrs[0].name, "class");
    }

    #[test]
    fn each_of_takes_a_name_or_a_pair() {
        let mut got = Vec::new();
        outline(&parse("each x of list\n  p= x\nfor [k, v] of map\n  i\neach  y of z\n  b").unwrap(), &mut got);
        assert_eq!(
            got,
            [
                "each x of list@1:1",
                "p ()@2:3",
                "code= inline \"x\"@2:4",
                "each [k, v] of map@3:1",
                "i ()@4:3",
                "each  y of z@5:1",
                "b ()@6:3"
            ]
        );
    }

    #[test]
    fn names_and_paths_lose_javascripts_whitespace() {
        let first = |text: &str| parse(text).unwrap().nodes.remove(0).kind;
        assert!(matches!(first("block \u{feff}a\u{feff}"), NodeKind::NamedBlock { name, .. } if name == "a"));
        assert!(matches!(first("block a\u{85}"), NodeKind::NamedBlock { name, .. } if name == "a\u{85}"));
        assert!(matches!(first("include \u{feff}a\u{feff}"), NodeKind::Include { file, .. } if file.path == "a"));
        assert!(matches!(first("each \u{feff}a\u{feff} of b\n  p"), NodeKind::EachOf { val, .. } if val == "\u{feff}a\u{feff}"));
    }

    #[test]
    fn an_escaped_interpolation_keeps_its_mark() {
        let mut got = Vec::new();
        outline(&parse("p \\!{a} \\#{b}").unwrap(), &mut got);
        assert_eq!(got, ["p ()@1:1", "text \"!{a} #{b}\"@1:3"]);
    }

    #[test]
    fn else_if_chains_nest() {
        let block = parse("if a\n  p\nelse if b\n  i\nelse\n  b").unwrap();
        let NodeKind::Conditional(first) = &block.nodes[0].kind else { panic!("{block:?}") };
        let Some(Alternate::ElseIf(second)) = &first.alternate else { panic!("{first:?}") };
        let NodeKind::Conditional(second) = &second.kind else { panic!("{second:?}") };
        assert_eq!(second.test, "b");
        assert!(matches!(&second.alternate, Some(Alternate::Else(block)) if block.nodes.len() == 1));
    }

    /// `test/oracle/pug_cases.json`, as `node test/oracle/pug.js` recorded them from Pug 3.0.4.
    #[test]
    fn oracle_cases() {
        let cases: Vec<Value> = serde_json::from_str(include_str!("../../../test/oracle/pug_cases.json")).unwrap();
        let mismatches: Vec<String> = cases.iter().filter(|case| recorded_for_this_platform(case)).filter_map(disagreement).collect();
        assert!(mismatches.is_empty(), "{} of {} cases differ:\n{}", mismatches.len(), cases.len(), mismatches.join("\n"));
    }

    /// The cases are recorded on POSIX, where `\` in an `include` path isn't a separator; on Windows Pug answers
    /// differently, which `has_extension`'s own test covers.
    fn recorded_for_this_platform(case: &Value) -> bool {
        let source = case["source"].as_str().unwrap_or_default();
        !(cfg!(windows) && source.starts_with("include ") && source.contains('\\'))
    }

    /// Pug's own test templates, recorded by `node test/oracle/pug.js` into the file `PUG_ORACLE_RESULTS` names.
    #[test]
    #[ignore]
    fn upstream_templates() {
        let Ok(file) = std::env::var("PUG_ORACLE_RESULTS") else {
            println!("upstream_templates: PUG_ORACLE_RESULTS isn't set; run `node test/oracle/pug.js`");
            return;
        };
        let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        let mismatches: Vec<String> = cases.iter().filter_map(disagreement).collect();
        mismatches.iter().for_each(|m| println!("{m}"));
        println!("upstream: {}/{} agree", cases.len() - mismatches.len(), cases.len());
        assert!(mismatches.is_empty());
    }
}
