//! The files a Pug template names with `include` and `extends`, found as Pug 2 finds them, and the checks that follow
//! them: `pug.compile` adds `.pug` to a path without an extension, and pug-load reads a relative path from the template's
//! folder and an absolute one from `basedir`, here the app's asset root.

use super::parser::{self, Alternate, Block, FileReference, Node, NodeKind};
use super::{code, offset, parsed};
use crate::rails::paths;
use crate::util::{coded, error, lexically_normal, warning, LineIndex};
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tower_lsp_server::ls_types::{Diagnostic, Range};
use walkdir::WalkDir;

/// Where an absolute path starts, the first that exists: the folder pug-rails' README suggests, then Sprockets' default.
const ASSET_ROOTS: [&str; 2] = ["app/assets/templates", "app/assets/javascripts"];
/// Folders the search for templates that include another doesn't walk into, besides the hidden ones.
const SKIPPED_FOLDERS: [&str; 1] = ["node_modules"];
/// How many templates that search reads before it gives up, not knowing.
const MAX_SCANNED: usize = 2000;
const EXTENDS_NOT_FIRST_MESSAGE: &str = concat!(
    "Declaration of template inheritance (\"extends\") should be the first thing in the file. ",
    "There can only be one extends statement per file."
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Include,
    Extends,
}

impl Keyword {
    fn name(self) -> &'static str {
        match self {
            Keyword::Include => "Include",
            Keyword::Extends => "Extends",
        }
    }
}

/// The path of an `include` or `extends`, as written, and its byte span in the text.
#[derive(Debug, PartialEq, Eq)]
pub struct FileRef {
    pub keyword: Keyword,
    pub path: String,
    pub start: usize,
    pub end: usize,
}

/// A name completion offers after `include ` or `extends `.
#[derive(Debug, PartialEq, Eq)]
pub struct Candidate {
    pub label: String,
    pub folder: bool,
    pub file: PathBuf,
}

/// Node's `path.extname(path) != ""`.
fn has_extension(path: &str) -> bool {
    let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or_default();
    name != ".." && name.rfind('.').is_some_and(|dot| dot > 0)
}

/// The path Pug reads for `path`: `pug.compile` adds `.pug` when it has no extension, so `include row` never finds
/// pug-rails' `row.jst.pug`.
pub fn implied(path: &str) -> Cow<'_, str> {
    if has_extension(path) {
        Cow::Borrowed(path)
    } else {
        Cow::Owned(format!("{path}.pug"))
    }
}

/// The folder an absolute path starts from, standing in for the `basedir` pug-rails users set.
fn asset_root(current: &Path) -> Option<PathBuf> {
    let root = paths::rails_root(current)?;
    ASSET_ROOTS.iter().map(|dir| root.join(dir)).find(|dir| dir.is_dir())
}

/// The file Pug reads for `path` written in `current`, whether or not it exists; `None` for an absolute path without an
/// asset root.
pub fn resolve(current: &Path, path: &str) -> Option<PathBuf> {
    let path = implied(path);
    let joined = match path.strip_prefix('/') {
        Some(rooted) => asset_root(current)?.join(rooted),
        None => current.parent()?.join(&*path),
    };
    Some(lexically_normal(&joined))
}

/// Every node under `block`, depth first in document order, as pug-walk visits them.
fn nodes(block: &Block) -> Vec<&Node> {
    let mut out = Vec::new();
    push_block(block, &mut out);
    out
}

fn push_block<'a>(block: &'a Block, out: &mut Vec<&'a Node>) {
    block.nodes.iter().for_each(|node| push_node(node, out));
}

