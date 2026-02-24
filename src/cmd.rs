//! CLI command implementations.
//!
//! Each public function corresponds to a subcommand of the `kdb` binary:
//! `init`, `check`, `outline`, `symbols`, `refs`, `deps`, `graph`, `fmt`, and `lsp`.

use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::config;
use crate::fmt;
use crate::index::{self, VaultIndex, deps, refs};
use crate::lsp;
use crate::root;
use crate::symbols;

// --------------------
// src/cmd.rs
//
// fn lsp()         L33
// fn init()        L38
// fn check()       L83
// fn outline()    L101
// fn symbols()    L154
// fn refs()       L199
// fn deps()       L223
// fn graph()      L240
// fn fmt()        L255
// --------------------

/// Start the language server over stdio.
pub async fn lsp(path: Option<PathBuf>) -> Result<()> {
    lsp::serve(path).await
}

/// Initialize a kdb project by creating `.kdb/config.toml`.
pub fn init(path: Option<PathBuf>) -> Result<()> {
    let start = match path {
        Some(path) => root::make_absolute(&path)?,
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

    let marker_dir = root.join(root::ROOT_MARKER);
    if marker_dir.exists() {
        bail!("{} already exists in {}", root::ROOT_MARKER, root.display());
    }

    fs::create_dir_all(&marker_dir)
        .with_context(|| format!("failed to create {}", marker_dir.display()))?;

    let project_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("kdb")
        .replace('"', "\\\"");

    let config = root::config_path(&root);
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
    let start = match path {
        Some(path) => root::make_absolute(&path)?,
        None => env::current_dir().context("failed to read current directory")?,
    };

    let root = root::find_root(&start)?;
    let ignore_patterns = config::load_index_ignores(&root)?;
    let index = VaultIndex::build_with_ignores(&root, &ignore_patterns)?;
    let report = index.check();
    report.print(list_orphans);
    Ok(report.has_errors())
}

/// Print the heading tree for a single markdown file.
///
/// Displays an indented outline of all headings, useful for quickly seeing the
/// structure of a document from the terminal.
pub fn outline(file: PathBuf) -> Result<()> {
    let file_abs = root::make_absolute(&file)?;
    if !file_abs.is_file() {
        bail!("file not found: {}", file_abs.display());
    }

    let root = root::find_root(&file_abs)?;
    let file_abs = file_abs
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", file.display()))?;

    let rel_path = file_abs
        .strip_prefix(&root)
        .with_context(|| {
            format!(
                "file {} is not inside kdb root {}",
                file_abs.display(),
                root.display()
            )
        })
        .and_then(|path| {
            index::normalize_rel_path(path).with_context(|| {
                format!(
                    "file path {} resolves outside kdb root {}",
                    file_abs.display(),
                    root.display()
                )
            })
        })?;

    let ignore_patterns = config::load_index_ignores(&root)?;
    let index = VaultIndex::build_with_ignores(&root, &ignore_patterns)?;
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

/// Print symbols for a single markdown or supported code file.
pub fn symbols(path: PathBuf, as_json: bool, public_only: bool) -> Result<()> {
    let file_abs = root::make_absolute(&path)?;
    if !file_abs.is_file() {
        bail!("file not found: {}", file_abs.display());
    }

    let root = root::find_root(&file_abs)?;
    let file_abs = file_abs
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", path.display()))?;

    let rel_path = file_abs
        .strip_prefix(&root)
        .with_context(|| {
            format!(
                "file {} is not inside kdb root {}",
                file_abs.display(),
                root.display()
            )
        })
        .and_then(|path| {
            index::normalize_rel_path(path).with_context(|| {
                format!(
                    "file path {} resolves outside kdb root {}",
                    file_abs.display(),
                    root.display()
                )
            })
        })?;

    let mut rows = symbols::query::collect_rows(&root, &file_abs, &rel_path)?;
    if public_only {
        rows.retain(|row| row.is_public);
    }

    if as_json {
        symbols::render::print_json(&rows)?;
    } else {
        symbols::render::print_text(&rows);
    }

    Ok(())
}

/// Find inbound markdown references to a file or specific heading.
pub fn refs(target: String, as_json: bool, count_only: bool) -> Result<()> {
    let target = index::refs::parse_target(&target)?;

    let start = env::current_dir().context("failed to read current directory")?;
    let root = root::find_root(&start)?;
    let ignore_patterns = config::load_index_ignores(&root)?;
    let index = VaultIndex::build_with_ignores(&root, &ignore_patterns)?;
    let inbound = refs::collect_inbound(&index, &root, target)?;

    if count_only {
        println!("{}", inbound.len());
        return Ok(());
    }

    if as_json {
        refs::print_json(&inbound)?;
    } else {
        refs::print_text(&inbound);
    }

    Ok(())
}

/// List outbound markdown dependencies for a file.
pub fn deps(target: String, as_json: bool) -> Result<()> {
    let start = env::current_dir().context("failed to read current directory")?;
    let root = root::find_root(&start)?;
    let ignore_patterns = config::load_index_ignores(&root)?;
    let index = VaultIndex::build_with_ignores(&root, &ignore_patterns)?;
    let outbound = deps::collect_outbound(&index, &root, &target)?;

    if as_json {
        deps::print_json(&outbound)?;
    } else {
        deps::print_text(&outbound);
    }

    Ok(())
}

/// Stub for `kdb graph` until graph rendering lands.
pub fn graph(path: Option<PathBuf>, cluster: bool) -> Result<()> {
    let requested = path
        .as_ref()
        .map(|value| value.display().to_string())
        .unwrap_or_else(|| "<root>".to_string());
    let mode = if cluster { "cluster" } else { "plain" };
    bail!(
        "`kdb graph` is not implemented yet (path: {requested}, mode: {mode}). See .issues/iss-0021-graph-command.md"
    )
}

/// Generate or update code index headers for supported code files.
///
/// Walks the project root and rewrites Rust, TypeScript/JavaScript, Python,
/// and Go files with a managed index block at the top of each file.
pub fn fmt(path: Option<PathBuf>) -> Result<()> {
    let start = match path {
        Some(path) => root::make_absolute(&path)?,
        None => env::current_dir().context("failed to read current directory")?,
    };

    let root = root::find_root(&start)?;
    let ignore_patterns = config::load_index_ignores(&root)?;
    let report = fmt::format_workspace(&root, &ignore_patterns)?;
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
