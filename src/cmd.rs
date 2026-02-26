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
use crate::index::{self, ProjectIndex, VaultIndex, deps as md_deps, refs};
use crate::lang::CodeLanguage;
use crate::project::{self, ProjectContext};
use crate::symbols;
use crate::tree;

// --------------------------------------
// src/cmd.rs
//
// pub struct CmdContext              L38
//   pub fn from_path()               L49
//   pub fn build_index()             L59
//   pub fn build_project_index()     L64
//   pub fn rel_path()                L72
// pub fn init()                      L99
// pub fn check()                    L148
// pub fn tree()                     L165
// pub fn symbols()                  L213
// pub fn refs()                     L274
// pub fn deps()                     L331
// pub fn graph()                    L366
// pub fn format()                   L380
// --------------------------------------

/// CLI command context: resolved start path + project state.
pub struct CmdContext {
    /// Resolved absolute start path (from CLI arg or cwd).
    pub start: PathBuf,
    /// Discovered project context (root, ignore patterns, ignore set).
    pub project: ProjectContext,
    /// When true, ignore the disk cache and force a full rebuild.
    pub fresh: bool,
}

impl CmdContext {
    /// Resolve a start path and discover the project root.
    ///
    /// When `path` is `None`, falls back to the current working directory.
    pub fn from_path(path: Option<&Path>, fresh: bool) -> Result<Self> {
        let start = match path {
            Some(p) => project::root::make_absolute(p)?,
            None => env::current_dir().context("failed to read current directory")?,
        };
        let project = ProjectContext::discover(&start)?;
        Ok(Self {
            start,
            project,
            fresh,
        })
    }

    /// Build a [`VaultIndex`] (markdown only) using the project's ignore patterns.
    ///
    /// Uses the persistent cache when available.
    pub fn build_index(&self) -> Result<VaultIndex> {
        let result = index::cache::incremental_build(
            &self.project.root,
            &self.project.ignore_patterns,
            self.fresh,
        )?;
        VaultIndex::build_from_entries(
            &self.project.root,
            &self.project.ignore_patterns,
            result.vault_files,
        )
    }

    /// Build a [`ProjectIndex`] (vault + code) using the project's ignore patterns.
    ///
    /// Uses the persistent cache when available.
    pub fn build_project_index(&self) -> Result<ProjectIndex> {
        ProjectIndex::build_cached(
            &self.project.root,
            &self.project.ignore_patterns,
            self.fresh,
        )
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
pub fn check(path: Option<PathBuf>, list_orphans: bool, fresh: bool) -> Result<bool> {
    let has_scope = path.is_some();
    let ctx = CmdContext::from_path(path.as_deref(), fresh)?;
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
    let ctx = CmdContext::from_path(path.as_deref(), false)?;

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

/// Print symbols for one or more files and/or directories.
pub fn symbols(
    paths: Vec<PathBuf>,
    selectors: Vec<String>,
    as_json: bool,
    public_only: bool,
) -> Result<()> {
    assert!(!paths.is_empty(), "at least one path is required");

    let ctx = CmdContext::from_path(Some(&paths[0]), false)?;
    let files = symbols::query::expand_paths(&ctx.project, &paths)?;
    assert!(!files.is_empty(), "no supported files found in given paths");

    let multi = files.len() > 1;
    if multi && !selectors.is_empty() {
        bail!(
            "-s/--symbol requires a single definition file, got {} files",
            files.len()
        );
    }

    if selectors.is_empty() {
        let mut all_rows: Vec<(PathBuf, Vec<symbols::display::SymbolRow>)> = Vec::new();
        for (abs, rel) in &files {
            let mut rows = symbols::query::collect_rows(&ctx.project.root, abs, rel)?;
            if public_only {
                rows.retain(|row| row.is_public);
            }
            all_rows.push((rel.clone(), rows));
        }

        if as_json {
            let flat: Vec<_> = all_rows.iter().flat_map(|(_, rows)| rows).collect();
            let output = serde_json::to_string_pretty(&flat)
                .context("failed to serialize symbols as JSON")?;
            println!("{output}");
        } else if multi {
            symbols::display::print_multi_text(&all_rows);
        } else {
            symbols::display::print_text(&all_rows[0].1);
        }
    } else {
        let selector_strs: Vec<&str> = selectors.iter().map(String::as_str).collect();
        let mut all_rows = Vec::new();
        for (abs, rel) in &files {
            let rows = symbols::query::collect_body_rows(abs, rel, &selector_strs, public_only)?;
            all_rows.extend(rows);
        }

        if as_json {
            let output = serde_json::to_string_pretty(&all_rows)
                .context("failed to serialize symbol bodies as JSON")?;
            println!("{output}");
        } else {
            symbols::display::print_bodies_text(&all_rows);
        }
    }

    Ok(())
}

/// Find inbound markdown references or code symbol references.
pub fn refs(
    target: String,
    symbol: Option<String>,
    context_lines: Option<usize>,
    as_json: bool,
    count_only: bool,
    fresh: bool,
) -> Result<()> {
    let ctx = CmdContext::from_path(None, fresh)?;

    if let Some(symbol_name) = symbol {
        let index = ProjectIndex::build_cached_with_symbol_refs(
            &ctx.project.root,
            &ctx.project.ignore_patterns,
            ctx.fresh,
        )?;
        let inbound =
            refs::collect_symbol_refs(&index.code, &ctx.project.root, &target, &symbol_name)?;

        if count_only {
            println!("{}", inbound.len());
            return Ok(());
        }

        if as_json {
            let output = serde_json::to_string_pretty(&inbound)
                .context("failed to serialize symbol refs as JSON")?;
            println!("{output}");
        } else {
            let options = refs::SymbolRefRenderOptions::new(context_lines.unwrap_or(0));
            refs::print_symbol_refs_text(&ctx.project.root, &inbound, options)?;
        }

        return Ok(());
    }

    if context_lines.is_some() {
        bail!("--context is currently supported only with --symbol");
    }

    let target = index::refs::parse_target(&target)?;
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
pub fn deps(target: String, as_json: bool, fresh: bool) -> Result<()> {
    let ctx = CmdContext::from_path(None, fresh)?;
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

    let pi = ctx.build_project_index()?;

    let outbound = if is_markdown {
        md_deps::collect_outbound(&pi.vault, &source_file)?
    } else {
        md_deps::collect_code_outbound(&pi.code, &source_file)?
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
    let ctx = CmdContext::from_path(path.as_deref(), false)?;

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