fn push_node<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
    out.push(node);
    match &node.kind {
        NodeKind::Tag(tag) | NodeKind::MixinCall { tag, .. } => push_block(&tag.block, out),
        NodeKind::Conditional(conditional) => {
            push_block(&conditional.consequent, out);
            match &conditional.alternate {
                Some(Alternate::ElseIf(node)) => push_node(node, out),
                Some(Alternate::Else(block)) => push_block(block, out),
                None => {}
            }
        }
        NodeKind::Each { block, alternate, .. } => {
            push_block(block, out);
            alternate.iter().for_each(|block| push_block(block, out));
        }
        NodeKind::When { block: Some(block), .. }
        | NodeKind::While { block, .. }
        | NodeKind::Case { block, .. }
        | NodeKind::NamedBlock { block, .. }
        | NodeKind::MixinDefinition { block, .. }
        | NodeKind::Include { block, .. }
        | NodeKind::Filter { block, .. } => push_block(block, out),
        NodeKind::Code(code) => code.block.iter().for_each(|block| push_block(block, out)),
        _ => {}
    }
}

/// The `include` and `extends` paths of a tree.
fn file_references(tree: &Block) -> impl Iterator<Item = (Keyword, &FileReference)> {
    nodes(tree).into_iter().filter_map(|node| match &node.kind {
        NodeKind::Include { file, .. } | NodeKind::RawInclude { file, .. } => Some((Keyword::Include, file)),
        NodeKind::Extends { file } => Some((Keyword::Extends, file)),
        _ => None,
    })
}

/// The layout a template extends: its first statement, when that is an `extends`.
fn layout_of(tree: &Block) -> Option<&FileReference> {
    match tree.nodes.first().map(|node| &node.kind) {
        Some(NodeKind::Extends { file }) => Some(file),
        _ => None,
    }
}

struct Source<'a> {
    text: &'a str,
    index: LineIndex<'a>,
}

impl<'a> Source<'a> {
    fn new(text: &'a str) -> Self {
        Source { text, index: LineIndex::new(text) }
    }

    fn node_start(&self, node: &Node) -> usize {
        offset(&self.index, node.line, node.column)
    }

    /// From `start` to the end of its line's text.
    fn rest_of_line(&self, start: usize) -> Range {
        let line = self.index.line_of(start);
        let end = self.index.line_start(line) + self.index.line(line).trim_end().len();
        self.index.range(start, end.max(start))
    }

    fn file_ref(&self, keyword: Keyword, file: &FileReference) -> FileRef {
        let at = offset(&self.index, file.line, file.column);
        // Pug's column is past the spaces after the keyword, not past a tab.
        let start = at + self.text[at..].len() - self.text[at..].trim_start_matches([' ', '\t']).len();
        FileRef { keyword, path: file.path.clone(), start, end: start + file.path.len() }
    }
}

/// The `include` and `extends` paths of `text`, or of the lines above its first syntax error.
pub fn file_refs(text: &str) -> Vec<FileRef> {
    let Some(tree) = parsed(text) else { return Vec::new() };
    let source = Source::new(text);
    file_references(&tree).map(|(keyword, file)| source.file_ref(keyword, file)).collect()
}

/// The path the cursor, at byte `at`, is in or next to.
pub fn ref_at(text: &str, at: usize) -> Option<FileRef> {
    file_refs(text).into_iter().find(|r| (r.start..=r.end).contains(&at))
}

/// The file `r` names, when it exists.
pub fn target(current: &Path, r: &FileRef) -> Option<PathBuf> {
    resolve(current, &r.path).filter(|file| file.is_file())
}

/// What hovering a path shows: the file Pug reads for it.
pub fn hover(current: &Path, r: &FileRef) -> String {
    let title = format!("**{}** `{}`", r.keyword.name(), r.path);
    match resolve(current, &r.path) {
        Some(file) if file.is_file() => format!("{title}\n\n`{}`", paths::rails_relative(&file, current)),
        Some(file) => format!("{title}\n\nNo such file: `{}`", paths::rails_relative(&file, current)),
        None => format!("{title}\n\nAn absolute path starts from `{}` or `{}`; neither exists.", ASSET_ROOTS[0], ASSET_ROOTS[1]),
    }
}

