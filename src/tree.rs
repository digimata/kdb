//! Filtered project tree rendering for `kdb tree`.

use anyhow::{Context, Result, bail};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Serialize;
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use crate::project::ignore::build_ignore_globset;
use crate::project::paths::normalize_rel_path;

// -----------------------------------
// src/tree.rs
//
// pub struct TreeOptions          L33
// pub struct TreeNode             L44
// pub enum TreeNodeKind           L54
// pub fn build_tree()             L61
// pub fn render_text()           L124
// fn build_node()                L130
// fn append_children_lines()     L257
// fn build_optional_globset()    L272
// fn explode_patterns()          L292
// fn is_ignored_path()           L305
// fn path_matches()              L344
// fn display_rel_path()          L356
// struct ChildEntry              L364
// -----------------------------------

/// Options that control `kdb tree` filtering and shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeOptions {
    pub max_depth: Option<usize>,
    pub show_hidden: bool,
    pub dirs_only: bool,
    pub full_paths: bool,
    pub ignore_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
}

/// Machine-readable tree node used by `--json` output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub kind: TreeNodeKind,
    pub children: Vec<TreeNode>,
}

/// File-system node kind for `TreeNode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TreeNodeKind {
    File,
    Directory,
}

/// Build a filtered tree rooted at `start`, with paths resolved relative to
/// `root` and ignore patterns applied.
pub fn build_tree(
    root: &Path,
    start: &Path,
    ignore_patterns: &[String],
    options: TreeOptions,
) -> Result<TreeNode> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
    let start = start
        .canonicalize()
        .with_context(|| format!("failed to canonicalize tree path {}", start.display()))?;

    let rel_start = start
        .strip_prefix(&root)
        .with_context(|| {
            format!(
                "tree path {} is not inside kdb root {}",
                start.display(),
                root.display()
            )
        })
        .and_then(|path| {
            normalize_rel_path(path).with_context(|| {
                format!(
                    "tree path {} resolves outside kdb root {}",
                    start.display(),
                    root.display()
                )
            })
        })?;

    let config_ignore_set = build_ignore_globset(ignore_patterns)?;
    let cli_ignore_patterns = explode_patterns(&options.ignore_patterns);
    let include_patterns = explode_patterns(&options.include_patterns);
    let cli_ignore_set = build_optional_globset(&cli_ignore_patterns, false)?;
    let include_set = build_optional_globset(&include_patterns, false)?;

    if is_ignored_path(
        &config_ignore_set,
        cli_ignore_set.as_ref(),
        &rel_start,
        start.is_dir(),
        options.show_hidden,
    ) {
        bail!("tree path is ignored: {}", display_rel_path(&rel_start));
    }

    let options = &options;

    build_node(
        &root,
        &start,
        &rel_start,
        &config_ignore_set,
        cli_ignore_set.as_ref(),
        include_set.as_ref(),
        options,
        0,
    )
}

/// Render a tree node using classic connector glyphs.
pub fn render_text(tree: &TreeNode) -> String {
    let mut lines = vec![tree.name.clone()];
    append_children_lines(tree, "", &mut lines);
    lines.join("\n")
}

