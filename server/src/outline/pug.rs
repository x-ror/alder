//! Pug document symbols, from Pug's own tree.

use super::{block_end, Symbols, LANDMARKS};
use crate::pug;
use crate::pug::elements::static_string;
use crate::pug::lexer::{BlockMode, Kind, Token};
use crate::pug::parser::{AttrValue, Node, NodeKind, Tag};
use crate::util::LineIndex;
use std::collections::HashMap;
use tower_lsp_server::ls_types::{DocumentSymbol, SymbolKind};

/// Outline: tags with a `#id` or `.class`, a literal `id` or a landmark name, `block`s and `mixin` definitions, each
/// spanning its line and the lines of what Pug nests under it, a `#[…]` tag its brackets. With a syntax error, the symbols
/// of the lines above it. Lines are Pug's and the editor's, which a lone `\r` ends too.
pub fn document_symbols(text: &str) -> Vec<DocumentSymbol> {
    let Some((tokens, tree)) = pug::parsable(text) else { return Vec::new() };
    let index = LineIndex::with_lone_cr(text);
    let lines = (0..=index.line_of(usize::MAX)).map(|n| index.line(n)).collect();
    let mut outline = Outline { text, lines, index: &index, inline: inline_ends(&tokens), symbols: Symbols::new(&index) };
    for node in &tree.nodes {
        outline.node(node);
    }
    outline.symbols.finish()
}

/// Where each `#[…]` ends, past its `]`, by where the tag in it starts.
fn inline_ends(tokens: &[Token]) -> HashMap<usize, usize> {
    let mut open = Vec::new();
    let mut ends = HashMap::new();
    for (i, token) in tokens.iter().enumerate() {
        match token.kind {
            Kind::StartPugInterpolation => open.push(i + 1),
            Kind::EndPugInterpolation => {
                if let Some(first) = open.pop().and_then(|first| tokens.get(first)) {
                    ends.insert(first.span.start, token.span.end);
                }
            }
            _ => {}
        }
    }
    ends
}

/// The last line, 1-based, of a node and what Pug nests under it.
fn last_line(node: &Node) -> usize {
    pug::children(node).into_iter().map(last_line).fold(node.line, usize::max)
}

struct Outline<'a> {
    text: &'a str,
    lines: Vec<&'a str>,
    index: &'a LineIndex<'a>,
    inline: HashMap<usize, usize>,
    symbols: Symbols<'a>,
}

