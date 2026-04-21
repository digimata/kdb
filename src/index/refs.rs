use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    CodeIndex, HeadingKey, LinkRef, SymbolKey, SymbolRef, VaultIndex, parse_markdown_target,
    resolve_file_target, slug_anchor,
};

// ---------------------------------------------
// projects/kdb/src/index/refs.rs
//
// pub struct RefsTarget                     L56
// pub fn parse_target()                     L62
// pub fn collect_inbound()                  L82
// pub fn print_text()                      L138
// pub struct SymbolRefRenderOptions        L157
//   pub fn new()                           L163
// struct ContextWindow                     L169
//   fn around()                            L176
//   fn line_width()                        L194
// struct SourceContext                     L200
//   fn from_source()                       L205
//   fn line_count()                        L210
//   fn line_text()                         L214
// trait LineSource                         L219
// struct FsLineSource                      L224
//   fn new()                               L230
//   fn load_context()                      L237
//   fn context_for()                       L249
// struct SymbolRefTextRenderer             L265
//   fn new()                               L271
//   fn render()                            L278
//   fn render_compact()                    L292
//   fn render_with_context()               L309
//   fn render_context_block()              L319
//   fn render_context_line()               L333
// pub fn collect_symbol_refs()             L349
// pub fn print_symbol_refs_text()          L384
// pub fn print_symbol_refs_files()         L393
// pub fn print_files()                     L406
// fn collect_symbol_keys()                 L418
// pub(super) fn normalize_symbol_refs()    L440
// struct SymbolSelector                    L460
//   fn parse()                             L466
//   fn matches()                           L493
//   fn display()                           L503
// fn line_marker()                         L511
// fn fallback_line_text()                  L515
// fn normalize_symbol_name()               L523
// ---------------------------------------------

/// Parsed target for `kdb refs`: a file path with an optional heading anchor.
#[derive(Debug, Clone)]
pub struct RefsTarget {
    pub file: String,
    pub anchor: Option<String>,
}

/// Parse a raw CLI target string into a [`RefsTarget`] (file path + optional anchor).
pub fn parse_target(raw: &str) -> Result<RefsTarget> {
    let target = parse_markdown_target(raw).with_context(|| {
        format!(
            "invalid refs target `{}` (expected <file.md> or <file.md>#<heading>)",
            raw
        )
    })?;

    let file = target.file.with_context(|| {
        format!(
            "invalid refs target `{}` (expected <file.md> or <file.md>#<heading>)",
            raw
        )
    })?;

    let anchor = target.anchor.map(|value| slug_anchor(&value));
    Ok(RefsTarget { file, anchor })
}

/// Collect inbound markdown link references for `kdb refs <file>[#heading]`.
pub fn collect_inbound(
    index: &VaultIndex,
    root: &Path,
    target: RefsTarget,
) -> Result<Vec<LinkRef>> {
    let target_file = resolve_file_target(root, &target.file)?;
    if !index.files.contains_key(&target_file) {
        bail!(
            "target file is not an indexed markdown file: {}",
            target_file.display()
        );
    }

    let mut inbound = if let Some(anchor) = target.anchor {
        let heading_exists = index.files.get(&target_file).is_some_and(|entry| {
            entry
                .headings
                .iter()
                .any(|heading| heading.anchor == anchor)
        });
        if !heading_exists {
            bail!(
                "target heading not found: {}#{}",
                target_file.display(),
                anchor
            );
        }

        index
            .heading_inbound
            .get(&HeadingKey {
                file: target_file,
                anchor,
            })
            .cloned()
            .unwrap_or_default()
    } else {
        index
            .file_inbound
            .get(&target_file)
            .cloned()
            .unwrap_or_default()
    };

    inbound.sort_by(|left, right| {
        left.source_file
            .cmp(&right.source_file)
            .then_with(|| left.source_line.cmp(&right.source_line))
            .then_with(|| left.source_column.cmp(&right.source_column))
            .then_with(|| left.raw.cmp(&right.raw))
    });

    Ok(inbound)
}

/// Print inbound markdown link references in text form.
pub fn print_text(inbound: &[LinkRef]) {
    if inbound.is_empty() {
        println!("(no references)");
        return;
    }

    for link_ref in inbound {
        println!(
            "{}:{}:{}  {}",
            link_ref.source_file.display(),
            link_ref.source_line,
            link_ref.source_column,
            link_ref.raw
        );
    }
}

/// Rendering options for symbol reference text output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolRefRenderOptions {
    pub context_lines: usize,
}

impl SymbolRefRenderOptions {
    /// Construct text rendering options for symbol refs.
    pub fn new(context_lines: usize) -> Self {
        Self { context_lines }
    }
}

