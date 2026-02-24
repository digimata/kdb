//! Markdown parser, vault indexer, and link resolver.
//!
//! This is the core of kdb. It parses markdown files to extract headings and
//! links, builds an in-memory index of the entire vault, and validates that
//! all references resolve to real targets.
//!
//! # Architecture
//!
//! 1. **Parsing** — [`parse_markdown`] extracts headings and links from a single file.
//! 2. **Indexing** — [`VaultIndex::build`] walks the vault directory and parses every
//!    `.md` file, building maps of files, headings, and inbound links.
//! 3. **Checking** — [`VaultIndex::check`] validates all links and reports broken
//!    references and orphan files.
//! 4. **Resolution** — [`resolve_target_path`] resolves a link target relative to
//!    its source file, handling both markdown and wikilink syntax.

pub mod deps;
pub mod refs;

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;
use walkdir::WalkDir;

// ----------------------------------------------
// src/index/mod.rs
//
// struct VaultIndex                          L90
// struct FileEntry                          L105
// struct Heading                            L118
// enum LinkKind                             L134
// struct LinkTarget                         L143
// struct Link                               L152
// struct HeadingKey                         L167
// struct LinkRef                            L176
// struct ParsedDocument                     L189
// struct BrokenLink                         L198
// struct CheckReport                        L213
//   fn CheckReport::has_errors()            L222
//   fn CheckReport::print()                 L227
//   fn VaultIndex::build()                  L290
//   fn VaultIndex::build_with_ignores()     L298
//   fn VaultIndex::upsert_file()            L337
//   fn VaultIndex::reload_file()            L360
//   fn VaultIndex::remove_file()            L389
//   fn VaultIndex::check()                  L397
//   fn VaultIndex::populate_inbound()       L436
//   fn VaultIndex::resolve_link()           L489
// enum ResolveError                         L522
//   fn ResolveError::message()              L529
// fn parse_markdown()                       L550
// fn build_ignore_globset()                 L690
// fn discover_markdown_files()              L703
// fn rel_path_from_root()                   L770
// fn path_is_ignored()                      L775
// fn resolve_target_file()                  L798
// fn resolve_target_path()                  L811
// fn resolve_file_target()                  L839
// fn normalize_rel_path()                   L863
// fn parse_markdown_target()                L890
// fn parse_wikilink_target()                L938
// fn slug_anchor()                          L975
// fn is_external()                          L984
// fn slug()                                 L996
// fn build_line_starts()                   L1028
// fn line_col()                            L1039
// fn normalize_inline_whitespace()         L1050
// fn heading_level_number()                L1055
// fn range_contains_offset()               L1066
// struct ActiveHeading                     L1073
// ----------------------------------------------

/// Regex for matching `[[wikilink]]` syntax in raw markdown source.
static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]\r\n]+)\]\]").expect("valid wikilink regex"));

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Complete index of a markdown vault.
///
/// Built by scanning all `.md` files under a root directory. Provides the
/// foundation for link validation, reference lookups, and LSP features.
#[derive(Debug, Clone)]
pub struct VaultIndex {
    /// Canonical absolute path to the vault root directory.
    pub root: PathBuf,
    /// Compiled user ignore patterns used during discovery and incremental updates.
    ignore_set: GlobSet,
    /// All indexed markdown files, keyed by their path relative to `root`.
    pub files: BTreeMap<PathBuf, FileEntry>,
    /// Inbound links grouped by target file path.
    pub file_inbound: HashMap<PathBuf, Vec<LinkRef>>,
    /// Inbound links grouped by target heading (file + anchor).
    pub heading_inbound: HashMap<HeadingKey, Vec<LinkRef>>,
}

/// A single indexed markdown file.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Path relative to the vault root (e.g. `notes/react.md`).
    pub rel_path: PathBuf,
    /// Absolute path on disk.
    pub abs_path: PathBuf,
    /// All headings found in this file, in document order.
    pub headings: Vec<Heading>,
    /// All internal links found in this file (external URLs are excluded).
    pub links: Vec<Link>,
}

