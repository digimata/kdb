//! CLI command implementations.
//!
//! Each public function corresponds to a subcommand of the `kdb` binary:
//! `init`, `check`, `outline`, `tree`, `symbols`, `refs`, `deps`, `graph`, `fmt`, and `lsp`.

use anyhow::{Context, Result, bail};
use serde_json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::fmt;
use crate::index::{self, VaultIndex, deps as md_deps, refs};
use crate::lang::CodeLanguage;
use crate::project::{self, ProjectContext};
use crate::symbols;
use crate::tree;

// ------------------------------
// src/cmd.rs
//
// pub struct CmdContext      L42
//   pub fn from_path()       L53
//   pub fn build_index()     L63
//   pub fn rel_path()        L71
// pub fn init()              L98
// pub fn check()            L147
// pub fn outline()          L167
// pub fn tree()             L197
// pub fn symbols()          L245
// pub fn refs()             L295
// pub fn deps()             L318
// pub fn graph()            L353
// pub fn format()           L367
// ------------------------------

/// CLI command context: resolved start path + project state.
///
/// Wraps [`ProjectContext`] with the CLI-specific conveniences shared by most
/// subcommands: start-path resolution, on-demand index building, and the
/// canonicalize → strip_prefix → normalize chain.
pub struct CmdContext {
    /// Resolved absolute start path (from CLI arg or cwd).
    pub start: PathBuf,
    /// Discovered project context (root, ignore patterns, ignore set).
    pub project: ProjectContext,
}

impl CmdContext {
    /// Resolve a start path and discover the project root.
    ///
    /// When `path` is `None`, falls back to the current working directory.
    pub fn from_path(path: Option<&Path>) -> Result<Self> {
        let start = match path {
            Some(p) => project::root::make_absolute(p)?,
            None => env::current_dir().context("failed to read current directory")?,
        };
        let project = ProjectContext::discover(&start)?;
        Ok(Self { start, project })
    }

    /// Build a [`VaultIndex`] using the project's ignore patterns.
    pub fn build_index(&self) -> Result<VaultIndex> {
        VaultIndex::build_with_ignores(&self.project.root, &self.project.ignore_patterns)
    }

    /// Canonicalize an absolute path and return its root-relative form.
    ///
    /// Performs `canonicalize` → `strip_prefix(root)` → `normalize_rel_path`,
    /// producing a clean relative path suitable for index lookups.
    pub fn rel_path(&self, abs: &Path) -> Result<PathBuf> {
        let canonical = abs
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", abs.display()))?;
        let root = &self.project.root;
        canonical
            .strip_prefix(root)
            .with_context(|| {
                format!(
                    "path {} is not inside kdb root {}",
                    canonical.display(),
                    root.display()
                )
            })
            .and_then(|rel| {
                project::paths::normalize_rel_path(rel).with_context(|| {
                    format!(
                        "path {} resolves outside kdb root {}",
                        canonical.display(),
                        root.display()
                    )
                })
            })
    }
}

/// Initialize a kdb project by creating `.kdb/config.toml`.
pub fn init(path: Option<PathBuf>) -> Result<()> {
    let start = match path {
        Some(path) => project::root::make_absolute(&path)?,
        None => env::current_dir().context("failed to read current directory")?,
    };

    if !start.exists() {
        bail!("path does not exist: {}", start.display());
    }

    if !start.is_dir() {
        bail!("init path must be a directory: {}", start.display());
    }

    let root = start
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", start.display()))?;

    let marker_dir = root.join(project::root::ROOT_MARKER);
    if marker_dir.exists() {
        bail!(
            "{} already exists in {}",
            project::root::ROOT_MARKER,
            root.display()
        );
    }

    fs::create_dir_all(&marker_dir)
        .with_context(|| format!("failed to create {}", marker_dir.display()))?;

    let project_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("kdb")
        .replace('"', "\\\"");

    let config = project::root::config_path(&root);
    let default_config = format!("[project]\nname = \"{project_name}\"\n");
    fs::write(&config, default_config)
        .with_context(|| format!("failed to write {}", config.display()))?;

    println!("initialized kdb project at {}", root.display());
    Ok(())
}

/// Validate all links in the vault and report broken references and orphan files.
///
/// Returns `Ok(true)` if any issues were found (caller should exit with code 1),
/// or `Ok(false)` if the vault is clean.
pub fn check(path: Option<PathBuf>, list_orphans: bool) -> Result<bool> {
    let has_scope = path.is_some();
    let ctx = CmdContext::from_path(path.as_deref())?;
    let index = ctx.build_index()?;
    let mut report = index.check();

    if has_scope {
        let scope_rel = ctx.rel_path(&ctx.start)?;
        let scope_is_dir = ctx.start.is_dir();
        report = report.scoped_to(&scope_rel, scope_is_dir);
    }

    report.print(list_orphans);
    Ok(report.has_errors())
}

/// Print the heading tree for a single markdown file.
///
/// Displays an indented outline of all headings, useful for quickly seeing the
/// structure of a document from the terminal.
pub fn outline(file: PathBuf) -> Result<()> {
    let file_abs = project::root::make_absolute(&file)?;
    if !file_abs.is_file() {
        bail!("file not found: {}", file_abs.display());
    }

    let ctx = CmdContext::from_path(Some(&file))?;
    let rel_path = ctx.rel_path(&file_abs)?;
    let index = ctx.build_index()?;
    let file_entry = index.files.get(&rel_path).with_context(|| {
        format!(
            "file {} is not an indexed markdown file",
            rel_path.display()
        )
    })?;

    if file_entry.headings.is_empty() {
        println!("(no headings)");
        return Ok(());
    }

    for heading in &file_entry.headings {
        let indent = "  ".repeat(usize::from(heading.level.saturating_sub(1)));
        println!("{indent}- {}", heading.title);
    }

    Ok(())
}

