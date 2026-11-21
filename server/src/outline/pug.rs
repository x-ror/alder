//! Pug document symbols, from Pug's own tree.

use super::{block_end, Symbols, LANDMARKS};
use crate::pug::lexer::{unit_offset, BlockMode};
use crate::pug::parser::{self, Alternate, AttrValue, Block, Node, NodeKind, Tag};
use crate::util::{LineIndex, BOM};
use tower_lsp_server::ls_types::{DocumentSymbol, SymbolKind};

/// Outline: tags with a `#id` or `.class`, a literal `id` or a landmark name, `block`s and `mixin` definitions, each
/// spanning its line and the lines nested under it. With a syntax error, the symbols of the lines above it.
pub fn document_symbols(text: &str) -> Vec<DocumentSymbol> {
    let Some(tree) = parsed(text) else { return Vec::new() };
    let index = LineIndex::new(text);
    let mut outline = Outline { text, lines: text.split('\n').collect(), index: &index, symbols: Symbols::new(&index) };
    outline.block(&tree);
    outline.symbols.finish()
}

/// The template's tree, or the tree of the lines before its first error.
fn parsed(text: &str) -> Option<Block> {
    let error = match parser::parse(text) {
        Ok(tree) => return Some(tree),
        Err(error) => error,
    };
    let before: usize = text.split_inclusive('\n').take(error.line.saturating_sub(1)).map(str::len).sum();
    parser::parse(&text[..before]).ok()
}

struct Outline<'a> {
    text: &'a str,
    lines: Vec<&'a str>,
    index: &'a LineIndex<'a>,
    symbols: Symbols<'a>,
}

impl Outline<'_> {
    fn block(&mut self, block: &Block) {
        block.nodes.iter().for_each(|node| self.node(node));
    }

    fn node(&mut self, node: &Node) {
        match &node.kind {
            NodeKind::Tag(tag) => {
                if let Some(name) = self.tag_name(node, tag) {
                    self.add(node, name, SymbolKind::FIELD);
                }
                self.block(&tag.block);
            }
            NodeKind::NamedBlock { name, mode, block } => {
                let mode = match mode {
                    BlockMode::Replace => "",
                    BlockMode::Append => "append ",
                    BlockMode::Prepend => "prepend ",
                };
                self.add(node, format!("block {mode}{name}"), SymbolKind::NAMESPACE);
                self.block(block);
            }
            NodeKind::MixinDefinition { name, block, .. } => {
                self.add(node, format!("mixin {name}"), SymbolKind::FUNCTION);
                self.block(block);
            }
            NodeKind::MixinCall { tag, .. } => self.block(&tag.block),
            NodeKind::Conditional(conditional) => {
                self.block(&conditional.consequent);
                match &conditional.alternate {
                    Some(Alternate::ElseIf(node)) => self.node(node),
                    Some(Alternate::Else(block)) => self.block(block),
                    None => {}
                }
            }
            NodeKind::Each { block, alternate, .. } => {
                self.block(block);
                alternate.iter().for_each(|block| self.block(block));
            }
            NodeKind::When { block: Some(block), .. }
            | NodeKind::While { block, .. }
            | NodeKind::Case { block, .. }
            | NodeKind::Include { block, .. } => self.block(block),
            NodeKind::Code(code) => code.block.iter().for_each(|block| self.block(block)),
            _ => {}
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
            let implicit_div = !tag.interpolated && self.text[self.offset(node)..].starts_with(['.', '#']);
            return Some(if implicit_div { shorthand } else { name + &shorthand });
        }
        if let Some(id) = tag.attrs.iter().filter(|attr| attr.name == "id").find_map(|attr| string_literal(&attr.val)) {
            return Some(format!("{name}#{id}"));
        }
        LANDMARKS.contains(&name.as_str()).then_some(name)
    }

    fn add(&mut self, node: &Node, name: String, kind: SymbolKind) {
        let start = self.offset(node);
        let line = node.line.saturating_sub(1).min(self.lines.len() - 1);
        let line_end = |line: usize| self.index.line_start(line) + self.index.line(line).trim_end().len();
        let selection = (start, line_end(line).max(start));
        let range = (start, line_end(block_end(&self.lines, line, 1)).max(start));
        self.symbols.add(name, kind, range, selection);
    }

    /// The byte offset of Pug's 1-based line and UTF-16 column, which don't count the byte order mark.
    fn offset(&self, node: &Node) -> usize {
        let line = node.line.saturating_sub(1).min(self.lines.len() - 1);
        let raw = self.index.line(line);
        let bom = if line == 0 && raw.starts_with(BOM) { BOM.len_utf8() } else { 0 };
        self.index.line_start(line) + bom + unit_offset(&raw[bom..], node.column.saturating_sub(1))
    }
}

/// The text of a quoted attribute value without interpolation or escapes: `main` for `id="main"`.
fn string_literal(value: &AttrValue) -> Option<&str> {
    let AttrValue::Expr(expr) = value else { return None };
    let quote = expr.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let inner = expr.strip_prefix(quote)?.strip_suffix(quote)?;
    (!inner.is_empty() && !inner.contains([quote, '\\'])).then_some(inner)
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
    fn a_syntax_error_keeps_the_symbols_above_it() {
        assert_eq!(names(&document_symbols("nav#top\n  a\nmain\n    p\n  i#late\n")), ["nav#top", "main"]);
        assert!(document_symbols("p#!x\nmain").is_empty());
    }
}