/// The keyword, the byte column where the last part of the path starts and the path typed before byte `col`, in a line
/// that starts with `include `, `include:filter ` or `extends `.
pub fn path_prefix(line: &str, col: usize) -> Option<(Keyword, usize, &str)> {
    static TYPING: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[ \t]*(include(?::[\w-]+(?:\([^)]*\))?)*|extends) +(.*)$").unwrap());
    let found = TYPING.captures(line.get(..col)?)?;
    let keyword = if found[1].starts_with("extends") { Keyword::Extends } else { Keyword::Include };
    let typed = found.get(2)?;
    let last_part = typed.as_str().rfind('/').map_or(0, |slash| slash + 1);
    Some((keyword, typed.start() + last_part, typed.as_str()))
}

/// What can follow `typed` (up to its last `/`): the folders and files in the folder it names, from the template's folder
/// or, after a leading `/`, the asset root. A `.pug` file is named without its extension, as it is written; `extends`
/// takes Pug files only.
pub fn candidates(current: &Path, keyword: Keyword, typed: &str) -> Vec<Candidate> {
    let folder = &typed[..typed.rfind('/').map_or(0, |slash| slash + 1)];
    let base = match folder.strip_prefix('/') {
        Some(rooted) => asset_root(current).map(|root| root.join(rooted)),
        None => current.parent().map(|dir| dir.join(folder)),
    };
    let Some(Ok(entries)) = base.map(std::fs::read_dir) else { return Vec::new() };
    let mut found: Vec<Candidate> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let file = entry.path();
            if name.starts_with('.') || file == current {
                return None;
            }
            if file.is_dir() {
                return Some(Candidate { label: format!("{name}/"), folder: true, file });
            }
            if keyword == Keyword::Extends && !(name.ends_with(".pug") || name.ends_with(".jade")) {
                return None;
            }
            let label = name.strip_suffix(".pug").filter(|stem| !has_extension(stem)).map_or_else(|| name.clone(), str::to_string);
            Some(Candidate { label, folder: false, file })
        })
        .collect();
    found.sort_by(|a, b| a.label.cmp(&b.label));
    found
}

/// The problems with what a template that parses includes and extends, which Pug reports when it compiles it (a mixin
/// call, when it renders it).
pub fn diagnostics(text: &str, tree: &Block, current: &Path) -> Vec<Diagnostic> {
    let source = Source::new(text);
    let current = lexically_normal(current);
    let mut out = missing_files(&source, tree, &current);
    out.extend(misplaced_extends(&source, tree));
    out.extend(unexpected_blocks(&source, tree, &current));
    out.extend(unknown_mixins(&source, tree, &current));
    out
}

/// Paths whose file doesn't exist; an absolute path without an asset root is left alone, since `basedir` can be set.
fn missing_files(source: &Source, tree: &Block, current: &Path) -> Vec<Diagnostic> {
    file_references(tree)
        .filter_map(|(keyword, file)| {
            let missing = resolve(current, &file.path).filter(|found| !found.is_file())?;
            let r = source.file_ref(keyword, file);
            let message = format!("Missing file {}", paths::rails_relative(&missing, current));
            Some(coded(error(source.index.range(r.start, r.end), message), code::MISSING_FILE))
        })
        .collect()
}

/// pug-linker's `EXTENDS_NOT_FIRST`: every `extends` but one that is the template's first statement.
fn misplaced_extends(source: &Source, tree: &Block) -> Vec<Diagnostic> {
    let extends = nodes(tree).into_iter().filter(|node| matches!(node.kind, NodeKind::Extends { .. }));
    extends
        .skip(usize::from(layout_of(tree).is_some()))
        .map(|node| coded(error(source.rest_of_line(source.node_start(node)), EXTENDS_NOT_FIRST_MESSAGE), code::EXTENDS_NOT_FIRST))
        .collect()
}