/// Print a filtered project tree for a path under the current kdb root.
pub fn tree(
    path: Option<PathBuf>,
    level: Option<usize>,
    as_json: bool,
    all: bool,
    dirs_only: bool,
    full_path: bool,
    ignore: Vec<String>,
    pattern: Vec<String>,
) -> Result<()> {
    let has_explicit_path = path.is_some();
    let ctx = CmdContext::from_path(path.as_deref())?;

    if !ctx.start.exists() {
        bail!("path does not exist: {}", ctx.start.display());
    }

    let tree_start = if has_explicit_path {
        ctx.start.clone()
    } else {
        ctx.project.root.clone()
    };

    let tree = tree::build_tree(
        &ctx.project.root,
        &tree_start,
        &ctx.project.ignore_patterns,
        tree::TreeOptions {
            max_depth: level,
            show_hidden: all,
            dirs_only,
            full_paths: full_path,
            ignore_patterns: ignore,
            include_patterns: pattern,
        },
    )?;
    if as_json {
        let output =
            serde_json::to_string_pretty(&tree).context("failed to serialize tree as JSON")?;
        println!("{output}");
    } else {
        println!("{}", tree::render_text(&tree));
    }

    Ok(())
}

/// Print symbols for a single markdown or supported code file.
pub fn symbols(
    path: PathBuf,
    symbol: Option<String>,
    as_json: bool,
    public_only: bool,
) -> Result<()> {
    let file_abs = project::root::make_absolute(&path)?;
    if !file_abs.is_file() {
        bail!("file not found: {}", file_abs.display());
    }

    let file_abs = file_abs
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", path.display()))?;
    let ctx = CmdContext::from_path(Some(&path))?;
    let rel_path = ctx.rel_path(&file_abs)?;

    if let Some(selector) = symbol {
        let rows = symbols::query::collect_body_rows(
            &file_abs,
            &rel_path,
            selector.as_str(),
            public_only,
        )?;
        if as_json {
            let output = serde_json::to_string_pretty(&rows)
                .context("failed to serialize symbol bodies as JSON")?;
            println!("{output}");
        } else {
            symbols::display::print_bodies_text(&rows);
        }
    } else {
        let mut rows = symbols::query::collect_rows(&ctx.project.root, &file_abs, &rel_path)?;
        if public_only {
            rows.retain(|row| row.is_public);
        }

        if as_json {
            let output = serde_json::to_string_pretty(&rows)
                .context("failed to serialize symbols as JSON")?;
            println!("{output}");
        } else {
            symbols::display::print_text(&rows);
        }
    }

    Ok(())
}

/// Find inbound markdown references to a file or specific heading.
pub fn refs(target: String, as_json: bool, count_only: bool) -> Result<()> {
    let target = index::refs::parse_target(&target)?;
    let ctx = CmdContext::from_path(None)?;
    let index = ctx.build_index()?;
    let inbound = refs::collect_inbound(&index, &ctx.project.root, target)?;

    if count_only {
        println!("{}", inbound.len());
        return Ok(());
    }

    if as_json {
        let output =
            serde_json::to_string_pretty(&inbound).context("failed to serialize refs as JSON")?;
        println!("{output}");
    } else {
        refs::print_text(&inbound);
    }

    Ok(())
}

/// List outbound dependencies for a markdown or supported code file.
pub fn deps(target: String, as_json: bool) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let source_file = index::resolve_file_target(&ctx.project.root, &target)?;
    let is_markdown = source_file
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));

    if !is_markdown && CodeLanguage::from_path(&source_file).is_none() {
        bail!(
            "deps is not supported for file type: {}",
            source_file.display()
        );
    }

    let index = ctx.build_index()?;

    let outbound = if is_markdown {
        md_deps::collect_outbound(&index, &source_file)?
    } else {
        md_deps::collect_code_outbound(&index, &source_file)?
    };

    if as_json {
        let output =
            serde_json::to_string_pretty(&outbound).context("failed to serialize deps as JSON")?;
        println!("{output}");
    } else {
        md_deps::print_text(&outbound);
    }

    Ok(())
}

/// Stub for `kdb graph` until graph rendering lands.
pub fn graph(path: Option<PathBuf>) -> Result<()> {
    let requested = path
        .as_ref()
        .map(|value| value.display().to_string())
        .unwrap_or_else(|| "<root>".to_string());
    bail!(
        "`kdb graph` is not implemented yet (path: {requested}). See .issues/iss-0021-graph-command.md"
    )
}

/// Generate or update code index headers for supported code files.
///
/// Walks the project root and rewrites Rust, TypeScript/JavaScript, Python,
/// and Go files with a managed index block at the top of each file.
pub fn format(path: Option<PathBuf>) -> Result<()> {
    let has_explicit_path = path.is_some();
    let ctx = CmdContext::from_path(path.as_deref())?;

    if !ctx.start.exists() {
        bail!("path does not exist: {}", ctx.start.display());
    }

    let fmt_target = if has_explicit_path {
        ctx.start
    } else {
        ctx.project.root.clone()
    };
    let report = fmt::format_path(&ctx.project.root, &fmt_target, &ctx.project.ignore_patterns)?;
    println!(
        "kdb fmt: updated {} of {} files",
        report.updated_files, report.scanned_files
    );

    if !report.warnings.is_empty() {
        eprintln!("kdb fmt: {} warning(s)", report.warnings.len());
        for warning in &report.warnings {
            eprintln!(
                "warning: {} ({})",
                warning.message,
                warning.rel_path.display()
            );
        }
    }

    Ok(())
}
