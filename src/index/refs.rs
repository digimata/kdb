use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

use super::{
    HeadingKey, LinkRef, VaultIndex, normalize_rel_path, parse_markdown_target, slug_anchor,
};

// --------------------------------
// ## Index
//
// struct RefsTarget            L22
// fn parse_target()            L27
// fn collect_inbound()         L46
// fn print_text()             L101
// fn print_json()             L118
// fn resolve_target_file()    L125
// fn json_row()               L145
// --------------------------------

#[derive(Debug, Clone)]
pub struct RefsTarget {
    pub file: String,
    pub anchor: Option<String>,
}

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

pub fn collect_inbound(
    index: &VaultIndex,
    root: &Path,
    target: RefsTarget,
) -> Result<Vec<LinkRef>> {
    let target_file = resolve_target_file(root, &target.file)?;
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

pub fn print_json(inbound: &[LinkRef]) -> Result<()> {
    let rows = inbound.iter().map(json_row).collect::<Vec<_>>();
    let output = serde_json::to_string_pretty(&rows).context("failed to serialize refs as JSON")?;
    println!("{output}");
    Ok(())
}

fn resolve_target_file(root: &Path, file: &str) -> Result<PathBuf> {
    let path = Path::new(file);
    if path.is_absolute() {
        let canonical = path
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", path.display()))?;
        let rel = canonical.strip_prefix(root).with_context(|| {
            format!(
                "target file {} is not inside kdb root {}",
                canonical.display(),
                root.display()
            )
        })?;
        return normalize_rel_path(rel)
            .with_context(|| format!("target path resolves outside root: {}", file));
    }

    normalize_rel_path(path).with_context(|| format!("target path resolves outside root: {file}"))
}

fn json_row(link_ref: &LinkRef) -> Value {
    let mut object = Map::new();
    object.insert(
        "source_file".to_string(),
        Value::String(link_ref.source_file.to_string_lossy().to_string()),
    );
    object.insert("line".to_string(), Value::from(link_ref.source_line as u64));
    object.insert(
        "column".to_string(),
        Value::from(link_ref.source_column as u64),
    );
    object.insert("raw".to_string(), Value::String(link_ref.raw.clone()));
    Value::Object(object)
}