/// A parsed heading from a markdown file.
#[derive(Debug, Clone)]
pub struct Heading {
    /// The heading text content (e.g. `"Getting Started"`).
    pub title: String,
    /// URL-safe anchor slug (e.g. `"getting-started"`). Deduplicated with
    /// numeric suffixes when multiple headings produce the same slug.
    pub anchor: String,
    /// Heading depth: 1 for `#`, 2 for `##`, etc.
    pub level: u8,
    /// 1-based line number in the source file.
    pub line: usize,
    /// 1-based column number in the source file.
    pub column: usize,
}

/// Whether a link uses standard markdown syntax or wikilink syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    /// Standard markdown: `[text](path.md#anchor)`
    Markdown,
    /// Obsidian-style wikilink: `[[path#anchor]]`
    Wikilink,
}

/// The parsed destination of a link, split into file and anchor components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkTarget {
    /// Target file path (e.g. `"hooks.md"`), or `None` for same-file anchors like `#section`.
    pub file: Option<String>,
    /// Target heading anchor (e.g. `"useEffect"`), or `None` for file-only links.
    pub anchor: Option<String>,
}

/// A parsed link from a markdown file.
#[derive(Debug, Clone)]
pub struct Link {
    /// Whether this is a markdown link or wikilink.
    pub kind: LinkKind,
    /// The raw link text as it appears in the source (e.g. `"hooks.md#useEffect"` or `"[[hooks#useEffect]]"`).
    pub raw: String,
    /// The parsed target components.
    pub target: LinkTarget,
    /// 1-based line number in the source file.
    pub line: usize,
    /// 1-based column number in the source file.
    pub column: usize,
}

/// Uniquely identifies a heading within the vault (file path + anchor slug).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HeadingKey {
    /// Relative path to the file containing the heading.
    pub file: PathBuf,
    /// The heading's anchor slug.
    pub anchor: String,
}

/// A record of where a link originates, used for inbound reference tracking.
#[derive(Debug, Clone)]
pub struct LinkRef {
    /// Relative path to the file containing the link.
    pub source_file: PathBuf,
    /// 1-based line number of the link.
    pub source_line: usize,
    /// 1-based column number of the link.
    pub source_column: usize,
    /// Raw link text for display in diagnostics.
    pub raw: String,
}

/// The result of parsing a single markdown file.
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    /// All headings found in the document.
    pub headings: Vec<Heading>,
    /// All internal links found in the document.
    pub links: Vec<Link>,
}

/// A broken link detected during validation.
#[derive(Debug, Clone)]
pub struct BrokenLink {
    /// File containing the broken link.
    pub source_file: PathBuf,
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number.
    pub column: usize,
    /// Raw link text.
    pub raw: String,
    /// Human-readable explanation of why the link is broken.
    pub reason: String,
}

/// Results from running `kdb check` on a vault.
#[derive(Debug, Clone, Default)]
pub struct CheckReport {
    /// Links that reference files or headings that don't exist.
    pub broken_links: Vec<BrokenLink>,
    /// Files that have no inbound links from other files.
    pub orphans: Vec<PathBuf>,
}

impl CheckReport {
    /// Returns `true` if the report contains any broken links.
    pub fn has_errors(&self) -> bool {
        !self.broken_links.is_empty()
    }

    /// Print a human-readable summary to stdout.
    pub fn print(&self, list_orphans: bool) {
        for broken in &self.broken_links {
            println!(
                "{}:{}:{} broken link {} ({})",
                broken.source_file.display(),
                broken.line,
                broken.column,
                broken.raw,
                broken.reason
            );
        }

        if list_orphans {
            for orphan in &self.orphans {
                println!("{} orphan file (0 inbound links)", orphan.display());
            }
        } else if !self.orphans.is_empty() {
            let noun = if self.orphans.len() == 1 {
                "orphan file"
            } else {
                "orphan files"
            };
            println!(
                "{} {} (run `kdb check --orphans` to list)",
                self.orphans.len(),
                noun
            );
        }

        if self.broken_links.is_empty() && self.orphans.is_empty() {
            println!("kdb check: no issues found");
            return;
        }

        if !self.broken_links.is_empty() {
            let noun = if self.broken_links.len() == 1 {
                "error"
            } else {
                "errors"
            };
            println!("{} {noun}", self.broken_links.len());
        }

        if !self.orphans.is_empty() {
            let noun = if self.orphans.len() == 1 {
                "warning"
            } else {
                "warnings"
            };
            println!("{} {noun}", self.orphans.len());
        }
    }
}

