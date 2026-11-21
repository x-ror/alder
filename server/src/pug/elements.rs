//! A Pug template's tags as the element model the HTML rules share (`attributes::Element`), and the HTML check Pug makes
//! itself: content in a void element, which pug-code-gen refuses to compile. Attribute values are JavaScript, so only a
//! string literal is a value that can be read.

use super::parser::{self, Attr, AttrValue, Block, Node, NodeKind, Tag};
use super::{code, nodes, offset};
use crate::attributes::{Attribute, Element, Value};
use crate::util::{coded, error, LineIndex};
use std::ops::Range;
use tower_lsp_server::ls_types::Diagnostic;

/// `void-elements` 2.0.1, the tags pug-code-gen closes itself, outside an XML doctype.
const VOID_ELEMENTS: [&str; 16] =
    ["area", "base", "br", "col", "embed", "hr", "img", "input", "keygen", "link", "menuitem", "meta", "param", "source", "track", "wbr"];
/// Keywords whose lines decide whether, or how often, the lines nested under them render.
const CONTROL_FLOW: [&str; 13] =
    ["if", "unless", "else", "each", "for", "while", "case", "when", "default", "mixin", "block", "append", "prepend"];

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

/// pug-code-gen's `SELF_CLOSING_CONTENT`, the first one in the template: a void element or a tag closed with `/` that holds
/// anything but blank text.
pub(crate) fn void_children(text: &str, tree: &Block) -> Option<Diagnostic> {
    let xml = is_xml(tree);
    let closed = |tag: &Tag| tag.self_closing || (!xml && VOID_ELEMENTS.contains(&tag.name.as_str()));
    let has_content =
        |tag: &Tag| tag.block.nodes.iter().any(|node| !matches!(&node.kind, NodeKind::Text { val, .. } if val.trim().is_empty()));
    let (node, tag) = nodes(tree).into_iter().find_map(|node| match &node.kind {
        NodeKind::Tag(tag) if !tag.interpolated && closed(tag) && has_content(tag) => Some((node, tag)),
        _ => None,
    })?;
    let index = LineIndex::new(text);
    let start = offset(&index, node.line, node.column);
    let message = format!("{0} is a self closing element: <{0}/> but contains nested content.", tag.name);
    Some(coded(error(index.range(start, start + tag.name.len()), message), code::VOID_CHILDREN))
}

/// Whether a line decides whether, or how often, what is nested under it renders: a keyword such as `if`, `each`, `mixin`
/// or `block`, code (`- …`), or a mixin call (`+…`), whose block the mixin may render any number of times.
pub(crate) fn controls_flow(line: &str) -> bool {
    let line = line.trim_start();
    let word = line.split(|c: char| !c.is_ascii_alphanumeric()).next().unwrap_or_default();
    line.starts_with(['-', '+']) || CONTROL_FLOW.contains(&word)
}

/// pug-code-gen's XML mode, where void elements are written as any other tag.
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
    if let Some(content) = string_literal(val) {
        return at.map_or(Value::Literal, |at| Value::Text(at + 1..at + 1 + content.len()));
    }
    let number = !val.is_empty() && val.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    if val == "true" || number {
        Value::Literal
    } else {
        Value::Dynamic
    }
}

/// The content of a JavaScript string literal that renders as written: no escapes, and no `${}` in a template literal.
fn string_literal(val: &str) -> Option<&str> {
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
        assert_eq!(found("mixin m\n  img\n    p\n"), Some((1, 2, 5, message)));
        assert_eq!(found("foo/\n  p\n").map(|d| d.3), Some("foo is a self closing element: <foo/> but contains nested content.".into()));
        for fine in [
            "img\n",
            "img \n",
            "img\n  //- note\n",
            "img.\n  \n",
            "IMG x\n",
            "doctype xml\nimg\n  p\n",
            "#{t}/\n  p\n",
            "script.\n  img x\n",
        ] {
            assert_eq!(found(fine), None, "{fine:?}");
        }
    }

    #[test]
    fn control_flow_lines() {
        for line in ["  if x", "else if y", "each a in b", "- var x = 1", "+card", "block content", "mixin m", "when 1", "unless x"] {
            assert!(controls_flow(line), "{line}");
        }
        for line in ["p.if", "#a", "iframe", "= x", "| each", "blockquote"] {
            assert!(!controls_flow(line), "{line}");
        }
    }
}