/// pug-linker's `UNEXPECTED_BLOCK`: a block of an extending template's own top level that its layouts don't have.
fn unexpected_blocks(source: &Source, tree: &Block, current: &Path) -> Vec<Diagnostic> {
    let Some(layout) = layout_of(tree) else { return Vec::new() };
    let own: Vec<(&Node, &str, &Block)> = tree
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            NodeKind::NamedBlock { name, block, .. } => Some((node, name.as_str(), block)),
            _ => None,
        })
        .collect();
    let mut linker = Linker::new(current);
    let Some(mut known) = linker.follow(current, &layout.path, Linker::layout_block_names) else { return Vec::new() };
    // A block nested in one of the template's own takes that one's place in the layout, so the layout has it too.
    for (_, _, block) in &own {
        let Some(nested) = linker.block_names(block, current) else { return Vec::new() };
        known.extend(nested);
    }
    own.into_iter()
        .filter(|(_, name, _)| !known.contains(*name))
        .map(|(node, name, _)| {
            let message = format!("Unexpected block {name}");
            coded(error(source.rest_of_line(source.node_start(node)), message), code::UNEXPECTED_BLOCK)
        })
        .collect()
}

/// `+name` calls with no `mixin name` in the template, the files it includes or its layouts, which fail when the template
/// renders. Not reported in a template another one includes or extends, whose mixins it may call.
fn unknown_mixins(source: &Source, tree: &Block, current: &Path) -> Vec<Diagnostic> {
    let calls: Vec<(&Node, &str)> = nodes(tree)
        .into_iter()
        .filter_map(|node| match &node.kind {
            // `+#{expr}` names its mixin at render time.
            NodeKind::MixinCall { tag, .. } if !tag.name.starts_with("#{") => Some((node, tag.name.as_str())),
            _ => None,
        })
        .collect();
    if calls.is_empty() {
        return Vec::new();
    }
    let Some(defined) = Linker::new(current).mixin_names(tree, current) else { return Vec::new() };
    let unknown: Vec<(&Node, &str)> = calls.into_iter().filter(|(_, name)| !defined.contains(*name)).collect();
    if unknown.is_empty() || pulled_in_elsewhere(current) {
        return Vec::new();
    }
    unknown
        .into_iter()
        .map(|(node, name)| {
            let start = source.node_start(node);
            let end = source.text[start..].find(name).map_or(start, |at| start + at + name.len());
            let message = format!("Mixin `{name}` is not defined here, in an included file or in a layout");
            coded(warning(source.index.range(start, end), message), code::UNKNOWN_MIXIN)
        })
        .collect()
}