// ---------------------------------------------------------------------------
// Vault index
// ---------------------------------------------------------------------------

impl VaultIndex {
    /// Build an index of the entire vault by discovering and parsing all markdown files.
    ///
    /// Walks the directory tree under `root`, parses each `.md` file, and builds
    /// both the file map and inbound link graphs.
    pub fn build(root: &Path) -> Result<Self> {
        Self::build_with_ignores(root, &[])
    }

    /// Build an index of the entire vault with user-defined ignore patterns.
    ///
    /// Ignore patterns use glob syntax and are matched against slash-separated
    /// paths relative to `root`.
    pub fn build_with_ignores(root: &Path, ignore_patterns: &[String]) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
        let ignore_set = build_ignore_globset(ignore_patterns)?;

        let mut files = BTreeMap::new();
        for rel_path in discover_markdown_files(&root, &ignore_set)? {
            let abs_path = root.join(&rel_path);
            let source = match std::fs::read_to_string(&abs_path) {
                Ok(s) => s,
                Err(_) => continue, // skip files with invalid UTF-8
            };
            let parsed = parse_markdown(&source);
            files.insert(
                rel_path.clone(),
                FileEntry {
                    rel_path,
                    abs_path,
                    headings: parsed.headings,
                    links: parsed.links,
                },
            );
        }

