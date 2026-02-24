use anyhow::{bail, Context, Result};
use std::path::Path;

use super::{
    parse_markdown_target, resolve_file_target, slug_anchor, HeadingKey, LinkRef, VaultIndex,
};

// -------------------------------
// src/index/refs.rs
//
// pub struct RefsTarget       L18
// pub fn parse_target()       L23
// pub fn collect_inbound()    L42
// pub fn print_text()         L97
// -------------------------------

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
    let target_file = resolve_file_target(root, &target.file)?;
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
