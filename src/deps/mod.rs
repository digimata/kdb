//! Code dependency extraction for `kdb deps`.

use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::index::deps::Dependency;
use crate::lang::CodeLanguage;

mod go;
mod python;
mod rust;
mod typescript;
mod utils;

// --------------------------------
// qmd/src/deps/mod.rs
//
// mod go                       L11
// mod python                   L12
// mod rust                     L13
// mod typescript               L14
// mod utils                    L15
// pub fn collect_outbound()    L29
// --------------------------------

/// Collect outbound code dependencies by dispatching to the appropriate language extractor.
pub fn collect_outbound(root: &Path, source_file: &Path) -> Result<Vec<Dependency>> {
    let language = CodeLanguage::from_path(source_file).with_context(|| {
        format!(
            "deps is not supported for file type: {}",
            source_file.display()
        )
    })?;

    let source_abs = root.join(source_file);
    let source = fs::read_to_string(&source_abs)
        .with_context(|| format!("failed to read {}", source_abs.display()))?;

    let mut deps = BTreeSet::new();
    match language {
        CodeLanguage::Rust => rust::collect(root, source_file, &source, &mut deps),
        CodeLanguage::JavaScript | CodeLanguage::TypeScript | CodeLanguage::Tsx => {
            typescript::collect(root, source_file, &source, &mut deps)
        }
        CodeLanguage::Python => python::collect(root, source_file, &source, &mut deps),
        CodeLanguage::Go => go::collect(root, source_file, &source, &mut deps),
    }

    Ok(deps.into_iter().collect())
}
