//! Project root discovery.
//!
//! A kdb project is identified by a `.kdb/` marker directory at its root. This
//! module walks upward from a given starting path until it finds that marker,
//! similar to how cargo finds `Cargo.toml`.

use anyhow::{Context, Result, bail};
use std::env;
use std::path::{Path, PathBuf};

// -----------------------------
// qmd/src/project/root.rs
//
// pub const ROOT_MARKER     L22
// pub const CONFIG_FILE     L25
// pub fn config_path()      L28
// pub fn find_root()        L36
// pub fn make_absolute()    L71
// -----------------------------

/// Marker directory that identifies the root of a kdb project.
pub const ROOT_MARKER: &str = ".kdb";

/// Default config filename inside [ROOT_MARKER].
pub const CONFIG_FILE: &str = "config.toml";

/// Return the canonical config path for a project root.
pub fn config_path(root: &Path) -> PathBuf {
    root.join(ROOT_MARKER).join(CONFIG_FILE)
}

/// Walk upward from `start` to find the nearest directory containing [`ROOT_MARKER`].
///
/// If `start` is a file, the search begins from its parent directory. Returns
/// the canonical path to the project root, or an error if no marker is found.
pub fn find_root(start: &Path) -> Result<PathBuf> {
    if !start.exists() {
        bail!("path does not exist: {}", start.display());
    }

    let start_abs = start
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", start.display()))?;

    let mut cursor = if start_abs.is_file() {
        start_abs
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(start_abs)
    } else {
        start_abs
    };

    loop {
        if cursor.join(ROOT_MARKER).is_dir() {
            return Ok(cursor);
        }
        if !cursor.pop() {
            break;
        }
    }

    bail!(
        "could not find {} starting from {}",
        ROOT_MARKER,
        start.display()
    )
}

/// Convert a potentially relative path to absolute using the current working directory.
pub fn make_absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .context("failed to read current directory")?
            .join(path))
    }
}
