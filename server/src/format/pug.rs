//! Pug formatting that can't change the compiled template, read from Pug's own tokens: each line that starts a node is
//! indented one unit per nesting level, a text block (after `tag.`, `//`, `//-`, `-` or a filter) moves to one unit
//! below its line with its lines' indentation past the block's kept, the lines of a multi-line attribute list or mixin
//! call move with their line, a line inside a value that spans lines keeps every byte, and trailing whitespace goes
//! where Pug doesn't read it as text. Every line stays; the blank ones that may go are reported.

use crate::format::blank_lines::LineInfo;
use crate::format::{IndentStyle, Indented, Settings};
use crate::pug::lexer::{lex, Kind, Token};
use crate::util::{leading_ws, raw_lines, BOM};

const BLANK_SPACE: [char; 2] = [' ', '\t'];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Blank,
    /// Starts a node `depth` levels deep.
    Node {
        depth: usize,
    },
    /// A line of the text block under line `owner`, which Pug reads from its `indent`th character on.
    Text {
        owner: usize,
        indent: usize,
    },
    /// Goes on inside the brackets opened on line `owner`, between two attributes or arguments.
    Continues {
        owner: usize,
    },
    /// Inside a value that spans lines, an attribute's JavaScript or a mixin call's arguments, where every byte counts;
    /// or a first line that starts with whitespace, which Pug reads as text.
    Inside,
}

struct Classified {
    role: Role,
    /// Its trailing whitespace is text, or JavaScript that goes on past the line end.
    keeps_trailing: bool,
}

/// The formatted text, or `None` when Pug would reject the text, it has a line break Pug reads where the editor doesn't
/// (a lone `\r`), or its lines can't be told apart from Pug's tokens.
pub(crate) fn format(text: &str, settings: &Settings) -> Option<Indented> {
    if settings.indent_size == 0 && settings.indent_style == IndentStyle::Spaces {
        return None;
    }
    let body = text.strip_prefix(BOM).unwrap_or(text);
    if crate::pug::has_lone_cr(body) {
        return None;
    }
    crate::pug::parser::parse(text).ok()?;
    let lines: Vec<(usize, &str, &str)> = raw_lines(body).collect();
    let rows: Vec<&str> = lines.iter().map(|&(_, line, _)| line).collect();
    let classified = classify(&rows, &lex(text).ok()?)?;
    let unit = settings.indent_unit();
    let indent_of = |line: usize| match classified[line].role {
        Role::Node { depth } => Some(unit.repeat(depth)),
        _ => None,
    };
    let trim = settings.trailing_whitespace;
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..text.len() - body.len()]);
    for (i, (&(_, line, cr), Classified { role, keeps_trailing })) in lines.iter().zip(&classified).enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let tail = |s| line_end(s, trim && !keeps_trailing);
        let ws = leading_ws(line);
        match *role {
            Role::Blank if trim => {}
            Role::Blank | Role::Inside => out.push_str(tail(line)),
            Role::Node { depth } => {
                out.push_str(&unit.repeat(depth));
                out.push_str(tail(&line[ws.len()..]));
            }
            Role::Text { owner, indent } => {
                let prefix = indent_of(owner)? + &unit;
                let (skipped, read) = line.split_at(char_offset(line, indent));
                // Pug skips the start of a blank line whatever it is; a no-break space there would be lost.
                if !skipped.trim_start_matches(BLANK_SPACE).is_empty() {
                    return None;
                }
                let read = tail(read);
                if !read.is_empty() {
                    out.push_str(&prefix);
                    out.push_str(read);
                } else if !trim {
                    // As long as the new block indentation at most: Pug reads none of it.
                    out.push_str(&line[..char_offset(line, prefix.len())]);
                }
            }
            Role::Continues { owner } => match ws.strip_prefix(leading_ws(rows[owner])) {
                Some(past_owner) => {
                    out.push_str(&indent_of(owner)?);
                    out.push_str(past_owner);
                    out.push_str(tail(&line[ws.len()..]));
                }
                None => out.push_str(tail(line)),
            },
        }
        out.push_str(cr);
    }
    let infos = classified.iter().map(|line| LineInfo { removable: line.role == Role::Blank, ..LineInfo::default() }).collect();
    Some(Indented { text: out, lines: infos, notes: vec![] })
}