fn build_node(
    root: &Path,
    abs_path: &Path,
    rel_path: &Path,
    config_ignore_set: &GlobSet,
    cli_ignore_set: Option<&GlobSet>,
    include_set: Option<&GlobSet>,
    options: &TreeOptions,
    depth: usize,
) -> Result<TreeNode> {
    let metadata = fs::metadata(abs_path)
        .with_context(|| format!("failed to read metadata for {}", abs_path.display()))?;
    let is_dir = metadata.is_dir();

    let display_path = display_rel_path(rel_path);
    let name = if depth == 0 {
        abs_path.to_string_lossy().to_string()
    } else if options.full_paths {
        display_path.clone()
    } else {
        abs_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("<unknown>")
            .to_string()
    };

    let mut node = TreeNode {
        name,
        path: display_path,
        kind: if is_dir {
            TreeNodeKind::Directory
        } else {
            TreeNodeKind::File
        },
        children: Vec::new(),
    };

    if !is_dir || options.max_depth.is_some_and(|limit| depth >= limit) {
        return Ok(node);
    }

    let mut children = Vec::new();
    let entries = fs::read_dir(abs_path)
        .with_context(|| format!("failed to read directory {}", abs_path.display()))?;
    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", abs_path.display()))?;
        let child_abs = entry.path();
        let child_rel = child_abs
            .strip_prefix(root)
            .with_context(|| {
                format!(
                    "child path {} is not inside root {}",
                    child_abs.display(),
                    root.display()
                )
            })
            .and_then(|path| {
                normalize_rel_path(path).with_context(|| {
                    format!(
                        "child path {} resolves outside root {}",
                        child_abs.display(),
                        root.display()
                    )
                })
            })?;

        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read file type for {}", child_abs.display()))?;
        let child_is_dir = file_type.is_dir();

        if is_ignored_path(
            config_ignore_set,
            cli_ignore_set,
            &child_rel,
            child_is_dir,
            options.show_hidden,
        ) {
            continue;
        }
        if options.dirs_only && !child_is_dir {
            continue;
        }

        let child_name = entry.file_name().to_string_lossy().to_string();
        children.push(ChildEntry {
            abs_path: child_abs,
            rel_path: child_rel,
            name: child_name,
            is_dir: child_is_dir,
        });
    }

    children.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => left.name.cmp(&right.name),
    });

    for child in children {
        let child_node = build_node(
            root,
            &child.abs_path,
            &child.rel_path,
            config_ignore_set,
            cli_ignore_set,
            include_set,
            options,
            depth + 1,
        )?;

        if let Some(include_set) = include_set {
            let matched_self = path_matches(include_set, &child.rel_path, child.is_dir);
            let matched_descendant = child.is_dir && !child_node.children.is_empty();
            if !(matched_self || matched_descendant) {
                continue;
            }
        }

        node.children.push(child_node);
    }

    Ok(node)
}

fn append_children_lines(node: &TreeNode, prefix: &str, out: &mut Vec<String>) {
    for (index, child) in node.children.iter().enumerate() {
        let is_last = index + 1 == node.children.len();
        let connector = if is_last { "└── " } else { "├── " };
        out.push(format!("{prefix}{connector}{}", child.name));

        let child_prefix = if is_last {
            format!("{prefix}    ")
        } else {
            format!("{prefix}│   ")
        };
        append_children_lines(child, &child_prefix, out);
    }
}

fn build_optional_globset(patterns: &[String], literal_separator: bool) -> Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(literal_separator)
            .build()
            .with_context(|| format!("invalid tree pattern `{pattern}`"))?;
        builder.add(glob);
    }

    let set = builder
        .build()
        .context("failed to compile tree pattern set")?;
    Ok(Some(set))
}

fn explode_patterns(patterns: &[String]) -> Vec<String> {
    let mut expanded = Vec::new();
    for pattern in patterns {
        for part in pattern.split('|') {
            let item = part.trim();
            if !item.is_empty() {
                expanded.push(item.to_string());
            }
        }
    }
    expanded
}

fn is_ignored_path(
    config_ignore_set: &GlobSet,
    cli_ignore_set: Option<&GlobSet>,
    rel_path: &Path,
    is_dir: bool,
    show_hidden: bool,
) -> bool {
    if rel_path.as_os_str().is_empty() {
        return false;
    }

    let file_name = rel_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    if !show_hidden && file_name.starts_with('.') {
        return true;
    }

    let slash = rel_path.to_string_lossy().replace('\\', "/");
    if config_ignore_set.is_match(&slash) {
        return true;
    }
    if is_dir {
        if config_ignore_set.is_match(format!("{slash}/")) {
            return true;
        }
    }

    if let Some(cli_ignore_set) = cli_ignore_set {
        if path_matches(cli_ignore_set, rel_path, is_dir) {
            return true;
        }
    }

    false
}

fn path_matches(set: &GlobSet, rel_path: &Path, is_dir: bool) -> bool {
    let slash = rel_path.to_string_lossy().replace('\\', "/");
    if set.is_match(&slash) {
        return true;
    }
    if is_dir && set.is_match(format!("{slash}/")) {
        return true;
    }

    false
}

fn display_rel_path(rel_path: &Path) -> String {
    if rel_path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        rel_path.to_string_lossy().replace('\\', "/")
    }
}

struct ChildEntry {
    abs_path: PathBuf,
    rel_path: PathBuf,
    name: String,
    is_dir: bool,
}