impl Outline<'_> {
    fn node(&mut self, node: &Node) {
        match &node.kind {
            NodeKind::Tag(tag) => {
                if let Some(name) = self.tag_name(node, tag) {
                    self.add(node, name, SymbolKind::FIELD);
                }
            }
            NodeKind::NamedBlock { name, mode, .. } => {
                let mode = match mode {
                    BlockMode::Replace => "",
                    BlockMode::Append => "append ",
                    BlockMode::Prepend => "prepend ",
                };
                self.add(node, format!("block {mode}{name}"), SymbolKind::NAMESPACE);
            }
            NodeKind::MixinDefinition { name, .. } => self.add(node, format!("mixin {name}"), SymbolKind::FUNCTION),
            // What is commented out isn't part of the page.
            NodeKind::BlockComment { .. } => return,
            NodeKind::MixinCall { .. }
            | NodeKind::Conditional(_)
            | NodeKind::Each { .. }
            | NodeKind::When { .. }
            | NodeKind::EachOf { .. }
            | NodeKind::While { .. }
            | NodeKind::Case { .. }
            | NodeKind::Include { .. }
            | NodeKind::Code(_)
            | NodeKind::Filter { .. }
            | NodeKind::Text { .. }
            | NodeKind::MixinBlock
            | NodeKind::YieldBlock
            | NodeKind::Extends { .. }
            | NodeKind::RawInclude { .. }
            | NodeKind::Comment { .. }
            | NodeKind::Doctype(_) => {}
        }
        for child in pug::children(node) {
            self.node(child);
        }
    }

    /// `section#main.wide` as written (`.card` for an implicit `div`), `form#signup` for a literal `id="signup"`,
    /// `nav` for a landmark; `None` for other tags.
    fn tag_name(&self, node: &Node, tag: &Tag) -> Option<String> {
        let name = if tag.interpolated { format!("#{{{}}}", tag.name) } else { tag.name.clone() };
        let shorthand: String = tag
            .attrs
            .iter()
            .filter_map(|attr| match &attr.val {
                AttrValue::Shorthand(value) => Some(format!("{}{value}", if attr.name == "id" { '#' } else { '.' })),
                _ => None,
            })
            .collect();
        if !shorthand.is_empty() {
            let implicit_div = !tag.interpolated && self.text[node.span.clone()].starts_with(['.', '#']);
            return Some(if implicit_div { shorthand } else { name + &shorthand });
        }
        if let Some(id) = tag.attrs.iter().filter(|attr| attr.name == "id").find_map(|attr| match &attr.val {
            AttrValue::Expr(val) => static_string(val).filter(|id| !id.is_empty()),
            _ => None,
        }) {
            return Some(format!("{name}#{id}"));
        }
        LANDMARKS.contains(&name.as_str()).then_some(name)
    }

    fn add(&mut self, node: &Node, name: String, kind: SymbolKind) {
        let start = node.span.start;
        let line_of = |line: usize| line.saturating_sub(1).min(self.lines.len() - 1);
        let line_end = |line: usize| self.index.line_start(line) + self.lines[line].trim_end().len();
        if let Some(&end) = self.inline.get(&start) {
            self.symbols.add(name, kind, (start, end.max(start)), (start, end.max(start)));
            return;
        }
        let selection = (start, line_end(line_of(node.line)).max(start));
        let range = (start, line_end(block_end(&self.lines, line_of(last_line(node)), 1)).max(start));
        self.symbols.add(name, kind, range, selection);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::names;
    use tower_lsp_server::ls_types::{Position, Range};

    #[test]
    fn symbols_nest_by_indentation() {
        let text = concat!(
            "extends layout\n",
            "block content\n",
            "  section#main.wide(data-x=1)\n",
            "    .card\n",
            "      p plain\n",
            "    if user\n",
            "      form(id=\"signup\" method=\"post\")\n",
            "    else\n",
            "      nav\n",
            "  each item in items\n",
            "    +card(item)\n",
            "      li#item: a.link(href=\"#\")\n",
            "  p(id=\"a\" + b)\n",
            "  #{tag}.x\n",
            "block append scripts\n",
            "  script.\n",
            "    var x = \"#not-a-tag\";\n",
            "mixin card(item)\n",
            "  article.card= item.title\n",
        );
        assert_eq!(
            names(&document_symbols(text)),
            [
                "block content[section#main.wide[.card,form#signup,nav],li#item[a.link],#{tag}.x]",
                "block append scripts",
                "mixin card[article.card]",
            ]
        );
        let symbols = document_symbols(text);
        assert_eq!(symbols[0].range, Range::new(Position::new(1, 0), Position::new(13, 10)));
        assert_eq!(symbols[0].selection_range, Range::new(Position::new(1, 0), Position::new(1, 13)));
        let section = &symbols[0].children.as_ref().unwrap()[0];
        assert_eq!(section.range, Range::new(Position::new(2, 2), Position::new(8, 9)));
    }

    #[test]
    fn positions_count_utf16_units_and_the_byte_order_mark() {
        let symbols = document_symbols("\u{feff}p 😀 #[span#a x]\n");
        assert_eq!(names(&symbols), ["span#a"]);
        assert_eq!(symbols[0].selection_range.start, Position::new(0, 8));
    }

    #[test]
    fn inline_tags_span_their_brackets() {
        assert_eq!(names(&document_symbols("p #[a#x hi] #[b#y bye]\n")), ["a#x", "b#y"]);
        assert_eq!(names(&document_symbols("p See #[a.link(href=\"/\") docs]\n  span#child more\n")), ["a.link", "span#child"]);
        assert_eq!(names(&document_symbols("p #[a#x #[b#y z]]\n")), ["a#x[b#y]"]);
        let symbols = document_symbols("p #[a#x hi] after\n");
        assert_eq!(symbols[0].range, Range::new(Position::new(0, 4), Position::new(0, 11)));
    }

    #[test]
    fn children_are_what_pug_nests() {
        let text = "section#main(\n  data-a=\"1\"\n)\n  .card\n    p hi\n";
        assert_eq!(names(&document_symbols(text)), ["section#main[.card]"]);
        assert_eq!(names(&document_symbols("form(id=`signup`)\nform(id=\"a\" , method=\"post\")\n")), ["form#signup", "form#a"]);
        assert_eq!(names(&document_symbols("nav#top\n  //\n    #[span#old x]\n")), ["nav#top"], "commented out");
    }

    #[test]
    fn a_lone_cr_ends_a_line() {
        let symbols = document_symbols("nav#top\r  section#a\r\n  p 😀 #[span#b x]\r");
        assert_eq!(names(&symbols), ["nav#top[section#a,span#b]"]);
        assert_eq!(symbols[0].range, Range::new(Position::new(0, 0), Position::new(2, 18)));
        let children = symbols[0].children.as_ref().unwrap();
        assert_eq!(children[0].selection_range, Range::new(Position::new(1, 2), Position::new(1, 11)));
        assert_eq!(children[1].range, Range::new(Position::new(2, 9), Position::new(2, 18)));
    }

    #[test]
    fn a_syntax_error_keeps_the_symbols_above_it() {
        assert_eq!(names(&document_symbols("nav#top\n  a\nmain\n    p\n  i#late\n")), ["nav#top", "main"]);
        assert!(document_symbols("p#!x\nmain").is_empty());
    }
}