        let mut index = Self {
            root,
            ignore_set,
            files,
            file_inbound: HashMap::new(),
            heading_inbound: HashMap::new(),
        };
        index.populate_inbound();
        Ok(index)
    }

    /// Insert or replace a file in the index using provided source text.
    ///
    /// This supports incremental index updates from unsaved LSP document state.
    pub fn upsert_file(&mut self, rel_path: PathBuf, abs_path: PathBuf, source: &str) {
        if path_is_ignored(&self.ignore_set, &rel_path, false) {
            self.remove_file(&rel_path);
            return;
        }

        let parsed = parse_markdown(source);
        self.files.insert(
            rel_path.clone(),
            FileEntry {
                rel_path,
                abs_path,
                headings: parsed.headings,
                links: parsed.links,
            },
        );
        self.populate_inbound();
    }

    /// Reload a file from disk and update the index.
    ///
    /// If the file no longer exists or can't be read as UTF-8, it is removed
    /// from the index.
    pub fn reload_file(&mut self, rel_path: &Path) {
        if path_is_ignored(&self.ignore_set, rel_path, false) {
            self.remove_file(rel_path);
            return;
        }

        let abs_path = self.root.join(rel_path);
        let source = match std::fs::read_to_string(&abs_path) {
            Ok(source) => source,
            Err(_) => {
                self.remove_file(rel_path);
                return;
            }
        };

        let parsed = parse_markdown(&source);
        self.files.insert(
            rel_path.to_path_buf(),
            FileEntry {
                rel_path: rel_path.to_path_buf(),
                abs_path,
                headings: parsed.headings,
                links: parsed.links,
            },
        );
        self.populate_inbound();
    }

    /// Remove a file from the index.
    pub fn remove_file(&mut self, rel_path: &Path) {
        if self.files.remove(rel_path).is_some() {
            self.populate_inbound();
        }
    }

    /// Validate all links in the vault and return a report of broken references
    /// and orphan files.
    pub fn check(&self) -> CheckReport {
        let mut report = CheckReport::default();

        for (source_file, file_entry) in &self.files {
            for link in &file_entry.links {
                if let Err(error) = self.resolve_link(source_file, link) {
                    report.broken_links.push(BrokenLink {
                        source_file: source_file.clone(),
                        line: link.line,
                        column: link.column,
                        raw: link.raw.clone(),
                        reason: error.message(),
                    });
                }
            }
        }

        for path in self.files.keys() {
            let inbound_from_other_files = self
                .file_inbound
                .get(path)
                .map(|refs| {
                    refs.iter()
                        .filter(|link_ref| link_ref.source_file != *path)
                        .count()
                })
                .unwrap_or(0);

            if inbound_from_other_files == 0 {
                report.orphans.push(path.clone());
            }
        }

        report.orphans.sort();
        report
    }

    /// Walk all links and record which files and headings they point to,
    /// building the inbound reference maps.
    fn populate_inbound(&mut self) {
        let mut file_inbound: HashMap<PathBuf, Vec<LinkRef>> = HashMap::new();
        let mut heading_inbound: HashMap<HeadingKey, Vec<LinkRef>> = HashMap::new();

        for (source_file, file_entry) in &self.files {
            for link in &file_entry.links {
                let Ok(target_file) = resolve_target_file(source_file, link) else {
                    continue;
                };

                if !self.files.contains_key(&target_file) {
                    continue;
                }

                let link_ref = LinkRef {
                    source_file: source_file.clone(),
                    source_line: link.line,
                    source_column: link.column,
                    raw: link.raw.clone(),
                };

                file_inbound
                    .entry(target_file.clone())
                    .or_default()
                    .push(link_ref.clone());

                if let Some(anchor) = link.target.anchor.as_deref().map(slug) {
                    let heading_exists = self.files.get(&target_file).is_some_and(|target| {
                        target
                            .headings
                            .iter()
                            .any(|heading| heading.anchor == anchor)
                    });

                    if heading_exists {
                        heading_inbound
                            .entry(HeadingKey {
                                file: target_file.clone(),
                                anchor,
                            })
                            .or_default()
                            .push(link_ref);
                    }
                }
            }
        }

        self.file_inbound = file_inbound;
        self.heading_inbound = heading_inbound;
    }

    /// Try to resolve a single link against the index. Returns `Err` with the
    /// reason if the target file or heading doesn't exist.
    fn resolve_link(
        &self,
        source_file: &Path,
        link: &Link,
    ) -> std::result::Result<(), ResolveError> {
        let target_file = resolve_target_file(source_file, link)?;
        if !self.files.contains_key(&target_file) {
            return Err(ResolveError::MissingFile(target_file));
        }

        if let Some(raw_anchor) = link.target.anchor.as_deref() {
            let anchor = slug(raw_anchor);
            let heading_exists = self.files.get(&target_file).is_some_and(|target| {
                target
                    .headings
                    .iter()
                    .any(|heading| heading.anchor == anchor)
            });

            if !heading_exists {
                return Err(ResolveError::MissingHeading {
                    file: target_file,
                    anchor,
                });
            }
        }

        Ok(())
    }
}

/// Why a link failed to resolve.
#[derive(Debug)]
enum ResolveError {
    OutsideRoot,
    MissingFile(PathBuf),
    MissingHeading { file: PathBuf, anchor: String },
}