#[derive(Debug, Clone, Copy)]
struct ContextWindow {
    start_line: usize,
    end_line: usize,
    match_line: usize,
}

impl ContextWindow {
    fn around(match_line: usize, context_lines: usize, total_lines: usize) -> Self {
        if total_lines == 0 {
            return Self {
                start_line: match_line,
                end_line: match_line,
                match_line,
            };
        }

        let start_line = match_line.saturating_sub(context_lines).max(1);
        let end_line = match_line.saturating_add(context_lines).min(total_lines);
        Self {
            start_line,
            end_line,
            match_line,
        }
    }

    fn line_width(self) -> usize {
        self.end_line.to_string().len().max(1)
    }
}

#[derive(Debug, Clone, Default)]
struct SourceContext {
    lines: Vec<String>,
}

impl SourceContext {
    fn from_source(source: String) -> Self {
        let lines = source.lines().map(ToString::to_string).collect();
        Self { lines }
    }

    fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn line_text(&self, line: usize) -> Option<&str> {
        self.lines.get(line.saturating_sub(1)).map(String::as_str)
    }
}

trait LineSource {
    fn context_for(&mut self, rel_path: &Path) -> Result<&SourceContext>;
}

#[derive(Debug)]
struct FsLineSource<'a> {
    root: &'a Path,
    cache: HashMap<PathBuf, SourceContext>,
}

impl<'a> FsLineSource<'a> {
    fn new(root: &'a Path) -> Self {
        Self {
            root,
            cache: HashMap::new(),
        }
    }

    fn load_context(&self, rel_path: &Path) -> Result<SourceContext> {
        let source = fs::read_to_string(self.root.join(rel_path)).with_context(|| {
            format!(
                "failed to read source for context rendering: {}",
                rel_path.display()
            )
        })?;
        Ok(SourceContext::from_source(source))
    }
}

impl LineSource for FsLineSource<'_> {
    fn context_for(&mut self, rel_path: &Path) -> Result<&SourceContext> {
        if !self.cache.contains_key(rel_path) {
            let context = self.load_context(rel_path)?;
            self.cache.insert(rel_path.to_path_buf(), context);
        }

        self.cache.get(rel_path).with_context(|| {
            format!(
                "failed to retrieve cached context for {}",
                rel_path.display()
            )
        })
    }
}

#[derive(Debug)]
struct SymbolRefTextRenderer<'a> {
    line_source: FsLineSource<'a>,
    options: SymbolRefRenderOptions,
}

impl<'a> SymbolRefTextRenderer<'a> {
    fn new(root: &'a Path, options: SymbolRefRenderOptions) -> Self {
        Self {
            line_source: FsLineSource::new(root),
            options,
        }
    }

    fn render(&mut self, inbound: &[SymbolRef]) -> Result<()> {
        if inbound.is_empty() {
            println!("(no references)");
            return Ok(());
        }

        if self.options.context_lines == 0 {
            self.render_compact(inbound);
            return Ok(());
        }

        self.render_with_context(inbound)
    }

    fn render_compact(&self, inbound: &[SymbolRef]) {
        let mut first_group = true;
        let mut current_file: Option<&Path> = None;

        for row in inbound {
            if current_file != Some(row.source_file.as_path()) {
                if !first_group {
                    println!();
                }
                println!("── {}", row.source_file.display());
                current_file = Some(row.source_file.as_path());
                first_group = false;
            }
            println!("  {}:{}   {}", row.line, row.column, row.snippet);
        }
    }

    fn render_with_context(&mut self, inbound: &[SymbolRef]) -> Result<()> {
        for (index, row) in inbound.iter().enumerate() {
            self.render_context_block(row)?;
            if index + 1 < inbound.len() {
                println!("--");
            }
        }
        Ok(())
    }

    fn render_context_block(&mut self, row: &SymbolRef) -> Result<()> {
        println!("{}:{}:{}", row.source_file.display(), row.line, row.column);
        let source = self.line_source.context_for(&row.source_file)?;
        let window =
            ContextWindow::around(row.line, self.options.context_lines, source.line_count());
        let width = window.line_width();

        for line_number in window.start_line..=window.end_line {
            Self::render_context_line(source, row, window, line_number, width);
        }

        Ok(())
    }

    fn render_context_line(
        source: &SourceContext,
        row: &SymbolRef,
        window: ContextWindow,
        line_number: usize,
        width: usize,
    ) {
        let marker = line_marker(line_number, window.match_line);
        let text = source
            .line_text(line_number)
            .unwrap_or_else(|| fallback_line_text(row, line_number));
        println!("{marker} {line_number:>width$} | {text}");
    }
}

