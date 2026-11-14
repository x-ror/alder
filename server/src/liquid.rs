//! Liquid (Jekyll, the `liquid` gem): the files `render` and `include` name, and unbalanced tags.

use crate::rails::paths::files_under;
use crate::util::{coded, error, slash_path, LineIndex};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tower_lsp_server::ls_types::Diagnostic;

/// Stable diagnostic codes.
pub mod code {
    pub const UNCLOSED_TAG: &str = "Syntax/LiquidUnclosedTag";
    pub const UNCLOSED_BLOCK: &str = "Syntax/LiquidUnclosedBlock";
    pub const STRAY_END_TAG: &str = "Syntax/LiquidStrayEndTag";
    pub const MISMATCHED_END_TAG: &str = "Syntax/LiquidMismatchedEndTag";
}

const JEKYLL_CONFIG: &str = "_config.yml";
const JEKYLL_INCLUDES: &str = "_includes";
/// The suffixes of a partial `render` finds next to the template: `_card.liquid`, `_card.html.liquid`.
const PARTIAL_SUFFIXES: [&str; 2] = [".html.liquid", ".liquid"];
/// How deep below the template's folder completion looks for partials.
const COMPLETION_DEPTH: usize = 3;

/// Tags that open a block; an unknown `endfoo` belongs to a custom tag the app registers and is left alone.
const BLOCKS: &[&str] = &["if", "unless", "for", "case", "capture", "comment", "raw", "tablerow", "form", "paginate", "highlight"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Render,
    Include,
    IncludeRelative,
}

impl Tag {
    fn name(self) -> &'static str {
        match self {
            Tag::Render => "Render",
            Tag::Include => "Include",
            Tag::IncludeRelative => "Include relative",
        }
    }
}

/// The file a `{% render %}` or `{% include %}` names, as byte columns of its line.
#[derive(Debug, PartialEq, Eq)]
pub struct LiquidRef {
    pub tag: Tag,
    pub name: String,
    pub start: usize,
    pub end: usize,
}

fn tag_of(word: &str) -> Tag {
    match word {
        "render" => Tag::Render,
        "include" => Tag::Include,
        _ => Tag::IncludeRelative,
    }
}