impl ResolveError {
    fn message(&self) -> String {
        match self {
            Self::OutsideRoot => "target resolves outside root".to_string(),
            Self::MissingFile(path) => format!("target file not found: {}", path.display()),
            Self::MissingHeading { file, anchor } => {
                format!("target heading not found: {}#{}", file.display(), anchor)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Markdown parsing
// ---------------------------------------------------------------------------

/// Parse a markdown string and extract all headings and internal links.
///
/// Handles both standard markdown links (`[text](path.md#anchor)`) and
/// wikilinks (`[[path#anchor]]`). External URLs (http, mailto, etc.) are
/// excluded. Headings get auto-generated anchor slugs that are deduplicated
/// with numeric suffixes when collisions occur.
pub fn parse_markdown(content: &str) -> ParsedDocument {
    let line_starts = build_line_starts(content);
    let mut headings = Vec::new();
    let mut links = Vec::new();
    let mut active_heading: Option<ActiveHeading> = None;
    let mut excluded_wikilink_ranges: Vec<(usize, usize)> = Vec::new();
    let mut code_block_start: Option<usize> = None;

    for (event, range) in Parser::new_ext(content, Options::all()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let (line, column) = line_col(&line_starts, range.start);
                active_heading = Some(ActiveHeading {
                    level: heading_level_number(level),
                    line,
                    column,
                    title: String::new(),
                });
            }
            Event::Start(Tag::CodeBlock(_)) => {
                code_block_start = Some(range.start);
            }
            Event::End(TagEnd::Heading(..)) => {
                if let Some(active) = active_heading.take() {
                    let title = normalize_inline_whitespace(active.title);
                    headings.push(Heading {
                        title,
                        anchor: String::new(),
                        level: active.level,
                        line: active.line,
                        column: active.column,
                    });
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = code_block_start.take() {
                    excluded_wikilink_ranges.push((start, range.end));
                }
            }
            Event::Text(text) => {
                if let Some(active) = &mut active_heading {
                    active.title.push_str(&text);
                }
            }
            Event::Code(text) => {
                if let Some(active) = &mut active_heading {
                    active.title.push_str(&text);
                }
                excluded_wikilink_ranges.push((range.start, range.end));
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(active) = &mut active_heading {
                    active.title.push(' ');
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                if let Some(target) = parse_markdown_target(dest_url.as_ref()) {
                    let (line, column) = line_col(&line_starts, range.start);
                    links.push(Link {
                        kind: LinkKind::Markdown,
                        raw: dest_url.to_string(),
                        target,
                        line,
                        column,
                    });
                }
            }
            _ => {}
        }
    }

    // Assign anchor slugs, deduplicating collisions with numeric suffixes.
    let mut anchor_counts: HashMap<String, usize> = HashMap::new();
    for heading in &mut headings {
        let base = slug(&heading.title);
        let count = anchor_counts.entry(base.clone()).or_insert(0);
        let anchor = if *count == 0 {
            base
        } else {
            format!("{}-{}", base, *count)
        };
        *count += 1;
        heading.anchor = anchor;
    }

    // Second pass: scan raw source for wikilinks since pulldown-cmark doesn't
    // parse `[[...]]` syntax natively.
    for captures in WIKILINK_RE.captures_iter(content) {
        let Some(full_match) = captures.get(0) else {
            continue;
        };
        let Some(inner_match) = captures.get(1) else {
            continue;
        };

        if range_contains_offset(&excluded_wikilink_ranges, full_match.start()) {
            continue;
        }

        let raw = inner_match.as_str();
        if let Some(target) = parse_wikilink_target(raw) {
            let (line, column) = line_col(&line_starts, full_match.start());
            links.push(Link {
                kind: LinkKind::Wikilink,
                raw: format!("[[{raw}]]"),
                target,
                line,
                column,
            });
        }
    }

    ParsedDocument { headings, links }
}

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

/// Recursively discover all `.md` files under `root`.
///
/// Returns a sorted list of paths relative to `root`. Symlinks are not
/// followed and paths that resolve outside the root are rejected. Both built-in
/// ignored directories and user-defined ignore patterns are respected.
/// Directories to skip during discovery. These are common build artifacts,
/// version control, and dependency directories that never contain useful
/// knowledge-base markdown.
const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".next",
    ".cache",
    "vendor",
    "__pycache__",
    ".venv",
];

fn build_ignore_globset(ignore_patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in ignore_patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .with_context(|| format!("invalid ignore pattern `{pattern}`"))?;
        builder.add(glob);
    }

    builder.build().context("failed to compile ignore patterns")
}

fn discover_markdown_files(root: &Path, ignore_set: &GlobSet) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }

            let Some(rel) = rel_path_from_root(root, entry.path()) else {
                return false;
            };
            if rel.as_os_str().is_empty() {
                return true;
            }

            let name = entry.file_name().to_string_lossy();
            if IGNORED_DIRS.contains(&name.as_ref()) {
                return false;
            }