/// The byte offset of the `n`th character of `line`, or its end.
fn char_offset(line: &str, n: usize) -> usize {
    line.char_indices().nth(n).map_or(line.len(), |(at, _)| at)
}

fn line_end(content: &str, trim: bool) -> &str {
    if trim {
        content.trim_end_matches(BLANK_SPACE)
    } else {
        content
    }
}

/// Each line's role, from the tokens of the text whose lines are `rows`.
fn classify(rows: &[&str], tokens: &[Token]) -> Option<Vec<Classified>> {
    let blank = |line: usize| rows[line].trim_matches(BLANK_SPACE).is_empty();
    let mut roles: Vec<Option<Role>> = vec![None; rows.len()];
    let mut keeps = vec![false; rows.len()];
    if !rows[0].is_empty() {
        roles[0] = Some(if leading_ws(rows[0]).is_empty() { Role::Node { depth: 0 } } else { Role::Inside });
    }
    let (mut depth, mut last_node, mut text_start) = (0usize, 0, None);
    for (i, token) in tokens.iter().enumerate() {
        if token.end.line > rows.len() {
            return None;
        }
        let line = token.start.line - 1;
        match &token.kind {
            Kind::StartPipelessText => text_start = Some(i),
            Kind::EndPipelessText => {
                let start = text_start.take()?;
                // Pug starts the first token of a block's first line past the block's indentation.
                let indent = tokens.get(start + 1)?.start.column - 1;
                let unbuffered = start > 0 && matches!(tokens[start - 1].kind, Kind::Comment { buffer: false, .. });
                for at in tokens[start].start.line..=line {
                    roles[at] = Some(Role::Text { owner: last_node, indent });
                    keeps[at] = !unbuffered;
                }
            }
            _ if text_start.is_some() => {}
            // An outdent at the end of the input comes after the last line's content.
            Kind::Indent(_) | Kind::Outdent | Kind::Newline if token.start.column == 1 => {
                match token.kind {
                    Kind::Indent(_) => depth += 1,
                    Kind::Outdent => depth = depth.checked_sub(1)?,
                    _ => {}
                }
                if !blank(line) {
                    roles[line] = Some(Role::Node { depth });
                    last_node = line;
                }
            }
            Kind::Indent(_) | Kind::Outdent | Kind::Newline | Kind::Eos => {}
            // Pug doesn't count the line breaks in these, so the lines of the tokens after them are off.
            Kind::AndAttributes(js) | Kind::Call { name: js, .. } if js.contains('\n') => return None,
            kind => {
                let end = token.end.line - 1;
                roles[line + 1..=end].fill(Some(Role::Inside));
                keeps[line..end].fill(true);
                // Pug joins consecutive `-` lines into one piece of JavaScript, so a string can go on past the line end.
                keeps[end] = matches!(
                    kind,
                    Kind::Text(_)
                        | Kind::TextHtml(_)
                        | Kind::Comment { buffer: true, .. }
                        | Kind::Doctype(_)
                        | Kind::Code { buffer: false, .. }
                );
            }
        }
    }
    let mut owner = 0;
    let classified = roles.iter().zip(keeps).enumerate().map(|(line, (role, keeps_trailing))| {
        let role = match role {
            Some(role) => *role,
            None if blank(line) => Role::Blank,
            None => Role::Continues { owner },
        };
        if matches!(role, Role::Node { .. }) {
            owner = line;
        }
        Classified { role, keeps_trailing }
    });
    Some(classified.collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reindent(text: &str, settings: &Settings) -> Option<String> {
        format(text, settings).map(|indented| indented.text)
    }

    /// `text` formatted with the defaults, checked to compile as it did and to stay as it is when formatted again.
    fn fmt(text: &str) -> String {
        let once = reindent(text, &Settings::default()).expect("formattable");
        assert!(crate::pug::same_compilation(text, &once), "compiles as before:\n{text}\n---\n{once}");
        assert_eq!(reindent(&once, &Settings::default()).as_deref(), Some(once.as_str()), "idempotent");
        once
    }

    #[test]
    fn reindents_nodes_by_depth() {
        assert_eq!(fmt("html\n    body\n        p x\n\n    footer\n"), "html\n  body\n    p x\n\n  footer\n");
        assert_eq!(fmt("ul\n\tli\n\t\ta\n"), "ul\n  li\n    a\n");
        let four = Settings { indent_size: 4, ..Settings::default() };
        assert_eq!(reindent("ul\n  li\n", &four).as_deref(), Some("ul\n    li\n"));
        let tabs = Settings { indent_style: IndentStyle::Tabs, ..Settings::default() };
        assert_eq!(reindent("ul\n  li\n    a\n", &tabs).as_deref(), Some("ul\n\tli\n\t\ta\n"));
        let branches = "if a\n    p x\nelse if b\n    each i in list\n        +card(i)\n    else\n        p none\nelse\n    case c\n        when 1\n            b one\n        default\n            i\n";
        let want = "if a\n  p x\nelse if b\n  each i in list\n    +card(i)\n  else\n    p none\nelse\n  case c\n    when 1\n      b one\n    default\n      i\n";
        assert_eq!(fmt(branches), want);
    }

    #[test]
    fn text_blocks_move_as_a_block() {
        let script = "div\n        script.\n              if (a) {\n                b();  \n              }\n        p\n";
        assert_eq!(fmt(script), "div\n  script.\n    if (a) {\n      b();  \n    }\n  p\n");
        let filter = "div\n    :markdown-it\n        # Title  \n          indented\n";
        assert_eq!(fmt(filter), "div\n  :markdown-it\n    # Title  \n      indented\n");
        let code = "div\n    -\n        var a = `x  \n          y`\n    p= a\n";
        assert_eq!(fmt(code), "div\n  -\n    var a = `x  \n      y`\n  p= a\n");
        // A tab past the block's indentation is text.
        assert_eq!(fmt("div\n    p.\n        a\n        \tb\n"), "div\n  p.\n    a\n    \tb\n");
    }

    #[test]
    fn comments_keep_their_text() {
        let buffered = "div\n    // note  \n        more  \n          deeper\n";
        assert_eq!(fmt(buffered), "div\n  // note  \n    more  \n      deeper\n");
        // `//-` is dropped from the output, its whitespace with it.
        assert_eq!(fmt("div\n    //- note  \n        more  \n"), "div\n  //- note\n    more\n");
    }

    #[test]
    fn blank_lines_in_text_blocks_keep_what_pug_reads() {
        // Past the block's indentation a blank line's spaces are text; up to it, nothing.
        assert_eq!(fmt("div\n    p.\n        a\n          \n      \n        b\n"), "div\n  p.\n    a\n      \n\n    b\n");
        // The blank lines that end a block are text too, so they stay.
        assert_eq!(fmt("p.\n    a\n\n\nspan\n"), "p.\n  a\n\n\nspan\n");
    }

    #[test]
    fn attribute_lists_move_with_their_line() {
        let text = "div\n    a(href=\"/x\"\n      class=\"y\") link\n    +card(item,\n          other)\n";
        assert_eq!(fmt(text), "div\n  a(href=\"/x\"\n    class=\"y\") link\n  +card(item,\n          other)\n");
        // A value that goes on over lines keeps its bytes, the spaces at the end of its first line too.
        let value = "div\n    a(style={  \n      color: 'red'\n    }) x\n";
        assert_eq!(fmt(value), "div\n  a(style={  \n      color: 'red'\n    }) x\n");
        let template = "div\n    p(title=`a  \n    b`)\n";
        assert_eq!(fmt(template), "div\n  p(title=`a  \n    b`)\n");
    }

    #[test]
    fn trailing_whitespace_goes_only_where_pug_reads_none() {
        // Text, a tag's trailing spaces (Pug reads them as text), `|`, inline HTML, buffered comments and doctype keep it.
        for kept in
            ["p hi  \n", "div  \n", "| a  \n", "<b>  \n", "// c  \n", "doctype html  \n", "p #{x}  \n", "a(href='x')  \n", "+m(a)  \n"]
        {
            assert_eq!(fmt(kept), kept, "{kept:?}");
        }
        assert_eq!(
            fmt("p= x \t\nif x  \n  p.  \n    a\neach i in xs  \n  span\nwhile x  \n  i\ncase x  \n  when 1  \n    b\nblock content  \n//- c  \n\n  \n"),
            "p= x\nif x\n  p.\n    a\neach i in xs\n  span\nwhile x\n  i\ncase x\n  when 1\n    b\nblock content\n//- c\n\n\n"
        );
        assert_eq!(fmt("each x of xs  \n    span\n"), "each x of xs\n  span\n");
        // Pug joins `-` lines into one piece of JavaScript: these spaces are in a string.
        let literal = "- var a = `x  \n- y`\np= a\n";
        assert_eq!(fmt(literal), literal);
        assert!(!crate::pug::same_compilation(literal, "- var a = `x\n- y`\np= a\n"));
        let off = Settings { trailing_whitespace: false, ..Settings::default() };
        assert_eq!(
            reindent("div  \n    - x  \n  \n    p.\n          a\n           \n", &off).as_deref(),
            Some("div  \n  - x  \n  \n  p.\n    a\n     \n")
        );
    }

    #[test]
    fn keeps_line_endings_and_the_byte_order_mark() {
        assert_eq!(fmt("\u{feff}div\r\n    p\r\n"), "\u{feff}div\r\n  p\r\n");
        assert_eq!(fmt("div\r\n    p.\r\n        a  \r\n"), "div\r\n  p.\r\n    a  \r\n");
    }

    #[test]
    fn refuses_what_it_cannot_keep() {
        assert_eq!(reindent("div\n    p\n  a\n", &Settings::default()), None, "inconsistent indentation");
        assert_eq!(reindent("div\n\tp\n  a\n", &Settings::default()), None, "tabs and spaces");
        assert_eq!(reindent("div\r    p\r", &Settings::default()), None, "a lone CR");
        let attributes = "div\n    a&attributes({\n  b: 1})\n";
        assert!(crate::pug::parser::parse(attributes).is_ok());
        assert_eq!(reindent(attributes, &Settings::default()), None, "Pug's lines are off after it");
        assert_eq!(reindent("p", &Settings { indent_size: 0, ..Settings::default() }), None);
    }

    #[test]
    fn a_no_break_space_pug_skips_in_a_text_block_is_not_dropped() {
        assert_eq!(reindent("p.\n    a\n \u{a0} \n    b\n", &Settings::default()), None);
    }

    #[test]
    fn a_first_line_with_leading_whitespace_is_text() {
        assert_eq!(fmt("  p  \n"), "  p  \n");
    }

    #[test]
    fn only_blank_lines_outside_text_may_go() {
        let removable = |text: &str| format(text, &Settings::default()).unwrap().lines.iter().map(|l| l.removable).collect::<Vec<_>>();
        assert_eq!(removable("p\n\nq\n"), [false, true, false, true]);
        assert_eq!(removable("p.\n  a\n\n  b\nq\n"), [false, false, false, false, false, true]);
        assert_eq!(removable("a(href=x\n\n  title=y)\n"), [false, true, false, true]);
    }
}
