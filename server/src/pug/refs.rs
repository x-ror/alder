//! The files a Pug template names with `include` and `extends`, found as Pug 3 finds them, and the checks that follow
//! them: `pug.compile` adds `.pug` to a path without an extension, and pug-load reads a relative path from the template's
//! folder and an absolute one from `basedir`, here the app's asset root.

use super::parser::{self, Block, FileReference, Node, NodeKind};
use super::{code, nodes, offset, parsed};
use crate::rails::paths;
use crate::util::{coded, error, lexically_normal, warning, LineIndex};
use dashmap::DashMap;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, LazyLock};
use tower_lsp_server::ls_types::{Diagnostic, Range};
use walkdir::WalkDir;

/// Where an absolute path starts, the first that exists: the folder pug-rails' README suggests, then Sprockets' default.
const ASSET_ROOTS: [&str; 2] = ["app/assets/templates", "app/assets/javascripts"];
/// Folders the search for templates that include another doesn't walk into, besides the hidden ones.
const SKIPPED_FOLDERS: [&str; 1] = ["node_modules"];
/// How many files and folders that search visits before it gives up, not knowing.
const MAX_VISITED: usize = 20_000;
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
            if name.starts_with('.') || lexically_normal(&file) == lexically_normal(current) {
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
/// call, when it renders it); only those whose code `reports` takes are looked for.
pub fn diagnostics(text: &str, tree: &Block, current: &Path, reports: &dyn Fn(&str) -> bool) -> Vec<Diagnostic> {
    let source = Source::new(text);
    let current = lexically_normal(current);
    let mut linker = Linker::new(&current);
    let mut out = Vec::new();
    if reports(code::MISSING_FILE) {
        out.extend(missing_files(&source, tree, &current));
    }
    if reports(code::EXTENDS_NOT_FIRST) {
        out.extend(misplaced_extends(&source, tree));
    }
    if reports(code::UNEXPECTED_BLOCK) {
        out.extend(unexpected_blocks(&source, tree, &current, &mut linker));
    }
    if reports(code::UNKNOWN_MIXIN) {
        out.extend(unknown_mixins(&source, tree, &current, &mut linker));
    }
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
fn unexpected_blocks(source: &Source, tree: &Block, current: &Path, linker: &mut Linker) -> Vec<Diagnostic> {
    let Some(layout) = layout_of(tree) else { return Vec::new() };
    let own: Vec<(&Node, &str, &Block)> = tree
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            NodeKind::NamedBlock { name, block, .. } => Some((node, name.as_str(), block)),
            _ => None,
        })
        .collect();
    let Some(mut known) = linker.follow(current, &layout.path, Lookup::LayoutBlocks) else { return Vec::new() };
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
/// renders. Not reported in a template another one in its app includes or extends, whose mixins it may call, nor outside
/// an app, where what includes it can't be told.
fn unknown_mixins(source: &Source, tree: &Block, current: &Path, linker: &mut Linker) -> Vec<Diagnostic> {
    let calls: Vec<(&Node, &str)> = nodes(tree)
        .into_iter()
        .filter_map(|node| match &node.kind {
            // `+#{expr}` names its mixin at render time.
            NodeKind::MixinCall { tag, .. } if !tag.name.starts_with("#{") => Some((node, tag.name.as_str())),
            _ => None,
        })
        .collect();
    let Some(app) = paths::app_root(current).filter(|_| !calls.is_empty()) else { return Vec::new() };
    let Some(defined) = linker.mixin_names(tree, current) else { return Vec::new() };
    let unknown: Vec<(&Node, &str)> = calls.into_iter().filter(|(_, name)| !defined.contains(*name)).collect();
    if unknown.is_empty() || pulled_in_elsewhere(&app, current) {
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

/// Per app folder, the templates another template in it includes or extends; `None` when it has too many files to tell.
static PULLED_IN: LazyLock<DashMap<PathBuf, Option<Arc<HashSet<PathBuf>>>>> = LazyLock::new(DashMap::new);

/// Drops what `pulled_in_elsewhere` remembers, when a watched template may have changed it.
pub fn clear_cache() {
    PULLED_IN.clear();
}

/// Whether a template in the app folder `app` includes or extends `current`, as the files on disk say.
fn pulled_in_elsewhere(app: &Path, current: &Path) -> bool {
    let pulled_in = PULLED_IN.entry(app.to_path_buf()).or_insert_with(|| pulled_in(app).map(Arc::new)).clone();
    pulled_in.is_none_or(|files| files.contains(current))
}

/// The templates under `root` that another one includes or extends.
fn pulled_in(root: &Path) -> Option<HashSet<PathBuf>> {
    let wanted = |entry: &walkdir::DirEntry| {
        let name = entry.file_name().to_string_lossy();
        entry.depth() == 0 || !(name.starts_with('.') || SKIPPED_FOLDERS.contains(&name.as_ref()))
    };
    let is_template = |entry: &walkdir::DirEntry| {
        let name = entry.file_name().to_string_lossy();
        entry.file_type().is_file() && (name.ends_with(".pug") || name.ends_with(".jade"))
    };
    let mut found = HashSet::new();
    for (visited, entry) in WalkDir::new(root).into_iter().filter_entry(wanted).enumerate() {
        if visited == MAX_VISITED {
            return None;
        }
        let Some(entry) = entry.ok().filter(is_template) else { continue };
        let file = entry.path();
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        if !(text.contains("include") || text.contains("extends")) {
            continue;
        }
        // A template that doesn't parse doesn't compile, so it pulls in nothing.
        let Ok(tree) = parser::parse(&text) else { continue };
        found.extend(file_references(&tree).filter_map(|(_, r)| resolve(file, &r.path)).filter(|target| target != file));
    }
    Some(found)
}

/// What `Linker::follow` looks for in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Lookup {
    /// `block_names` of an included file, none when it extends a layout.
    IncludedBlocks,
    LayoutBlocks,
    Mixins,
}

type Names = HashSet<String>;

/// Follows includes and layouts as pug-linker does. Every lookup gives `None` when a file on the way is missing, doesn't
/// parse or includes itself again (Pug then recurses until its stack runs out), since what it would add is unknown.
struct Linker {
    open: Vec<PathBuf>,
    /// Each file read, `None` when it can't be read or doesn't parse.
    trees: HashMap<PathBuf, Option<Rc<Block>>>,
    /// What a lookup found in a file. It is the same wherever the file is reached from: one that reaches an open file is
    /// in a cycle with it.
    found: HashMap<(Lookup, PathBuf), Option<Names>>,
}

impl Linker {
    fn new(current: &Path) -> Self {
        Linker { open: vec![current.to_path_buf()], trees: HashMap::new(), found: HashMap::new() }
    }

    /// `lookup` on the tree of the file `path`, written in `from`, names.
    fn follow(&mut self, from: &Path, path: &str, lookup: Lookup) -> Option<Names> {
        let file = resolve(from, path)?;
        if let Some(found) = self.found.get(&(lookup, file.clone())) {
            return found.clone();
        }
        if self.open.contains(&file) {
            return None;
        }
        let tree = self.tree(&file)?;
        self.open.push(file.clone());
        let found = match lookup {
            Lookup::IncludedBlocks if layout_of(&tree).is_some() => Some(Names::new()),
            Lookup::IncludedBlocks => self.block_names(&tree, &file),
            Lookup::LayoutBlocks => self.layout_block_names(&tree, &file),
            Lookup::Mixins => self.mixin_names(&tree, &file),
        };
        self.open.pop();
        self.found.insert((lookup, file), found.clone());
        found
    }

    fn tree(&mut self, file: &Path) -> Option<Rc<Block>> {
        let read = || parser::parse(&std::fs::read_to_string(file).ok()?).ok().map(Rc::new);
        self.trees.entry(file.to_path_buf()).or_insert_with(read).clone()
    }

    /// The blocks in `block` and in the files it includes; an included file that extends a layout loses its blocks.
    fn block_names(&mut self, block: &Block, file: &Path) -> Option<Names> {
        let mut names = Names::new();
        for node in nodes(block) {
            match &node.kind {
                NodeKind::NamedBlock { name, .. } => {
                    names.insert(name.clone());
                }
                NodeKind::Include { file: included, .. } => names.extend(self.follow(file, &included.path, Lookup::IncludedBlocks)?),
                _ => {}
            }
        }
        Some(names)
    }

    /// The blocks a template that extends `tree` can fill: its own and its includes', and its layouts' up the chain.
    fn layout_block_names(&mut self, tree: &Block, file: &Path) -> Option<Names> {
        let mut names = self.block_names(tree, file)?;
        if let Some(layout) = layout_of(tree) {
            names.extend(self.follow(file, &layout.path, Lookup::LayoutBlocks)?);
        }
        Some(names)
    }

    /// The mixins `tree`, the files it includes and its layouts define.
    fn mixin_names(&mut self, tree: &Block, file: &Path) -> Option<Names> {
        let mut names = Names::new();
        for node in nodes(tree) {
            match &node.kind {
                NodeKind::MixinDefinition { name, .. } => {
                    names.insert(name.clone());
                }
                NodeKind::Include { file: other, .. } | NodeKind::Extends { file: other } => {
                    names.extend(self.follow(file, &other.path, Lookup::Mixins)?);
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
        diagnostics(&text, &tree, &path, &|_| true)
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
        assert_eq!(labels(Keyword::Include, "../users/"), ["cell.jst.pug", "notes.md", "old.jade", "parts/", "row"], "not itself");
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

    #[test]
    fn mixins_are_not_reported_outside_an_app() {
        let root = temp("no-app");
        write_files(&root, &[("x.pug", "+missing\n")]);
        assert!(found(&root, "x.pug").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn what_includes_a_template_is_read_again_once_the_cache_is_cleared() {
        let root = temp("includers");
        let unknown = [at(0, code::UNKNOWN_MIXIN, "Mixin `m` is not defined here, in an included file or in a layout")];
        write_files(&root, &[("app/assets/templates/_row.pug", "+m\n")]);
        assert_eq!(found(&root, "app/assets/templates/_row.pug"), unknown);
        write_files(&root, &[("app/assets/templates/list.pug", "mixin m\n  p\ninclude _row\n")]);
        assert_eq!(found(&root, "app/assets/templates/_row.pug"), unknown, "remembered");
        clear_cache();
        assert!(found(&root, "app/assets/templates/_row.pug").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_reached_by_many_paths_is_followed_once() {
        // Each level includes both files of the next: 2^40 paths to the last level.
        const LEVELS: usize = 40;
        let root = temp("diamond");
        let mut files: Vec<(String, String)> = (0..LEVELS)
            .flat_map(|level| {
                let next = format!("include a{0}\ninclude b{0}\n", level + 1);
                [(format!("app/a{level}.pug"), next.clone()), (format!("app/b{level}.pug"), next)]
            })
            .collect();
        files.extend([(format!("app/a{LEVELS}.pug"), "mixin m\n  p\n".into()), (format!("app/b{LEVELS}.pug"), String::new())]);
        files.push(("app/page.pug".into(), "extends a0\nblock x\n+m\n+n\n".into()));
        let files: Vec<(&str, &str)> = files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect();
        write_files(&root, &files);
        let message = "Mixin `n` is not defined here, in an included file or in a layout";
        assert_eq!(
            found(&root, "app/page.pug"),
            [at(1, code::UNEXPECTED_BLOCK, "Unexpected block x"), at(3, code::UNKNOWN_MIXIN, message)]
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