            !path_is_ignored(ignore_set, &rel, true)
        })
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let is_markdown = entry
            .path()
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));
        if !is_markdown {
            continue;
        }

        let rel = entry.path().strip_prefix(root).with_context(|| {
            format!(
                "failed to strip root {} from {}",
                root.display(),
                entry.path().display()
            )
        })?;

        let rel = normalize_rel_path(rel).with_context(|| {
            format!(
                "markdown path {} resolves outside root {}",
                entry.path().display(),
                root.display()
            )
        })?;

        if path_is_ignored(ignore_set, &rel, false) {
            continue;
        }

        paths.push(rel);
    }

    paths.sort();
    Ok(paths)
}

fn rel_path_from_root(root: &Path, path: &Path) -> Option<PathBuf> {
    let rel = path.strip_prefix(root).ok()?;
    normalize_rel_path(rel)
}

fn path_is_ignored(ignore_set: &GlobSet, rel_path: &Path, is_dir: bool) -> bool {
    let slash = rel_path.to_string_lossy().replace('\\', "/");
    if slash.is_empty() {
        return false;
    }

    if ignore_set.is_match(&slash) {
        return true;
    }

    if is_dir {
        return ignore_set.is_match(format!("{slash}/"));
    }

    false
}

// ---------------------------------------------------------------------------
// Link resolution
// ---------------------------------------------------------------------------

/// Resolve the target file path for a link, returning an error if it escapes
/// the vault root.
fn resolve_target_file(
    source_file: &Path,
    link: &Link,
) -> std::result::Result<PathBuf, ResolveError> {
    resolve_target_path(source_file, link.kind, &link.target).ok_or(ResolveError::OutsideRoot)
}

/// Resolve a link target to a relative path within the vault.
///
/// Handles both markdown and wikilink conventions:
/// - Markdown links are resolved relative to the source file's directory.
/// - Wikilinks auto-append `.md` if no extension is present.
/// - Returns `None` if the resolved path escapes the vault root.
pub fn resolve_target_path(
    source_file: &Path,
    kind: LinkKind,
    target: &LinkTarget,
) -> Option<PathBuf> {
    let candidate = match target.file.as_deref() {
        Some(raw_file) => {
            let mut rel = PathBuf::from(raw_file);
            if matches!(kind, LinkKind::Wikilink) && rel.extension().is_none() {
                rel.set_extension("md");
            }

            if rel.is_absolute() {
                return None;
            }

            let base = source_file.parent().unwrap_or(Path::new(""));
            base.join(rel)
        }
        None => source_file.to_path_buf(),
    };

    normalize_rel_path(&candidate)
}

/// Resolve a CLI file target (`<file>`) to a normalized path inside `root`.
///
/// Supports both absolute and root-relative paths.
pub fn resolve_file_target(root: &Path, file: &str) -> Result<PathBuf> {
    let path = Path::new(file);
    if path.is_absolute() {
        let canonical = path
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", path.display()))?;
        let rel = canonical.strip_prefix(root).with_context(|| {
            format!(
                "target file {} is not inside kdb root {}",
                canonical.display(),
                root.display()
            )
        })?;
        return normalize_rel_path(rel)
            .with_context(|| format!("target path resolves outside root: {}", file));
    }

    normalize_rel_path(path).with_context(|| format!("target path resolves outside root: {file}"))
}

/// Normalize a relative path by resolving `.` and `..` components.
///
/// Returns `None` if the path would escape above the root (i.e. more `..`
/// components than depth), or if it contains absolute path components.
pub fn normalize_rel_path(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    Some(normalized)
}

// ---------------------------------------------------------------------------
// Link target parsing
// ---------------------------------------------------------------------------