/// The names of the `render`, `include` and `include_relative` tags on `line`. Jekyll's unquoted `include footer.html`
/// counts; a dynamic name (`{{ var }}`) and an unquoted `render` argument (a variable) do not.
pub fn refs(line: &str) -> Vec<LiquidRef> {
    static NAME: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"\{%-?\s*(render|include_relative|include)\s+(?:'([^'{}]*)'|"([^"{}]*)"|([^\s'"{}%,]+))"#).unwrap()
    });
    NAME.captures_iter(line)
        .filter_map(|found| {
            let tag = tag_of(&found[1]);
            let name = found.get(2).or(found.get(3)).or_else(|| found.get(4).filter(|_| tag != Tag::Render))?;
            (!name.as_str().is_empty()).then(|| LiquidRef { tag, name: name.as_str().to_string(), start: name.start(), end: name.end() })
        })
        .collect()
}

/// The reference whose name the cursor, at byte `col`, is in or next to.
pub fn ref_at(line: &str, col: usize) -> Option<LiquidRef> {
    refs(line).into_iter().find(|r| (r.start..=r.end).contains(&col))
}

/// The tag and the part of the quoted name typed before byte `col`, inside `{% render '…` or `{% include "…`.
pub fn name_prefix(line: &str, col: usize) -> Option<(Tag, String)> {
    static TYPING: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r#"\{%-?\s*(render|include_relative|include)\s+['"]([^'"{}%]*)$"#).unwrap());
    let found = TYPING.captures(line.get(..col)?)?;
    Some((tag_of(&found[1]), found[2].to_string()))
}

/// The nearest folder above `current` with a Jekyll `_config.yml`.
fn jekyll_root(current: &Path) -> Option<&Path> {
    current.ancestors().skip(1).find(|dir| dir.join(JEKYLL_CONFIG).is_file())
}

/// The files the reference stands for: Jekyll's `_includes/<name>` and the file next to the template for
/// `include_relative`; otherwise the `liquid` gem's `_<name>.liquid` next to the template.
pub fn resolve(current: &Path, r: &LiquidRef) -> Vec<PathBuf> {
    let name = Path::new(&r.name);
    let Some(dir) = current.parent().filter(|_| !name.is_absolute()) else { return vec![] };
    let named = match (r.tag, jekyll_root(current)) {
        (Tag::IncludeRelative, _) => vec![dir.join(name)],
        (Tag::Include, Some(root)) => vec![root.join(JEKYLL_INCLUDES).join(name)],
        _ => {
            let Some(file) = name.file_name().and_then(|file| file.to_str()) else { return vec![] };
            PARTIAL_SUFFIXES.iter().map(|suffix| dir.join(name.with_file_name(format!("_{file}{suffix}")))).collect()
        }
    };
    named.into_iter().filter(|file| file.is_file()).collect()
}

/// The names the reference's quotes can hold, with the file each stands for.
pub fn candidates(current: &Path, tag: Tag) -> Vec<(String, PathBuf)> {
    let Some(dir) = current.parent() else { return vec![] };
    let (root, partials) = match (tag, jekyll_root(current)) {
        (Tag::IncludeRelative, _) => (dir.to_path_buf(), false),
        (Tag::Include, Some(root)) => (root.join(JEKYLL_INCLUDES), false),
        _ => (dir.to_path_buf(), true),
    };
    let depth = if root == dir { COMPLETION_DEPTH } else { usize::MAX };
    let mut named: Vec<(String, PathBuf)> = files_under(&root, depth)
        .into_iter()
        .filter(|file| file != current)
        .filter_map(|file| {
            let relative = slash_path(file.strip_prefix(&root).ok()?);
            let name = if partials { partial_name(&relative)? } else { relative };
            Some((name, file))
        })
        .collect();
    named.sort();
    named
}

/// `shared/_card.liquid` is `shared/card`.
fn partial_name(relative: &str) -> Option<String> {
    let (folder, file) = relative.rsplit_once('/').map_or(("", relative), |(folder, file)| (folder, file));
    let stem = PARTIAL_SUFFIXES.iter().find_map(|suffix| file.strip_suffix(suffix))?.strip_prefix('_')?;
    Some(if folder.is_empty() { stem.to_string() } else { format!("{folder}/{stem}") })
}

/// What hovering a reference shows: the files it resolves to, from the Jekyll site's or the template's folder.
pub fn hover(current: &Path, r: &LiquidRef) -> String {
    let found = resolve(current, r);
    if found.is_empty() {
        return format!("**{}** `{}`\n\nNo matching file found.", r.tag.name(), r.name);
    }
    let base = jekyll_root(current).or(current.parent());
    let list: Vec<String> =
        found.iter().map(|file| format!("- `{}`", slash_path(base.and_then(|b| file.strip_prefix(b).ok()).unwrap_or(file)))).collect();
    format!("**{}** `{}`\n\n{}", r.tag.name(), r.name, list.join("\n"))
}

/// What `{% comment %}` and `{% raw %}` hold is text, up to their end tag.
struct Verbatim {
    end: &'static str,
    /// Comments nest: Liquid counts the `{% comment %}` inside one.
    nested: usize,
}

struct OpenBlock {
    name: &'static str,
    start: usize,
    end: usize,
}

struct Scanner {
    stack: Vec<OpenBlock>,
    verbatim: Option<Verbatim>,
    /// Byte spans, code and message of each problem.
    found: Vec<(usize, usize, &'static str, String)>,
}

impl Scanner {
    /// A tag called `name` whose `{% … %}`, or statement of a `{% liquid %}`, spans `start..end`.
    fn tag(&mut self, name: &str, start: usize, end: usize) {
        if let Some(verbatim) = &mut self.verbatim {
            if name == "comment" && verbatim.end == "endcomment" {
                verbatim.nested += 1;
            } else if name == verbatim.end && verbatim.nested > 0 {
                verbatim.nested -= 1;
            } else if name == verbatim.end {
                self.verbatim = None;
                self.stack.pop();
            }
            return;
        }
        if let Some(block) = BLOCKS.iter().find(|block| **block == name) {
            self.stack.push(OpenBlock { name: block, start, end });
            self.verbatim = match *block {
                "comment" => Some(Verbatim { end: "endcomment", nested: 0 }),
                "raw" => Some(Verbatim { end: "endraw", nested: 0 }),
                _ => None,
            };
        } else if let Some(block) = name.strip_prefix("end").filter(|block| BLOCKS.contains(block)) {
            self.close(block, name, start, end);
        }
    }

    /// An end tag closes the innermost open block of its kind; the blocks inside it were left open.
    fn close(&mut self, block: &str, name: &str, start: usize, end: usize) {
        if let Some(index) = self.stack.iter().rposition(|open| open.name == block) {
            for open in self.stack.split_off(index + 1) {
                self.unclosed(&open);
            }
            self.stack.pop();
        } else if let Some(open) = self.stack.last() {
            let message = format!("`{name}` doesn't close the open `{}` tag; Liquid expects `end{}`", open.name, open.name);
            self.found.push((start, end, code::MISMATCHED_END_TAG, message));
            // Read as a typo for the end tag it should be, so the block isn't reported a second time.
            self.stack.pop();
        } else {
            self.found.push((start, end, code::STRAY_END_TAG, format!("`{name}` has no open `{block}` tag to close")));
        }
    }

    fn unclosed(&mut self, open: &OpenBlock) {
        let message = format!("`{}` is never closed; add `end{}`", open.name, open.name);
        self.found.push((open.start, open.end, code::UNCLOSED_BLOCK, message));
    }
}

/// The offset of the next `{{` or `{%` at or after `from`.
fn next_opener(text: &str, from: usize) -> Option<usize> {
    let mut at = from;
    while let Some(found) = text[at..].find('{') {
        at += found;
        if matches!(text.as_bytes().get(at + 1), Some(b'{' | b'%')) {
            return Some(at);
        }
        at += 1;
    }
    None
}

fn word_at(text: &str) -> &str {
    let end = text.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(text.len());
    &text[..end]
}

/// Tags and outputs that are never closed, block tags without their end tag and end tags without their block.
pub fn diagnostics(text: &str) -> Vec<Diagnostic> {
    let mut scanner = Scanner { stack: vec![], verbatim: None, found: vec![] };
    let mut at = 0;
    while let Some(open) = next_opener(text, at) {
        let is_tag = text[open..].starts_with("{%");
        if !is_tag && scanner.verbatim.is_some() {
            at = open + 2;
            continue;
        }
        let Some(length) = text[open + 2..].find(if is_tag { "%}" } else { "}}" }) else {
            if scanner.verbatim.is_none() {
                scanner.found.push((open, open + 2, code::UNCLOSED_TAG, format!("`{}` is never closed", &text[open..open + 2])));
            }
            break;
        };
        let close = open + 2 + length;
        at = close + 2;
        if !is_tag {
            continue;
        }
        let body = &text[open + 2..close];
        let inner = body.strip_prefix('-').unwrap_or(body);
        let name = word_at(inner.trim_start());
        if name == "liquid" && scanner.verbatim.is_none() {
            let statements = open + 2 + (body.len() - inner.len()) + (inner.len() - inner.trim_start().len()) + name.len();
            for (offset, line) in crate::util::lines_with_offsets(&text[statements..close]) {
                let trimmed = line.trim_start();
                let word = word_at(trimmed);
                let start = statements + offset + (line.len() - trimmed.len());
                scanner.tag(word, start, start + word.len());
            }
        } else {
            scanner.tag(name, open, close + 2);
        }
    }
    for open in std::mem::take(&mut scanner.stack) {
        scanner.unclosed(&open);
    }
    let lines = LineIndex::new(text);
    scanner.found.sort_by_key(|(start, ..)| *start);
    scanner.found.into_iter().map(|(start, end, code, message)| coded(error(lines.range(start, end), message), code)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::{code as code_of, write_files};

    #[test]
    fn refs_read_literal_names_only() {
        let line = r#"{% render 'card' %}{%- include "a/b.html", x: 1 -%} {% include footer.html %} {% include_relative x.md %}"#;
        let found: Vec<_> = refs(line).into_iter().map(|r| (r.tag, r.name)).collect();
        assert_eq!(
            found,
            [
                (Tag::Render, "card".to_string()),
                (Tag::Include, "a/b.html".to_string()),
                (Tag::Include, "footer.html".to_string()),
                (Tag::IncludeRelative, "x.md".to_string()),
            ]
        );
        assert_eq!(&line[refs(line)[0].start..refs(line)[0].end], "card");
        for dynamic in
            ["{{ x }}", "{% render '{{ x }}' %}", "{% render card %}", "{% render '' %}", "{% included 'a' %}", "{% assign render = 'a' %}"]
        {
            assert!(refs(dynamic).is_empty(), "{dynamic}");
        }
        let accented = "é {% render 'card' %}";
        assert_eq!(refs(accented).iter().map(|r| &accented[r.start..r.end]).collect::<Vec<_>>(), ["card"]);
    }

    #[test]
    fn the_cursor_finds_its_reference_and_typed_prefix() {
        let line = "{% render 'ca' %}";
        assert_eq!(ref_at(line, 12).map(|r| r.name), Some("ca".to_string()));
        assert!(ref_at(line, 3).is_none());
        assert_eq!(name_prefix(line, 12), Some((Tag::Render, "c".to_string())));
        assert_eq!(name_prefix("{%- include_relative \"", 22), Some((Tag::IncludeRelative, String::new())));
        assert!(name_prefix("{% render 'ca' %} ", 18).is_none());
        assert!(name_prefix("{% assign x = 'ca", 17).is_none());
    }

    fn temp(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("rt-liquid-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn lookup(current: &Path, line: &str) -> Vec<PathBuf> {
        refs(line).first().map(|r| resolve(current, r)).unwrap_or_default()
    }

    #[test]
    fn jekyll_includes_come_from_the_site_root() {
        let root = temp("jekyll");
        write_files(
            &root,
            &[
                ("_config.yml", ""),
                ("_includes/footer.html", ""),
                ("_includes/shared/nav.html", ""),
                ("blog/post.md", ""),
                ("blog/side.html", ""),
            ],
        );
        let post = root.join("blog/post.md");
        assert_eq!(lookup(&post, "{% include footer.html %}"), [root.join("_includes/footer.html")]);
        assert_eq!(lookup(&post, "{% include 'shared/nav.html' %}"), [root.join("_includes/shared/nav.html")]);
        assert_eq!(lookup(&post, "{% include_relative side.html %}"), [root.join("blog/side.html")]);
        assert!(lookup(&post, "{% include 'missing.html' %}").is_empty());
        assert!(lookup(&post, "{% include '/etc/passwd' %}").is_empty());
        let included: Vec<_> = candidates(&post, Tag::Include).into_iter().map(|(name, _)| name).collect();
        assert_eq!(included, ["footer.html", "shared/nav.html"]);
        let relative: Vec<_> = candidates(&post, Tag::IncludeRelative).into_iter().map(|(name, _)| name).collect();
        assert_eq!(relative, ["side.html"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn without_jekyll_a_partial_is_underscored_next_to_the_template() {
        let root = temp("gem");
        write_files(
            &root,
            &[("show.liquid", ""), ("_card.liquid", ""), ("_row.html.liquid", ""), ("shared/_nav.liquid", ""), ("plain.liquid", "")],
        );
        let show = root.join("show.liquid");
        assert_eq!(lookup(&show, "{% render 'card' %}"), [root.join("_card.liquid")]);
        assert_eq!(lookup(&show, "{% include 'row' %}"), [root.join("_row.html.liquid")]);
        assert_eq!(lookup(&show, "{% render 'shared/nav' %}"), [root.join("shared/_nav.liquid")]);
        assert!(lookup(&show, "{% render 'plain' %}").is_empty());
        let named: Vec<_> = candidates(&show, Tag::Render).into_iter().map(|(name, _)| name).collect();
        assert_eq!(named, ["card", "row", "shared/nav"]);
        assert!(hover(&show, &refs("{% render 'card' %}")[0]).contains("- `_card.liquid`"));
        assert!(hover(&show, &refs("{% render 'nope' %}")[0]).contains("No matching file found."));
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn found(text: &str) -> Vec<(String, u32, u32)> {
        let at = |d: &Diagnostic| (code_of(d).unwrap().to_string(), d.range.start.line, d.range.start.character);
        diagnostics(text).iter().map(at).collect()
    }

    fn at(code: &str, line: u32, column: u32) -> (String, u32, u32) {
        (code.to_string(), line, column)
    }

    #[test]
    fn balanced_blocks_are_clean() {
        for text in [
            "{% if a %}x{% elsif b %}y{% else %}z{% endif %}",
            "{%- for i in (1..3) -%}{% case i %}{% when 1 %}a{% endcase %}{%- endfor -%}",
            "{% unless a %}{% capture b %}{{ c }}{% endcapture %}{% endunless %}",
            "{% form 'x' %}{% paginate a by 2 %}{% tablerow i in a %}{% endtablerow %}{% endpaginate %}{% endform %}",
            "{% comment %}{% if a %} {{ never closed {% endcomment %}{% raw %}{% for %}{{ x {% endraw %}",
            "{% comment %}{% comment %}inner{% endcomment %}{% endcomment %}",
            "{% custom %}x{% endcustom %}{% assign a = 1 %}{% # note %}{%- liquid\n  if a\n    echo b\n  endif\n-%}",
            "{% highlight ruby %}{{ x }}{% endhighlight %}",
            "",
        ] {
            assert_eq!(found(text), [], "{text}");
        }
    }

    #[test]
    fn unbalanced_tags_are_reported_where_they_are() {
        assert_eq!(found("{% if a %}\n  x\n"), [at(code::UNCLOSED_BLOCK, 0, 0)]);
        assert_eq!(found("x\n{% endif %}"), [at(code::STRAY_END_TAG, 1, 0)]);
        assert_eq!(found("{% if a %}{% endfor %}"), [at(code::MISMATCHED_END_TAG, 0, 10)]);
        assert_eq!(found("{% if a %}{% for b in c %}{% endif %}"), [at(code::UNCLOSED_BLOCK, 0, 10)]);
        assert_eq!(found("é {{ x"), [at(code::UNCLOSED_TAG, 0, 2)]);
        assert_eq!(found("{% comment %}never ends"), [at(code::UNCLOSED_BLOCK, 0, 0)]);
        assert_eq!(found("{% raw %}{{ {% endraw %}{% endraw %}"), [at(code::STRAY_END_TAG, 0, 24)]);
        assert_eq!(found("{% if a %}{% endif %}{% if b %}{{ c"), [at(code::UNCLOSED_BLOCK, 0, 21), at(code::UNCLOSED_TAG, 0, 31)]);
    }

    #[test]
    fn a_liquid_tag_holds_statements_one_per_line() {
        assert_eq!(found("{% liquid\n  if a\n    echo b\n%}"), [at(code::UNCLOSED_BLOCK, 1, 2)]);
        assert_eq!(found("{%- liquid\n  endfor\n-%}"), [at(code::STRAY_END_TAG, 1, 2)]);
        assert_eq!(found("{% liquid\n  comment\n    if a\n  endcomment\n%}"), []);
        assert_eq!(found("{% liquid if a %}{% endif %}"), []);
    }

    #[test]
    fn broken_fixture() {
        let got = found(include_str!("../../test/broken/bad.html.liquid"));
        let want = [at(code::STRAY_END_TAG, 0, 0), at(code::MISMATCHED_END_TAG, 3, 0), at(code::UNCLOSED_TAG, 4, 3)];
        assert_eq!(got, want);
    }
}