/// Collect inbound code symbol references for `kdb refs <file> -s <symbol>`.
pub fn collect_symbol_refs(
    index: &CodeIndex,
    root: &Path,
    file_target: &str,
    symbol_selector: &str,
) -> Result<Vec<SymbolRef>> {
    let target_file = resolve_file_target(root, file_target)?;
    if !index.code_imports.contains_key(&target_file) {
        bail!(
            "target file is not an indexed supported code file: {}",
            target_file.display()
        );
    }

    let selector = SymbolSelector::parse(symbol_selector)?;
    let keys = collect_symbol_keys(index, &target_file, &selector);
    if keys.is_empty() {
        bail!(
            "symbol not found: {} in {}",
            selector.display(),
            target_file.display()
        );
    }

    let mut refs = Vec::new();
    for key in keys {
        if let Some(rows) = index.symbols.refs.get(&key) {
            refs.extend(rows.iter().cloned());
        }
    }
    normalize_symbol_refs(&mut refs);
    Ok(refs)
}

/// Print inbound code symbol references in text form.
pub fn print_symbol_refs_text(
    root: &Path,
    inbound: &[SymbolRef],
    options: SymbolRefRenderOptions,
) -> Result<()> {
    SymbolRefTextRenderer::new(root, options).render(inbound)
}

/// Print unique file paths from symbol references, one per line.
pub fn print_symbol_refs_files(inbound: &[SymbolRef]) {
    let mut seen = Vec::new();
    for row in inbound {
        if !seen.contains(&row.source_file) {
            seen.push(row.source_file.clone());
        }
    }
    for path in &seen {
        println!("{}", path.display());
    }
}

/// Print unique file paths from markdown link references, one per line.
pub fn print_files(inbound: &[LinkRef]) {
    let mut seen = Vec::new();
    for row in inbound {
        if !seen.contains(&row.source_file) {
            seen.push(row.source_file.clone());
        }
    }
    for path in &seen {
        println!("{}", path.display());
    }
}

fn collect_symbol_keys(
    index: &CodeIndex,
    file: &Path,
    selector: &SymbolSelector,
) -> Vec<SymbolKey> {
    let mut keys = index
        .symbols
        .refs
        .keys()
        .filter(|key| key.file == file && selector.matches(key))
        .cloned()
        .collect::<Vec<_>>();

    keys.sort_by(|left, right| {
        left.line
            .cmp(&right.line)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.parent.cmp(&right.parent))
    });
    keys
}

pub(super) fn normalize_symbol_refs(rows: &mut Vec<SymbolRef>) {
    rows.sort_by(|left, right| {
        right
            .is_definition
            .cmp(&left.is_definition)
            .then_with(|| left.source_file.cmp(&right.source_file))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.column.cmp(&right.column))
            .then_with(|| left.snippet.cmp(&right.snippet))
    });
    rows.dedup_by(|left, right| {
        left.source_file == right.source_file
            && left.line == right.line
            && left.column == right.column
            && left.snippet == right.snippet
            && left.is_definition == right.is_definition
    });
}

#[derive(Debug, Clone)]
struct SymbolSelector {
    parent: Option<String>,
    name: String,
}

impl SymbolSelector {
    fn parse(raw: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            bail!("symbol selector cannot be empty");
        }

        if let Some((parent, name)) = trimmed.rsplit_once("::") {
            let parent = parent.trim();
            let name = normalize_symbol_name(name);
            if parent.is_empty() || name.is_empty() {
                bail!("invalid symbol selector: {raw}");
            }

            return Ok(Self {
                parent: Some(parent.to_string()),
                name,
            });
        }

        let name = normalize_symbol_name(trimmed);
        if name.is_empty() {
            bail!("invalid symbol selector: {raw}");
        }

        Ok(Self { parent: None, name })
    }

    fn matches(&self, key: &SymbolKey) -> bool {
        if key.name != self.name {
            return false;
        }
        match &self.parent {
            Some(parent) => key.parent.as_deref() == Some(parent.as_str()),
            None => true,
        }
    }

    fn display(&self) -> String {
        match &self.parent {
            Some(parent) => format!("{parent}::{}", self.name),
            None => self.name.clone(),
        }
    }
}

fn line_marker(line_number: usize, match_line: usize) -> char {
    if line_number == match_line { '>' } else { ' ' }
}

fn fallback_line_text<'a>(row: &'a SymbolRef, line_number: usize) -> &'a str {
    if line_number == row.line {
        row.snippet.as_str()
    } else {
        ""
    }
}

fn normalize_symbol_name(value: &str) -> String {
    value.trim().trim_end_matches("()").to_string()
}