/// Parse a standard markdown link URL into a [`LinkTarget`].
///
/// Returns `None` for external URLs, empty links, and links to non-markdown files.
/// A bare anchor like `#section` is parsed as a same-file heading reference.
pub fn parse_markdown_target(raw: &str) -> Option<LinkTarget> {
    let raw = raw.trim();
    if raw.is_empty() || is_external(raw) {
        return None;
    }

    if let Some(anchor) = raw.strip_prefix('#') {
        let anchor = anchor.trim();
        if anchor.is_empty() {
            return None;
        }
        return Some(LinkTarget {
            file: None,
            anchor: Some(anchor.to_string()),
        });
    }

    let (file, anchor) = match raw.split_once('#') {
        Some((file, anchor)) => (file.trim(), Some(anchor.trim())),
        None => (raw, None),
    };

    if file.is_empty() {
        return None;
    }

    let is_markdown_path = Path::new(file)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));
    if !is_markdown_path {
        return None;
    }

    let anchor = anchor
        .filter(|anchor| !anchor.is_empty())
        .map(ToString::to_string);

    Some(LinkTarget {
        file: Some(file.to_string()),
        anchor,
    })
}

/// Parse the inner content of a `[[wikilink]]` into a [`LinkTarget`].
///
/// Supports `file`, `file#anchor`, `#anchor`, and display text via `|`
/// (e.g. `[[file|display text]]` — the display text is ignored).
pub fn parse_wikilink_target(raw: &str) -> Option<LinkTarget> {
    let body = raw.split('|').next()?.trim();
    if body.is_empty() {
        return None;
    }

    if let Some(anchor) = body.strip_prefix('#') {
        let anchor = anchor.trim();
        if anchor.is_empty() {
            return None;
        }
        return Some(LinkTarget {
            file: None,
            anchor: Some(anchor.to_string()),
        });
    }

    let (file, anchor) = match body.split_once('#') {
        Some((file, anchor)) => (Some(file.trim()), Some(anchor.trim())),
        None => (Some(body), None),
    };

    let file = file
        .filter(|file| !file.is_empty())
        .map(ToString::to_string);
    let anchor = anchor
        .filter(|anchor| !anchor.is_empty())
        .map(ToString::to_string);

    if file.is_none() && anchor.is_none() {
        return None;
    }

    Some(LinkTarget { file, anchor })
}

/// Public wrapper around [`slug`] for use by the LSP module.
pub fn slug_anchor(input: &str) -> String {
    slug(input)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` for URLs that point outside the vault (http, mailto, etc.).
fn is_external(raw: &str) -> bool {
    raw.contains("://")
        || raw.starts_with("mailto:")
        || raw.starts_with("tel:")
        || raw.starts_with("data:")
}

/// Convert a heading title to a URL-safe anchor slug.
///
/// Lowercases the input, replaces whitespace/hyphens/underscores with single
/// dashes, strips non-alphanumeric characters, and trims trailing dashes.
/// Returns `"section"` for inputs that produce an empty slug.
fn slug(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;

    for ch in input.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
            continue;
        }

        if (ch.is_ascii_whitespace() || ch == '-' || ch == '_') && !out.is_empty() && !last_dash {
            out.push('-');
            last_dash = true;
        }
    }

    while out.ends_with('-') {
        out.pop();
    }

    if out.is_empty() {
        "section".to_string()
    } else {
        out
    }
}

/// Build an index of byte offsets where each line starts in the content.
///
/// Used together with [`line_col`] to convert byte offsets from the parser
/// into human-readable 1-based line and column numbers.
fn build_line_starts(content: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

/// Convert a byte offset into a 1-based (line, column) pair.
fn line_col(line_starts: &[usize], byte_index: usize) -> (usize, usize) {
    let line_idx = match line_starts.binary_search(&byte_index) {
        Ok(index) => index,
        Err(0) => 0,
        Err(index) => index - 1,
    };
    let line_start = line_starts[line_idx];
    (line_idx + 1, byte_index.saturating_sub(line_start) + 1)
}

/// Collapse runs of whitespace into single spaces.
fn normalize_inline_whitespace(input: String) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Convert a pulldown-cmark heading level enum to a numeric level (1-6).
fn heading_level_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn range_contains_offset(ranges: &[(usize, usize)], offset: usize) -> bool {
    ranges
        .iter()
        .any(|(start, end)| offset >= *start && offset < *end)
}

/// Accumulator for heading text while parsing events between heading open/close tags.
struct ActiveHeading {
    level: u8,
    line: usize,
    column: usize,
    title: String,
}
