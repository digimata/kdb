//! CLI command implementations.
//!
//! Each public function corresponds to a subcommand of the `kdb` binary:
//! `init`, `check`, `outline`, and `lsp`.

use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config;
use crate::index::{VaultIndex, normalize_rel_path};
use crate::root;

/// Initialize a kdb project by creating `.kdb/config.toml`.
pub fn init(path: Option<PathBuf>) -> Result<()> {
    let start = match path {
        Some(path) => make_absolute(&path)?,
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
pub fn check(path: Option<PathBuf>) -> Result<bool> {
    let start = match path {
        Some(path) => make_absolute(&path)?,
        None => env::current_dir().context("failed to read current directory")?,
    };

    let root = root::find_root(&start)?;
    let ignore_patterns = config::load_index_ignores(&root)?;
    let index = VaultIndex::build_with_ignores(&root, &ignore_patterns)?;
    let report = index.check();
    report.print();
    Ok(report.has_issues())
}

/// Print the heading tree for a single markdown file.
///
/// Displays an indented outline of all headings, useful for quickly seeing the
/// structure of a document from the terminal.
pub fn outline(file: PathBuf) -> Result<()> {
    let file_abs = make_absolute(&file)?;
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
            normalize_rel_path(path).with_context(|| {
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

/// Start the language server over stdio.
///
/// The LSP is the primary way editors like Zed interact with kdb, providing
/// go-to-definition, autocomplete, diagnostics, and document symbols.
pub async fn lsp(path: Option<PathBuf>) -> Result<()> {
    crate::lsp::serve(path).await
}

/// Convert a potentially relative path to absolute using the current working directory.
fn make_absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .context("failed to read current directory")?
            .join(path))
    }
}
