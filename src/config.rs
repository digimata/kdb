//! Project configuration loading.
//!
//! kdb stores project settings in `.kdb/config.toml`. We currently read
//! indexing options from that file so callers can customize discovery.

use anyhow::{bail, Context, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::root;

// ----------------------------------
// src/config.rs
//
// pub const CUSTOM_IGNORE_FILE     L21
// pub fn load_index_ignores()    L24
// fn parse_index_ignores()       L40
// ----------------------------------

/// Project-local ignore file loaded by discovery walkers.
pub const CUSTOM_IGNORE_FILE: &str = ".kdbignore";

/// Load user-configured index ignore patterns from `.kdb/config.toml`.
///
/// Reads `[index].ignore` as an array of strings. Missing config files or
/// missing fields default to an empty pattern list.
pub fn load_index_ignores(root: &Path) -> Result<Vec<String>> {
    let config_path = root::config_path(root);
    let raw = match fs::read_to_string(&config_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", config_path.display()));
        }
    };

    let value: toml::Value = toml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", config_path.display()))?;
    parse_index_ignores(&value)
        .with_context(|| format!("failed to parse {}", config_path.display()))
}

fn parse_index_ignores(value: &toml::Value) -> Result<Vec<String>> {
    let table = value
        .as_table()
        .context("config root must be a TOML table")?;
    let Some(index) = table.get("index") else {
        return Ok(Vec::new());
    };

    let index_table = index.as_table().context("`index` must be a TOML table")?;
    let Some(ignore) = index_table.get("ignore") else {
        return Ok(Vec::new());
    };

    let entries = ignore
        .as_array()
        .context("`index.ignore` must be an array of strings")?;

    let mut patterns = Vec::new();
    for entry in entries {
        let pattern = entry
            .as_str()
            .context("`index.ignore` entries must be strings")?
            .trim();
        if pattern.is_empty() {
            bail!("`index.ignore` entries must not be empty");
        }
        patterns.push(pattern.to_string());
    }

    Ok(patterns)
}