/// Whether a template in the same app (or, outside one, the same folder tree) includes or extends `current`.
fn pulled_in_elsewhere(current: &Path) -> bool {
    let Some(root) = paths::app_root(current).or_else(|| current.parent().map(Path::to_path_buf)) else { return true };
    let wanted = |entry: &walkdir::DirEntry| {
        let name = entry.file_name().to_string_lossy();
        entry.depth() == 0 || !(name.starts_with('.') || SKIPPED_FOLDERS.contains(&name.as_ref()))
    };
    let is_template = |entry: &walkdir::DirEntry| {
        let name = entry.file_name().to_string_lossy();
        entry.file_type().is_file() && (name.ends_with(".pug") || name.ends_with(".jade"))
    };
    let templates = WalkDir::new(&root).into_iter().filter_entry(wanted).flatten().filter(is_template);
    for (scanned, entry) in templates.enumerate() {
        if scanned == MAX_SCANNED {
            return true;
        }
        let file = entry.path();
        if file == current {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        if !(text.contains("include") || text.contains("extends")) {
            continue;
        }
        // A template that doesn't parse doesn't compile, so it pulls in nothing.
        let Ok(tree) = parser::parse(&text) else { continue };
        if file_references(&tree).any(|(_, r)| resolve(file, &r.path).as_deref() == Some(current)) {
            return true;
        }
    }
    false
}

/// Follows includes and layouts as pug-linker does. Every lookup gives `None` when a file on the way is missing, doesn't
/// parse or includes itself again (Pug then recurses until its stack runs out), since what it would add is unknown.
struct Linker {
    open: Vec<PathBuf>,
}

impl Linker {
    fn new(current: &Path) -> Self {
        Linker { open: vec![current.to_path_buf()] }
    }

    /// `visit` on the tree of the file `path`, written in `from`, names.
    fn follow<T>(&mut self, from: &Path, path: &str, visit: impl FnOnce(&mut Self, &Block, &Path) -> Option<T>) -> Option<T> {
        let file = resolve(from, path)?;
        if self.open.contains(&file) {
            return None;
        }
        let tree = parser::parse(&std::fs::read_to_string(&file).ok()?).ok()?;
        self.open.push(file.clone());
        let found = visit(self, &tree, &file);
        self.open.pop();
        found
    }

    /// The blocks in `block` and in the files it includes; an included file that extends a layout loses its blocks.
    fn block_names(&mut self, block: &Block, file: &Path) -> Option<HashSet<String>> {
        let mut names = HashSet::new();
        for node in nodes(block) {
            match &node.kind {
                NodeKind::NamedBlock { name, .. } => {
                    names.insert(name.clone());
                }
                NodeKind::Include { file: included, .. } => {
                    let found = self.follow(file, &included.path, |linker, tree, path| match layout_of(tree) {
                        Some(_) => Some(HashSet::new()),
                        None => linker.block_names(tree, path),
                    })?;
                    names.extend(found);
                }
                _ => {}
            }
        }
        Some(names)
    }

    /// The blocks a template that extends `tree` can fill: its own and its includes', and its layouts' up the chain.
    fn layout_block_names(&mut self, tree: &Block, file: &Path) -> Option<HashSet<String>> {
        let mut names = self.block_names(tree, file)?;
        if let Some(layout) = layout_of(tree) {
            names.extend(self.follow(file, &layout.path, Self::layout_block_names)?);
        }
        Some(names)
    }

    /// The mixins `tree`, the files it includes and its layouts define.
    fn mixin_names(&mut self, tree: &Block, file: &Path) -> Option<HashSet<String>> {
        let mut names = HashSet::new();
        for node in nodes(tree) {
            match &node.kind {
                NodeKind::MixinDefinition { name, .. } => {
                    names.insert(name.clone());
                }
                NodeKind::Include { file: other, .. } | NodeKind::Extends { file: other } => {
                    names.extend(self.follow(file, &other.path, Self::mixin_names)?);
                }
                _ => {}
            }
        }
        Some(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::write_files;

    fn temp(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("rt-pug-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    /// (line, code, message) of the reference diagnostics of `file` under `root`.
    fn found(root: &Path, file: &str) -> Vec<(u32, String, String)> {
        let path = root.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        let tree = parser::parse(&text).unwrap();
        diagnostics(&text, &tree, &path)
            .into_iter()
            .map(|d| (d.range.start.line, crate::util::code(&d).unwrap_or_default().to_string(), d.message))
            .collect()
    }

    fn at(line: u32, code: &str, message: &str) -> (u32, String, String) {
        (line, code.to_string(), message.to_string())
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

    #[test]
    fn relative_paths_resolve_from_the_folder_and_absolute_ones_from_the_asset_root() {
        let root = temp("resolve");
        let current = root.join("app/assets/templates/users/show.jst.pug");
        assert_eq!(resolve(&current, "row"), Some(root.join("app/assets/templates/users/row.pug")));
        assert_eq!(resolve(&current, "../shared/./card.pug"), Some(root.join("app/assets/templates/shared/card.pug")));
        assert_eq!(resolve(&current, "/layout"), None, "no asset root");
        write_files(&root, &[("app/assets/javascripts/application.js", "")]);
        assert_eq!(resolve(&current, "/layout"), Some(root.join("app/assets/javascripts/layout.pug")));
        write_files(&root, &[("app/assets/templates/layout.pug", "")]);
        assert_eq!(resolve(&current, "/layout"), Some(root.join("app/assets/templates/layout.pug")), "templates first");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn spans_cover_the_path_as_written() {
        let text = "\u{feff}extends  layout\nblock content\n  include:markdown(x=1) notes.md\n  div\n    include \tpartials/row\n";
        let spans: Vec<(Keyword, &str)> = file_refs(text).iter().map(|r| (r.keyword, &text[r.start..r.end])).collect();
        assert_eq!(spans, [(Keyword::Extends, "layout"), (Keyword::Include, "notes.md"), (Keyword::Include, "partials/row")]);
        let offset = text.find("notes").unwrap() + 2;
        assert_eq!(ref_at(text, offset).map(|r| r.path), Some("notes.md".to_string()));
        assert_eq!(ref_at("include a\np #[b", 9).map(|r| r.path), Some("a".to_string()), "lines above a syntax error");
    }

    #[test]
    fn the_typed_path_after_a_keyword() {
        assert_eq!(path_prefix("include shared/ca", 17), Some((Keyword::Include, 15, "shared/ca")));
        assert_eq!(path_prefix("  include:markdown(a=1) ", 24), Some((Keyword::Include, 24, "")));
        assert_eq!(path_prefix("extends /lay", 12), Some((Keyword::Extends, 9, "/lay")));
        assert!(path_prefix("include", 7).is_none());
        assert!(path_prefix("p include x", 11).is_none());
        assert!(path_prefix("includes x", 10).is_none());
    }

    #[test]
    fn candidates_are_named_as_written() {
        let root = temp("candidates");
        write_files(
            &root,
            &[
                ("app/assets/templates/users/show.pug", ""),
                ("app/assets/templates/users/row.pug", ""),
                ("app/assets/templates/users/cell.jst.pug", ""),
                ("app/assets/templates/users/old.jade", ""),
                ("app/assets/templates/users/notes.md", ""),
                ("app/assets/templates/users/.hidden.pug", ""),
                ("app/assets/templates/users/parts/a.pug", ""),
                ("app/assets/templates/layout.pug", ""),
            ],
        );
        let current = root.join("app/assets/templates/users/show.pug");
        let labels = |keyword, typed| candidates(&current, keyword, typed).into_iter().map(|c| c.label).collect::<Vec<_>>();
        assert_eq!(labels(Keyword::Include, "r"), ["cell.jst.pug", "notes.md", "old.jade", "parts/", "row"]);
        assert_eq!(labels(Keyword::Extends, ""), ["cell.jst.pug", "old.jade", "parts/", "row"]);
        assert_eq!(labels(Keyword::Include, "parts/x"), ["a"]);
        assert_eq!(labels(Keyword::Extends, "/"), ["layout", "users/"]);
        assert!(labels(Keyword::Include, "missing/").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_files_and_misplaced_extends() {
        let root = temp("missing");
        write_files(
            &root,
            &[
                ("app/assets/templates/layout.pug", "block content\n"),
                ("app/assets/templates/users/row.pug", "p row\n"),
                (
                    "app/assets/templates/users/show.pug",
                    concat!(
                        "extends ../layout\nblock content\n  include row\n  include missing\n  include:markdown notes.md\n",
                        "  include /shared/nav\n  include row.jst.pug\n  div\n    extends ../layout\n",
                    ),
                ),
                ("app/assets/templates/users/doctype.pug", "doctype html\n//- dropped\nextends ../layout\n"),
                ("app/assets/templates/users/first.pug", "//- dropped\n\nextends ../layout\n"),
            ],
        );
        let missing = |file: &str| format!("Missing file app/assets/templates/{file}");
        assert_eq!(
            found(&root, "app/assets/templates/users/show.pug"),
            [
                at(3, code::MISSING_FILE, &missing("users/missing.pug")),
                at(4, code::MISSING_FILE, &missing("users/notes.md")),
                at(5, code::MISSING_FILE, &missing("shared/nav.pug")),
                at(6, code::MISSING_FILE, &missing("users/row.jst.pug")),
                at(8, code::EXTENDS_NOT_FIRST, EXTENDS_NOT_FIRST_MESSAGE),
            ]
        );
        assert_eq!(found(&root, "app/assets/templates/users/doctype.pug"), [at(2, code::EXTENDS_NOT_FIRST, EXTENDS_NOT_FIRST_MESSAGE)]);
        assert!(found(&root, "app/assets/templates/users/first.pug").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn blocks_the_layout_chain_lacks() {
        let root = temp("blocks");
        write_files(
            &root,
            &[
                ("base.pug", "html\n  head\n    block head\n  body\n    include nav\n    block body\n"),
                ("nav.pug", "nav\n  block nav\n"),
                ("extending.pug", "extends base\nblock body\n  block hidden\n"),
                ("layout.pug", "extends base\nblock body\n  main\n    block content\n  include extending\n"),
                (
                    "page.pug",
                    "extends layout\nblock head\nblock append nav\nblock content\n  block inner\nblock inner\nblock hidden\nblock nope\n",
                ),
                ("broken.pug", "extends broken-layout\nblock nope\n"),
                ("broken-layout.pug", "div\n    p\n  a\n"),
                ("lost.pug", "extends gone\nblock nope\n"),
                ("cycle-a.pug", "extends cycle-b\nblock nope\n"),
                ("cycle-b.pug", "extends cycle-a\n"),
            ],
        );
        assert_eq!(
            found(&root, "page.pug"),
            [at(6, code::UNEXPECTED_BLOCK, "Unexpected block hidden"), at(7, code::UNEXPECTED_BLOCK, "Unexpected block nope")]
        );
        assert!(found(&root, "broken.pug").is_empty(), "a layout that doesn't parse");
        let gone = format!("Missing file {}", crate::util::slash_path(&root.join("gone.pug")));
        assert_eq!(found(&root, "lost.pug"), [at(0, code::MISSING_FILE, &gone)], "only the missing layout");
        assert!(found(&root, "cycle-a.pug").is_empty(), "a cycle");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mixins_from_includes_and_layouts() {
        let root = temp("mixins");
        write_files(
            &root,
            &[
                ("app/assets/templates/mixins/cards.pug", "mixin card(x)\n  .card= x\ninclude more\n"),
                ("app/assets/templates/mixins/more.pug", "mixin badge\n  span\n"),
                ("app/assets/templates/layout.pug", "include mixins/cards\nmixin frame\n  block\nblock content\n"),
                (
                    "app/assets/templates/page.jst.pug",
                    "extends layout\nblock content\n  +card(1)\n  +badge\n  +frame: p\n  +#{dynamic}\n  +missing(2)\n  +local\nmixin local\n  p\n",
                ),
                ("app/assets/templates/cycle.pug", "include cycle\n+missing\n"),
                ("app/assets/templates/broken.pug", "include broken-mixins\n+missing\n"),
                ("app/assets/templates/broken-mixins.pug", "mixin x\n    p\n  a\n"),
                ("app/assets/templates/partial.pug", "+fromIncluder\n"),
                ("app/assets/templates/includer.jst.pug", "mixin fromIncluder\n  p\ninclude partial\n"),
            ],
        );
        assert_eq!(
            found(&root, "app/assets/templates/page.jst.pug"),
            [at(6, code::UNKNOWN_MIXIN, "Mixin `missing` is not defined here, in an included file or in a layout")]
        );
        assert!(found(&root, "app/assets/templates/cycle.pug").is_empty(), "an include cycle");
        assert!(found(&root, "app/assets/templates/broken.pug").is_empty(), "an include that doesn't parse");
        assert!(found(&root, "app/assets/templates/partial.pug").is_empty(), "included elsewhere");
        let _ = std::fs::remove_dir_all(&root);
    }
}
