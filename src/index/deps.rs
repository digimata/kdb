use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{resolve_target_path, VaultIndex};

// -------------------------------------
// src/index/deps.rs
//
// pub struct Dependency             L18
// pub fn collect_outbound()         L23
// pub fn collect_code_outbound()    L48
// pub fn print_text()               L70
// -------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Dependency {
    pub file: PathBuf,
    pub anchor: Option<String>,
}

pub fn collect_outbound(index: &VaultIndex, source_file: &Path) -> Result<Vec<Dependency>> {
    let file_entry = index.files.get(source_file).with_context(|| {
        format!(
            "target file is not an indexed markdown file: {}",
            source_file.display()
        )
    })?;

    let mut outbound = BTreeSet::new();

    for link in &file_entry.links {
        let Some(target_file) = resolve_target_path(&file_entry.rel_path, link.kind, &link.target)
        else {
            continue;
        };

        outbound.insert(Dependency {
            file: target_file,
            anchor: link.target.anchor.clone(),
        });
    }

    Ok(outbound.into_iter().collect())
}

pub fn collect_code_outbound(index: &VaultIndex, source_file: &Path) -> Result<Vec<Dependency>> {
    let imports = index.code_imports.get(source_file).with_context(|| {
        format!(
            "target file is not an indexed supported code file: {}",
            source_file.display()
        )
    })?;

    let mut outbound = BTreeSet::new();
    for import in imports {
        let Some(file) = &import.resolved_path else {
            continue;
        };
        outbound.insert(Dependency {
            file: file.clone(),
            anchor: None,
        });
    }

    Ok(outbound.into_iter().collect())
}

pub fn print_text(outbound: &[Dependency]) {
    if outbound.is_empty() {
        println!("(no dependencies)");
        return;
    }

    for dep in outbound {
        if let Some(anchor) = &dep.anchor {
            println!("{}#{anchor}", dep.file.display());
        } else {
            println!("{}", dep.file.display());
        }
    }
}
