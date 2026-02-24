use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{VaultIndex, resolve_target_path};

// ----------------------------
// src/index/deps.rs
//
// struct Dependency        L19
// fn collect_outbound()    L24
// fn print_text()          L49
// fn print_json()          L64
// fn json_row()            L71
// ----------------------------

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

pub fn print_json(outbound: &[Dependency]) -> Result<()> {
    let rows = outbound.iter().map(json_row).collect::<Vec<_>>();
    let output = serde_json::to_string_pretty(&rows).context("failed to serialize deps as JSON")?;
    println!("{output}");
    Ok(())
}

fn json_row(dep: &Dependency) -> Value {
    let mut object = Map::new();
    object.insert(
        "file".to_string(),
        Value::String(dep.file.to_string_lossy().to_string()),
    );
    match &dep.anchor {
        Some(anchor) => {
            object.insert("anchor".to_string(), Value::String(anchor.clone()));
        }
        None => {
            object.insert("anchor".to_string(), Value::Null);
        }
    }
    Value::Object(object)
}
