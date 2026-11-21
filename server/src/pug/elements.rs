//! A Pug template's tags as the element model the HTML rules share (`attributes::Element`), and the HTML check Pug makes
//! itself: content in a void element, which pug-code-gen refuses to compile. Attribute values are JavaScript, so only a
//! string literal is a value that can be read.

use super::lexer::{is_js_whitespace, js_trim};
use super::parser::{self, Alternate, Attr, AttrValue, Block, Code, Node, NodeKind, Tag};
use super::{children, code, nodes, offset};
use crate::attributes::{Attribute, Element, Value};
use crate::util::{coded, error, LineIndex};
use std::collections::HashMap;
use std::ops::Range;
use tower_lsp_server::ls_types::Diagnostic;

/// `void-elements` 3.1.0, the tags pug-code-gen closes itself, outside an XML doctype.
const VOID_ELEMENTS: [&str; 14] =
    ["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];
/// The tags of a template Pug accepts and that outputs HTML; `#{}` tag names and mixin calls are left out.
pub(crate) fn markup(text: &str) -> Vec<Element> {
    let Ok(tree) = parser::parse(text) else { return Vec::new() };
    if is_xml(&tree) {
        return Vec::new();
    }
    let index = LineIndex::new(text);
    let element = |node: &Node| match &node.kind {
        NodeKind::Tag(tag) if !tag.interpolated => Some(element(text, &index, node, tag)),
        _ => None,
    };
    nodes(&tree).into_iter().filter_map(element).collect()
}

/// Each tag's start and the range its descendant tags start in, from Pug's tree: an inline `#[a …]` holds only what is
/// inside its brackets, and `a: button` holds the button though both share a line.
pub(crate) fn contents(text: &str) -> Vec<(usize, Range<usize>)> {
    let Ok(tree) = parser::parse(text) else { return Vec::new() };
    let index = LineIndex::new(text);
    let tag_start = |node: &Node| match &node.kind {
        NodeKind::Tag(tag) if !tag.interpolated => Some(offset(&index, node.line, node.column)),
        _ => None,
    };
    let content = |node: &Node| {
        let NodeKind::Tag(tag) = &node.kind else { return None };
        let at = tag_start(node)?;
        let last = nodes(&tag.block).into_iter().filter_map(tag_start).max()?;
        Some((at, at + 1..last + 1))
    };
    nodes(&tree).into_iter().filter_map(content).collect()
}

/// Where Pug's tree puts each tag: in which tags and in which branches of control flow.
#[derive(Default)]
pub(crate) struct Ancestry {
    /// By the offset each tag starts at.
    tags: HashMap<usize, Around>,
    /// Where each node starts that decides whether, or how often, what is in it or after it renders, in document order.
    flow: Vec<usize>,
}

#[derive(Default, Clone)]
struct Around {
    /// The names of the tags around, in lower case.
    tags: Vec<String>,
    /// The control flow around, as where it starts and which of its branches.
    branches: Vec<(usize, usize)>,
}

impl Ancestry {
    /// Whether the tag starting at `at` is inside a `name` tag.
    pub(crate) fn inside(&self, at: usize, name: &str) -> bool {
        self.tags.get(&at).is_some_and(|around| around.tags.iter().any(|tag| tag == name))
    }

    /// Whether one of the tags starting at `first` and `second` (later) may render without the other: they are in
    /// different branches, or control flow starts between them.
    pub(crate) fn separated(&self, first: usize, second: usize) -> bool {
        let branches = |at| self.tags.get(&at).map(|around| &around.branches);
        branches(first).is_none() || branches(first) != branches(second) || self.flow.iter().any(|&at| first < at && at < second)
    }
}

/// The ancestry of the tags of a template Pug accepts.
pub(crate) fn ancestry(text: &str) -> Ancestry {
    let Ok(tree) = parser::parse(text) else { return Ancestry::default() };
    let index = LineIndex::new(text);
    let mut found = Ancestry::default();
    tree.nodes.iter().for_each(|node| visit(&index, node, &Around::default(), &mut found));
    found
}

fn visit(index: &LineIndex, node: &Node, around: &Around, found: &mut Ancestry) {
    let start = offset(index, node.line, node.column);
    let in_branch = |branch: usize| {
        let mut inner = around.clone();
        inner.branches.push((start, branch));
        inner
    };
    let each = |nodes: &[Node], inner: &Around, found: &mut Ancestry| nodes.iter().for_each(|child| visit(index, child, inner, found));
    match &node.kind {
        NodeKind::Tag(tag) => {
            let mut inner = around.clone();
            if !tag.interpolated {
                found.tags.insert(start, around.clone());
                inner.tags.push(tag.name.to_ascii_lowercase());
            }
            each(&tag.block.nodes, &inner, found);
        }
        NodeKind::Conditional(conditional) => {
            found.flow.push(start);
            each(&conditional.consequent.nodes, &in_branch(0), found);
            match &conditional.alternate {
                Some(Alternate::ElseIf(node)) => visit(index, node, &in_branch(1), found),
                Some(Alternate::Else(block)) => each(&block.nodes, &in_branch(1), found),
                None => {}
            }
        }
        NodeKind::Each { block, alternate, .. } => {
            found.flow.push(start);
            each(&block.nodes, &in_branch(0), found);
            alternate.iter().for_each(|block| each(&block.nodes, &in_branch(1), found));
        }
        NodeKind::EachOf { .. }
        | NodeKind::While { .. }
        | NodeKind::Case { .. }
        | NodeKind::When { .. }
        | NodeKind::MixinDefinition { .. }
        | NodeKind::MixinCall { .. }
        | NodeKind::NamedBlock { .. }
        | NodeKind::MixinBlock
        | NodeKind::Code(Code { buffer: false, .. }) => {
            found.flow.push(start);
            let inner = in_branch(0);
            children(node).into_iter().for_each(|child| visit(index, child, &inner, found));
        }
        _ => children(node).into_iter().for_each(|child| visit(index, child, around, found)),
    }
}

/// pug-code-gen's `SELF_CLOSING_CONTENT`, the first one in the template: a tag closed with `/` (whatever its name, `#{}` too)
/// or a void element outside XML mode that holds anything but blank text. A `doctype` sets XML mode from where it is on.
pub(crate) fn void_children(text: &str, tree: &Block) -> Option<Diagnostic> {
    let has_content = |tag: &Tag| {
        tag.block.nodes.iter().any(|node| !matches!(&node.kind, NodeKind::Text { val, .. } if val.chars().all(is_js_whitespace)))
    };
    let mut xml = false;
    let (node, tag) = nodes(tree).into_iter().find_map(|node| match &node.kind {
        NodeKind::Doctype(name) => {
            // An empty `doctype` keeps the one before it.
            if !name.is_empty() {
                xml = name.eq_ignore_ascii_case("xml");
            }
            None
        }
        NodeKind::Tag(tag) => {
            let void = !tag.interpolated && !xml && VOID_ELEMENTS.contains(&tag.name.as_str());
            ((tag.self_closing || void) && has_content(tag)).then_some((node, tag))
        }
        _ => None,
    })?;
    let index = LineIndex::new(text);
    let start = offset(&index, node.line, node.column);
    // pug-code-gen names the tag by its `name`, which an interpolated one doesn't have.
    let (name, written) = if tag.interpolated { ("undefined", "#{}".len() + tag.name.len()) } else { (tag.name.as_str(), tag.name.len()) };
    let message = format!("{name} is a self closing element: <{name}/> but contains nested content.");
    Some(coded(error(index.range(start, start + written), message), code::VOID_CHILDREN))
}

/// Whether the template sets pug-code-gen's XML mode anywhere: an XML document, which the HTML rules don't apply to.
fn is_xml(tree: &Block) -> bool {
    nodes(tree).into_iter().any(|node| matches!(&node.kind, NodeKind::Doctype(name) if name.eq_ignore_ascii_case("xml")))
}

fn element(text: &str, index: &LineIndex, node: &Node, tag: &Tag) -> Element {
    let start = offset(index, node.line, node.column);
    // `.card` and `#main` are a `div` whose name isn't written.
    let at = if text[start..].starts_with(&tag.name) { start..start + tag.name.len() } else { start..start };
    Element {
        name: tag.name.to_ascii_lowercase(),
        attributes: tag.attrs.iter().map(|attr| attribute(text, index, attr)).collect(),
        text: own_text(text, index, tag),
        open: !tag.attribute_blocks.is_empty(),
        head_end: Some(head_end(text, at.end)),
        at,
        ..Element::default()
    }
}

fn attribute(text: &str, index: &LineIndex, attr: &Attr) -> Attribute {
    let start = offset(index, attr.line, attr.column);
    let name = attr.name.to_ascii_lowercase();
    if let AttrValue::Shorthand(written) = &attr.val {
        let at = start + 1..start + 1 + written.len();
        if text.get(at.clone()) != Some(written.as_str()) {
            return Attribute { name, at: start..start, in_data: false, value: Value::Literal };
        }
        return Attribute { name, at: at.clone(), in_data: false, value: Value::Text(at) };
    }
    let quoted = text[start..].starts_with(['"', '\'']);
    let name_start = start + usize::from(quoted);
    let at = if text[name_start..].starts_with(&attr.name) { name_start..name_start + attr.name.len() } else { start..start };
    let value = match &attr.val {
        AttrValue::Expr(val) => {
            let after_name = name_start + attr.name.len() + usize::from(quoted);
            expression_value(val, text.get(after_name..).and_then(|rest| value_offset(rest, val)).map(|at| after_name + at))
        }
        _ => Value::Empty,
    };
    Attribute { name, at, in_data: false, value }
}

/// Where `val` starts in `rest`, the text after an attribute's name: past `=` or `!=` and the spaces around it.
fn value_offset(rest: &str, val: &str) -> Option<usize> {
    let after = rest.trim_start();
    let after = after.strip_prefix('!').unwrap_or(after).strip_prefix('=')?.trim_start();
    after.starts_with(val).then(|| rest.len() - after.len())
}

/// A string literal is text that can be read (where `at`, the literal's offset, is known); `true` and numbers set the attribute
/// to something unreadable; any other JavaScript is known at render time only.
fn expression_value(val: &str, at: Option<usize>) -> Value {
    if let Some(content) = static_string(val) {
        let quote = val.len() - val.trim_start_matches(is_js_whitespace).len() + 1;
        return at.map_or(Value::Literal, |at| Value::Text(at + quote..at + quote + content.len()));
    }
    let val = js_trim(val);
    let number = !val.is_empty() && val.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    if val == "true" || number {
        Value::Literal
    } else {
        Value::Dynamic
    }
}

/// The content of an attribute value that is a JavaScript string literal rendering as written: no escapes, and no `${}` in
/// a template literal. Pug keeps the whitespace around the value (`id="a" , x`).
pub(crate) fn static_string(val: &str) -> Option<&str> {
    let val = js_trim(val);
    let quote = val.chars().next().filter(|c| matches!(c, '"' | '\'' | '`'))?;
    let content = val.strip_prefix(quote)?.strip_suffix(quote)?;
    let plain = !content.contains([quote, '\\', '\n']) && !(quote == '`' && content.contains("${"));
    plain.then_some(content)
}

/// The text that is all of a tag's content, where the template spells it out: one piece of text, no interpolation.
fn own_text(text: &str, index: &LineIndex, tag: &Tag) -> Option<Range<usize>> {
    let [node] = tag.block.nodes.as_slice() else { return None };
    let NodeKind::Text { val, html: false } = &node.kind else { return None };
    let start = offset(index, node.line, node.column);
    // The token starts at the space or `|` before the text.
    let found = text[start..].split('\n').next()?.find(val.as_str())?;
    Some(start + found..start + found + val.len())
}

/// Where a tag's first attribute list starts, or where one would go: past the name and the `#id` and `.class` after it.
fn head_end(text: &str, name_end: usize) -> usize {
    let is_name_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-');
    let mut at = name_end;
    loop {
        let rest = &text[at..];
        let Some(after) = rest.strip_prefix(['#', '.']) else { return at };
        let name = after.len() - after.trim_start_matches(is_name_char).len();
        if name == 0 {
            return at;
        }
        at += 1 + name;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each attribute as its name, the text it is written as and its value's text.
    fn spelled<'a>(text: &'a str, element: &'a Element) -> Vec<(&'a str, &'a str, Option<&'a str>)> {
        let value = |value: &Value| match value {
            Value::Text(range) => Some(&text[range.clone()]),
            _ => None,
        };
        element.attributes.iter().map(|a| (a.name.as_str(), &text[a.at.clone()], value(&a.value))).collect()
    }

    #[test]
    fn tags_read_shorthands_and_literal_values() {
        let text = "a#main.x(href=\"/y\" title=user.name 'data-a'='z' hidden tabindex=0 alt=`b` src=`${x}`)\n";
        let found = markup(text);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].name.as_str(), &text[found[0].at.clone()]), ("a", "a"));
        assert_eq!(
            spelled(text, &found[0]),
            [
                ("id", "main", Some("main")),
                ("class", "x", Some("x")),
                ("href", "href", Some("/y")),
                ("title", "title", None),
                ("data-a", "data-a", Some("z")),
                ("hidden", "hidden", None),
                ("tabindex", "tabindex", None),
                ("alt", "alt", Some("b")),
                ("src", "src", None),
            ]
        );
        let values: Vec<&Value> = found[0].attributes.iter().map(|a| &a.value).collect();
        assert_eq!(values[3], &Value::Dynamic);
        assert_eq!(values[5], &Value::Empty);
        assert_eq!(values[6], &Value::Literal);
        assert_eq!(values[8], &Value::Dynamic);
        assert!(!found[0].open);
    }

    #[test]
    fn literals_are_read_past_the_whitespace_pug_keeps() {
        let text = "iframe(id=\"dup\" , title=\"\" , name=`x` )\n";
        assert_eq!(spelled(text, &markup(text)[0]), [("id", "id", Some("dup")), ("title", "title", Some("")), ("name", "name", Some("x"))]);
        assert_eq!(static_string("`signup`"), Some("signup"));
        assert_eq!(static_string("`a${b}`"), None);
    }

    #[test]
    fn escaped_strings_and_attribute_blocks_are_not_read() {
        let text = "img(alt=\"a\\\"b\" src = 'c')&attributes(attrs)\n";
        let found = markup(text);
        assert_eq!(spelled(text, &found[0]), [("alt", "alt", None), ("src", "src", Some("c"))]);
        assert_eq!(found[0].attributes[0].value, Value::Dynamic);
        assert!(found[0].open);
    }

    #[test]
    fn implied_divs_inline_tags_and_text() {
        let text = "\u{feff}.card\n  p Hi #[a(href=\"/x\") here] and\n  a(href=\"/y\") Read more\n  +card\n  #{tag} x\n";
        let found = markup(text);
        let names: Vec<(&str, &str)> = found.iter().map(|e| (e.name.as_str(), &text[e.at.clone()])).collect();
        assert_eq!(names, [("div", ""), ("p", "p"), ("a", "a"), ("a", "a")]);
        let own = |e: &Element| e.text.clone().map(|range| &text[range]);
        assert_eq!(found.iter().map(own).collect::<Vec<_>>(), [None, None, Some("here"), Some("Read more")]);
        assert_eq!(found[2].at.start, text.find("a(href=\"/x\")").unwrap());
    }

    #[test]
    fn templates_pug_rejects_or_xml_have_no_elements() {
        assert!(markup("p\n    a\n  b\n").is_empty());
        assert!(markup("doctype xml\nimg\n").is_empty());
    }

    #[test]
    fn heads_end_before_the_first_attribute_list() {
        fn head(text: &str) -> &str {
            &text[markup(text)[0].head_end.unwrap()..]
        }
        assert_eq!(head("img.big#logo(src=\"a\")\n"), "(src=\"a\")\n");
        assert_eq!(head("img.big\n"), "\n");
        assert_eq!(head("img/\n"), "/\n");
        assert_eq!(head(".card.\n  text\n"), ".\n  text\n");
    }

    #[test]
    fn content_in_a_void_element_is_reported_once_as_pug_does() {
        let found = |text: &str| {
            let tree = parser::parse(text).unwrap();
            void_children(text, &tree).map(|d| (d.range.start.line, d.range.start.character, d.range.end.character, d.message))
        };
        let message = "img is a self closing element: <img/> but contains nested content.".to_string();
        assert_eq!(found("p\n  img x\n  br\n    | y\n"), Some((1, 2, 5, message.clone())));
        assert_eq!(found("img= src\n"), Some((0, 0, 3, message.clone())));
        assert_eq!(found("mixin m\n  img\n    p\n"), Some((1, 2, 5, message.clone())));
        assert_eq!(found("each x of xs\n  img= x\n"), Some((1, 2, 5, message)));
        assert_eq!(found("foo/\n  p\n").map(|d| d.3), Some("foo is a self closing element: <foo/> but contains nested content.".into()));
        let undefined = "undefined is a self closing element: <undefined/> but contains nested content.".to_string();
        assert_eq!(found("#{t}/\n  p\n"), Some((0, 0, 4, undefined)), "an explicit `/` whatever the name");
        assert_eq!(found("img/\n  p\n").map(|d| d.0), Some(0));
        // XML mode starts at the `doctype`, and another one ends it.
        assert_eq!(found("img\n  p\ndoctype xml\n").map(|d| d.0), Some(0));
        assert_eq!(found("doctype xml\ndoctype html\nimg\n  p\n").map(|d| d.0), Some(2));
        assert_eq!(found("doctype xml  \nimg\n  p\n").map(|d| d.0), Some(1), "not a doctype Pug knows");
        for fine in [
            "img\n",
            "img \n",
            "img\n  //- note\n",
            "img.\n  \n",
            "IMG x\n",
            "doctype xml\nimg\n  p\n",
            "doctype XML\ndoctype\nimg\n  p\n",
            "div/\n",
            "#{t}\n  p\n",
            "script.\n  img x\n",
            "keygen x\n",
            "menuitem x\n",
        ] {
            assert_eq!(found(fine), None, "{fine:?}");
        }
    }

    #[test]
    fn ancestry_follows_the_tree() {
        let text = "svg: a x\np#x\nblock_item\np#x\nif x\n  #a\nelse\n  #b\ntemplate: #t\n";
        let found = ancestry(text);
        let at = |needle: &str| text.find(needle).unwrap();
        assert!(found.inside(at("a x"), "svg") && !found.inside(at("p#x"), "svg"));
        assert!(found.inside(at("#t"), "template"));
        assert!(!found.separated(at("p#x"), at("p#x\nif x")), "`block_item` is a tag");
        assert!(found.separated(at("#a"), at("#b")), "two branches");
        assert!(found.separated(at("p#x\nif x"), at("#t")), "an `if` between");
    }
}
